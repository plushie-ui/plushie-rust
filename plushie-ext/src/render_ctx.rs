//! Core rendering context passed through the widget dispatch pipeline.
//!
//! [`RenderCtx`] carries everything needed for rendering: the widget tree
//! state, image handles, theme, text defaults, and per-window context.
//! It is used by all widget types during the
//! immutable view phase.

use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::image_registry::ImageRegistry;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::shared_state::SharedState;

/// Renders child nodes through the main dispatch. Copy-able (all shared refs).
///
/// All widgets (built-in and custom) receive this in their `render()`
/// method. It carries everything needed for rendering: the widget tree
/// state, image handles, theme, text defaults, and per-window context.
///
/// The `R` parameter selects the renderer backend: `iced::Renderer` for
/// headless/windowed modes, `()` (null renderer) for mock mode.
pub struct RenderCtx<'a, R: PlushieRenderer = iced::Renderer> {
    pub caches: &'a SharedState,
    pub images: &'a ImageRegistry,
    pub theme: &'a Theme,
    /// Widget registry for unified dispatch. All widget types are
    /// registered here.
    pub registry: &'a crate::registry::WidgetRegistry<R>,
    pub default_text_size: Option<f32>,
    pub default_font: Option<iced::Font>,
    /// The plushie window ID this render is for.
    ///
    /// Top-level helper contexts may start empty before a `window` node
    /// sets the real id for its subtree.
    pub window_id: &'a str,
    /// The display scale factor for this window (1.0 = no scaling).
    /// Useful for DPI-aware canvas rendering.
    pub scale_factor: f32,
}

impl<R: PlushieRenderer> Clone for RenderCtx<'_, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<R: PlushieRenderer> Copy for RenderCtx<'_, R> {}

impl<R: PlushieRenderer> std::fmt::Debug for RenderCtx<'_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderCtx")
            .field("window_id", &self.window_id)
            .field("scale_factor", &self.scale_factor)
            .field("default_text_size", &self.default_text_size)
            .field("default_font", &self.default_font)
            .finish_non_exhaustive()
    }
}

impl<'a, R: PlushieRenderer> RenderCtx<'a, R> {
    /// Render a child node through the main dispatch.
    pub fn render_child(&self, node: &'a TreeNode) -> Element<'a, Message, Theme, R> {
        crate::widget::render(node, *self)
    }

    /// Create a new RenderCtx with a different theme, preserving all other fields.
    pub fn with_theme(&self, theme: &'a Theme) -> Self {
        RenderCtx { theme, ..*self }
    }

    /// Create a new RenderCtx for a child window subtree.
    pub fn with_window_id(&self, window_id: &'a str) -> Self {
        RenderCtx { window_id, ..*self }
    }

    /// Render all children of a node through the main dispatch.
    pub fn render_children(&self, node: &'a TreeNode) -> Vec<Element<'a, Message, Theme, R>> {
        node.children.iter().map(|c| self.render_child(c)).collect()
    }
}
