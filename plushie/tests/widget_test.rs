//! Behavioral tests for the composite Widget trait and EventResult type.
//!
//! Defines a ToggleButton widget and exercises the public API
//! without needing a running app or renderer.

use plushie::prelude::*;
use plushie::widget::{EventResult, Widget};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Test widget: ToggleButton
// ---------------------------------------------------------------------------

struct ToggleButton;

#[derive(Default)]
struct ToggleState {
    pressed: bool,
}

impl Widget for ToggleButton {
    type State = ToggleState;

    fn view(id: &str, props: &Value, state: &ToggleState) -> View {
        let label = props
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("Toggle");
        let style = if state.pressed {
            Style::primary()
        } else {
            Style::secondary()
        };
        button(id, label).style(style).into()
    }

    fn handle_event(
        event: &Event,
        state: &mut ToggleState,
    ) -> EventResult {
        match event.widget_match() {
            Some(Click(_)) => {
                state.pressed = !state.pressed;
                EventResult::emit("toggled", state.pressed)
            }
            _ => EventResult::Ignored,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a synthetic click event for testing handle_event directly.
fn click_event(id: &str) -> Event {
    Event::Widget(plushie::event::WidgetEvent {
        event_type: plushie::event::EventType::Click,
        id: id.to_string(),
        window_id: "main".to_string(),
        scope: vec![],
        value: Value::Null,
    })
}

/// Build a synthetic input event (non-click) for ignored-path testing.
fn input_event(id: &str, text: &str) -> Event {
    Event::Widget(plushie::event::WidgetEvent {
        event_type: plushie::event::EventType::Input,
        id: id.to_string(),
        window_id: "main".to_string(),
        scope: vec![],
        value: Value::String(text.to_string()),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn widget_trait_can_be_implemented() {
    // The fact that ToggleButton compiles and satisfies Widget
    // is the assertion. Call view to exercise the vtable.
    let state = ToggleState::default();
    let _view = ToggleButton::view("t", &json!({}), &state);
}

#[test]
fn event_result_emit_carries_family_and_value() {
    let result = EventResult::emit("selected", 42);
    match result {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "selected");
            assert_eq!(value, json!(42));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
fn event_result_update_state_is_constructible() {
    let result = EventResult::UpdateState;
    assert!(matches!(result, EventResult::UpdateState));
}

#[test]
fn event_result_consumed_is_constructible() {
    let result = EventResult::Consumed;
    assert!(matches!(result, EventResult::Consumed));
}

#[test]
fn event_result_ignored_is_constructible() {
    let result = EventResult::Ignored;
    assert!(matches!(result, EventResult::Ignored));
}

#[test]
fn event_result_emit_convenience_constructor() {
    // emit() accepts anything that converts Into<Value>.
    let bool_emit = EventResult::emit("toggled", true);
    match bool_emit {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "toggled");
            assert_eq!(value, json!(true));
        }
        other => panic!("expected Emit, got {other:?}"),
    }

    let string_emit = EventResult::emit("changed", "hello");
    match string_emit {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "changed");
            assert_eq!(value, json!("hello"));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
fn widget_view_returns_valid_json() {
    let state = ToggleState { pressed: false };
    let props = json!({"label": "Press me"});
    let view = ToggleButton::view("toggle_btn", &props, &state);

    assert_eq!(view.id, "toggle_btn");
    assert_eq!(view.type_name, "button");
    assert_eq!(view.props["label"], "Press me");
    assert_eq!(view.props["style"], "secondary");
}

#[test]
fn widget_handle_event_modifies_state() {
    let mut state = ToggleState::default();
    assert!(!state.pressed);

    let event = click_event("toggle_btn");
    let _result = ToggleButton::handle_event(&event, &mut state);
    assert!(state.pressed);

    // Click again to toggle back.
    let event = click_event("toggle_btn");
    let _result = ToggleButton::handle_event(&event, &mut state);
    assert!(!state.pressed);
}

#[test]
fn widget_handle_event_returns_emit() {
    let mut state = ToggleState::default();
    let event = click_event("toggle_btn");
    let result = ToggleButton::handle_event(&event, &mut state);

    match result {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "toggled");
            // State was false, click flipped to true.
            assert_eq!(value, json!(true));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
fn widget_handle_event_ignores_non_click() {
    let mut state = ToggleState::default();
    let event = input_event("toggle_btn", "text");
    let result = ToggleButton::handle_event(&event, &mut state);

    assert!(matches!(result, EventResult::Ignored));
    assert!(!state.pressed, "state should not change on ignored event");
}

#[test]
fn widget_view_reflects_pressed_state() {
    let props = json!({"label": "Toggle"});

    let unpressed = ToggleButton::view("t", &props, &ToggleState { pressed: false });
    assert_eq!(unpressed.props["style"], "secondary");

    let pressed = ToggleButton::view("t", &props, &ToggleState { pressed: true });
    assert_eq!(pressed.props["style"], "primary");
}
