//! Composite widget system for reusable, stateful components.
//!
//! A composite widget composes existing widgets (text, button, canvas,
//! etc.) with internal state and event interception. It's the Rust
//! equivalent of the Elixir SDK's `use Plushie.Widget`.
//!
//! Composite widgets produce [`View`] trees, not iced Elements. They
//! are expanded during tree normalization before the renderer sees
//! them. The renderer only sees built-in widget types.
//!
//! # Example: Star Rating
//!
//! ```ignore
//! use plushie::prelude::*;
//! use plushie::widget::{Widget, EventResult};
//!
//! struct StarRating;
//!
//! #[derive(Default)]
//! struct StarState {
//!     hover: Option<usize>,
//! }
//!
//! impl Widget for StarRating {
//!     type State = StarState;
//!
//!     fn view(id: &str, props: &Value, state: &StarState) -> View {
//!         row().id(id).spacing(4.0).children(
//!             (0..5).map(|i| {
//!                 let filled = /* check rating and hover */;
//!                 button(&format!("star-{i}"), if filled { "★" } else { "☆" })
//!             })
//!         ).into()
//!     }
//!
//!     fn handle_event(event: &Event, state: &mut StarState) -> EventResult {
//!         match event.widget_match() {
//!             Some(Click(id)) if id.starts_with("star-") => {
//!                 let n: usize = id["star-".len()..].parse().unwrap_or(0);
//!                 EventResult::emit("select", (n + 1).into())
//!             }
//!             Some(Enter(id)) if id.starts_with("star-") => {
//!                 state.hover = id["star-".len()..].parse().ok();
//!                 EventResult::UpdateState
//!             }
//!             Some(Exit(_)) => {
//!                 state.hover = None;
//!                 EventResult::UpdateState
//!             }
//!             _ => EventResult::Consumed,
//!         }
//!     }
//! }
//! ```

use serde_json::Value;

use crate::event::Event;
use crate::subscription::Subscription;
use crate::View;

/// A reusable, stateful widget that composes other widgets.
///
/// Composite widgets have internal state that persists across renders,
/// can intercept events from their children, and can declare their
/// own scoped subscriptions (e.g., timers for animation).
///
/// # Lifecycle
///
/// 1. **First render**: `State::default()` creates initial state.
/// 2. **View**: `view(id, props, state)` builds the widget's UI tree.
/// 3. **Event**: `handle_event(event, state)` intercepts child events.
/// 4. **Subscribe**: `subscribe(props, state)` declares scoped subscriptions.
/// 5. **Re-render**: After state changes, `view` is called again.
pub trait Widget: Send + Sync + 'static {
    /// Per-instance state persisted across renders.
    ///
    /// Must implement `Default` for initial state creation.
    type State: Default + Send + 'static;

    /// Build the widget's view tree.
    ///
    /// Called during normalization with the widget's props (from the
    /// parent's view) and the current internal state. Returns a
    /// `View` composed from built-in widgets.
    ///
    /// The `id` parameter is the widget's scoped ID in the parent tree.
    fn view(id: &str, props: &Value, state: &Self::State) -> View;

    /// Handle an event from an internal child widget.
    ///
    /// Return how to process the event:
    /// - [`EventResult::Emit`]: Transform and re-emit to the parent.
    /// - [`EventResult::UpdateState`]: Update internal state only.
    /// - [`EventResult::Consumed`]: Suppress the event.
    /// - [`EventResult::Ignored`]: Pass to the parent unchanged.
    fn handle_event(
        _event: &Event,
        _state: &mut Self::State,
    ) -> EventResult {
        EventResult::Ignored
    }

    /// Active subscriptions scoped to this widget instance.
    ///
    /// Timer events from these subscriptions are delivered through
    /// [`handle_event`](Widget::handle_event). The subscriptions
    /// are namespaced to avoid collisions with app subscriptions.
    fn subscribe(
        _props: &Value,
        _state: &Self::State,
    ) -> Vec<Subscription> {
        vec![]
    }
}

/// The result of handling an event in a composite widget.
#[derive(Debug)]
pub enum EventResult {
    /// Event handled: emit a transformed event to the parent.
    ///
    /// The emitted event arrives at the app's `update()` as a
    /// `WidgetEvent` with the given family and value.
    Emit {
        family: String,
        value: Value,
    },

    /// Event handled: update internal state only (no event emitted).
    ///
    /// The widget will be re-rendered with the new state.
    UpdateState,

    /// Event handled and suppressed (no event reaches the parent).
    Consumed,

    /// Event not handled by this widget. Pass to the parent
    /// unchanged.
    Ignored,
}

impl EventResult {
    /// Create an Emit result with the given family and value.
    pub fn emit(family: &str, value: impl Into<Value>) -> Self {
        Self::Emit {
            family: family.to_string(),
            value: value.into(),
        }
    }

    /// Create an Emit result with the family and update state.
    pub fn emit_and_update(family: &str, value: impl Into<Value>) -> Self {
        // Both effects happen: state is updated AND event is emitted.
        // The runtime handles this by applying the state change first,
        // then emitting. For now, represented as Emit (state mutation
        // already happened via &mut State before this returns).
        Self::Emit {
            family: family.to_string(),
            value: value.into(),
        }
    }
}
