//! Test factory helpers for widget authors.
//!
//! Provides [`TestEnv`] for setting up a render environment and
//! [`node`] / [`node_with_props`] / [`node_with_children`] for
//! constructing test tree nodes.
//!
//! # Example
//!
//! ```ignore
//! use plushie_ext::testing::*;
//! use plushie_ext::prelude::*;
//!
//! let test = TestEnv::default();
//! let ctx = test.render_ctx();
//! let element = my_widget.render(&node, &ctx);
//! ```

use iced::Theme;
use serde_json::{Value, json};

use crate::image_registry::ImageRegistry;
use crate::protocol::TreeNode;
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
        props: json!({}),
        children: vec![],
    }
}

/// Create a [`TreeNode`] with the given props and no children.
pub fn node_with_props(id: &str, type_name: &str, props: Value) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: type_name.to_string(),
        props,
        children: vec![],
    }
}

/// Create a [`TreeNode`] with children and empty props.
pub fn node_with_children(id: &str, type_name: &str, children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: type_name.to_string(),
        props: json!({}),
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
        props,
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
    pub shared_state: SharedState,
    pub images: ImageRegistry,
    pub theme: Theme,
    pub registry: WidgetRegistry,
    pub default_text_size: Option<f32>,
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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop_helpers::{prop_f32, prop_str};
    use crate::registry::GenerationCounter;

    // -- TreeNode constructors ------------------------------------------------

    #[test]
    fn node_has_empty_props_and_no_children() {
        let n = node("btn-1", "button");
        assert_eq!(n.id, "btn-1");
        assert_eq!(n.type_name, "button");
        assert!(n.children.is_empty());
        assert_eq!(n.props, json!({}));
    }

    #[test]
    fn node_with_props_stores_props() {
        let n = node_with_props("txt-1", "text", json!({"content": "hello", "size": 14}));
        assert_eq!(n.props["content"], "hello");
        assert_eq!(n.props["size"], 14);
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
        let props = n.props.as_object();
        assert_eq!(prop_str(props, "label"), Some("cpu".to_string()));
        assert!((prop_f32(props, "max").unwrap() - 100.0).abs() < 0.001);
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
