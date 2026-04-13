//! Test infrastructure for plushie apps.
//!
//! [`TestSession`] provides a headless testing environment that
//! exercises the full MVU cycle (init -> update -> view) without
//! rendering. Composite widgets are expanded and their events
//! are intercepted, matching runtime behavior.
//!
//! Interactions accept any [`Selector`] type. Bare strings are
//! automatically converted to ID selectors:
//!
//! ```ignore
//! use plushie::prelude::*;
//! use plushie::test::TestSession;
//!
//! let mut session = TestSession::<Counter>::start();
//! session.click("inc");                         // by ID
//! session.click(Selector::role("button"));      // by role
//! assert_eq!(session.model().count, 1);
//!
//! let btn = session.find("inc").unwrap();
//! assert_eq!(btn.widget_type(), "button");
//! ```

use plushie_core::protocol::TreeNode;
use plushie_core::Selector;
use serde_json::Value;

use crate::automation::Element;
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
///
/// All interaction methods accept `impl Into<Selector>`, so you can
/// pass bare `&str` (ID selector) or a typed `Selector` for richer
/// matching (text, role, label, focused).
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
    // Selector resolution
    // -----------------------------------------------------------------------

    /// Resolve a selector to a tree node, panicking with a clear
    /// message if the widget is not found.
    fn resolve(&self, selector: impl Into<Selector>) -> &TreeNode {
        let sel = selector.into();
        sel.find(&self.tree).unwrap_or_else(|| {
            panic!("widget not found: {sel}");
        })
    }

    // -----------------------------------------------------------------------
    // Interactions
    // -----------------------------------------------------------------------

    /// Simulate a click on a widget.
    pub fn click(&mut self, selector: impl Into<Selector>) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(EventType::Click, &id, Value::Null));
    }

    /// Simulate text input on a widget.
    pub fn type_text(&mut self, selector: impl Into<Selector>, text: &str) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Input,
            &id,
            Value::String(text.to_string()),
        ));
    }

    /// Simulate a toggle on a checkbox or toggler.
    pub fn toggle(&mut self, selector: impl Into<Selector>, checked: bool) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Toggle,
            &id,
            Value::Bool(checked),
        ));
    }

    /// Simulate a selection on a pick list, combo box, or radio.
    pub fn select(&mut self, selector: impl Into<Selector>, value: &str) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Select,
            &id,
            Value::String(value.to_string()),
        ));
    }

    /// Simulate a form submission (text input Enter key).
    pub fn submit(&mut self, selector: impl Into<Selector>, text: &str) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Submit,
            &id,
            Value::String(text.to_string()),
        ));
    }

    /// Simulate a slider value change.
    pub fn slide(&mut self, selector: impl Into<Selector>, value: f64) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(EventType::Slide, &id, serde_json::json!(value)));
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
                    scoped_id: plushie_core::ScopedId::new(widget_id, outer_scope, Some(window_id)),
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

    /// Find a widget in the view tree by selector.
    ///
    /// Returns an [`Element`] wrapper with typed accessors, or
    /// `None` if no matching widget exists.
    pub fn find(&self, selector: impl Into<Selector>) -> Option<Element<'_>> {
        let sel = selector.into();
        sel.find(&self.tree).map(Element::new)
    }

    /// Get the text content of a widget.
    pub fn text_content(&self, selector: impl Into<Selector>) -> Option<String> {
        self.find(selector)?.text().map(|s| s.to_string())
    }

    /// Get a string prop from a widget.
    pub fn prop_str(&self, selector: impl Into<Selector>, key: &str) -> Option<String> {
        self.find(selector)?.prop_str(key).map(|s| s.to_string())
    }

    /// Get a prop value from a widget as an owned JSON Value.
    pub fn prop(&self, selector: impl Into<Selector>, key: &str) -> Option<Value> {
        self.find(selector)?.prop(key)
    }

    /// Get the current normalized view tree.
    pub fn tree(&self) -> &TreeNode {
        &self.tree
    }

    // -----------------------------------------------------------------------
    // Assertions
    // -----------------------------------------------------------------------

    /// Assert that a matching widget exists in the view tree.
    pub fn assert_exists(&self, selector: impl Into<Selector>) {
        let sel = selector.into();
        assert!(
            sel.find(&self.tree).is_some(),
            "expected widget {sel} to exist in the view tree"
        );
    }

    /// Assert that no matching widget exists in the view tree.
    pub fn assert_not_exists(&self, selector: impl Into<Selector>) {
        let sel = selector.into();
        assert!(
            sel.find(&self.tree).is_none(),
            "expected widget {sel} to NOT exist in the view tree"
        );
    }

    /// Assert that a widget displays the expected text content.
    pub fn assert_text(&self, selector: impl Into<Selector>, expected: &str) {
        let sel = selector.into();
        let actual = sel
            .find(&self.tree)
            .and_then(|n| Element::new(n).text().map(|s| s.to_string()));
        assert_eq!(
            actual.as_deref(),
            Some(expected),
            "expected widget {sel} to display \"{expected}\", got {actual:?}",
        );
    }

    /// Assert that a widget prop has the expected value.
    pub fn assert_prop(&self, selector: impl Into<Selector>, key: &str, expected: &Value) {
        let sel = selector.into();
        let actual = sel
            .find(&self.tree)
            .and_then(|n| n.props.get_value(key));
        assert_eq!(
            actual.as_ref(),
            Some(expected),
            "expected widget {sel} prop \"{key}\" to be {expected}, got {actual:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn widget_event(event_type: EventType, id: &str, value: Value) -> Event {
    Event::Widget(WidgetEvent {
        event_type,
        scoped_id: plushie_core::ScopedId::parse(id),
        value,
    })
}
