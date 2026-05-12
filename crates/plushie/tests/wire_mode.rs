//! Wire-mode integration test.
//!
//! Drives a minimal Counter app through `plushie::run_with_renderer::<Counter>()`
//! against a real `plushie-renderer` subprocess in `--mock` mode and
//! asserts that the SDK's wire client negotiates the handshake, sends
//! the initial snapshot, and exits cleanly when the renderer stops
//! responding. This catches drift between the Rust SDK's wire entry
//! point and the renderer's protocol that the renderer-crate
//! integration tests can't see: they speak raw JSON, not the SDK's
//! own client.

#![cfg(feature = "wire")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use plushie::prelude::*;

// ---------------------------------------------------------------------------
// Renderer binary location and wrapping
// ---------------------------------------------------------------------------

fn plushie_binary() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("plushie-renderer");
    path.to_string_lossy().to_string()
}

/// Write a shell wrapper that pins `--mock --json`. `Bridge::spawn`
/// takes a single binary path with no args, so the only way to
/// launch the renderer in a test-friendly mode is to front it with
/// a wrapper script.
fn write_wrapper(renderer: &str) -> std::path::PathBuf {
    let mut wrapper = std::env::temp_dir();
    wrapper.push(format!(
        "plushie-wire-test-{}-{}.sh",
        std::process::id(),
        rand_suffix()
    ));
    let script = format!("#!/bin/sh\nexec {renderer} --mock --json \"$@\"\n");
    std::fs::write(&wrapper, script).expect("write wrapper script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&wrapper).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&wrapper, perms).unwrap();
    }
    wrapper
}

struct WrapperCleanup(std::path::PathBuf);

impl Drop for WrapperCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Test app: observes renderer-exit to confirm the round trip.
// ---------------------------------------------------------------------------
//
// In mock mode the renderer stays idle unless driven, and the wire
// runner's main loop blocks on bridge.recv_timeout. A long-lived
// integration test that also drives synthetic user events would need
// a side-channel to the renderer that run_wire doesn't expose. To
// keep scope tight while still exercising the real wire client, the
// test verifies:
//
//   * the SDK spawns the renderer
//   * Settings are sent, hello is received
//   * the initial snapshot is delivered
//   * the tree diff + patch path runs without panicking
//   * handle_renderer_exit fires when the renderer goes away
//
// To shut the renderer down deterministically, the test closes the
// wrapper subprocess's stdin by dropping the parent once init has
// been observed. The SDK then classifies the exit and returns.

#[derive(Clone)]
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
        .expect("shared handles must be installed before run_wire")
        .lock()
        .unwrap()
        .take()
        .expect("shared handles already taken")
}

#[derive(Clone)]
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

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        if let Some(Click("inc")) = event.widget_match() {
            next.count += 1;
        }
        (next, Command::none())
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
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
        // Use a very short heartbeat so the wire runner trips out of
        // recv_timeout quickly: mock mode is silent once the initial
        // snapshot lands, and the test finishes fast by having the
        // runner classify the silence as a heartbeat timeout. Zero
        // restarts so run_wire returns directly rather than looping.
        let mut policy = plushie::settings::RestartPolicy::default();
        policy.max_restarts = 0;
        policy.restart_delay = Duration::from_millis(10);
        policy.heartbeat_interval = Some(Duration::from_millis(250));
        policy
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[test]
fn wire_mode_handshake_and_exit() {
    let binary = plushie_binary();
    if !std::path::Path::new(&binary).exists() {
        eprintln!(
            "wire_mode_handshake_and_exit: renderer binary not found at {binary}; \
             build it with `cargo build -p plushie-renderer` before running this test."
        );
        return;
    }

    let init_ran = Arc::new(AtomicBool::new(false));
    let exit_reason = Arc::new(std::sync::Mutex::new(None));
    let (tx, rx) = mpsc::channel::<()>();
    install_shared(Observed {
        init_ran: init_ran.clone(),
        exit_reason: exit_reason.clone(),
        tx: Arc::new(std::sync::Mutex::new(Some(tx))),
    });

    let wrapper = write_wrapper(&binary);
    let wrapper_path = wrapper.to_string_lossy().into_owned();
    let _cleanup = WrapperCleanup(wrapper.clone());

    // run_wire blocks; drive it from a worker thread so the test can
    // detect progress and enforce a wall-clock deadline.
    let handle = std::thread::spawn(move || plushie::run_with_renderer::<Counter>(&wrapper_path));

    // Wait for init() to run: that confirms the SDK reached the
    // post-handshake state (settings sent, hello received, snapshot
    // about to be sent).
    rx.recv_timeout(Duration::from_secs(10))
        .expect("Counter::init must run within 10s; run_wire likely stuck on handshake");
    assert!(
        init_ran.load(Ordering::SeqCst),
        "init marker not set even though the signal arrived"
    );

    // Wait for run_wire to return. The short heartbeat in the test's
    // restart_policy causes the loop to trip out quickly once the
    // renderer goes silent post-snapshot.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline && !handle.is_finished() {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        handle.is_finished(),
        "run_wire did not terminate after heartbeat timeout (exit reason so far: {:?})",
        exit_reason.lock().unwrap()
    );

    // The call may return Ok (shutdown) or Err (RendererExit). Both
    // are acceptable outcomes; what matters is that the exit hook
    // fired and the thread didn't panic.
    let result = handle.join().expect("run_wire thread panicked");
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
}
