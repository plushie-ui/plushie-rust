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
use plushie::test::{TestSession, WidgetTestSession};
use plushie::widget::{EventResult, Widget, WidgetRegistrar, WidgetView};
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

// ---------------------------------------------------------------------------
// Nested composite widgets: re-entrant scope walk through multiple
// interceptors.
// ---------------------------------------------------------------------------
//
// Scenario: an Outer composite wraps an Inner composite. The Inner
// returns Ignored on a child click so the scope walk continues up to
// Outer; Outer transforms the click into a typed `Payment` event and
// emits it. The emitted event must reach `A::update` carrying Outer's
// interceptor identity in `scoped_id`, not the original button or
// Inner's.
//
// This pins the documented contract for `EventResult::Emit` scope
// walks: each composite in the scope chain sees the event in
// innermost-first order, and an `Emit` is routed to `A::update` with
// the emitting widget's ID as its scoped identity.

#[derive(WidgetEvent)]
enum OuterEvent {
    // Typed transformation of the inner click. Payload carries the
    // amount Outer computed, not the original click's null value.
    Payment(u64),
}

struct Inner;

impl Widget for Inner {
    type State = NoState;
    type Props = UntypedProps;

    fn view(id: &str, _props: &UntypedProps, _state: &NoState) -> View {
        // Wrap the child in a container carrying Inner's own ID so
        // the button's scope chain includes Inner as an ancestor.
        column().id(id).child(button("pay-btn", "Pay")).into()
    }

    fn handle_event(_event: &Event, _state: &mut NoState) -> EventResult {
        // Inner stays out of the way; the click must fall through to
        // Outer in the scope walk.
        EventResult::Ignored
    }
}

struct Outer;

impl Widget for Outer {
    type State = NoState;
    type Props = UntypedProps;

    fn view(id: &str, _props: &UntypedProps, _state: &NoState) -> View {
        // Outer's expanded view wraps an Inner `__widget__` placeholder
        // under Outer's own ID. WidgetRegistrar isn't reachable from
        // inside a Widget::view, but a nested placeholder still
        // resolves through WidgetStateStore as long as the app's view
        // registered an expander for the same ID above; see the
        // `nested_app::view` below where Inner is registered under
        // "inner".
        let inner_placeholder = WidgetView::<Inner>::new("inner").placeholder();
        column().id(id).child(inner_placeholder).into()
    }

    fn handle_event(event: &Event, _state: &mut NoState) -> EventResult {
        // Pattern-match the raw click bubbling up from the inner's
        // button; transform it into a typed `Payment` event.
        match event.widget_match() {
            Some(Click(_)) => EventResult::emit_event(OuterEvent::Payment(42)),
            _ => EventResult::Ignored,
        }
    }
}

// Host app that registers both widgets and records every event its
// update function sees.
#[derive(Clone)]
struct NestedApp {
    events: Vec<Event>,
}

impl App for NestedApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Self { events: Vec::new() }, Command::None)
    }

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        next.events.push(event);
        (next, Command::None)
    }

    fn view(_model: &Self, widgets: &mut WidgetRegistrar) -> ViewList {
        // Register both widgets. Outer wraps Inner through a
        // manually-built `__widget__` placeholder inside its own
        // `view()`, so only one call to WidgetView::register is
        // needed per widget.
        let outer = WidgetView::<Outer>::new("outer").register(widgets);
        // Register Inner's expander under the id the placeholder in
        // Outer's view uses.
        let _inner_register = WidgetView::<Inner>::new("inner");
        // The side effect of registering is attaching the expander
        // to the WidgetRegistrar. Call register() but discard the
        // returned placeholder View; Outer's view already provides
        // a placeholder node at the same ID in the tree.
        let _ = _inner_register.register(widgets);
        window("main")
            .child(column().id("root").child(outer))
            .into()
    }
}

#[test]
fn nested_outer_intercepts_inner_click_and_emits_with_outer_identity() {
    let mut session = TestSession::<NestedApp>::start();

    // Click the inner button via its scope-qualified selector.
    // After normalization the button's scoped ID is
    // "main#root/outer/inner/pay-btn"; TestSession.click resolves
    // the selector against the tree and dispatches a Click event
    // carrying that scope chain.
    session.click("pay-btn");

    // A::update must have received exactly one widget event
    // (Outer's emitted Payment) after init.
    let widget_events: Vec<&Event> = session
        .model()
        .events
        .iter()
        .filter(|e| matches!(e, Event::Widget(_)))
        .collect();
    assert_eq!(
        widget_events.len(),
        1,
        "expected exactly one widget event (the emitted Payment), got {:?}",
        session.model().events
    );

    let widget = widget_events[0].as_widget().expect("Widget event");

    // Family must match the typed `WidgetEvent` derive's snake_case
    // mapping: `OuterEvent::Payment(_)` -> "payment".
    assert_eq!(
        widget.event_type.as_family(),
        "payment",
        "emitted family must be the outer's transformed name, not the inner click's"
    );

    // Payload must be the value Outer chose, not the original null
    // click payload.
    assert_eq!(
        widget.value,
        json!(42),
        "emitted payload must be Outer's transformed value"
    );

    // The interceptor identity on the emitted event is Outer's own
    // scoped ID, not Inner's and not the button's. Outer lives at
    // the top of the scope chain (above the app's "root" column),
    // so its full ID is "main#root/outer".
    assert_eq!(
        widget.scoped_id.id, "outer",
        "emitted event's local id must be the interceptor's, not the inner target's"
    );
    assert_eq!(
        widget.scoped_id.window_id.as_deref(),
        Some("main"),
        "window_id must survive the Emit re-emit"
    );
    assert!(
        !widget.scoped_id.scope.iter().any(|s| s == "inner"),
        "emitted event's scope must not include Inner; found {:?}",
        widget.scoped_id.scope
    );
}
