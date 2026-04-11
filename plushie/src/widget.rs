//! Composite widget system for reusable, stateful components.
//!
//! A composite widget composes existing widgets (text, button, canvas,
//! etc.) with internal state and event interception.
//!
//! # Defining a widget
//!
//! ```ignore
//! use plushie::prelude::*;
//! use plushie::widget::{Widget, EventResult, WidgetView};
//!
//! struct StarRating;
//!
//! #[derive(Default)]
//! struct StarState { hover: Option<usize> }
//!
//! impl Widget for StarRating {
//!     type State = StarState;
//!
//!     fn view(id: &str, props: &Value, state: &StarState) -> View {
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
//!         column()
//!             .child(WidgetView::<StarRating>::new("rating")
//!                 .prop("rating", model.rating))
//!             .child(text(&format!("Rating: {}", model.rating)))
//!     ).into()
//! }
//! ```

use std::any::Any;
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
/// State must implement `Default` for initial creation. No
/// serialization constraints: state is stored in memory as the
/// concrete Rust type using `Box<dyn Any>`.
pub trait Widget: Send + Sync + 'static {
    /// Per-instance state persisted across renders.
    type State: Default + Send + 'static;

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

/// The result of widget interception, including context about which
/// widget intercepted the event and the remaining scope above it.
#[derive(Debug)]
pub struct Interception {
    /// What the widget decided to do with the event.
    pub result: EventResult,
    /// The ID of the composite widget that intercepted the event.
    pub widget_id: String,
    /// Scope chain above the intercepting widget (for Emit events).
    pub outer_scope: Vec<String>,
    /// The window the event originated from.
    pub window_id: String,
}

// ---------------------------------------------------------------------------
// WidgetView - placeholder builder for using widgets in views
// ---------------------------------------------------------------------------

/// A view placeholder for a composite widget.
///
/// When the view tree is expanded, the widget's `view()` method is
/// called with the stored props and the widget's persisted state.
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

impl<W: Widget> WidgetView<W> {
    /// Register the widget expander and produce a View placeholder.
    ///
    /// Call this inside `App::view` to place a composite widget in
    /// the view tree:
    ///
    /// ```ignore
    /// fn view(model: &Self::Model, widgets: &mut WidgetRegistrar) -> View {
    ///     window("main").child(
    ///         WidgetView::<StarRating>::new("rating")
    ///             .prop("rating", model.rating)
    ///             .register(widgets)
    ///     ).into()
    /// }
    /// ```
    pub fn register(self, registrar: &mut WidgetRegistrar) -> View {
        let expander: Box<dyn DynWidgetExpander> =
            Box::new(WidgetExpander::<W>(std::marker::PhantomData));
        registrar.register(self.id.clone(), expander);

        let mut props = self.props;
        props.insert("__widget__".to_string(), Value::Bool(true));

        View {
            id: self.id,
            type_name: "__widget__".to_string(),
            props: Value::Object(props),
            children: vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Type-erased widget expansion (using Box<dyn Any> for state)
// ---------------------------------------------------------------------------

/// Type-erased interface for expanding widgets and handling events.
pub(crate) trait DynWidgetExpander: Send {
    fn expand(&self, id: &str, props: &Value, state: &dyn Any) -> View;
    fn handle_event(&self, event: &Event, state: &mut dyn Any) -> EventResult;
    fn default_state(&self) -> Box<dyn Any + Send>;
}

struct WidgetExpander<W: Widget>(std::marker::PhantomData<W>);

impl<W: Widget> DynWidgetExpander for WidgetExpander<W> {
    fn expand(&self, id: &str, props: &Value, state: &dyn Any) -> View {
        let state = state.downcast_ref::<W::State>()
            .expect("widget state type mismatch");
        W::view(id, props, state)
    }

    fn handle_event(&self, event: &Event, state: &mut dyn Any) -> EventResult {
        let state = state.downcast_mut::<W::State>()
            .expect("widget state type mismatch");
        W::handle_event(event, state)
    }

    fn default_state(&self) -> Box<dyn Any + Send> {
        Box::new(W::State::default())
    }
}

// ---------------------------------------------------------------------------
// WidgetRegistrar
// ---------------------------------------------------------------------------

/// Collects widget expanders during `App::view()`.
///
/// Passed to `App::view()` so composite widgets can register their
/// type-erased expanders explicitly rather than through a thread-local.
pub struct WidgetRegistrar {
    expanders: HashMap<String, Box<dyn DynWidgetExpander>>,
}

impl WidgetRegistrar {
    pub fn new() -> Self {
        Self { expanders: HashMap::new() }
    }

    /// Register a widget expander for the given ID.
    pub(crate) fn register(&mut self, id: String, expander: Box<dyn DynWidgetExpander>) {
        self.expanders.insert(id, expander);
    }

    /// Take all registered expanders (consumed by WidgetStateStore).
    pub(crate) fn take_all(self) -> HashMap<String, Box<dyn DynWidgetExpander>> {
        self.expanders
    }
}

// ---------------------------------------------------------------------------
// Widget state store
// ---------------------------------------------------------------------------

/// Stores per-widget-instance state and expanders.
pub(crate) struct WidgetStateStore {
    states: HashMap<String, Box<dyn Any + Send>>,
    expanders: HashMap<String, Box<dyn DynWidgetExpander>>,
}

impl WidgetStateStore {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            expanders: HashMap::new(),
        }
    }

    /// Expand all __widget__ nodes in a TreeNode tree.
    ///
    /// Merges expanders from the registrar (collected during
    /// `App::view()`) and then recursively expands widget
    /// placeholder nodes by calling their `view()` methods.
    pub fn expand_tree(&mut self, tree: &View, registrar: WidgetRegistrar) -> View {
        // Merge newly registered expanders and initialize state
        // for any widgets we haven't seen before.
        for (id, expander) in registrar.take_all() {
            if !self.states.contains_key(&id) {
                self.states.insert(id.clone(), expander.default_state());
            }
            self.expanders.insert(id, expander);
        }
        self.expand_node(tree)
    }

    fn expand_node(&self, node: &View) -> View {
        if node.type_name == "__widget__" {
            if let Some(expander) = self.expanders.get(&node.id) {
                let state = self.states.get(&node.id).expect("widget state missing");
                let expanded = expander.expand(&node.id, &node.props, state.as_ref());
                return self.expand_node(&expanded);
            }
        }

        let children = node.children.iter()
            .map(|c| self.expand_node(c))
            .collect();

        View {
            id: node.id.clone(),
            type_name: node.type_name.clone(),
            props: node.props.clone(),
            children,
        }
    }

    /// Handle an event through widget interception.
    ///
    /// Walks the event's scope chain (innermost ancestor first) and
    /// gives each registered composite widget a chance to handle the
    /// event. Returns the result along with the interceptor's ID and
    /// the remaining scope above it.
    pub fn intercept_event(&mut self, event: &Event) -> Option<Interception> {
        let (scope, window_id) = match event {
            Event::Widget(w) => (&w.scope, &w.window_id),
            _ => return None,
        };

        for (i, ancestor_id) in scope.iter().enumerate() {
            if let Some(expander) = self.expanders.get(ancestor_id) {
                let state = self.states.get_mut(ancestor_id)?;
                let result = expander.handle_event(event, state.as_mut());
                match result {
                    EventResult::Ignored => continue,
                    other => {
                        return Some(Interception {
                            result: other,
                            widget_id: ancestor_id.clone(),
                            // Remaining scope above the interceptor.
                            outer_scope: scope[i + 1..].to_vec(),
                            window_id: window_id.clone(),
                        });
                    }
                }
            }
        }

        None
    }
}
