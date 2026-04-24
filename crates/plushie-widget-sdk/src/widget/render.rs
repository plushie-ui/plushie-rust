//! Main render dispatch: maps a [`TreeNode`] to an iced [`Element`].
//!
//! This is the immutable side of the prepare/render split. All
//! mutable cache state must be pre-populated by `registry.prepare_walk()` (via PlushieWidget::prepare)
//! before calling [`render`]. Recursion depth is bounded by a
//! thread-local counter.

use std::cell::Cell;

use iced::widget::text;
use iced::{Color, Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::render_ctx::RenderCtx;
use crate::shared_state::MAX_TREE_DEPTH;
use crate::validate;

// ---------------------------------------------------------------------------
// Main render dispatch
// ---------------------------------------------------------------------------

/// Map a TreeNode to an iced Element. Unknown types render as an empty container.
///
/// This is the immutable side of the prepare/render split. All mutable
/// cache state (text_editor Content, markdown Items, combo_box State, canvas
/// Cache, etc.) must be pre-populated by `registry.prepare_walk()` (via PlushieWidget::prepare) before calling
/// this function. `render` works exclusively with shared (`&`) references
/// to caches, so it can run inside iced's `view()` which only has `&self`.
pub fn render<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    // Track recursion depth via thread-local counter. Each call increments
    // on entry; the DepthGuard decrements on drop (including early returns).
    thread_local! {
        static RENDER_DEPTH: Cell<usize> = const { Cell::new(0) };
    }
    struct DepthGuard;
    impl Drop for DepthGuard {
        fn drop(&mut self) {
            RENDER_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        }
    }

    let depth = RENDER_DEPTH.with(|d| {
        let new = d.get() + 1;
        d.set(new);
        new
    });
    let _guard = DepthGuard;

    if depth > MAX_TREE_DEPTH {
        log::warn!(
            "[id={}] render depth exceeds {MAX_TREE_DEPTH}, returning placeholder",
            node.id
        );
        return text("Max depth exceeded")
            .color(Color::from_rgb(1.0, 0.0, 0.0))
            .into();
    }

    if validate::is_validate_props_enabled() {
        validate::validate_props(node);
    }

    let element = ctx.registry.render_node(node, &ctx);

    // Merge widget-inferred defaults with the author's explicit a11y prop.
    // Explicit values win per field; inferred values fill in the gaps so a
    // single `a11y.label` override doesn't silently discard an inferred
    // `description` (e.g. a text_input's placeholder).
    //
    // Panic-isolated to match the render dispatch: a buggy third-party
    // widget must not take down the renderer via infer_a11y either.
    let inferred = ctx.registry.infer_a11y_for_node(node);
    let explicit = crate::a11y::A11yOverrides::from_props(&node.props);
    let overrides = match (inferred, explicit) {
        (Some(inf), Some(exp)) => Some(crate::a11y::A11yOverrides::merge(&inf, &exp)),
        (Some(inf), None) => Some(inf),
        (None, Some(exp)) => Some(exp),
        (None, None) => None,
    };

    if let Some(overrides) = overrides {
        return crate::a11y::A11yOverride::wrap(element, overrides).into();
    }

    element
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_registry::ImageRegistry;
    use crate::protocol::TreeNode;
    use crate::registry::WidgetRegistry;
    use crate::shared_state::SharedState;
    use crate::widget::widget_set::iced_widget_set;

    #[test]
    fn image_registry_handle_lookup() {
        let mut registry = ImageRegistry::new();
        // Minimal valid 1x1 RGBA PNG.
        let png_bytes: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, // 8-bit RGBA
            0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, // IDAT
            0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF,
            0x89, 0x99, 0x3D, 0x1D, 0x00, 0x00, 0x00, 0x00, // IEND
            0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        registry
            .create_from_bytes("test_sprite", png_bytes)
            .expect("test sprite should be valid");
        assert!(
            registry.get("test_sprite").is_some(),
            "registered handle should be retrievable"
        );
        assert!(
            registry.get("nonexistent").is_none(),
            "unregistered name should return None"
        );
    }

    // -----------------------------------------------------------------------
    // Render smoke tests: verify render() doesn't panic for common types
    // -----------------------------------------------------------------------

    use crate::testing::{
        node_with_props as smoke_node, node_with_props_and_children as smoke_node_with_children,
    };

    fn smoke_text_child() -> TreeNode {
        smoke_node("child", "text", serde_json::json!({"content": "hi"}))
    }

    fn smoke_registry() -> WidgetRegistry {
        let mut registry = WidgetRegistry::new();
        registry.register_set(&iced_widget_set());
        registry
    }

    fn smoke_ctx<'a>(
        caches: &'a SharedState,
        images: &'a ImageRegistry,
        theme: &'a iced::Theme,
        registry: &'a WidgetRegistry,
    ) -> RenderCtx<'a> {
        RenderCtx {
            caches,
            images,
            theme,
            theme_chrome: crate::theming::ThemeChrome::default(),
            registry,
            default_text_size: None,
            default_font: None,
            window_id: "",
            scale_factor: 1.0,
        }
    }

    fn render_prepared(mut node: TreeNode) {
        let mut caches: SharedState = SharedState::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let mut registry = smoke_registry();
        registry.prepare_walk(&mut node, &mut caches, &theme);
        let ctx = smoke_ctx(&caches, &images, &theme, &registry);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_text() {
        let node = smoke_node("t", "text", serde_json::json!({"content": "hello"}));
        render_prepared(node);
    }

    #[test]
    fn render_smoke_column_empty() {
        let node = smoke_node("c", "column", serde_json::json!({}));
        render_prepared(node);
    }

    #[test]
    fn render_smoke_row_empty() {
        let node = smoke_node("r", "row", serde_json::json!({}));
        render_prepared(node);
    }

    #[test]
    fn render_smoke_container_with_child() {
        let node = smoke_node_with_children(
            "ct",
            "container",
            serde_json::json!({}),
            vec![smoke_text_child()],
        );
        render_prepared(node);
    }

    #[test]
    fn render_smoke_button_with_child() {
        let node = smoke_node_with_children(
            "btn",
            "button",
            serde_json::json!({}),
            vec![smoke_text_child()],
        );
        render_prepared(node);
    }

    #[test]
    fn render_smoke_checkbox() {
        let node = smoke_node(
            "cb",
            "checkbox",
            serde_json::json!({"label": "Accept", "checked": true}),
        );
        render_prepared(node);
    }

    #[test]
    fn render_smoke_space() {
        let node = smoke_node("sp", "space", serde_json::json!({}));
        render_prepared(node);
    }

    #[test]
    fn render_smoke_rule() {
        let node = smoke_node("rl", "rule", serde_json::json!({"direction": "horizontal"}));
        render_prepared(node);
    }

    #[test]
    fn render_smoke_progress_bar() {
        let node = smoke_node(
            "pb",
            "progress_bar",
            serde_json::json!({"value": 50.0, "min": 0.0, "max": 100.0}),
        );
        render_prepared(node);
    }

    #[test]
    fn render_smoke_slider() {
        let node = smoke_node(
            "sl",
            "slider",
            serde_json::json!({"min": 0.0, "max": 100.0, "value": 50.0}),
        );
        render_prepared(node);
    }

    #[test]
    fn render_smoke_text_input() {
        let node = smoke_node(
            "ti",
            "text_input",
            serde_json::json!({"placeholder": "Type here", "value": ""}),
        );
        render_prepared(node);
    }

    #[test]
    fn render_smoke_toggler() {
        let node = smoke_node("tg", "toggler", serde_json::json!({"is_toggled": false}));
        render_prepared(node);
    }

    #[test]
    fn render_smoke_stack_empty() {
        let node = smoke_node("st", "stack", serde_json::json!({}));
        render_prepared(node);
    }

    // -----------------------------------------------------------------------
    // Error path tests: unknown type and missing props
    // -----------------------------------------------------------------------

    #[test]
    fn render_unknown_type_returns_element_without_panic() {
        let node = smoke_node("unk", "definitely_not_a_widget", serde_json::json!({}));
        let caches: SharedState = SharedState::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let registry = smoke_registry();
        let ctx = smoke_ctx(&caches, &images, &theme, &registry);
        // Should produce the empty container fallback, not panic.
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_text_input_missing_props_does_not_panic() {
        let node = smoke_node("ti_empty", "text_input", serde_json::json!({}));
        render_prepared(node);
    }

    // -----------------------------------------------------------------------
    // A11y auto-inference tests
    // -----------------------------------------------------------------------

    /// Helper: extract auto-inferred overrides the same way render() does,
    /// without actually rendering (avoids needing image handles etc.).
    fn infer_a11y_overrides(node: &TreeNode) -> Option<crate::a11y::A11yOverrides> {
        let props = &node.props;
        let registry = smoke_registry();
        let inferred = registry.infer_a11y_for_node(node);
        let explicit = crate::a11y::A11yOverrides::from_props(props);
        match (inferred, explicit) {
            (Some(inf), Some(exp)) => Some(crate::a11y::A11yOverrides::merge(&inf, &exp)),
            (Some(inf), None) => Some(inf),
            (None, Some(exp)) => Some(exp),
            (None, None) => None,
        }
    }

    #[test]
    fn a11y_image_alt_uses_native_iced_method_not_override() {
        // Image/SVG alt text is handled by iced's native .alt() method,
        // not by A11yOverride wrapping. No override should be created.
        let node = smoke_node(
            "img1",
            "image",
            serde_json::json!({"source": "logo.png", "alt": "Company logo"}),
        );
        assert!(
            infer_a11y_overrides(&node).is_none(),
            "image with alt should NOT get A11yOverride (uses native .alt())"
        );
    }

    #[test]
    fn a11y_svg_alt_uses_native_iced_method_not_override() {
        let node = smoke_node(
            "svg1",
            "svg",
            serde_json::json!({"source": "icon.svg", "alt": "Settings icon"}),
        );
        assert!(
            infer_a11y_overrides(&node).is_none(),
            "svg with alt should NOT get A11yOverride (uses native .alt())"
        );
    }

    #[test]
    fn a11y_auto_infer_text_input_placeholder_as_description() {
        let node = smoke_node(
            "ti1",
            "text_input",
            serde_json::json!({"placeholder": "Search...", "value": ""}),
        );
        let overrides =
            infer_a11y_overrides(&node).expect("should infer overrides from placeholder");
        assert_eq!(overrides.description(), Some("Search..."));
        assert!(overrides.label().is_none());
    }

    #[test]
    fn a11y_explicit_overrides_take_precedence_over_alt() {
        let node = smoke_node(
            "img2",
            "image",
            serde_json::json!({
                "source": "logo.png",
                "alt": "Auto alt",
                "a11y": {"label": "Explicit label"}
            }),
        );
        let overrides = infer_a11y_overrides(&node).expect("should have explicit overrides");
        // Explicit label wins; no double-wrapping.
        assert_eq!(overrides.label(), Some("Explicit label"));
    }

    #[test]
    fn a11y_no_wrapping_without_alt_or_a11y() {
        let node = smoke_node("txt1", "text", serde_json::json!({"content": "hello"}));
        assert!(
            infer_a11y_overrides(&node).is_none(),
            "plain text node should not get a11y wrapping"
        );
    }

    #[test]
    fn a11y_no_wrapping_image_without_alt() {
        let node = smoke_node(
            "img3",
            "image",
            serde_json::json!({"source": "decorative.png"}),
        );
        assert!(
            infer_a11y_overrides(&node).is_none(),
            "image without alt should not get a11y wrapping"
        );
    }

    #[test]
    fn a11y_auto_infer_combo_box_placeholder_as_description() {
        let node = smoke_node(
            "cb1",
            "combo_box",
            serde_json::json!({"placeholder": "Select an option...", "value": ""}),
        );
        let overrides =
            infer_a11y_overrides(&node).expect("should infer overrides from placeholder");
        assert_eq!(overrides.description(), Some("Select an option..."));
        assert!(overrides.label().is_none());
    }

    #[test]
    fn a11y_auto_infer_text_editor_placeholder_as_description() {
        let node = smoke_node(
            "te1",
            "text_editor",
            serde_json::json!({"placeholder": "Write something..."}),
        );
        let overrides =
            infer_a11y_overrides(&node).expect("should infer overrides from placeholder");
        assert_eq!(overrides.description(), Some("Write something..."));
        assert!(overrides.label().is_none());
    }

    #[test]
    fn a11y_no_wrapping_combo_box_without_placeholder() {
        let node = smoke_node("cb2", "combo_box", serde_json::json!({"value": "selected"}));
        assert!(
            infer_a11y_overrides(&node).is_none(),
            "combo_box without placeholder should not get a11y wrapping"
        );
    }

    #[test]
    fn a11y_no_wrapping_text_input_without_placeholder() {
        let node = smoke_node(
            "ti2",
            "text_input",
            serde_json::json!({"value": "typed text"}),
        );
        assert!(
            infer_a11y_overrides(&node).is_none(),
            "text_input without placeholder should not get a11y wrapping"
        );
    }

    #[test]
    fn a11y_explicit_label_merges_with_inferred_description() {
        // Before the merge fix, an explicit `a11y.label` on a text_input
        // would discard the inferred description from the placeholder.
        // After the fix, both survive on the merged result.
        let node = smoke_node(
            "search",
            "text_input",
            serde_json::json!({
                "placeholder": "Search...",
                "a11y": {"label": "Global search"}
            }),
        );
        let overrides = infer_a11y_overrides(&node)
            .expect("merged overrides should be present when either side sets fields");
        assert_eq!(
            overrides.label(),
            Some("Global search"),
            "explicit label should win"
        );
        assert_eq!(
            overrides.description(),
            Some("Search..."),
            "inferred description should survive merge"
        );
    }

    #[test]
    fn a11y_auto_infer_button_mnemonic() {
        let node = smoke_node(
            "save",
            "button",
            serde_json::json!({"label": "Save", "mnemonic": "S"}),
        );
        let overrides = infer_a11y_overrides(&node).expect("should infer mnemonic");

        assert_eq!(overrides.core().mnemonic, Some('S'));
    }

    #[test]
    fn a11y_auto_infer_access_key_alias() {
        let node = smoke_node(
            "remember",
            "checkbox",
            serde_json::json!({"label": "Remember me", "access_key": "R"}),
        );
        let overrides = infer_a11y_overrides(&node).expect("should infer access key");

        assert_eq!(overrides.core().mnemonic, Some('R'));
    }

    #[test]
    fn a11y_top_level_mnemonic_wins_over_access_key() {
        let node = smoke_node(
            "choice",
            "radio",
            serde_json::json!({"value": "yes", "mnemonic": "Y", "access_key": "N"}),
        );
        let overrides = infer_a11y_overrides(&node).expect("should infer mnemonic");

        assert_eq!(overrides.core().mnemonic, Some('Y'));
    }

    #[test]
    fn a11y_explicit_mnemonic_wins_over_top_level_prop() {
        let node = smoke_node(
            "save",
            "button",
            serde_json::json!({
                "label": "Save",
                "mnemonic": "S",
                "a11y": {"mnemonic": "V"}
            }),
        );
        let overrides = infer_a11y_overrides(&node).expect("should merge mnemonic overrides");

        assert_eq!(overrides.core().mnemonic, Some('V'));
    }
}
