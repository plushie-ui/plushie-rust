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
//! Output that matters to the caller (pass/fail summaries, snapshot
//! JSON) goes through `stderr` / return values; they never call
//! `std::process::exit`.

use crate::{App, Error, Result};

/// Run a `.plushie` automation script against a headless
/// [`TestSession`](crate::test::TestSession).
///
/// Parses the file at `path`, runs each instruction in order, and
/// prints a one-line pass/fail summary to stderr. Returns `Ok(())`
/// when every instruction passed, and an error otherwise. Failures
/// are listed on stderr (line number + message) before returning so
/// the caller sees exactly what broke.
///
/// # Errors
///
/// Returns [`Error::InvalidSettings`] when the file cannot be read
/// or parsed, and a generic [`Error::Startup`] when one or more
/// instructions fail (the summary on stderr has the details).
pub fn script<A: App>(path: &str) -> Result {
    let file = crate::automation::file::parse_file(path)
        .map_err(|e| Error::InvalidSettings(format!("{path}: {e}")))?;
    let mut session = crate::test::TestSession::<A>::start().allow_diagnostics();
    let result = crate::automation::runner::run::<A>(&file, &mut session);
    print_summary(path, &result);
    if result.is_ok() {
        Ok(())
    } else {
        Err(Error::Startup(format!(
            "{} instruction(s) failed in {path}",
            result.failures.len()
        )))
    }
}

/// Replay a `.plushie` script against a live renderer (windowed).
///
/// Mirrors [`script`] but forces the `windowed` backend regardless of
/// the header's `backend:` field, so the caller can visually inspect
/// the replay. Today the runner drives a [`TestSession`] (no real
/// renderer) regardless of the header; windowed dispatch is deferred
/// to a follow-on hat once the runner grows a renderer-backed path.
///
/// # Errors
///
/// Currently returns [`Error::Startup`] with a message explaining
/// that windowed replay is not yet available. The parse + header
/// validation still runs first so a malformed script surfaces its
/// own error before the not-yet-supported notice.
pub fn replay<A: App>(path: &str) -> Result {
    // Parse the file early so genuinely broken scripts still surface
    // a meaningful error rather than the not-yet-supported notice.
    let _file = crate::automation::file::parse_file(path)
        .map_err(|e| Error::InvalidSettings(format!("{path}: {e}")))?;
    Err(Error::Startup(format!(
        "windowed replay is not yet wired through the automation runner; \
         file at {path} parsed OK but no windowed backend is available. \
         Use plushie::automation::cli::script for the headless path \
         until the renderer-backed runner lands."
    )))
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

fn print_summary(path: &str, result: &crate::automation::runner::RunResult) {
    if result.is_ok() {
        eprintln!("{path}: {} instruction(s) passed", result.passed);
        return;
    }
    eprintln!(
        "{path}: {} passed, {} failed",
        result.passed,
        result.failures.len()
    );
    for (line_no, msg) in &result.failures {
        eprintln!("  line {line_no}: {msg}");
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
        fn view(_m: &Self::Model, _w: &mut WidgetRegistrar) -> crate::View {
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
    fn replay_parses_then_refuses() {
        // Write a valid .plushie file to a tmp path, confirm replay
        // surfaces the not-yet-supported error (not a parse error).
        let path = std::env::temp_dir().join("plushie_cli_replay_test.plushie");
        std::fs::write(&path, "app: Noop\n-----\n").unwrap();
        let err = replay::<NoopApp>(path.to_str().unwrap()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("windowed replay"), "got: {msg}");
        let _ = std::fs::remove_file(&path);
    }
}
