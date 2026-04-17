//! Async delivery-contract tests.
//!
//! Covers the three non-happy-path outcomes documented on
//! [`Command::async_task`] and [`Command::cancel`]: an `Err` resolves
//! to `AsyncEvent(Err(..))`, a cancel-before-completion delivers
//! nothing, and a panicking future resolves to a typed `Err` carrying
//! `{"error": "panic", ...}` without tearing down the harness.

use plushie::prelude::*;
use plushie::test::TestSession;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// App fixture: each click kicks off a pre-canned async/stream task.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Outcomes {
    last_ok: Option<Value>,
    last_err: Option<Value>,
    event_count: usize,
    started: bool,
}

struct AsyncApp {
    outcomes: Outcomes,
    mode: AsyncMode,
}

#[derive(Debug, Clone, Copy)]
enum AsyncMode {
    Ok,
    Err,
    Panic,
    CancelFirst,
    StartOnly,
}

impl App for AsyncApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            AsyncApp {
                outcomes: Outcomes::default(),
                mode: AsyncMode::Ok,
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Some(Click("start")) = event.widget_match() {
            model.outcomes.started = true;
            return match model.mode {
                AsyncMode::Ok => Command::async_task("job", || async { Ok(json!("done")) }),
                AsyncMode::Err => Command::async_task("job", || async { Err(json!("boom")) }),
                AsyncMode::Panic => {
                    Command::async_task("job", || async { panic!("simulated task panic") })
                }
                AsyncMode::CancelFirst => Command::batch([
                    Command::async_task("job", || async { Ok(json!("should not arrive")) }),
                    Command::cancel("job"),
                ]),
                AsyncMode::StartOnly => {
                    Command::async_task("job", || async { Ok(json!("pending")) })
                }
            };
        }
        if let Some(a) = event.as_async()
            && a.tag == "job"
        {
            model.outcomes.event_count += 1;
            match &a.result {
                Ok(v) => model.outcomes.last_ok = Some(v.clone()),
                Err(v) => model.outcomes.last_err = Some(v.clone()),
            }
        }
        Command::none()
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main").child(button("start", "Start")).into()
    }
}

fn session_in(mode: AsyncMode) -> TestSession<AsyncApp> {
    let mut s = TestSession::<AsyncApp>::start();
    s.model_mut().mode = mode;
    s
}

// ---------------------------------------------------------------------------
// Ok path (baseline): AsyncEvent(Ok(..)) reaches update.
// ---------------------------------------------------------------------------

#[test]
fn ok_path_delivers_result_to_update() {
    let mut s = session_in(AsyncMode::Ok);
    s.click("start");
    assert_eq!(s.model().outcomes.event_count, 1);
    assert_eq!(s.model().outcomes.last_ok, Some(json!("done")));
    assert!(s.model().outcomes.last_err.is_none());
}

// ---------------------------------------------------------------------------
// Err branch: Err(..) reaches update with the same payload.
// ---------------------------------------------------------------------------

#[test]
fn err_branch_delivers_err_to_update() {
    let mut s = session_in(AsyncMode::Err);
    s.click("start");
    assert_eq!(s.model().outcomes.event_count, 1);
    assert_eq!(s.model().outcomes.last_err, Some(json!("boom")));
    assert!(s.model().outcomes.last_ok.is_none());
}

// ---------------------------------------------------------------------------
// Cancel path: queue a task, then drop it before it runs.
// ---------------------------------------------------------------------------

#[test]
fn cancel_before_completion_delivers_nothing() {
    // Emit Async + Cancel from the same update: the Cancel wins
    // because the task was still queued when it arrived.
    let mut s = session_in(AsyncMode::CancelFirst);
    s.click("start");
    assert!(
        s.model().outcomes.started,
        "update should have observed the click"
    );
    assert_eq!(
        s.model().outcomes.event_count,
        0,
        "no AsyncEvent should be delivered for a cancelled task"
    );
}

#[test]
fn cancel_pending_noop_when_queue_is_empty() {
    // `cancel_pending` mirrors `Command::cancel`: safe to call when
    // nothing is queued, and leaves the queue empty.
    let mut s = session_in(AsyncMode::StartOnly);
    assert_eq!(s.pending_async_count(), 0);
    s.cancel_pending("job");
    assert_eq!(s.pending_async_count(), 0);
}

// ---------------------------------------------------------------------------
// Panic path: the future panics; the harness delivers Err and survives.
// ---------------------------------------------------------------------------

#[test]
fn panic_in_async_task_delivers_typed_error_and_harness_survives() {
    let mut s = session_in(AsyncMode::Panic);
    s.click("start");
    assert_eq!(
        s.model().outcomes.event_count,
        1,
        "panic must resolve to exactly one AsyncEvent(Err(..))"
    );
    let err = s
        .model()
        .outcomes
        .last_err
        .as_ref()
        .expect("panic should land in last_err");
    assert_eq!(
        err.get("error").and_then(Value::as_str),
        Some("panic"),
        "err payload must be {{\"error\": \"panic\", ...}}; got {err:?}"
    );
    let msg = err
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        msg.contains("simulated task panic"),
        "panic message must carry through: {msg:?}"
    );

    // Harness is still usable: subsequent clicks still drive update.
    s.model_mut().mode = AsyncMode::Ok;
    s.click("start");
    assert_eq!(s.model().outcomes.event_count, 2);
    assert_eq!(s.model().outcomes.last_ok, Some(json!("done")));
}
