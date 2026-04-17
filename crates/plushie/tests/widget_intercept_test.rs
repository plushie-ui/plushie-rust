//! Behavioural tests for composite-widget event interception.
//!
//! Each variant of `EventResult` reaches the real `intercept_event`
//! loop through `WidgetTestSession`, a harness that wraps a
//! `Widget` in a throwaway host app and records the events the
//! host's `update` actually sees. The tests verify the observable
//! result of interception rather than the `EventResult` struct
//! shape; direct tests on `handle_event` already cover that, and
//! the interception loop is what regresses silently when the
//! semantics drift.

use plushie::WidgetEvent;
use plushie::prelude::*;
use plushie::test::WidgetTestSession;
use plushie::widget::{EventResult, Widget};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Widget fixtures: one per EventResult variant behaviour under test.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct NoState;

// -- Consumed: widget swallows the click, the app never sees it. -------------

struct Consumer;

impl Widget for Consumer {
    type State = NoState;
    type Props = UntypedProps;

    fn view(id: &str, _props: &UntypedProps, _state: &NoState) -> View {
        // Wrap the child in a container carrying the widget's own ID
        // so the child's scope chain has the widget as an ancestor.
        // Without that step the harness's button would land at the
        // top level and intercept_event would skip it.
        column().id(id).child(button("inner", "press")).into()
    }

    fn handle_event(_event: &Event, _state: &mut NoState) -> EventResult {
        // Unconditionally consume the click. The harness's update
        // must not observe it.
        EventResult::Consumed
    }
}

// -- Emit: widget rewrites the click family and payload. ---------------------

#[derive(WidgetEvent)]
enum EmitterEvent {
    Picked(u64),
}

struct Emitter;

impl Widget for Emitter {
    type State = NoState;
    type Props = UntypedProps;

    fn view(id: &str, _props: &UntypedProps, _state: &NoState) -> View {
        column().id(id).child(button("inner", "pick")).into()
    }

    fn handle_event(_event: &Event, _state: &mut NoState) -> EventResult {
        EventResult::emit_event(EmitterEvent::Picked(42))
    }
}

// -- Ignored: widget stays out of the way; event falls through. --------------

struct Passthrough;

impl Widget for Passthrough {
    type State = NoState;
    type Props = UntypedProps;

    fn view(id: &str, _props: &UntypedProps, _state: &NoState) -> View {
        column().id(id).child(button("inner", "through")).into()
    }

    fn handle_event(_event: &Event, _state: &mut NoState) -> EventResult {
        EventResult::Ignored
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn consumed_stops_event_before_update() {
    let mut session = WidgetTestSession::<Consumer>::start("consume");
    session.click("inner");
    assert!(
        session.events().is_empty(),
        "Consumer must suppress the click; harness saw: {:?}",
        session.events()
    );
}

#[test]
fn emit_rewrites_event_family_and_reaches_update() {
    let mut session = WidgetTestSession::<Emitter>::start("emit");
    session.click("inner");

    let (family, value) = session
        .last_event()
        .expect("update must see the emitted event");
    assert_eq!(family, "picked", "family must match the emitted variant");
    assert_eq!(value, &json!(42), "payload must match emit_event's value");
}

#[test]
fn ignored_falls_through_to_update_unchanged() {
    let mut session = WidgetTestSession::<Passthrough>::start("through");
    session.click("inner");

    let (family, value) = session
        .last_event()
        .expect("update must see the original click");
    assert_eq!(
        family, "click",
        "Ignored must forward the event's original family"
    );
    assert_eq!(value, &Value::Null, "click events carry a null payload");
}
