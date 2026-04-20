//! Wire-mode integration test for image commands.
//!
//! The image command builders recently had a wire-op-name bug
//! (`create_from_bytes` / `create_from_rgba` / `update_raw`) that slipped
//! through because nothing in the test suite exercised image commands
//! through the real subprocess renderer; direct-mode tests went through
//! a different dispatch path. This test drives `create_image`,
//! `create_image_rgba`, `update_image`, `delete_image`, and
//! `list_images` through `plushie::run_with_renderer` against a real
//! `plushie-renderer` in mock --json mode and asserts the handles that
//! survive match what a correct dispatch would produce. An unknown-op
//! regression would leave stale handles behind (or miss the intended
//! ones) and fail the assertion.
//!
//! Mock mode is used because the image registry still runs its full
//! `apply_op` dispatch there, so the wire-op-name contract is the same
//! as in windowed mode without needing a display server.

#![cfg(feature = "wire")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use plushie::event::SystemEventType;
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

fn write_wrapper(renderer: &str) -> std::path::PathBuf {
    let mut wrapper = std::env::temp_dir();
    wrapper.push(format!(
        "plushie-wire-image-test-{}-{}.sh",
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
// Test harness
// ---------------------------------------------------------------------------
//
// The app drives the image lifecycle from a timer tick in `update()`
// rather than from `init()` so the wire runner has finished the
// handshake before the first image op is queued. The list_images
// response surfaces as a System(ImageList) event; the handles are
// captured into shared state for the assertion at the end of the run.

struct Shared {
    init_ran: Arc<AtomicBool>,
    handles: Arc<std::sync::Mutex<Option<Vec<String>>>>,
    init_tx: Arc<std::sync::Mutex<Option<mpsc::Sender<()>>>>,
    done_tx: Arc<std::sync::Mutex<Option<mpsc::Sender<()>>>>,
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

struct ImageApp {
    shared: Shared,
}

impl App for ImageApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        let shared = take_shared();
        shared.init_ran.store(true, Ordering::SeqCst);
        if let Some(tx) = shared.init_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        // A non-empty byte string; sniff_image_format will log
        // `unrecognized` but the registry still accepts it, which
        // mirrors app code that passes an arbitrary blob.
        let png_ish = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        let rgba = vec![0u8; 2 * 2 * 4];
        let rgba_updated = vec![0xffu8; 2 * 2 * 4];
        let cmd = Command::batch([
            Command::create_image("logo", png_ish),
            Command::create_image_rgba("pixels", 2, 2, rgba),
            Command::update_image_rgba("pixels", 2, 2, rgba_updated),
            Command::delete_image("logo"),
            Command::list_images("check"),
        ]);
        let model = Self { shared };
        (model, cmd)
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Event::System(sys) = &event
            && sys.event_type == SystemEventType::ImageList
            && sys.tag.as_deref() == Some("check")
        {
            let handles = sys
                .value
                .as_ref()
                .and_then(|v| v.get("handles"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            *model.shared.handles.lock().unwrap() = Some(handles);
            if let Some(tx) = model.shared.done_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
        }
        Command::none()
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> Option<View> {
        Some(window("main").child(text("image ops").id("display")).into())
    }

    fn restart_policy() -> plushie::settings::RestartPolicy {
        // Long-enough heartbeat that the wire runner stays blocked on
        // the bridge until list_images returns. The test signals on
        // the done channel and then drops the wrapper to tear down,
        // so the heartbeat only serves as a safety net.
        let mut policy = plushie::settings::RestartPolicy::default();
        policy.max_restarts = 0;
        policy.restart_delay = Duration::from_millis(10);
        policy.heartbeat_interval = Some(Duration::from_secs(5));
        policy
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[test]
fn wire_image_ops_survive_round_trip() {
    let binary = plushie_binary();
    if !std::path::Path::new(&binary).exists() {
        eprintln!(
            "wire_image_ops_survive_round_trip: renderer binary not found at {binary}; \
             build it with `cargo build -p plushie-renderer` before running this test."
        );
        return;
    }

    let init_ran = Arc::new(AtomicBool::new(false));
    let handles = Arc::new(std::sync::Mutex::new(None));
    let (init_tx, init_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    install_shared(Shared {
        init_ran: init_ran.clone(),
        handles: handles.clone(),
        init_tx: Arc::new(std::sync::Mutex::new(Some(init_tx))),
        done_tx: Arc::new(std::sync::Mutex::new(Some(done_tx))),
    });

    let wrapper = write_wrapper(&binary);
    let wrapper_path = wrapper.to_string_lossy().into_owned();
    let _cleanup = WrapperCleanup(wrapper.clone());

    let handle = std::thread::spawn(move || plushie::run_with_renderer::<ImageApp>(&wrapper_path));

    init_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("ImageApp::init must run within 10s");
    assert!(init_ran.load(Ordering::SeqCst));

    // Wait for the list_images response to arrive.
    done_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("list_images response must arrive within 10s; wire op-name mismatch?");

    // Stop the run loop: close stdin by dropping the wrapper
    // reference, then wait for run_wire to unwind on heartbeat
    // timeout.
    drop(_cleanup);

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline && !handle.is_finished() {
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = handle.join();

    let observed = handles
        .lock()
        .unwrap()
        .take()
        .expect("handles slot must be populated by the list_images response");

    // After the sequence:
    //   create_image("logo", ...)               -> logo registered
    //   create_image_rgba("pixels", ...)        -> pixels registered
    //   update_image_rgba("pixels", ...)        -> pixels re-registered
    //   delete_image("logo")                    -> logo removed
    //   list_images("check")                    -> response
    //
    // The registry should contain exactly "pixels". A wire-op-name
    // regression would either leave "logo" behind (delete_image
    // unknown) or drop "pixels" (create_image / create_image_rgba
    // unknown), both of which fail the assertion.
    assert_eq!(
        observed,
        vec!["pixels".to_string()],
        "image registry handles after the round trip",
    );
}
