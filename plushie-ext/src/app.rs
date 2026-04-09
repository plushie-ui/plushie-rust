//! Application builder for registering widgets and extensions.
//!
//! Create a [`PlushieAppBuilder`], register widgets (via
//! [`PlushieWidget`](crate::registry::PlushieWidget) or the legacy
//! [`WidgetExtension`](crate::extensions::WidgetExtension) trait), and
//! pass it to `plushie::run()`.
//!
//! # Example
//!
//! ```ignore
//! use plushie_ext::prelude::*;
//! use plushie_ext::widgets::builtins::iced_widget_set;
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
use crate::extensions::{ExtensionDispatcher, WidgetExtension};
use crate::registry::{PlushieWidget, WidgetRegistry, WidgetSet};

/// Builder for registering widgets before starting the renderer.
///
/// Supports both the new [`PlushieWidget`] trait and the legacy
/// [`WidgetExtension`] trait (during the transition period).
///
/// The `R` parameter selects the renderer backend. Defaults to
/// `iced::Renderer` which is used by headless and windowed modes.
pub struct PlushieAppBuilder<R: PlushieRenderer = iced::Renderer> {
    extensions: Vec<Box<dyn WidgetExtension<R>>>,
    registry: WidgetRegistry<R>,
}

impl<R: PlushieRenderer> std::fmt::Debug for PlushieAppBuilder<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlushieAppBuilder")
            .field("extensions", &self.extensions.len())
            .field("registry", &self.registry)
            .finish()
    }
}

impl<R: PlushieRenderer> PlushieAppBuilder<R> {
    /// Create an empty builder with no widgets or extensions registered.
    pub fn new() -> Self {
        Self {
            extensions: vec![],
            registry: WidgetRegistry::new(),
        }
    }

    // -- New PlushieWidget API ------------------------------------------------

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
    pub fn build_registry(self) -> WidgetRegistry<R> {
        self.registry
    }

    // -- Legacy WidgetExtension API (transition period) -----------------------

    /// Register a legacy [`WidgetExtension`].
    pub fn extension(mut self, ext: impl WidgetExtension<R> + 'static) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    /// Register a pre-boxed legacy [`WidgetExtension`].
    pub fn extension_boxed(mut self, ext: Box<dyn WidgetExtension<R>>) -> Self {
        self.extensions.push(ext);
        self
    }

    /// Return all type names handled by registered legacy extensions.
    pub fn extension_type_names(&self) -> Vec<&str> {
        self.extensions
            .iter()
            .flat_map(|e| e.type_names().iter().copied())
            .collect()
    }

    /// Consume the builder and produce an [`ExtensionDispatcher`].
    ///
    /// This is the legacy path. During the transition, both
    /// `build_registry()` and `build_dispatcher()` are available.
    pub fn build_dispatcher(self) -> ExtensionDispatcher<R> {
        ExtensionDispatcher::new(self.extensions)
    }
}

impl<R: PlushieRenderer> Default for PlushieAppBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}
