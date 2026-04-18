//! Shared test helpers used across the plushie integration test suite.
//!
//! Rust's integration tests are separate binaries. Each test file
//! that wants these helpers must add `mod common;` at the top. Items
//! go unused when a file only imports a subset, so `#[allow(dead_code)]`
//! is applied at the module level to keep the helpers additive.

#![allow(dead_code)]

use plushie::automation::Element;
use plushie::event::{Event, EventType, WidgetEvent};
use plushie_core::ScopedId;
use plushie_core::protocol::{Props, TreeNode};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Synthetic widget events (used by event_test, widget_test, and others)
// ---------------------------------------------------------------------------

/// Build a synthetic click event targeted at `id` in the "main" window.
pub fn click_event(id: &str) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Click,
        scoped_id: ScopedId::new(id, vec![], Some("main".to_string())),
        value: Value::Null,
    })
}

/// Build a synthetic input event carrying text.
pub fn input_event(id: &str, text: &str) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Input,
        scoped_id: ScopedId::new(id, vec![], Some("main".to_string())),
        value: json!(text),
    })
}

/// Build a synthetic toggle event.
pub fn toggle_event(id: &str, checked: bool) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Toggle,
        scoped_id: ScopedId::new(id, vec![], Some("main".to_string())),
        value: json!(checked),
    })
}

/// Build a synthetic slider-change event.
pub fn slide_event(id: &str, value: f64) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Slide,
        scoped_id: ScopedId::new(id, vec![], Some("main".to_string())),
        value: json!(value),
    })
}

/// Build a synthetic click event with an explicit scope chain.
pub fn scoped_click(id: &str, scope: Vec<&str>) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Click,
        scoped_id: ScopedId::new(
            id,
            scope.into_iter().map(String::from).collect(),
            Some("main".to_string()),
        ),
        value: Value::Null,
    })
}

// ---------------------------------------------------------------------------
// Synthetic tree nodes (used by automation_test and similar)
// ---------------------------------------------------------------------------

/// Build a `text` leaf node with the given id and content.
pub fn text_node(id: &str, content: &str) -> TreeNode {
    let mut props = serde_json::Map::new();
    props.insert("content".to_string(), json!(content));
    TreeNode {
        id: id.to_string(),
        type_name: "text".to_string(),
        props: Props::from_json(Value::Object(props)),
        children: vec![],
    }
}

/// Build a `button` leaf node with the given id and label.
pub fn button_node(id: &str, label: &str) -> TreeNode {
    let mut props = serde_json::Map::new();
    props.insert("label".to_string(), json!(label));
    TreeNode {
        id: id.to_string(),
        type_name: "button".to_string(),
        props: Props::from_json(Value::Object(props)),
        children: vec![],
    }
}

/// Build a `column` container node with the given id and children.
pub fn container_node(id: &str, children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: "column".to_string(),
        props: Props::from_json(Value::Object(Default::default())),
        children,
    }
}

/// Build a `window` node with the given id and children.
pub fn window_node(id: &str, children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: "window".to_string(),
        props: Props::from_json(Value::Object(Default::default())),
        children,
    }
}

/// Build an a11y-annotated leaf node of the given type and role.
pub fn a11y_node(id: &str, type_name: &str, role: &str) -> TreeNode {
    let mut props = serde_json::Map::new();
    props.insert("a11y".to_string(), json!({"role": role}));
    TreeNode {
        id: id.to_string(),
        type_name: type_name.to_string(),
        props: Props::from_json(Value::Object(props)),
        children: vec![],
    }
}

/// Wrap a `TreeNode` in the automation `Element` wrapper for tests
/// that want the typed accessors.
pub fn element(node: &TreeNode) -> Element<'_> {
    Element::new(node)
}
