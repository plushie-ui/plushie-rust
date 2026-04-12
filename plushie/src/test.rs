//! Test infrastructure for plushie apps.
//!
//! [`TestSession`] provides a headless testing environment that
//! exercises the full MVU cycle (init -> update -> view) without
//! rendering. Composite widgets are expanded and their events
//! are intercepted, matching runtime behavior.
//!
//! ```ignore
//! use plushie::prelude::*;
//! use plushie::test::TestSession;
//!
//! let mut session = TestSession::<Counter>::start();
//! session.click("inc");
//! session.click("inc");
//! assert_eq!(session.model().count, 2);
//! ```

use plushie_core::protocol::TreeNode;
use serde_json::Value;

use crate::App;
use crate::event::{Event, EventType, WidgetEvent};
use crate::runtime;
use crate::widget::{EventResult, Interception, WidgetStateStore};

// ---------------------------------------------------------------------------
// TestSession
// ---------------------------------------------------------------------------

/// A headless test session for a plushie app.
///
/// Runs the app's MVU loop without rendering. Composite widgets are
/// expanded during each view cycle and their `handle_event` is called
/// before events reach the app's `update`.
pub struct TestSession<A: App> {
    model: A::Model,
    tree: TreeNode,
    widget_store: WidgetStateStore,
}

impl<A: App> TestSession<A> {
    /// Start a new test session by calling `App::init()`.
    pub fn start() -> Self {
        let (model, _cmd) = A::init();
        let mut widget_store = WidgetStateStore::new();
        let (tree, _) = runtime::prepare_tree::<A>(&model, &mut widget_store);
        Self {
            model,
            tree,
            widget_store,
        }
    }

    /// Access the current model state.
    pub fn model(&self) -> &A::Model {
        &self.model
    }

    /// Access the current model state mutably.
    pub fn model_mut(&mut self) -> &mut A::Model {
        &mut self.model
    }

    // -----------------------------------------------------------------------
    // Interactions
    // -----------------------------------------------------------------------

    /// Simulate a click on a widget with the given ID.
    pub fn click(&mut self, id: &str) {
        self.dispatch(widget_event(EventType::Click, id, Value::Null));
    }

    /// Simulate text input on a widget.
    pub fn type_text(&mut self, id: &str, text: &str) {
        self.dispatch(widget_event(
            EventType::Input,
            id,
            Value::String(text.to_string()),
        ));
    }

    /// Simulate a toggle on a checkbox or toggler.
    pub fn toggle(&mut self, id: &str, checked: bool) {
        self.dispatch(widget_event(EventType::Toggle, id, Value::Bool(checked)));
    }

    /// Simulate a selection on a pick list, combo box, or radio.
    pub fn select(&mut self, id: &str, value: &str) {
        self.dispatch(widget_event(
            EventType::Select,
            id,
            Value::String(value.to_string()),
        ));
    }

    /// Simulate a form submission (text input Enter key).
    pub fn submit(&mut self, id: &str, text: &str) {
        self.dispatch(widget_event(
            EventType::Submit,
            id,
            Value::String(text.to_string()),
        ));
    }

    /// Simulate a slider value change.
    pub fn slide(&mut self, id: &str, value: f64) {
        self.dispatch(widget_event(EventType::Slide, id, serde_json::json!(value)));
    }

    /// Dispatch a raw event through the widget interception layer
    /// and then to the app's update function.
    pub fn dispatch(&mut self, event: Event) {
        match self.widget_store.intercept_event(&event) {
            Some(Interception {
                result: EventResult::Consumed,
                ..
            })
            | Some(Interception {
                result: EventResult::UpdateState,
                ..
            }) => {
                // Widget handled it; don't deliver to app.
            }
            Some(Interception {
                result: EventResult::Emit { family, value },
                widget_id,
                outer_scope,
                window_id,
            }) => {
                let new_event = Event::Widget(WidgetEvent {
                    event_type: crate::event::family_to_event_type(&family),
                    id: widget_id,
                    window_id,
                    scope: outer_scope,
                    value,
                });
                let _cmd = A::update(&mut self.model, new_event);
            }
            Some(Interception {
                result: EventResult::Ignored,
                ..
            })
            | None => {
                let _cmd = A::update(&mut self.model, event);
            }
        }

        // Re-render and expand widgets.
        let (tree, _) = runtime::prepare_tree::<A>(&self.model, &mut self.widget_store);
        self.tree = tree;
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Find a node in the view tree by ID.
    pub fn find(&self, id: &str) -> Option<&TreeNode> {
        find_node(&self.tree, id)
    }

    /// Get the text content of a widget by ID.
    pub fn text_content(&self, id: &str) -> Option<String> {
        let node = self.find(id)?;
        node.props
            .get_str("content")
            .or_else(|| node.props.get_str("label"))
            .map(|s| s.to_string())
    }

    /// Get a string prop from a widget by ID and prop name.
    pub fn prop_str(&self, id: &str, key: &str) -> Option<&str> {
        let node = self.find(id)?;
        node.props.get_str(key)
    }

    /// Get a prop value from a widget by ID and prop name (Wire mode only).
    pub fn prop(&self, id: &str, key: &str) -> Option<&Value> {
        let node = self.find(id)?;
        node.props.get(key)
    }

    /// Get the current normalized view tree.
    pub fn tree(&self) -> &TreeNode {
        &self.tree
    }

    // -----------------------------------------------------------------------
    // Assertions
    // -----------------------------------------------------------------------

    /// Assert that a widget with the given ID exists in the view tree.
    pub fn assert_exists(&self, id: &str) {
        assert!(
            self.find(id).is_some(),
            "expected widget \"{id}\" to exist in the view tree"
        );
    }

    /// Assert that no widget with the given ID exists in the view tree.
    pub fn assert_not_exists(&self, id: &str) {
        assert!(
            self.find(id).is_none(),
            "expected widget \"{id}\" to NOT exist in the view tree"
        );
    }

    /// Assert that a text widget displays the expected content.
    pub fn assert_text(&self, id: &str, expected: &str) {
        let actual = self.text_content(id);
        assert_eq!(
            actual.as_deref(),
            Some(expected),
            "expected widget \"{id}\" to display \"{expected}\", got {:?}",
            actual
        );
    }

    /// Assert that a prop has the expected value.
    pub fn assert_prop(&self, id: &str, key: &str, expected: &Value) {
        let actual = self.prop(id, key);
        assert_eq!(
            actual,
            Some(expected),
            "expected widget \"{id}\" prop \"{key}\" to be {expected}, got {actual:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn widget_event(event_type: EventType, id: &str, value: Value) -> Event {
    Event::Widget(WidgetEvent {
        event_type,
        id: id.to_string(),
        window_id: "main".to_string(),
        scope: vec![],
        value,
    })
}

fn find_node<'a>(node: &'a TreeNode, target_id: &str) -> Option<&'a TreeNode> {
    // Extract local name: split on both # and /, take the last segment.
    let local_id = node
        .id
        .rsplit_once('/')
        .or_else(|| node.id.rsplit_once('#'))
        .map(|(_, l)| l)
        .unwrap_or(&node.id);
    if local_id == target_id || node.id == target_id {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_node(child, target_id) {
            return Some(found);
        }
    }
    None
}
