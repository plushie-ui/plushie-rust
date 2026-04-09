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
/// All widgets (both [`PlushieWidget`] and [`WidgetExtension`]) are
/// registered in the [`WidgetRegistry`]. Extensions are wrapped via
/// [`ExtensionAdapter`](crate::extension_adapter::ExtensionAdapter).
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

    /// Consume the builder and produce the [`WidgetRegistry`] and an
    /// empty [`ExtensionDispatcher`] (for infrastructure that still
    /// references it).
    pub fn build(self) -> (WidgetRegistry<R>, ExtensionDispatcher<R>) {
        let dispatcher = ExtensionDispatcher::new(vec![]);
        (self.registry, dispatcher)
    }

    /// Register a [`WidgetExtension`] via the [`ExtensionAdapter`], adding
    /// it to the widget registry. The adapter bridges the WidgetExtension
    /// API to PlushieWidget so extensions work through unified dispatch.
    ///
    /// [`ExtensionAdapter`]: crate::extension_adapter::ExtensionAdapter
    pub fn extension(mut self, ext: impl WidgetExtension<R> + 'static) -> Self {
        self.registry
            .register(Box::new(crate::extension_adapter::ExtensionAdapter::new(
                ext,
            )));
        self
    }

    /// Register a pre-boxed [`WidgetExtension`] via the
    /// [`ExtensionAdapter`](crate::extension_adapter::ExtensionAdapter).
    pub fn extension_boxed(mut self, ext: Box<dyn WidgetExtension<R>>) -> Self {
        // Create adapter from the boxed extension directly.
        self.registry.register(Box::new(
            crate::extension_adapter::ExtensionAdapter::from_boxed(ext),
        ));
        self
    }

    /// Return type names for non-built-in widgets (extensions registered
    /// via `.extension()`). Used by the hello message.
    pub fn extension_type_names(&self) -> Vec<&str> {
        let builtins = crate::widgets::render::builtin_widget_types();
        self.registry
            .type_names()
            .into_iter()
            .filter(|name| !builtins.contains(name))
            .collect()
    }

    /// Consume the builder and produce an [`ExtensionDispatcher`].
    /// Returns an empty dispatcher (all extensions are in the registry).
    pub fn build_dispatcher(self) -> ExtensionDispatcher<R> {
        ExtensionDispatcher::new(vec![])
    }
}

impl<R: PlushieRenderer> Default for PlushieAppBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}
