//! Application builder for registering widgets.
//!
//! Create a [`PlushieAppBuilder`], register widgets (via
//! [`PlushieWidget`](crate::registry::PlushieWidget)), and pass it
//! to `plushie::run()`.
//!
//! # Example
//!
//! ```ignore
//! use plushie_widget_sdk::app::PlushieAppBuilder;
//! use plushie_widget_sdk::widget::widget_set::iced_widget_set;
//!
//! fn main() -> iced::Result {
//!     plushie::run(
//!         PlushieAppBuilder::new()
//!             .widget_set(&iced_widget_set())
//!             .widget(MyGauge::new())
//!     )
//! }
//! ```

use crate::PlushieRenderer;
use crate::registry::{PlushieWidget, WidgetRegistry, WidgetSet};

/// Builder for registering widgets before starting the renderer.
///
/// All widgets are registered in the [`WidgetRegistry`] via the
/// [`PlushieWidget`] trait.
///
/// The `R` parameter selects the renderer backend. Defaults to
/// `iced::Renderer` which is used by headless and windowed modes.
pub struct PlushieAppBuilder<R: PlushieRenderer = iced::Renderer> {
    registry: WidgetRegistry<R>,
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
        }
    }

    /// Register a [`PlushieWidget`] implementation.
    ///
    /// If the widget's type name is already registered, the new widget
    /// replaces it (last-registered wins).
    pub fn widget(mut self, widget: impl PlushieWidget<R> + 'static) -> Self {
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
            .filter(|name| !builtins.contains(name))
            .collect()
    }
}

impl<R: PlushieRenderer> Default for PlushieAppBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}
