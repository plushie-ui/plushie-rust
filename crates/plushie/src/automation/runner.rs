//! Automation script runner.
//!
//! Executes parsed `.plushie` file instructions against a
//! [`TestSession`](crate::test::TestSession). Each instruction
//! maps to a TestSession method call.
//!
//! ```ignore
//! let file = plushie::automation::file::parse_file("test.plushie")?;
//! let session = TestSession::<MyApp>::start();
//! let result = plushie::automation::runner::run(&file, &mut session);
//! ```

use crate::App;
use crate::automation::file::{Instruction, PlushieFile};
use crate::test::TestSession;

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

fn execute_instruction<A: App>(
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
            if tree_contains_text(tree, text) {
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
            Err(
                "assert_model requires App::Model: Debug; use a wrapper runner with the bound"
                    .to_string(),
            )
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

/// Check if any node in the tree contains the given text in its
/// content, label, value, or placeholder props.
fn tree_contains_text(node: &plushie_core::protocol::TreeNode, text: &str) -> bool {
    for key in &["content", "label", "value", "placeholder"] {
        if node.props.get_str(key) == Some(text) {
            return true;
        }
    }
    node.children.iter().any(|c| tree_contains_text(c, text))
}
