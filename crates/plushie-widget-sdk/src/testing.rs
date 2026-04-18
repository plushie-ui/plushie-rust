//! Test factory helpers for widget authors.
//!
//! Provides [`TestEnv`] for setting up a render environment and
//! [`node`] / [`node_with_props`] / [`node_with_children`] for
//! constructing test tree nodes.
//!
//! # Example
//!
//! ```ignore
//! use plushie_widget_sdk::testing::*;
//! use plushie_widget_sdk::prelude::*;
//!
//! let test = TestEnv::default();
//! let ctx = test.render_ctx();
//! let element = my_widget.render(&node, &ctx);
//! ```

use iced::Theme;
use serde_json::Value;

use crate::image_registry::ImageRegistry;
use crate::protocol::{Props, TreeNode};
use crate::registry::WidgetRegistry;
use crate::render_ctx::RenderCtx;
use crate::shared_state::SharedState;

// ---------------------------------------------------------------------------
// TreeNode constructors
// ---------------------------------------------------------------------------

/// Create a minimal [`TreeNode`] with empty props and no children.
pub fn node(id: &str, type_name: &str) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: type_name.to_string(),
        props: Props::default(),
        children: vec![],
    }
}

/// Create a [`TreeNode`] with the given props and no children.
pub fn node_with_props(id: &str, type_name: &str, props: Value) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: type_name.to_string(),
        props: Props::from_json(props),
        children: vec![],
    }
}

/// Create a [`TreeNode`] with children and empty props.
pub fn node_with_children(id: &str, type_name: &str, children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: type_name.to_string(),
        props: Props::default(),
        children,
    }
}

/// Create a [`TreeNode`] with both props and children.
///
/// For container-type widgets that need both configuration
/// (via props) and nested content (via children).
pub fn node_with_props_and_children(
    id: &str,
    type_name: &str,
    props: Value,
    children: Vec<TreeNode>,
) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: type_name.to_string(),
        props: Props::from_json(props),
        children,
    }
}

// ---------------------------------------------------------------------------
// TestEnv: owns all render dependencies
// ---------------------------------------------------------------------------

/// Owns all the dependencies needed to construct a [`RenderCtx`] for
/// testing widget `render()` methods.
///
/// All fields are public so tests can customize before calling
/// [`render_ctx`](Self::render_ctx).
///
/// # Example
///
/// ```ignore
/// let test = TestEnv::default();
/// let ctx = test.render_ctx();
/// let element = my_widget.render(&node, &ctx);
/// ```
///
/// With customization:
///
/// ```ignore
/// let test = TestEnv {
///     theme: Theme::Light,
///     ..TestEnv::default()
/// };
/// let ctx = test.render_ctx();
/// ```
pub struct TestEnv {
    /// Shared state used by widget caches.
    pub shared_state: SharedState,
    /// Image registry stub for tests.
    pub images: ImageRegistry,
    /// Active iced theme for the test.
    pub theme: Theme,
    /// Widget registry pre-populated with the built-in widget set.
    pub registry: WidgetRegistry,
    /// Default text size (pixels) applied to widgets without overrides.
    pub default_text_size: Option<f32>,
    /// Default font applied to widgets without overrides.
    pub default_font: Option<iced::Font>,
}

impl std::fmt::Debug for TestEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestEnv")
            .field("images", &self.images)
            .field("registry", &self.registry)
            .field("default_text_size", &self.default_text_size)
            .field("default_font", &self.default_font)
            .finish_non_exhaustive()
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        let mut registry = WidgetRegistry::new();
        registry.register_set(&crate::widget::widget_set::iced_widget_set());
        Self {
            shared_state: SharedState::new(),
            images: ImageRegistry::new(),
            theme: Theme::Dark,
            registry,
            default_text_size: None,
            default_font: None,
        }
    }
}

impl TestEnv {
    /// Build a [`RenderCtx`] from the owned test state.
    pub fn render_ctx(&self) -> RenderCtx<'_> {
        RenderCtx {
            caches: &self.shared_state,
            images: &self.images,
            theme: &self.theme,
            registry: &self.registry,
            default_text_size: self.default_text_size,
            default_font: self.default_font,
            window_id: "",
            scale_factor: 1.0,
        }
    }

    /// Drive a stateful widget's prepare/render cycle in one call.
    ///
    /// Calls `widget.prepare(node, window_id, &theme)` then
    /// `widget.render(node, ctx)`. Prefer this over calling the two
    /// separately so the mutable and immutable borrows don't fight.
    pub fn prepare_and_render<'a, W>(
        &'a self,
        widget: &'a mut W,
        node: &'a TreeNode,
        window_id: &str,
    ) -> iced::Element<'a, crate::message::Message, iced::Theme, iced::Renderer>
    where
        W: crate::registry::PlushieWidget<iced::Renderer>,
    {
        widget.prepare(node, window_id, &self.theme);
        let ctx = self.render_ctx();
        widget.render(node, &ctx)
    }

    /// Drive a widget's `handle_message` and return the emitted
    /// events directly.
    ///
    /// `HandleResult::Handled(v)` yields `v`. `HandleResult::Fallthrough`
    /// yields an empty `Vec`. This flattening matches what a host
    /// actually observes over the wire: a widget that returned
    /// Fallthrough lets the registry run its default conversion, but
    /// for unit-testing the widget alone the distinction is rarely
    /// what the author wants to assert.
    pub fn handle_message_events<W>(
        &self,
        widget: &mut W,
        msg: &crate::message::Message,
    ) -> Vec<crate::protocol::OutgoingEvent>
    where
        W: crate::registry::PlushieWidget<iced::Renderer>,
    {
        match widget.handle_message(msg) {
            crate::registry::HandleResult::Handled(v) => v,
            crate::registry::HandleResult::Fallthrough => Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop_helpers::{prop_f32, prop_str};
    use crate::registry::GenerationCounter;
    use serde_json::json;

    // -- TreeNode constructors ------------------------------------------------

    #[test]
    fn node_has_empty_props_and_no_children() {
        let n = node("btn-1", "button");
        assert_eq!(n.id, "btn-1");
        assert_eq!(n.type_name, "button");
        assert!(n.children.is_empty());
        assert_eq!(n.props.to_value(), json!({}));
    }

    #[test]
    fn node_with_props_stores_props() {
        let n = node_with_props("txt-1", "text", json!({"content": "hello", "size": 14}));
        assert_eq!(n.props.to_value()["content"], "hello");
        assert_eq!(n.props.to_value()["size"], 14);
    }

    #[test]
    fn node_with_children_stores_children() {
        let children = vec![node("a", "text"), node("b", "button")];
        let n = node_with_children("col-1", "column", children);
        assert_eq!(n.children.len(), 2);
        assert_eq!(n.children[0].id, "a");
        assert_eq!(n.children[1].id, "b");
    }

    #[test]
    fn node_props_work_with_prop_helpers() {
        let n = node_with_props("s-1", "sparkline", json!({"label": "cpu", "max": 100.0}));
        assert_eq!(prop_str(&n.props, "label"), Some("cpu".to_string()));
        assert!((prop_f32(&n.props, "max").unwrap() - 100.0).abs() < 0.001);
    }

    // -- TestEnv --------------------------------------------------------------

    #[test]
    fn default_env_has_no_text_defaults() {
        let test = TestEnv::default();
        let ctx = test.render_ctx();
        assert!(ctx.default_text_size.is_none());
        assert!(ctx.default_font.is_none());
    }

    #[test]
    fn env_inherits_text_defaults() {
        let test = TestEnv {
            default_text_size: Some(18.0),
            default_font: Some(iced::Font::MONOSPACE),
            ..TestEnv::default()
        };

        let ctx = test.render_ctx();
        assert_eq!(ctx.default_text_size, Some(18.0));
        assert_eq!(ctx.default_font, Some(iced::Font::MONOSPACE));
    }

    #[test]
    fn env_theme_is_customizable() {
        let _test = TestEnv {
            theme: Theme::Light,
            ..TestEnv::default()
        };
    }

    // -- GenerationCounter ----------------------------------------------------

    #[test]
    fn generation_counter_lifecycle() {
        let mut counter = GenerationCounter::new();
        assert_eq!(counter.get(), 0);
        counter.bump();
        counter.bump();
        assert_eq!(counter.get(), 2);
    }
}
