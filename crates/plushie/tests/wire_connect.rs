//! Integration test for `plushie::run_connect`.
//!
//! Spawns `plushie-renderer --listen <socket> --mock --json` in a
//! child process, parses the socket path + token from the renderer's
//! stdout, then drives a minimal Counter app through
//! `plushie::run_connect` against the listening socket. Confirms the
//! SDK's socket transport negotiates the handshake, exchanges the
//! token, sends the initial snapshot, and exits cleanly when the
//! renderer goes silent past the heartbeat threshold.

#![cfg(all(feature = "wire", unix))]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use plushie::prelude::*;

// ---------------------------------------------------------------------------
// Renderer binary location
// ---------------------------------------------------------------------------

fn renderer_binary() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("plushie-renderer");
    path.to_string_lossy().to_string()
}

// ---------------------------------------------------------------------------
// Observed handles shared between the test thread and the test App
// ---------------------------------------------------------------------------
//
// The test App's init() / handle_renderer_exit() hooks record
// progress through a global Mutex so the test can wait on them.

struct Observed {
    init_ran: Arc<AtomicBool>,
    exit_reason: Arc<std::sync::Mutex<Option<String>>>,
    tx: Arc<std::sync::Mutex<Option<mpsc::Sender<()>>>>,
}

static SHARED: std::sync::OnceLock<std::sync::Mutex<Option<Observed>>> = std::sync::OnceLock::new();

fn install_shared(observed: Observed) {
    let slot = SHARED.get_or_init(|| std::sync::Mutex::new(None));
    *slot.lock().unwrap() = Some(observed);
}

fn take_shared() -> Observed {
    SHARED
        .get()
        .expect("shared handles must be installed before run_connect")
        .lock()
        .unwrap()
        .take()
        .expect("shared handles already taken")
}

// ---------------------------------------------------------------------------
// Test App
// ---------------------------------------------------------------------------

struct Counter {
    count: i32,
    observed: Observed,
}

impl App for Counter {
    type Model = Self;

    fn init() -> (Self, Command) {
        let observed = take_shared();
        observed.init_ran.store(true, Ordering::SeqCst);
        if let Some(tx) = observed.tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        let model = Self { count: 0, observed };
        (model, Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Some(Click("inc")) = event.widget_match() {
            model.count += 1;
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main")
            .child(
                column()
                    .child(text(&format!("{}", model.count)).id("display"))
                    .child(button("inc", "+")),
            )
            .into()
    }

    fn handle_renderer_exit(model: &mut Self, reason: plushie::settings::ExitReason) {
        *model.observed.exit_reason.lock().unwrap() = Some(reason.label().to_string());
    }

    fn restart_policy() -> plushie::settings::RestartPolicy {
        // Short heartbeat so the wire runner trips out of
        // recv_timeout quickly: mock mode stays silent once the
        // initial snapshot lands.
        let mut policy = plushie::settings::RestartPolicy::default();
        policy.max_restarts = 0;
        policy.restart_delay = Duration::from_millis(10);
        policy.heartbeat_interval = Some(Duration::from_millis(250));
        policy
    }
}

// ---------------------------------------------------------------------------
// Renderer subprocess harness
// ---------------------------------------------------------------------------

struct ListeningRenderer {
    child: Child,
    socket: String,
    token: String,
    _stdout_thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ListeningRenderer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn unique_socket_path() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    format!("/tmp/plushie-test-{pid}-{nanos}.sock")
}

fn start_listening_renderer(binary: &str) -> ListeningRenderer {
    let socket = unique_socket_path();
    let _ = std::fs::remove_file(&socket);

    let mut child = ProcessCommand::new(binary)
        .args(["--listen", &socket, "--mock", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn renderer --listen");

    // Parse the connection-info banner printed on stdout: we need
    // the token, and waiting for it also tells us the listener is
    // accepting connections.
    let stdout = child.stdout.take().expect("child stdout piped");
    let (token_tx, token_rx) = mpsc::channel::<String>();

    let stdout_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut sent = false;
        for line in reader.lines().map_while(Result::ok) {
            eprintln!("[renderer stdout] {line}");
            if !sent && let Some(rest) = line.trim().strip_prefix("Token:") {
                let token = rest.trim().to_string();
                let _ = token_tx.send(token);
                sent = true;
            }
        }
    });

    let token = token_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("renderer must print its token within 5s");

    ListeningRenderer {
        child,
        socket,
        token,
        _stdout_thread: Some(stdout_thread),
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[test]
fn wire_connect_handshake_and_exit() {
    let binary = renderer_binary();
    if !std::path::Path::new(&binary).exists() {
        eprintln!(
            "wire_connect_handshake_and_exit: renderer binary not found at {binary}; \
             build it with `cargo build -p plushie-renderer` before running this test."
        );
        return;
    }

    let renderer = start_listening_renderer(&binary);
    // The renderer hasn't accepted a connection yet; it prints the
    // banner and then blocks in accept(). Our run_connect call
    // drives the accept.

    let init_ran = Arc::new(AtomicBool::new(false));
    let exit_reason = Arc::new(std::sync::Mutex::new(None));
    let (tx, rx) = mpsc::channel::<()>();
    install_shared(Observed {
        init_ran: init_ran.clone(),
        exit_reason: exit_reason.clone(),
        tx: Arc::new(std::sync::Mutex::new(Some(tx))),
    });

    let opts = plushie::ConnectOpts {
        socket: Some(renderer.socket.clone()),
        token: Some(renderer.token.clone()),
    };

    let handle = std::thread::spawn(move || plushie::run_connect::<Counter>(opts));

    rx.recv_timeout(Duration::from_secs(10))
        .expect("Counter::init must run within 10s; run_connect likely stuck on handshake");
    assert!(
        init_ran.load(Ordering::SeqCst),
        "init marker not set even though the signal arrived"
    );

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && !handle.is_finished() {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        handle.is_finished(),
        "run_connect did not terminate after heartbeat timeout (exit reason so far: {:?})",
        exit_reason.lock().unwrap()
    );

    let result = handle.join().expect("run_connect thread panicked");
    // Either Ok(shutdown) or Err(RendererExit) is acceptable; the
    // point is that the handshake went through, init ran, and the
    // loop unwound cleanly.
    let _ = result;
    let reason = exit_reason
        .lock()
        .unwrap()
        .clone()
        .expect("handle_renderer_exit should have recorded a reason");
    assert!(
        !reason.is_empty(),
        "exit reason label should be non-empty, got: {reason}"
    );

    // Drop the renderer harness explicitly so it kills the child
    // and cleans up the socket before the test exits.
    drop(renderer);
}
