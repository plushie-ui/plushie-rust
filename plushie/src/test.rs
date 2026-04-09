//! Test infrastructure for plushie apps.
//!
//! [`TestSession`] provides a headless testing environment that
//! exercises the full MVU cycle (init -> update -> view) without
//! rendering. Tests interact with the app through the same event
//! types used at runtime.
//!
//! ```
//! use plushie::prelude::*;
//! use plushie::test::TestSession;
//!
//! # struct Counter { count: i32 }
//! # impl App for Counter {
//! #     type Model = Self;
//! #     fn init() -> (Self, Command) { (Counter { count: 0 }, Command::none()) }
//! #     fn update(m: &mut Self, e: Event) -> Command {
//! #         match e.widget_match() {
//! #             Some(Click("inc")) => m.count += 1,
//! #             _ => {}
//! #         }
//! #         Command::none()
//! #     }
//! #     fn view(m: &Self) -> View {
//! #         window("main").child(text(&format!("{}", m.count))).into()
//! #     }
//! # }
//! let mut session = TestSession::<Counter>::start();
//! session.click("inc");
//! session.click("inc");
//! assert_eq!(session.model().count, 2);
//! ```

use serde_json::Value;

use crate::event::{Event, EventType, WidgetEvent};
use crate::runtime::normalize;
use crate::App;

// ---------------------------------------------------------------------------
// TestSession
// ---------------------------------------------------------------------------

/// A headless test session for a plushie app.
///
/// Runs the app's MVU loop without rendering: init creates the model,
/// interactions dispatch events through update, and the view tree is
/// available for assertions.
///
/// ```ignore
/// let mut session = TestSession::<MyApp>::start();
/// session.click("save");
/// assert_eq!(session.model().saved, true);
/// session.assert_text("status", "Saved!");
/// ```
pub struct TestSession<A: App> {
    model: A::Model,
    tree: Value,
}

impl<A: App> TestSession<A> {
    /// Start a new test session by calling `App::init()`.
    pub fn start() -> Self {
        let (model, _cmd) = A::init();
        let view = A::view(&model);
        let (tree, _) = normalize::normalize(&view.0);
        Self { model, tree }
    }

    /// Access the current model state.
    pub fn model(&self) -> &A::Model {
        &self.model
    }

    /// Access the current model state mutably (for assertions on
    /// interior state that isn't directly observable through the view).
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
        self.dispatch(widget_event(
            EventType::Toggle,
            id,
            Value::Bool(checked),
        ));
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
        self.dispatch(widget_event(
            EventType::Slide,
            id,
            serde_json::json!(value),
        ));
    }

    /// Dispatch a raw event to the app's update function.
    pub fn dispatch(&mut self, event: Event) {
        let _cmd = A::update(&mut self.model, event);
        // Re-render the view after each update.
        let view = A::view(&self.model);
        let (tree, _) = normalize::normalize(&view.0);
        self.tree = tree;
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Find a node in the view tree by ID.
    pub fn find(&self, id: &str) -> Option<&Value> {
        find_node(&self.tree, id)
    }

    /// Get the text content of a widget by ID.
    ///
    /// Looks for the "content" prop (used by text widgets) or
    /// "label" prop (used by buttons).
    pub fn text_content(&self, id: &str) -> Option<String> {
        let node = self.find(id)?;
        node["props"]["content"]
            .as_str()
            .or_else(|| node["props"]["label"].as_str())
            .map(|s| s.to_string())
    }

    /// Get a prop value from a widget by ID and prop name.
    pub fn prop(&self, id: &str, key: &str) -> Option<&Value> {
        let node = self.find(id)?;
        let val = &node["props"][key];
        if val.is_null() { None } else { Some(val) }
    }

    /// Get the current normalized view tree as JSON.
    pub fn tree(&self) -> &Value {
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

/// Create a widget event for testing.
fn widget_event(event_type: EventType, id: &str, value: Value) -> Event {
    Event::Widget(WidgetEvent {
        event_type,
        id: id.to_string(),
        window_id: "main".to_string(),
        scope: vec![],
        value,
    })
}

/// Recursively search for a node by ID in a JSON tree.
fn find_node<'a>(node: &'a Value, target_id: &str) -> Option<&'a Value> {
    let id = node["id"].as_str().unwrap_or("");

    // Check if the node's local ID matches (strip scope prefix).
    let local_id = id.rsplit_once('/').map(|(_, l)| l).unwrap_or(id);
    if local_id == target_id || id == target_id {
        return Some(node);
    }

    // Search children.
    if let Some(children) = node["children"].as_array() {
        for child in children {
            if let Some(found) = find_node(child, target_id) {
                return Some(found);
            }
        }
    }

    None
}
