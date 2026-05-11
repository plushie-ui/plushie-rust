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

/// Captured data produced by a `.plushie` instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// Source line that produced this capture.
    pub line: usize,
    /// Capture instruction name.
    pub kind: &'static str,
    /// User-supplied capture tag or path.
    pub name: String,
    /// Captured value.
    pub value: String,
}

/// Result of running a `.plushie` file.
#[derive(Debug)]
pub struct RunResult {
    /// Instructions that succeeded.
    pub passed: usize,
    /// Instructions that failed, with line numbers and error messages.
    pub failures: Vec<(usize, String)>,
    /// Captures produced by instructions such as `tree_hash`.
    pub captures: Vec<Capture>,
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
    let mut captures = Vec::new();

    if let Err(msg) = apply_viewport_header(file, session) {
        failures.push((0, msg));
    }
    for (line_no, instruction) in &file.instructions {
        match execute_instruction(session, instruction) {
            Ok(capture) => {
                passed += 1;
                if let Some(mut capture) = capture {
                    capture.line = *line_no;
                    captures.push(capture);
                }
            }
            Err(msg) => failures.push((*line_no, msg)),
        }
    }

    RunResult {
        passed,
        failures,
        captures,
    }
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
    let mut captures = Vec::new();

    if let Err(msg) = apply_viewport_header(file, session) {
        failures.push((0, msg));
    }
    for (line_no, instruction) in &file.instructions {
        let result = match instruction {
            Instruction::AssertModel(expected) => {
                let actual = format!("{:?}", session.model());
                if actual.contains(expected.as_str()) {
                    Ok(None)
                } else {
                    Err(format!(
                        "expected model debug string to contain \"{expected}\", got {actual}"
                    ))
                }
            }
            other => execute_instruction(session, other),
        };
        match result {
            Ok(capture) => {
                passed += 1;
                if let Some(mut capture) = capture {
                    capture.line = *line_no;
                    captures.push(capture);
                }
            }
            Err(msg) => failures.push((*line_no, msg)),
        }
    }

    RunResult {
        passed,
        failures,
        captures,
    }
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
    run_with_backend_result::<A>(file).map(|_| ())
}

/// Run a parsed `.plushie` file through the selected backend and
/// return the successful run summary.
///
/// This is the backend-aware form to use when callers need captures
/// such as `tree_hash` values. Instruction failures still surface as
/// [`Error::Startup`] with line-level details.
///
/// # Errors
///
/// Returns [`Error::InvalidSettings`] for an unknown backend,
/// [`Error::Startup`] for script instruction failures, and propagates
/// backend-specific startup errors.
pub fn run_with_backend_result<A: App>(file: &PlushieFile) -> Result<RunResult, Error> {
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
                Ok(result)
            } else {
                Err(Error::Startup(format_run_failures(&result)))
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
fn run_windowed<A: App>(file: &PlushieFile) -> Result<RunResult, Error> {
    crate::automation::runner_wire::run_windowed_result::<A>(file)
}

/// Wire-feature-disabled stub. The `A` type parameter is retained so
/// the two definitions share a signature from the caller's point of
/// view.
#[cfg(not(feature = "wire"))]
fn run_windowed<A: App>(_file: &PlushieFile) -> Result<RunResult, Error> {
    let _ = std::marker::PhantomData::<A>;
    Err(Error::NoRunnerFeature)
}

pub(crate) fn execute_instruction<A: App>(
    session: &mut TestSession<A>,
    instruction: &Instruction,
) -> Result<Option<Capture>, String> {
    if let Some(sel) = instruction_selector(instruction)
        && session.find(sel.clone()).is_none()
    {
        return Err(format!("target not found: {sel}"));
    }

    match instruction {
        Instruction::Click(sel) => {
            session.click(sel.clone());
            Ok(None)
        }
        Instruction::TypeText(sel, text) => {
            session.type_text(sel.clone(), text);
            Ok(None)
        }
        Instruction::TypeKey(key) => {
            session.type_key(key.as_str());
            Ok(None)
        }
        Instruction::Press(key) => {
            session.press(key.as_str());
            Ok(None)
        }
        Instruction::Release(key) => {
            session.release(key.as_str());
            Ok(None)
        }
        Instruction::Toggle(sel, value) => {
            match value {
                Some(v) => session.set_toggle(sel.clone(), *v),
                None => session.toggle(sel.clone()),
            }
            Ok(None)
        }
        Instruction::Select(sel, value) => {
            session.select(sel.clone(), value);
            Ok(None)
        }
        Instruction::Slide(sel, value) => {
            session.slide(sel.clone(), *value);
            Ok(None)
        }
        Instruction::Scroll(sel, dx, dy) => {
            session.scroll(sel.clone(), *dx, *dy);
            Ok(None)
        }
        Instruction::MoveTo(_x, _y) => {
            // Coordinate-based move is a no-op in headless mode
            // (requires renderer layout knowledge).
            Ok(None)
        }
        Instruction::MoveToSelector(_sel) => {
            // Selector-based move is a no-op in headless mode.
            Ok(None)
        }
        Instruction::Wait(_ms) => {
            // Waits are ignored in test mode (synchronous execution).
            Ok(None)
        }
        Instruction::Expect(text) => {
            let tree = session.tree();
            if plushie_core::Selector::text(text).find(tree).is_some() {
                Ok(None)
            } else {
                Err(format!("expected text \"{text}\" not found in tree"))
            }
        }
        Instruction::AssertText(sel, expected) => {
            let actual = session.text_content(sel.clone());
            if actual.as_deref() == Some(expected.as_str()) {
                Ok(None)
            } else {
                Err(format!(
                    "expected {sel} text \"{expected}\", got {actual:?}"
                ))
            }
        }
        Instruction::AssertExists(sel) => {
            if session.find(sel.clone()).is_some() {
                Ok(None)
            } else {
                Err(format!("expected {sel} to exist"))
            }
        }
        Instruction::AssertNotExists(sel) => {
            if session.find(sel.clone()).is_none() {
                Ok(None)
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
        Instruction::Screenshot(name) => Err(format!(
            "screenshot capture `{name}` is unsupported by the in-process automation backend"
        )),
        Instruction::TreeHash(name) => {
            let hash = session.tree_hash();
            Ok(Some(Capture {
                line: 0,
                kind: "tree_hash",
                name: name.clone(),
                value: hash,
            }))
        }
    }
}

pub(crate) fn format_run_failures(result: &RunResult) -> String {
    let mut msg = format!("{} instruction(s) failed", result.failures.len());
    for (line_no, detail) in &result.failures {
        if *line_no == 0 {
            msg.push_str(&format!("; header: {detail}"));
        } else {
            msg.push_str(&format!("; line {line_no}: {detail}"));
        }
    }
    msg
}

fn instruction_selector(instruction: &Instruction) -> Option<&plushie_core::Selector> {
    match instruction {
        Instruction::Click(sel)
        | Instruction::TypeText(sel, _)
        | Instruction::Toggle(sel, _)
        | Instruction::Select(sel, _)
        | Instruction::Slide(sel, _)
        | Instruction::Scroll(sel, _, _)
        | Instruction::MoveToSelector(sel) => Some(sel),
        _ => None,
    }
}

fn apply_viewport_header<A: App>(
    file: &PlushieFile,
    session: &mut TestSession<A>,
) -> Result<(), String> {
    if !file.header.viewport_explicit {
        return Ok(());
    }

    let window_ids = collect_window_ids(session.tree());
    if window_ids.is_empty() {
        return Err(
            "viewport header cannot be applied because the tree has no window nodes".into(),
        );
    }

    let (width, height) = file.header.viewport;
    for window_id in window_ids {
        session
            .window(&window_id)
            .resized(width as f32, height as f32);
    }
    Ok(())
}

fn collect_window_ids(tree: &plushie_core::protocol::TreeNode) -> Vec<String> {
    let mut ids = Vec::new();
    collect_window_ids_inner(tree, &mut ids);
    ids
}

fn collect_window_ids_inner(node: &plushie_core::protocol::TreeNode, ids: &mut Vec<String>) {
    if node.type_name == "window" && !node.id.is_empty() {
        ids.push(node.id.clone());
    }
    for child in &node.children {
        collect_window_ids_inner(child, ids);
    }
}
