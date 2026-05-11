//! CLI-dispatch helpers for automation primitives.
//!
//! These three functions exist so a caller can wire `--plushie-script`,
//! `--plushie-replay`, and `--plushie-inspect` into their own CLI
//! without re-implementing the parse + runner plumbing. The zero-config
//! [`crate::cli::run`] entry wires them up automatically; apps with a
//! bespoke CLI call them directly.
//!
//! Each helper is parameterised over the app type `A: App` and returns
//! a [`crate::Result`] (or a `String`, in the case of [`inspect`]).
//! Output that matters to the caller (captures, snapshot JSON) goes
//! through `stderr` / return values; they never call
//! `std::process::exit`.

use crate::{App, Error, Result};

/// Run a `.plushie` automation script against the backend named in
/// its header.
///
/// Parses the file at `path`, validates `backend:`, and dispatches
/// through [`crate::automation::runner::run_with_backend`]. Returns
/// `Ok(())` when every instruction passed, and an error otherwise.
///
/// # Errors
///
/// Returns [`Error::InvalidSettings`] when the file cannot be read
/// or parsed, and a generic [`Error::Startup`] when one or more
/// instructions fail with line-level details.
pub fn script<A: App>(path: &str) -> Result {
    let file = crate::automation::file::parse_file(path)
        .map_err(|e| Error::InvalidSettings(format!("{path}: {e}")))?;
    let result = crate::automation::runner::run_with_backend_result::<A>(&file)?;
    print_captures(path, &result);
    Ok(())
}

/// Replay a `.plushie` script against a live renderer (windowed).
///
/// Mirrors [`script`] but forces the `windowed` backend regardless of
/// the header's `backend:` field, so the caller can visually inspect
/// the replay. The runner locates the renderer binary via the normal
/// wire-mode discovery chain (`PLUSHIE_BINARY_PATH`, custom build,
/// downloaded stock binary, `PATH`), spawns it, and sends each
/// script step's resulting tree so the user sees the app execute the
/// script live on screen.
///
/// `wait` instructions pace the replay in wall-clock time so the
/// user can follow along.
///
/// # Errors
///
/// Returns [`Error::InvalidSettings`] when the file cannot be read
/// or parsed. Propagates renderer-discovery, spawn, handshake, and
/// framing errors from `crate::automation::runner_wire::run_windowed`
/// (wire feature only).
/// Instruction failures surface as [`Error::Startup`] with a one-line
/// summary.
pub fn replay<A: App>(path: &str) -> Result {
    let mut file = crate::automation::file::parse_file(path)
        .map_err(|e| Error::InvalidSettings(format!("{path}: {e}")))?;
    // Force the windowed backend regardless of the header; replay's
    // contract is "visual inspection", so mock / headless headers get
    // upgraded here. Script-without-upgrade users call `script`.
    file.header.backend = "windowed".to_string();
    crate::automation::runner::run_with_backend::<A>(&file)
}

/// Produce a pretty-JSON snapshot of the app's initial view tree.
///
/// Constructs a [`TestSession`](crate::test::TestSession), lets the
/// MVU init cycle run once, and returns the rendered tree as a
/// pretty-printed JSON string. The caller decides what to do with it
/// (print, pipe into `jq`, diff against a golden file, etc.).
///
/// # Errors
///
/// This helper is infallible today; the `Result` return shape exists
/// so future snapshot modes (e.g. lazy-initialised apps that can fail
/// in `init`) can surface their failures through the same API.
pub fn inspect<A: App>() -> std::result::Result<String, Error> {
    let session = crate::test::TestSession::<A>::start().allow_diagnostics();
    Ok(session.tree_snapshot())
}

fn print_captures(path: &str, result: &crate::automation::runner::RunResult) {
    for capture in &result.captures {
        eprintln!(
            "{path}: line {}: {} {} {}",
            capture.line, capture.kind, capture.name, capture.value
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::event::Event;
    use crate::ui::{column, text, window};
    use crate::widget::WidgetRegistrar;

    struct NoopApp;
    impl App for NoopApp {
        type Model = ();
        fn init() -> (Self::Model, Command) {
            ((), Command::none())
        }
        fn update(_m: &mut Self::Model, _e: Event) -> Command {
            Command::none()
        }
        fn view(_m: &Self::Model, _w: &mut WidgetRegistrar) -> crate::ViewList {
            window("main")
                .title("Noop")
                .child(column().child(text("hello")))
                .into()
        }
    }

    #[test]
    fn inspect_returns_tree_json() {
        let snapshot = inspect::<NoopApp>().unwrap();
        assert!(
            snapshot.contains("\"type\""),
            "expected tree JSON: {snapshot}"
        );
    }

    #[test]
    fn script_missing_file_errors() {
        let err = script::<NoopApp>("/nonexistent/nope.plushie").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("/nonexistent/nope.plushie"), "got: {msg}");
    }

    #[test]
    fn script_unknown_backend_is_invalid_settings() {
        let path = std::env::temp_dir().join("plushie_cli_script_unknown_backend.plushie");
        std::fs::write(&path, "app: Noop\nbackend: nope\n-----\nwait 1\n").unwrap();
        let err = script::<NoopApp>(path.to_str().unwrap()).unwrap_err();
        assert!(
            matches!(err, Error::InvalidSettings(_)),
            "expected InvalidSettings, got: {err}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn replay_parse_error_surfaces_before_spawn() {
        // A script missing the `-----` separator is a parse failure.
        // Replay should surface InvalidSettings before it ever tries
        // to spawn a renderer, so the path of an unparseable file is
        // safe to run without a renderer binary on the test host.
        let path = std::env::temp_dir().join("plushie_cli_replay_parse_error.plushie");
        std::fs::write(&path, "app: Noop\nno separator here\n").unwrap();
        let err = replay::<NoopApp>(path.to_str().unwrap()).unwrap_err();
        assert!(
            matches!(err, Error::InvalidSettings(_)),
            "expected InvalidSettings, got: {err}"
        );
        let _ = std::fs::remove_file(&path);
    }
}
