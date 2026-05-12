//! Integration test for `plushie::automation::cli::replay::<A>(path)`
//! against a real `plushie-renderer` subprocess.
//!
//! The renderer is normally run in windowed mode for replay, but CI
//! and sandboxed test hosts have no display server. We keep the test
//! useful (and preflight-green on headless CI) by wrapping the
//! renderer binary in a short shell script that pins `--mock`. The
//! SDK side still exercises the full runner_wire path: discovery,
//! spawn, settings send, hello read, codec negotiation, reader start,
//! tree snapshot, and clean teardown.
//!
//! The test calls `runner_wire::run_windowed_with_renderer` directly
//! so the wrapper path is passed explicitly, avoiding env-var
//! mutation and its `unsafe` requirement. Hosts without a
//! plushie-renderer binary skip the test with a helpful message
//! rather than failing, mirroring `wire_mode.rs`.

#![cfg(feature = "wire")]

use std::path::PathBuf;

use plushie::prelude::*;

fn plushie_renderer_binary() -> PathBuf {
    // Tests are compiled into target/debug/deps/<hash>; the renderer
    // binary lives at target/debug/plushie-renderer. Two pops off the
    // test-exe path get us there.
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("plushie-renderer");
    path
}

/// Write a shell wrapper that forces `--mock --json`, following the
/// same pattern as `wire_mode.rs`. Bridge::spawn takes a single path
/// with no args; a wrapper is the only way to pin mode flags.
fn write_wrapper(renderer: &str) -> PathBuf {
    let mut wrapper = std::env::temp_dir();
    let tag = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    wrapper.push(format!(
        "plushie-replay-windowed-test-{}-{tag}.sh",
        std::process::id()
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

struct WrapperCleanup(PathBuf);
impl Drop for WrapperCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Test app
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Counter {
    count: i32,
}

impl App for Counter {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Self { count: 0 }, Command::none())
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
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[test]
fn windowed_replay_drives_renderer() {
    let renderer = plushie_renderer_binary();
    if !renderer.exists() {
        eprintln!(
            "windowed_replay_drives_renderer: renderer binary not found at {}; \
             build it with `cargo build -p plushie-renderer` before running this test.",
            renderer.display()
        );
        return;
    }

    let wrapper = write_wrapper(&renderer.to_string_lossy());
    let _cleanup = WrapperCleanup(wrapper.clone());

    // Write a simple windowed script exercising click, assert_text,
    // and wait. assert_text on the running TestSession proves the
    // MVU cycle ran; the renderer subprocess only needs to accept
    // the tree snapshots.
    let mut script = std::env::temp_dir();
    script.push(format!(
        "plushie-replay-windowed-{}-{}.plushie",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(
        &script,
        "app: Counter\nbackend: windowed\n-----\nclick \"inc\"\nassert_text \"display\" \"1\"\nwait 10\n",
    )
    .expect("write script");
    struct ScriptCleanup(PathBuf);
    impl Drop for ScriptCleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _script_cleanup = ScriptCleanup(script.clone());

    // Use the explicit-binary form of the windowed runner so the
    // test doesn't depend on env-var mutation ordering with other
    // tests in the binary. This exercises the same code path replay
    // uses internally, just with a direct handle to the wrapper.
    let path = script.to_str().unwrap();
    let mut parsed = plushie::automation::file::parse_file(path).expect("parse .plushie");
    parsed.header.backend = "windowed".to_string();

    let wrapper_str = wrapper.to_str().unwrap().to_string();
    let result = plushie::automation::runner_wire::run_windowed_with_renderer::<Counter>(
        &wrapper_str,
        &parsed,
    );

    assert!(
        result.is_ok(),
        "windowed replay should succeed, got: {result:?}"
    );
}
