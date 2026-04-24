//! Dev-mode hot-reload integration test.
//!
//! Drives the wire runner with a mock renderer, publishes a
//! `ControlSignal::SwapRenderer` after `init` fires, and asserts that
//! the runner gracefully terminates the first subprocess and spawns
//! a fresh one while keeping the user's Model across the swap.
//!
//! Requires both the `wire` and `dev` features; only meaningful
//! when the mock renderer binary is available in the cargo target.

#![cfg(all(feature = "wire", feature = "dev"))]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use plushie::prelude::*;

fn plushie_binary() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("plushie-renderer");
    path.to_string_lossy().to_string()
}

/// Wrapper that pins `--mock --json` so the renderer binary can be
/// passed to `run_wire` (which spawns with no args). The wrapper
/// also logs each invocation to a temp file so the test can count
/// how many times it was spawned.
fn write_wrapper(renderer: &str, log_path: &std::path::Path) -> std::path::PathBuf {
    let mut wrapper = std::env::temp_dir();
    wrapper.push(format!(
        "plushie-hotreload-test-{}-{}.sh",
        std::process::id(),
        rand_suffix()
    ));
    let script = format!(
        "#!/bin/sh\n\
         echo spawn >> {log}\n\
         exec {renderer} --mock --json \"$@\"\n",
        log = log_path.display(),
        renderer = renderer,
    );
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

// -- Test harness ---------------------------------------------------------

struct Shared {
    init_count: Arc<AtomicUsize>,
    exit_count: Arc<AtomicUsize>,
    tx: Arc<std::sync::Mutex<Option<mpsc::Sender<()>>>>,
}

static SHARED: std::sync::OnceLock<std::sync::Mutex<Option<Shared>>> = std::sync::OnceLock::new();

fn install_shared(shared: Shared) {
    let slot = SHARED.get_or_init(|| std::sync::Mutex::new(None));
    *slot.lock().unwrap() = Some(shared);
}

fn take_shared() -> Shared {
    SHARED
        .get()
        .expect("shared must be installed before run_wire")
        .lock()
        .unwrap()
        .take()
        .expect("shared already taken")
}

struct SwapApp {
    #[allow(dead_code)]
    count: i32,
    shared: Shared,
}

impl App for SwapApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        // init only runs once in run_wire_inner even across restarts
        // and swaps: the Model is preserved. Count it anyway so the
        // test can assert exactly one call.
        let shared = take_shared();
        shared.init_count.fetch_add(1, Ordering::SeqCst);
        if let Some(tx) = shared.tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        let model = SwapApp { count: 0, shared };
        (model, Command::none())
    }

    fn update(_model: &mut Self, _event: Event) -> Command {
        Command::none()
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main").child(text("swap app").id("display")).into()
    }

    fn handle_renderer_exit(model: &mut Self, _reason: plushie::settings::ExitReason) {
        // Exit fires once per subprocess end, including swaps.
        model.shared.exit_count.fetch_add(1, Ordering::SeqCst);
    }

    fn restart_policy() -> plushie::settings::RestartPolicy {
        // Short heartbeat so the final teardown classifies silence
        // quickly once we stop driving the runner. Zero restart
        // attempts so crashes bail out cleanly; swaps don't count
        // against this.
        let mut policy = plushie::settings::RestartPolicy::default();
        policy.max_restarts = 0;
        policy.restart_delay = Duration::from_millis(10);
        // Heartbeat long enough that we can observe the swap path
        // without the watchdog tripping first. The test drives the
        // runner's teardown by closing its stdin once the swap has
        // been observed, not by waiting for a heartbeat.
        policy.heartbeat_interval = Some(Duration::from_secs(5));
        policy
    }
}

#[test]
fn control_signal_triggers_renderer_swap() {
    let binary = plushie_binary();
    if !std::path::Path::new(&binary).exists() {
        eprintln!(
            "control_signal_triggers_renderer_swap: renderer binary not found at {binary}; \
             build with `cargo build -p plushie-renderer` first."
        );
        return;
    }

    // Unique log path so concurrent test runs don't collide.
    let mut log_path = std::env::temp_dir();
    log_path.push(format!(
        "plushie-hotreload-log-{}-{}.txt",
        std::process::id(),
        rand_suffix()
    ));
    let _log_cleanup = WrapperCleanup(log_path.clone());
    std::fs::write(&log_path, "").unwrap();

    let wrapper = write_wrapper(&binary, &log_path);
    let wrapper_path = wrapper.to_string_lossy().into_owned();
    let _wrapper_cleanup = WrapperCleanup(wrapper);

    let init_count = Arc::new(AtomicUsize::new(0));
    let exit_count = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel::<()>();
    install_shared(Shared {
        init_count: init_count.clone(),
        exit_count: exit_count.clone(),
        tx: Arc::new(std::sync::Mutex::new(Some(tx))),
    });

    // Drain any leftover signals from other tests.
    let _ = plushie::dev::drain_control_signals();

    let handle = std::thread::spawn(move || plushie::run_with_renderer::<SwapApp>(&wrapper_path));

    // Wait for init.
    rx.recv_timeout(Duration::from_secs(10))
        .expect("init did not run within 10s");

    // Give the runner a moment to settle into the main loop.
    std::thread::sleep(Duration::from_millis(100));

    // Trigger the swap. The wire runner polls the control queue on
    // each iteration and should return ExitReason::RendererSwap.
    plushie::dev::send_control_signal(plushie::dev::ControlSignal::SwapRenderer);

    // Poll the spawn log until a second spawn line appears or a
    // deadline passes. Two spawn lines means the swap worked.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut spawn_count = 0;
    while std::time::Instant::now() < deadline {
        let log = std::fs::read_to_string(&log_path).unwrap_or_default();
        spawn_count = log.lines().filter(|l| l.trim() == "spawn").count();
        if spawn_count >= 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Stop the runner by letting the heartbeat expire. Wait for the
    // thread to finish so we can inspect final counts.
    let finish_deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < finish_deadline && !handle.is_finished() {
        std::thread::sleep(Duration::from_millis(100));
    }
    // If the runner is still alive, join will still complete once the
    // wrapper's subprocess exits on the heartbeat timeout.
    let _ = handle.join();

    assert!(
        spawn_count >= 2,
        "renderer should have been spawned at least twice after a swap signal, got {spawn_count}"
    );
    assert_eq!(
        init_count.load(Ordering::SeqCst),
        1,
        "init must run exactly once - Model is preserved across swaps"
    );
    assert!(
        exit_count.load(Ordering::SeqCst) >= 1,
        "handle_renderer_exit should have fired at least once"
    );
}
