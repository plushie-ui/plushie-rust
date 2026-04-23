//! Application builder for registering widgets.
//!
//! Create a [`PlushieAppBuilder`], register widgets (via
//! [`PlushieWidget`]), and pass it
//! to `plushie::run()`.
//!
//! # Example
//!
//! ```ignore
//! use plushie_widget_sdk::app::PlushieAppBuilder;
//! use plushie_widget_sdk::runtime::iced_widget_set;
//!
//! fn main() -> iced::Result {
//!     plushie::run(
//!         PlushieAppBuilder::new()
//!             .widget_set(&iced_widget_set())
//!             .widget(MyGauge::new())
//!     )
//! }
//! ```

use std::sync::Arc;

use crate::PlushieRenderer;
use crate::registry::{PlushieWidget, WidgetRegistry, WidgetSet};

/// Factory closure that produces a fresh [`WidgetRegistry`] for a
/// session.
///
/// Used by the renderer binary's multiplexed headless/mock path so
/// each session thread can construct its own registry without sharing
/// state across threads. The closure must be `Send + Sync` because it
/// is invoked from session worker threads; the registries it creates
/// do not need to be `Send`.
pub type SessionRegistryFactory<R> = Arc<dyn Fn() -> WidgetRegistry<R> + Send + Sync + 'static>;

/// Builder for registering widgets before starting the renderer.
///
/// All widgets are registered in the [`WidgetRegistry`] via the
/// [`PlushieWidget`] trait.
///
/// The `R` parameter selects the renderer backend. Defaults to
/// `iced::Renderer` which is used by headless and windowed modes.
///
/// The optional [`session_factory`](Self::with_session_factory) slot
/// provides a closure that builds a fresh registry for each session
/// in multiplexed headless/mock mode. Apps that use custom widgets
/// need to supply a factory so those widgets are available in every
/// session; without one, sessions fall back to the built-in iced set.
pub struct PlushieAppBuilder<R: PlushieRenderer = iced::Renderer> {
    registry: WidgetRegistry<R>,
    session_factory: Option<SessionRegistryFactory<R>>,
}

impl<R: PlushieRenderer> std::fmt::Debug for PlushieAppBuilder<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlushieAppBuilder")
            .field("registry", &self.registry)
            .finish()
    }
}

impl<R: PlushieRenderer> PlushieAppBuilder<R> {
    /// Create an empty builder with no widgets registered.
    pub fn new() -> Self {
        Self {
            registry: WidgetRegistry::new(),
            session_factory: None,
        }
    }

    /// Attach a factory closure that constructs a fresh
    /// [`WidgetRegistry`] for each session in multiplexed mode.
    ///
    /// The closure is invoked once per session thread. It must
    /// register every widget the app needs (including the built-in
    /// iced set via [`iced_widget_set`](crate::runtime::iced_widget_set)
    /// if the app uses them).
    ///
    /// Only consulted by `--max-sessions > 1` headless / mock modes.
    /// Windowed mode and single-session headless / mock build the
    /// registry from the builder's own accumulated registrations.
    pub fn with_session_factory(
        mut self,
        factory: impl Fn() -> WidgetRegistry<R> + Send + Sync + 'static,
    ) -> Self {
        self.session_factory = Some(Arc::new(factory));
        self
    }

    /// Take the session factory out of the builder, if any.
    pub fn take_session_factory(&mut self) -> Option<SessionRegistryFactory<R>> {
        self.session_factory.take()
    }

    /// Register a [`PlushieWidget`] implementation.
    ///
    /// # Panics
    ///
    /// Panics if any of the widget's type names is already
    /// registered. Use [`widget_override`](Self::widget_override) for
    /// an intentional override (e.g. a custom Button that shadows
    /// the built-in one).
    ///
    /// Collision here is almost always a typo (the widget picked
    /// "button" when the author meant "my_button"); failing loud
    /// catches it instead of silently hijacking the built-in.
    pub fn widget(mut self, widget: impl PlushieWidget<R> + 'static) -> Self {
        self.registry.register_strict(Box::new(widget));
        self
    }

    /// Register a widget, replacing any existing registration for the
    /// same type names. Use this when the override is deliberate.
    pub fn widget_override(mut self, widget: impl PlushieWidget<R> + 'static) -> Self {
        self.registry.register(Box::new(widget));
        self
    }

    /// Register all widgets from a [`WidgetSet`].
    ///
    /// For type name collisions with previously registered widgets,
    /// the set's widgets win (last-registered wins).
    pub fn widget_set(mut self, set: &dyn WidgetSet<R>) -> Self {
        self.registry.register_set(set);
        self
    }

    /// Return all type names handled by registered PlushieWidgets.
    pub fn widget_type_names(&self) -> Vec<&str> {
        self.registry.type_names()
    }

    /// Consume the builder and produce the [`WidgetRegistry`].
    pub fn build(self) -> WidgetRegistry<R> {
        self.registry
    }

    /// Return type names for non-built-in widgets (custom widgets).
    /// Used by the hello message.
    pub fn custom_type_names(&self) -> Vec<&str> {
        let builtins = crate::widget::widget_set::IcedWidgetSet::type_names();
        self.registry
            .type_names()
            .into_iter()
            .filter(|name| !builtins.iter().any(|b| b == name))
            .collect()
    }
}

impl<R: PlushieRenderer> Default for PlushieAppBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}
