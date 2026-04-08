//! Application builder for registering widget extensions.
//!
//! Extension packages create a [`PlushieAppBuilder`], register their
//! extensions, and pass it to `plushie::run()`. The default binary
//! passes an empty builder (no extensions).
//!
//! # Example
//!
//! ```ignore
//! use plushie_ext::prelude::*;
//!
//! fn main() -> iced::Result {
//!     plushie::run(
//!         PlushieAppBuilder::new()
//!             .extension(MyExtension::new())
//!             .extension(AnotherExtension::new())
//!     )
//! }
//! ```

use crate::PlushieRenderer;
use crate::extensions::{ExtensionDispatcher, WidgetExtension};

/// Builder for registering [`WidgetExtension`]s before starting the
/// renderer.
///
/// Each extension must have a unique `config_key()` and unique
/// `type_names()`. Duplicates panic at startup.
///
/// The `R` parameter selects the renderer backend. Defaults to
/// `iced::Renderer` which is used by headless and windowed modes.
/// Mock mode uses `ExtensionDispatcher::<()>::default()` directly.
pub struct PlushieAppBuilder<R: PlushieRenderer = iced::Renderer> {
    extensions: Vec<Box<dyn WidgetExtension<R>>>,
}

impl<R: PlushieRenderer> std::fmt::Debug for PlushieAppBuilder<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlushieAppBuilder")
            .field("extensions", &self.extensions.len())
            .finish()
    }
}

impl<R: PlushieRenderer> PlushieAppBuilder<R> {
    /// Create an empty builder with no extensions registered.
    pub fn new() -> Self {
        Self { extensions: vec![] }
    }

    /// Register a widget extension.
    pub fn extension(mut self, ext: impl WidgetExtension<R> + 'static) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    /// Register a pre-boxed widget extension.
    ///
    /// Useful for dynamically loaded extensions (e.g. via `libloading`)
    /// where the concrete type is erased at the plugin boundary.
    pub fn extension_boxed(mut self, ext: Box<dyn WidgetExtension<R>>) -> Self {
        self.extensions.push(ext);
        self
    }

    /// Return all type names handled by registered extensions.
    pub fn extension_type_names(&self) -> Vec<&str> {
        self.extensions
            .iter()
            .flat_map(|e| e.type_names().iter().copied())
            .collect()
    }

    /// Consume the builder and produce an [`ExtensionDispatcher`].
    pub fn build_dispatcher(self) -> ExtensionDispatcher<R> {
        ExtensionDispatcher::new(self.extensions)
    }
}

impl<R: PlushieRenderer> Default for PlushieAppBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}
