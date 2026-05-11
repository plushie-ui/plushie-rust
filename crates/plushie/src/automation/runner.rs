//! Automation script runner.
//!
//! Executes parsed `.plushie` file instructions against a
//! [`TestSession`]. Each instruction maps to a TestSession method
//! call.
//!
//! ```ignore
//! let file = plushie::automation::file::parse_file("test.plushie")?;
//! let session = TestSession::<MyApp>::start();
//! let result = plushie::automation::runner::run(&file, &mut session);
//! ```
//!
//! # Backends
//!
//! The parsed `.plushie` header carries a `backend:` field selecting
//! `mock`, `headless`, or `windowed` (see
//! [`crate::automation::Backend`]). [`run_with_backend`] honours that
//! field and dispatches to the matching path:
//!
//! - `Mock` / `Headless` run through a headless [`TestSession`]
//!   without spawning a renderer. This is the fast in-process path
//!   the CLI's `--plushie-script` uses.
//! - `Windowed` spawns the real `plushie-renderer` binary so the user
//!   can watch the script execute. Implemented in
//!   `crate::automation::runner_wire` and gated on the `wire`
//!   feature.

use crate::App;
use crate::automation::file::{Instruction, PlushieFile};
use crate::test::TestSession;
use crate::{Error, Result as PlushieResult};

/// Result of running a `.plushie` file.
#[derive(Debug)]
pub struct RunResult {
    /// Instructions that succeeded.
    pub passed: usize,
    /// Instructions that failed, with line numbers and error messages.
    pub failures: Vec<(usize, String)>,
}

impl RunResult {
    /// Returns true when every instruction in the run passed.
    pub fn is_ok(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Run a parsed `.plushie` file against a TestSession.
///
/// Executes each instruction in order. Assertions that fail are
/// collected as failures rather than panicking, so all instructions
/// are attempted. Returns a summary of passed and failed
/// instructions.
///
/// Note: the `assert_model` instruction requires the app's `Model`
/// type to implement `Debug`. Use [`run_with_model_debug`] if the
/// script contains `assert_model` calls. This function reports them
/// as failures with a descriptive message.
pub fn run<A: App>(file: &PlushieFile, session: &mut TestSession<A>) -> RunResult {
    let mut passed = 0;
    let mut failures = Vec::new();

    for (line_no, instruction) in &file.instructions {
        match execute_instruction(session, instruction) {
            Ok(()) => passed += 1,
            Err(msg) => failures.push((*line_no, msg)),
        }
    }

    RunResult { passed, failures }
}

/// Run a parsed `.plushie` file when the app's model implements
/// `Debug`, enabling the `assert_model` instruction to match against
/// the debug-formatted model string.
pub fn run_with_model_debug<A: App>(file: &PlushieFile, session: &mut TestSession<A>) -> RunResult
where
    A::Model: std::fmt::Debug,
{
    let mut passed = 0;
    let mut failures = Vec::new();

    for (line_no, instruction) in &file.instructions {
        let result = match instruction {
            Instruction::AssertModel(expected) => {
                let actual = format!("{:?}", session.model());
                if actual.contains(expected.as_str()) {
                    Ok(())
                } else {
                    Err(format!(
                        "expected model debug string to contain \"{expected}\", got {actual}"
                    ))
                }
            }
            other => execute_instruction(session, other),
        };
        match result {
            Ok(()) => passed += 1,
            Err(msg) => failures.push((*line_no, msg)),
        }
    }

    RunResult { passed, failures }
}

/// Run a parsed `.plushie` file, honouring the header's
/// `backend:` field.
///
/// Parses [`crate::automation::Backend`] from the header and
/// dispatches to the matching path:
///
/// - [`Backend::Mock`](crate::automation::Backend::Mock) and
///   [`Backend::Headless`](crate::automation::Backend::Headless)
///   construct a [`TestSession`] and delegate to [`run`]. The
///   renderer is not spawned.
/// - [`Backend::Windowed`](crate::automation::Backend::Windowed)
///   spawns the real `plushie-renderer` binary and drives it via
///   the wire protocol. Requires the `wire` feature.
///
/// Returns `Ok(())` when every instruction passes and an
/// [`Error::Startup`] summarising the failing lines otherwise. Errors
/// encountered before the script runs (unknown backend, wire feature
/// missing, renderer discovery failure) surface as
/// [`Error::InvalidSettings`], [`Error::NoRunnerFeature`], or
/// [`Error::BinaryNotFound`] from the relevant subsystem.
///
/// # Errors
///
/// Propagates the errors described above. The script itself is
/// reported through the returned result; instruction failures surface
/// as [`Error::Startup`] with a one-line summary.
pub fn run_with_backend<A: App>(file: &PlushieFile) -> PlushieResult {
    let backend =
        crate::automation::Backend::from_header(&file.header.backend).ok_or_else(|| {
            Error::InvalidSettings(format!(
                "unknown backend `{}` (expected mock, headless, or windowed)",
                file.header.backend
            ))
        })?;

    match backend {
        crate::automation::Backend::Mock | crate::automation::Backend::Headless => {
            let mut session = TestSession::<A>::start().allow_diagnostics();
            let result = run::<A>(file, &mut session);
            if result.is_ok() {
                Ok(())
            } else {
                Err(Error::Startup(format!(
                    "{} instruction(s) failed",
                    result.failures.len()
                )))
            }
        }
        crate::automation::Backend::Windowed => run_windowed::<A>(file),
    }
}

/// Route a windowed-backend run to the wire-based implementation.
///
/// Without the `wire` feature the SDK cannot spawn a renderer, so we
/// fail fast with [`Error::NoRunnerFeature`]. The feature-enabled
/// branch forwards to the renderer-spawn implementation.
#[cfg(feature = "wire")]
fn run_windowed<A: App>(file: &PlushieFile) -> PlushieResult {
    crate::automation::runner_wire::run_windowed::<A>(file)
}

/// Wire-feature-disabled stub. The `A` type parameter is retained so
/// the two definitions share a signature from the caller's point of
/// view.
#[cfg(not(feature = "wire"))]
fn run_windowed<A: App>(_file: &PlushieFile) -> PlushieResult {
    let _ = std::marker::PhantomData::<A>;
    Err(Error::NoRunnerFeature)
}

pub(crate) fn execute_instruction<A: App>(
    session: &mut TestSession<A>,
    instruction: &Instruction,
) -> Result<(), String> {
    match instruction {
        Instruction::Click(sel) => {
            session.click(sel.clone());
            Ok(())
        }
        Instruction::TypeText(sel, text) => {
            session.type_text(sel.clone(), text);
            Ok(())
        }
        Instruction::TypeKey(key) => {
            session.type_key(key.as_str());
            Ok(())
        }
        Instruction::Press(key) => {
            session.press(key.as_str());
            Ok(())
        }
        Instruction::Release(key) => {
            session.release(key.as_str());
            Ok(())
        }
        Instruction::Toggle(sel, value) => {
            match value {
                Some(v) => session.set_toggle(sel.clone(), *v),
                None => session.toggle(sel.clone()),
            }
            Ok(())
        }
        Instruction::Select(sel, value) => {
            session.select(sel.clone(), value);
            Ok(())
        }
        Instruction::Slide(sel, value) => {
            session.slide(sel.clone(), *value);
            Ok(())
        }
        Instruction::Scroll(sel, dx, dy) => {
            session.scroll(sel.clone(), *dx, *dy);
            Ok(())
        }
        Instruction::MoveTo(_x, _y) => {
            // Coordinate-based move is a no-op in headless mode
            // (requires renderer layout knowledge).
            Ok(())
        }
        Instruction::MoveToSelector(_sel) => {
            // Selector-based move is a no-op in headless mode.
            Ok(())
        }
        Instruction::Wait(_ms) => {
            // Waits are ignored in test mode (synchronous execution).
            Ok(())
        }
        Instruction::Expect(text) => {
            let tree = session.tree();
            if plushie_core::Selector::text(text).find(tree).is_some() {
                Ok(())
            } else {
                Err(format!("expected text \"{text}\" not found in tree"))
            }
        }
        Instruction::AssertText(sel, expected) => {
            let actual = session.text_content(sel.clone());
            if actual.as_deref() == Some(expected.as_str()) {
                Ok(())
            } else {
                Err(format!(
                    "expected {sel} text \"{expected}\", got {actual:?}"
                ))
            }
        }
        Instruction::AssertExists(sel) => {
            if session.find(sel.clone()).is_some() {
                Ok(())
            } else {
                Err(format!("expected {sel} to exist"))
            }
        }
        Instruction::AssertNotExists(sel) => {
            if session.find(sel.clone()).is_none() {
                Ok(())
            } else {
                Err(format!("expected {sel} to NOT exist"))
            }
        }
        Instruction::AssertModel(_expected) => {
            // AssertModel matches against the Debug string of the
            // model. The runner is generic over `A: App`, not
            // `A: App where Model: Debug`, so we can't format here.
            // Callers that need this instruction should resolve it
            // against `session.model()` in a wrapper runner that
            // adds the Debug bound.
            Err("assert_model requires App::Model: Debug; use run_with_model_debug".to_string())
        }
        Instruction::Screenshot(_name) => {
            // Screenshots are a no-op in headless TestSession.
            Ok(())
        }
        Instruction::TreeHash(_name) => {
            // Tree hash capture is a no-op in headless TestSession.
            Ok(())
        }
    }
}
