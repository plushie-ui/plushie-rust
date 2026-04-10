//! Composite widget system for reusable, stateful components.
//!
//! A composite widget composes existing widgets (text, button, canvas,
//! etc.) with internal state and event interception. It's the Rust
//! equivalent of the Elixir SDK's `use Plushie.Widget`.
//!
//! # Creating a widget
//!
//! ```ignore
//! use plushie::prelude::*;
//! use plushie::widget::{Widget, EventResult, WidgetView};
//!
//! struct StarRating;
//!
//! #[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
//! struct StarState { hover: Option<usize> }
//!
//! impl Widget for StarRating {
//!     type State = StarState;
//!
//!     fn view(id: &str, props: &Value, state: &StarState) -> View {
//!         // Build view from existing widgets
//!         row().id(id).spacing(4.0).children(
//!             (0..5).map(|i| button(&format!("star-{i}"), "★"))
//!         ).into()
//!     }
//!
//!     fn handle_event(event: &Event, state: &mut StarState) -> EventResult {
//!         match event.widget_match() {
//!             Some(Click(id)) if id.starts_with("star-") => {
//!                 EventResult::emit("select", 1)
//!             }
//!             _ => EventResult::Consumed,
//!         }
//!     }
//! }
//! ```
//!
//! # Using a widget in a view
//!
//! ```ignore
//! fn view(model: &Self) -> View {
//!     window("main").child(
//!         column().children([
//!             WidgetView::<StarRating>::new("rating")
//!                 .prop("rating", model.rating),
//!             text(&format!("Rating: {}", model.rating)),
//!         ])
//!     ).into()
//! }
//! ```

use std::collections::HashMap;

use serde_json::Value;

use crate::event::Event;
use crate::subscription::Subscription;
use crate::View;

// ---------------------------------------------------------------------------
// Widget trait
// ---------------------------------------------------------------------------

/// A reusable, stateful widget that composes other widgets.
///
/// Composite widgets have internal state that persists across renders,
/// can intercept events from their children, and can declare their
/// own scoped subscriptions.
///
/// State must implement `Default` (for initial creation), `Clone`
/// (for undo support), and `Serialize`/`Deserialize` (for state
/// persistence across renders).
pub trait Widget: Send + Sync + 'static {
    /// Per-instance state persisted across renders.
    type State: Default + Clone + Send + serde::Serialize + serde::de::DeserializeOwned + 'static;

    /// Build the widget's view tree from props and internal state.
    fn view(id: &str, props: &Value, state: &Self::State) -> View;

    /// Handle an event from an internal child widget.
    fn handle_event(
        _event: &Event,
        _state: &mut Self::State,
    ) -> EventResult {
        EventResult::Ignored
    }

    /// Active subscriptions scoped to this widget instance.
    fn subscribe(
        _props: &Value,
        _state: &Self::State,
    ) -> Vec<Subscription> {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// EventResult
// ---------------------------------------------------------------------------

/// The result of handling an event in a composite widget.
#[derive(Debug)]
pub enum EventResult {
    /// Emit a transformed event to the parent.
    Emit { family: String, value: Value },
    /// Update internal state only (no event emitted).
    UpdateState,
    /// Event handled and suppressed.
    Consumed,
    /// Event not handled, pass to parent unchanged.
    Ignored,
}

impl EventResult {
    /// Create an Emit result.
    pub fn emit(family: &str, value: impl Into<Value>) -> Self {
        Self::Emit {
            family: family.to_string(),
            value: value.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// WidgetView - placeholder builder for using widgets in views
// ---------------------------------------------------------------------------

/// A view placeholder for a composite widget.
///
/// Created via `WidgetView::<MyWidget>::new("id")`. When the view
/// tree is expanded, the widget's `view()` method is called with
/// the stored props and the widget's persisted state.
///
/// ```ignore
/// use plushie::widget::WidgetView;
///
/// WidgetView::<StarRating>::new("rating")
///     .prop("rating", 4)
///     .prop("readonly", false)
/// ```
pub struct WidgetView<W: Widget> {
    id: String,
    props: serde_json::Map<String, Value>,
    _marker: std::marker::PhantomData<W>,
}

impl<W: Widget> WidgetView<W> {
    /// Create a widget placeholder with the given ID.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            props: serde_json::Map::new(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Set a prop on the widget.
    pub fn prop(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.props.insert(key.to_string(), value.into());
        self
    }
}

impl<W: Widget> From<WidgetView<W>> for View {
    fn from(wv: WidgetView<W>) -> View {
        // Store the widget's type info as a special __widget__ node.
        // The expansion function is stored as a boxed trait object
        // in a thread-local registry, keyed by the node ID.
        let expander = Box::new(WidgetExpander::<W>(std::marker::PhantomData));
        register_widget_expander(&wv.id, expander);

        let mut props = wv.props;
        props.insert("__widget__".to_string(), Value::Bool(true));

        View(serde_json::json!({
            "id": wv.id,
            "type": "__widget__",
            "props": Value::Object(props),
            "children": [],
        }))
    }
}

// ---------------------------------------------------------------------------
// Type-erased widget expansion
// ---------------------------------------------------------------------------

/// Type-erased interface for expanding and handling widget events.
pub(crate) trait DynWidgetExpander: Send {
    fn expand(&self, id: &str, props: &Value, state_json: &Value) -> View;
    fn handle_event(&self, event: &Event, state_json: &mut Value) -> EventResult;
    fn default_state_json(&self) -> Value;
}

struct WidgetExpander<W: Widget>(std::marker::PhantomData<W>);

impl<W: Widget> DynWidgetExpander for WidgetExpander<W> {
    fn expand(&self, id: &str, props: &Value, state_json: &Value) -> View {
        let state: W::State = serde_json::from_value(state_json.clone())
            .unwrap_or_default();
        W::view(id, props, &state)
    }

    fn handle_event(&self, event: &Event, state_json: &mut Value) -> EventResult {
        let mut state: W::State = serde_json::from_value(state_json.clone())
            .unwrap_or_default();
        let result = W::handle_event(event, &mut state);
        *state_json = serde_json::to_value(&state).unwrap_or(Value::Null);
        result
    }

    fn default_state_json(&self) -> Value {
        serde_json::to_value(W::State::default()).unwrap_or(Value::Null)
    }
}

// Thread-local registry for widget expanders. Populated during view()
// construction, consumed during expansion.
thread_local! {
    static WIDGET_EXPANDERS: std::cell::RefCell<HashMap<String, Box<dyn DynWidgetExpander>>>
        = std::cell::RefCell::new(HashMap::new());
}

fn register_widget_expander(id: &str, expander: Box<dyn DynWidgetExpander>) {
    WIDGET_EXPANDERS.with(|map| {
        map.borrow_mut().insert(id.to_string(), expander);
    });
}

/// Take a widget expander from the thread-local registry.
pub(crate) fn take_widget_expander(id: &str) -> Option<Box<dyn DynWidgetExpander>> {
    WIDGET_EXPANDERS.with(|map| {
        map.borrow_mut().remove(id)
    })
}

// ---------------------------------------------------------------------------
// Widget state store (used by TestSession and runners)
// ---------------------------------------------------------------------------

/// Stores per-widget-instance state and expanders.
pub(crate) struct WidgetStateStore {
    states: HashMap<String, Value>,
    expanders: HashMap<String, Box<dyn DynWidgetExpander>>,
}

impl WidgetStateStore {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            expanders: HashMap::new(),
        }
    }

    /// Expand all __widget__ nodes in a view tree.
    ///
    /// Walks the tree, finds nodes with type "__widget__", and replaces
    /// them with the expanded view from `Widget::view()`. Widget state
    /// is persisted between expansions.
    pub fn expand_widgets(&mut self, tree: &Value) -> Value {
        self.collect_expanders(tree);
        self.expand_node(tree)
    }

    /// Collect expanders from the thread-local registry for all
    /// __widget__ nodes in the tree.
    fn collect_expanders(&mut self, node: &Value) {
        let type_name = node["type"].as_str().unwrap_or("");
        let id = node["id"].as_str().unwrap_or("");

        if type_name == "__widget__"
            && let Some(expander) = take_widget_expander(id)
        {
            if !self.states.contains_key(id) {
                self.states.insert(id.to_string(), expander.default_state_json());
            }
            self.expanders.insert(id.to_string(), expander);
        }

        if let Some(children) = node["children"].as_array() {
            for child in children {
                self.collect_expanders(child);
            }
        }
    }

    fn expand_node(&self, node: &Value) -> Value {
        let type_name = node["type"].as_str().unwrap_or("");
        let id = node["id"].as_str().unwrap_or("");

        if type_name == "__widget__"
            && let Some(expander) = self.expanders.get(id)
        {
            let state = self.states.get(id)
                .cloned()
                .unwrap_or(Value::Null);
            let props = &node["props"];
            let expanded = expander.expand(id, props, &state);
            return self.expand_node(&expanded.0);
        }

        // Recursively expand children.
        let children = node["children"]
            .as_array()
            .map(|arr| arr.iter().map(|c| self.expand_node(c)).collect::<Vec<_>>())
            .unwrap_or_default();

        let mut result = node.clone();
        if let Some(obj) = result.as_object_mut() {
            obj.insert("children".to_string(), Value::Array(children));
        }
        result
    }

    /// Handle an event through widget interception.
    ///
    /// Checks if the event targets a child of a registered widget.
    /// If so, calls the widget's handle_event and returns the result.
    pub fn intercept_event(&mut self, event: &Event) -> Option<EventResult> {
        // Get the widget ID from the event's scope.
        let scope = match event {
            Event::Widget(w) => &w.scope,
            _ => return None,
        };

        // Walk the scope chain (innermost to outermost) looking
        // for a registered widget.
        for ancestor_id in scope {
            if let Some(expander) = self.expanders.get(ancestor_id) {
                let state = self.states.entry(ancestor_id.clone())
                    .or_insert_with(|| expander.default_state_json());
                let result = expander.handle_event(event, state);
                match result {
                    EventResult::Ignored => continue,
                    other => return Some(other),
                }
            }
        }

        None
    }
}
