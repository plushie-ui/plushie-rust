//! Transport layer: configurable I/O source for the protocol channel.
//!
//! Three modes:
//!
//! - **stdio** (default): reads stdin, writes stdout. The host spawned
//!   plushie as a subprocess.
//!
//! - **exec** (`--exec` without `--listen`): spawns a command and pipes
//!   stdin/stdout to it. For clean runtimes (Gleam, Go) or SSH subsystems.
//!
//! - **listen** (`--listen` with optional `--exec`): creates a Unix socket
//!   or TCP listener, optionally spawns a command, and communicates over
//!   the accepted connection. For BEAM-based hosts where stdout is
//!   contaminated by the runtime.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::thread::{self, JoinHandle};

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

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

    /// Piped exec transport (`--exec` without `--listen`).
    pub fn exec(command: &str) -> io::Result<Self> {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let shell_flag = if cfg!(windows) { "/c" } else { "-c" };

        let mut child = Command::new(shell)
            .args([shell_flag, command])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| io::Error::other(format!("failed to exec '{command}': {e}")))?;

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

    /// Listen transport (`--listen` with optional `--exec`).
    pub fn listen(addr: &ListenAddr, exec_command: Option<&str>) -> io::Result<Self> {
        let token = generate_token();
        let (listener, display_addr, socket_path) = create_listener(addr)?;

        let (mut child, stderr_thread) = if let Some(command) = exec_command {
            let mut c = spawn_listen_child(command, &display_addr, &token)?;
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
                let listener = bind_unix(&path)?;
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
                let listener = bind_unix(path)?;
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

#[cfg(unix)]
fn bind_unix(path: &str) -> io::Result<std::os::unix::net::UnixListener> {
    let _ = std::fs::remove_file(path);
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }
    std::os::unix::net::UnixListener::bind(path)
}

#[cfg(unix)]
fn auto_socket_path() -> String {
    let dir = format!("/tmp/plushie-{}", random_hex(8));
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

fn spawn_listen_child(command: &str, socket_addr: &str, token: &str) -> io::Result<Child> {
    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    let shell_flag = if cfg!(windows) { "/c" } else { "-c" };

    let negotiation = format!(
        "{{\"token\":\"{token}\",\"protocol\":{}}}\n",
        plushie_widget_sdk::protocol::PROTOCOL_VERSION
    );

    let mut child = Command::new(shell)
        .args([shell_flag, command])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .env("PLUSHIE_SOCKET", socket_addr)
        .env("PLUSHIE_TOKEN", token)
        .spawn()
        .map_err(|e| io::Error::other(format!("failed to exec '{command}': {e}")))?;

    // Write negotiation JSON to child's stdin.
    if let Some(ref mut stdin) = child.stdin {
        let _ = stdin.write_all(negotiation.as_bytes());
        let _ = stdin.flush();
    }

    Ok(child)
}

fn print_connection_info(addr: &str, token: &str) {
    println!("Plushie renderer listening.\n");
    println!("  Address: {addr}");
    println!("  Token:   {token}\n");
    println!("Connect with:");
    println!("  mix plushie.connect MyApp {addr} --token {token}\n");
    println!("Via SSH:");
    println!("  ssh -T -R {addr}:{addr} server \\");
    println!("    'mix plushie.connect MyApp {addr} --token {token}'\n");
}

// ---------------------------------------------------------------------------
// Token generation
// ---------------------------------------------------------------------------

fn generate_token() -> String {
    random_hex(16)
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
