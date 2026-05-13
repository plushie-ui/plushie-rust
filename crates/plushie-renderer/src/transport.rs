//! Transport layer: configurable I/O source for the protocol channel.
//!
//! Three modes:
//!
//! - **stdio** (default): reads stdin, writes stdout. The host spawned
//!   plushie as a subprocess.
//!
//! - **exec** (`--exec-bin` with repeated `--exec-arg` without
//!   `--listen`): spawns a command and pipes stdin/stdout to it. For
//!   clean runtimes (Gleam, Go) or SSH subsystems.
//!
//! - **listen** (`--listen` with optional exec): creates a Unix socket or
//!   TCP listener, optionally spawns a command, and communicates over the
//!   accepted connection. For BEAM-based hosts where stdout is contaminated
//!   by the runtime.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::thread::{self, JoinHandle};

use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

/// Host process to spawn for renderer-owned exec modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExecCommand {
    /// Structured argv launch, bypassing shell parsing and quoting.
    Argv { program: String, args: Vec<String> },
}

impl ExecCommand {
    fn command(&self) -> Command {
        match self {
            ExecCommand::Argv { program, args } => {
                let mut cmd = Command::new(program);
                cmd.args(args);
                cmd
            }
        }
    }

    fn label(&self) -> &str {
        match self {
            ExecCommand::Argv { program, .. } => program,
        }
    }
}

/// The I/O endpoints for protocol communication.
pub(crate) struct Transport {
    pub reader: BufReader<Box<dyn Read + Send>>,
    pub writer: Box<dyn Write + Send>,
    _child: Option<Child>,
    _stderr_thread: Option<JoinHandle<()>>,
    _socket_path: Option<String>,
    transport_name: &'static str,
    /// Token the host must include in its Settings message.
    pub expected_token: Option<String>,
}

/// Where to listen for connections.
pub(crate) enum ListenAddr {
    /// Auto-select: Unix socket on Unix, TCP on Windows.
    Auto,
    /// Explicit Unix socket path.
    Unix(String),
    /// Explicit TCP address.
    Tcp(SocketAddr),
}

impl ListenAddr {
    /// Parse a --listen argument.
    ///
    /// - No argument or empty: Auto
    /// - Starts with `:` (e.g., `:4567`): TCP on localhost
    /// - Contains `:` with host (e.g., `0.0.0.0:4567`): TCP
    /// - Anything else: Unix socket path
    pub fn parse(arg: Option<&str>) -> Result<Self, String> {
        match arg {
            None => Ok(ListenAddr::Auto),
            Some("") => Ok(ListenAddr::Auto),
            Some(s) if s.starts_with(':') => {
                let port: u16 = s[1..]
                    .parse()
                    .map_err(|_| format!("invalid port in '{s}'"))?;
                Ok(ListenAddr::Tcp(SocketAddr::from(([127, 0, 0, 1], port))))
            }
            Some(s) if s.contains(':') && !s.starts_with('/') => {
                let addr: SocketAddr = s
                    .parse()
                    .map_err(|e| format!("invalid address '{s}': {e}"))?;
                Ok(ListenAddr::Tcp(addr))
            }
            Some(s) => Ok(ListenAddr::Unix(s.to_string())),
        }
    }
}

impl Transport {
    /// Standard I/O transport (default, no flags).
    pub fn stdio() -> Self {
        Self {
            reader: BufReader::with_capacity(64 * 1024, Box::new(io::stdin())),
            writer: Box::new(io::stdout()),
            _child: None,
            _stderr_thread: None,
            _socket_path: None,
            transport_name: "stdio",
            expected_token: None,
        }
    }

    /// Piped exec transport without `--listen`.
    pub fn exec(command: &ExecCommand, extra_env: &[String]) -> io::Result<Self> {
        let mut child = command
            .command()
            .env_clear()
            .envs(crate::env::child_env(extra_env))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| io::Error::other(format!("failed to exec '{}': {e}", command.label())))?;

        let child_stdout = child.stdout.take().expect("child stdout piped");
        let child_stdin = child.stdin.take().expect("child stdin piped");
        let child_stderr = child.stderr.take().expect("child stderr piped");
        let stderr_thread = spawn_stderr_forwarder(child_stderr);

        Ok(Self {
            reader: BufReader::with_capacity(64 * 1024, Box::new(child_stdout)),
            writer: Box::new(child_stdin),
            _child: Some(child),
            _stderr_thread: Some(stderr_thread),
            _socket_path: None,
            transport_name: "exec",
            expected_token: None,
        })
    }

    /// Listen transport (`--listen` with optional structured exec).
    pub fn listen(
        addr: &ListenAddr,
        exec_command: Option<&ExecCommand>,
        extra_env: &[String],
    ) -> io::Result<Self> {
        let token = generate_token();
        let (listener, display_addr, socket_path) = create_listener(addr)?;

        let (mut child, stderr_thread) = if let Some(command) = exec_command {
            let mut c = spawn_listen_child(command, &display_addr, &token, extra_env)?;
            let child_stderr = c.stderr.take().expect("child stderr piped");
            let stderr_thread = spawn_stderr_forwarder(child_stderr);

            log::info!("waiting for host to connect to {display_addr}...");
            (Some(c), Some(stderr_thread))
        } else {
            print_connection_info(&display_addr, &token);
            log::info!("waiting for connection on {display_addr}...");
            (None, None)
        };

        // Accept connection (with child health check if exec).
        let (reader, writer) = if let Some(ref mut c) = child {
            accept_with_child_check(&listener, c, &socket_path)?
        } else {
            accept_connection(&listener)?
        };
        log::info!("host connected");

        Ok(Self {
            reader: BufReader::with_capacity(64 * 1024, reader),
            writer,
            _child: child,
            _stderr_thread: stderr_thread,
            _socket_path: socket_path,
            transport_name: "listen",
            expected_token: Some(token),
        })
    }

    /// Name of this transport for the hello message.
    pub fn name(&self) -> &'static str {
        self.transport_name
    }

    /// Consume the transport into its constituent parts.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        BufReader<Box<dyn Read + Send>>,
        Box<dyn Write + Send>,
        TransportGuard,
        Option<String>,
    ) {
        (
            self.reader,
            self.writer,
            TransportGuard {
                _child: self._child,
                _stderr_thread: self._stderr_thread,
                _socket_path: self._socket_path,
            },
            self.expected_token,
        )
    }
}

// ---------------------------------------------------------------------------
// Transport guard
// ---------------------------------------------------------------------------

pub(crate) struct TransportGuard {
    _child: Option<Child>,
    _stderr_thread: Option<JoinHandle<()>>,
    _socket_path: Option<String>,
}

impl Drop for TransportGuard {
    fn drop(&mut self) {
        if let Some(ref mut child) = self._child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(ref path) = self._socket_path {
            let _ = std::fs::remove_file(path);
            // Try to remove parent dir (only succeeds if empty).
            if let Some(parent) = std::path::Path::new(path).parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Listener creation and accept
// ---------------------------------------------------------------------------

enum Listener {
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixListener),
    Tcp(TcpListener),
}

type ReaderWriter = (Box<dyn Read + Send>, Box<dyn Write + Send>);

fn create_listener(addr: &ListenAddr) -> io::Result<(Listener, String, Option<String>)> {
    match addr {
        ListenAddr::Auto => {
            #[cfg(unix)]
            {
                let path = auto_socket_path();
                let listener = bind_unix(&path, BindMode::Auto)?;
                Ok((Listener::Unix(listener), path.clone(), Some(path)))
            }
            #[cfg(not(unix))]
            {
                let listener = TcpListener::bind("127.0.0.1:0")?;
                let addr = listener.local_addr()?;
                Ok((Listener::Tcp(listener), addr.to_string(), None))
            }
        }
        #[allow(unused_variables)]
        ListenAddr::Unix(path) => {
            #[cfg(unix)]
            {
                let listener = bind_unix(path, BindMode::UserSpecified)?;
                Ok((Listener::Unix(listener), path.clone(), Some(path.clone())))
            }
            #[cfg(not(unix))]
            {
                Err(io::Error::other(
                    "Unix sockets not supported on this platform",
                ))
            }
        }
        ListenAddr::Tcp(addr) => {
            let listener = TcpListener::bind(addr)?;
            let bound = listener.local_addr()?;
            Ok((Listener::Tcp(listener), bound.to_string(), None))
        }
    }
}

/// Whether a Unix socket path was chosen automatically by the
/// renderer or supplied by the user. The two cases differ on whether
/// we create (and lock down) the parent directory ourselves.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindMode {
    /// Socket path constructed by [`auto_socket_path`]. The parent
    /// directory was created by the renderer and is ours to chmod 0o700.
    Auto,
    /// Socket path came from `--listen <path>`. We respect the user's
    /// directory choice; the parent's permissions stay untouched. A
    /// world-writable parent earns a warning but not a refusal.
    UserSpecified,
}

#[cfg(unix)]
fn bind_unix(path: &str, mode: BindMode) -> io::Result<std::os::unix::net::UnixListener> {
    use std::os::unix::fs::PermissionsExt;

    let _ = std::fs::remove_file(path);

    if let Some(parent) = std::path::Path::new(path).parent() {
        match mode {
            BindMode::Auto => {
                // Auto-chosen directory: create it (if needed) and
                // lock it down to user-only so only we can reach the
                // socket below.
                std::fs::create_dir_all(parent)?;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
            BindMode::UserSpecified => {
                // User chose this path. Don't override their directory
                // permissions. Surface a warning when the parent looks
                // reachable by other users so they can pick a safer
                // location.
                if let Ok(meta) = std::fs::metadata(parent) {
                    let mode_bits = meta.permissions().mode();
                    if mode_bits & 0o002 != 0 {
                        log::warn!(
                            "[code=listen_socket_parent_world_writable] \
                             parent directory {p:?} of listen socket {path:?} \
                             is world-writable (mode=0o{mode_bits:o}); \
                             other local users may symlink or replace files in it",
                            p = parent,
                        );
                    }
                }
            }
        }
    }

    let listener = std::os::unix::net::UnixListener::bind(path)?;
    // The socket file is always ours; lock it to user-only so other
    // local users can't connect even if the parent directory is loose.
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    Ok(listener)
}

/// Pick a directory for the auto-generated socket. Preference order:
/// - `$XDG_RUNTIME_DIR` (systemd-managed user runtime dir, 0700)
/// - `$TMPDIR` (macOS per-user, Linux temp override)
/// - `/tmp` fallback
#[cfg(unix)]
fn auto_socket_base() -> String {
    if let Ok(p) = std::env::var("XDG_RUNTIME_DIR")
        && !p.is_empty()
    {
        return p;
    }
    if let Ok(p) = std::env::var("TMPDIR")
        && !p.is_empty()
    {
        return p;
    }
    "/tmp".to_string()
}

#[cfg(unix)]
fn auto_socket_path() -> String {
    let base = auto_socket_base();
    let dir = format!("{base}/plushie-{}", random_hex(8));
    format!("{dir}/plushie.sock")
}

fn accept_connection(listener: &Listener) -> io::Result<ReaderWriter> {
    match listener {
        #[cfg(unix)]
        Listener::Unix(l) => {
            let (stream, _) = l.accept()?;
            let reader = stream.try_clone()?;
            Ok((Box::new(reader), Box::new(stream)))
        }
        Listener::Tcp(l) => {
            let (stream, _) = l.accept()?;
            stream.set_nodelay(true)?;
            let reader = stream.try_clone()?;
            Ok((Box::new(reader), Box::new(stream)))
        }
    }
}

fn accept_with_child_check(
    listener: &Listener,
    child: &mut Child,
    socket_path: &Option<String>,
) -> io::Result<ReaderWriter> {
    set_listener_nonblocking(listener, true)?;

    loop {
        match accept_connection(listener) {
            Ok(rw) => return Ok(rw),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if let Some(status) = child.try_wait().ok().flatten() {
                    if let Some(path) = &socket_path {
                        let _ = std::fs::remove_file(path);
                    }
                    return Err(io::Error::other(format!(
                        "child exited with {status} before connecting"
                    )));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(e),
        }
    }
}

fn set_listener_nonblocking(listener: &Listener, nonblocking: bool) -> io::Result<()> {
    match listener {
        #[cfg(unix)]
        Listener::Unix(l) => l.set_nonblocking(nonblocking),
        Listener::Tcp(l) => l.set_nonblocking(nonblocking),
    }
}

// ---------------------------------------------------------------------------
// Child spawning
// ---------------------------------------------------------------------------

fn spawn_listen_child(
    command: &ExecCommand,
    socket_addr: &str,
    token: &str,
    extra_env: &[String],
) -> io::Result<Child> {
    let token_sha256 = token_sha256(token);
    let negotiation = format!(
        "{{\"token_sha256\":\"{token_sha256}\",\"protocol\":{}}}\n",
        plushie_widget_sdk::protocol::PROTOCOL_VERSION
    );

    let mut child = command
        .command()
        .env_clear()
        .envs(crate::env::child_env(extra_env))
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .env("PLUSHIE_SOCKET", socket_addr)
        .env("PLUSHIE_TOKEN", token)
        .env("PLUSHIE_TOKEN_SHA256", token_sha256)
        .spawn()
        .map_err(|e| io::Error::other(format!("failed to exec '{}': {e}", command.label())))?;

    // Write negotiation JSON to child's stdin.
    if let Some(ref mut stdin) = child.stdin {
        let _ = stdin.write_all(negotiation.as_bytes());
        let _ = stdin.flush();
    }

    Ok(child)
}

/// Print the listen-mode address and token to stderr.
///
/// The renderer supports six host SDKs (Elixir, Gleam, Python, Ruby,
/// TypeScript, Rust), each with its own connect primitive, so this
/// output is deliberately host-agnostic. Stderr keeps stdout free for
/// machine-readable output and the existing log infrastructure.
fn print_connection_info(addr: &str, token: &str) {
    eprintln!("Plushie renderer listening.\n");
    eprintln!("  Address: {addr}");
    eprintln!("  Token:   {token}\n");
    eprintln!(
        "Use your Plushie SDK's connect primitive with the address and \
         token above. Each SDK documents the exact command and argument \
         order; see the SDK's README or run its CLI with `--help`.\n"
    );
    eprintln!("Via SSH, forward the socket to a remote host first:");
    eprintln!("  ssh -T -R {addr}:{addr} server <SDK-connect-command>\n");
}

// ---------------------------------------------------------------------------
// Token generation
// ---------------------------------------------------------------------------

fn generate_token() -> String {
    random_hex(16)
}

fn token_sha256(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("getrandom failed");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Stderr forwarder
// ---------------------------------------------------------------------------

fn spawn_stderr_forwarder(stderr: ChildStderr) -> JoinHandle<()> {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Some(Ok(line)) = lines.next() {
            eprintln!("[remote] {line}");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_listen_token_for_settings_contract() {
        assert_eq!(
            token_sha256("listen-token"),
            "af84a4f1a6d2ff0ec31b6cae05bca90736ddc3b8d925661db8bd19ecf37a6cab"
        );
    }

    #[cfg(unix)]
    mod unix {
        use super::*;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        fn write_script(name: &str, body: &str) -> PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "plushie-renderer-transport-test-{}-{}",
                std::process::id(),
                random_hex(4)
            ));
            fs::create_dir_all(&dir).expect("create script dir");
            let path = dir.join(name);
            fs::write(&path, body).expect("write script");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("mark script executable");
            path
        }

        #[test]
        fn exec_argv_passes_arguments_without_shell_quoting() {
            let script = write_script(
                "print-args.sh",
                "#!/bin/sh\nprintf '%s|%s\\n' \"$1\" \"$2\"\n",
            );
            let command = ExecCommand::Argv {
                program: script.display().to_string(),
                args: vec!["with space".to_string(), "semi;colon".to_string()],
            };

            let mut transport = Transport::exec(&command, &[]).expect("spawn exec child");
            let mut line = String::new();
            transport
                .reader
                .read_line(&mut line)
                .expect("read child stdout");

            assert_eq!(line, "with space|semi;colon\n");
            let status = transport
                ._child
                .as_mut()
                .expect("exec child")
                .wait()
                .expect("wait child");
            assert!(status.success());
        }

        #[test]
        fn listen_child_receives_argv_and_token_sha256_negotiation() {
            let output = std::env::temp_dir().join(format!(
                "plushie-renderer-listen-child-{}-{}.txt",
                std::process::id(),
                random_hex(4)
            ));
            let script = write_script(
                "record-listen.sh",
                "#!/bin/sh\nread line\n{\nprintf 'args=%s|%s\\n' \"$1\" \"$2\"\nprintf 'socket=%s\\n' \"$PLUSHIE_SOCKET\"\nprintf 'token=%s\\n' \"$PLUSHIE_TOKEN\"\nprintf 'sha=%s\\n' \"$PLUSHIE_TOKEN_SHA256\"\nprintf 'stdin=%s\\n' \"$line\"\n} > \"$3\"\n",
            );
            let command = ExecCommand::Argv {
                program: script.display().to_string(),
                args: vec![
                    "with space".to_string(),
                    "semi;colon".to_string(),
                    output.display().to_string(),
                ],
            };

            let mut child = spawn_listen_child(&command, "127.0.0.1:12345", "listen-token", &[])
                .expect("spawn listen child");
            let status = child.wait().expect("wait child");
            assert!(status.success());

            let recorded = fs::read_to_string(&output).expect("read child output");
            let expected_sha = token_sha256("listen-token");
            assert!(recorded.contains("args=with space|semi;colon\n"));
            assert!(recorded.contains("socket=127.0.0.1:12345\n"));
            assert!(recorded.contains("token=listen-token\n"));
            assert!(recorded.contains(&format!("sha={expected_sha}\n")));
            assert!(recorded.contains(&format!("\"token_sha256\":\"{expected_sha}\"")));
            assert!(!recorded.contains("\"token\":\""));
        }
    }
}
