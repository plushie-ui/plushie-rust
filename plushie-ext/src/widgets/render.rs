//! Main render dispatch: maps a [`TreeNode`] to an iced [`Element`].
//!
//! This is the immutable side of the ensure_caches/render split. All
//! mutable cache state must be pre-populated by [`super::ensure_caches`]
//! before calling [`render`]. Recursion depth is bounded by a
//! thread-local counter.

use std::cell::Cell;

use iced::widget::{Space, container, text};
use iced::{Color, Element, Theme};

use super::caches::MAX_TREE_DEPTH;

/// Returns the list of all built-in widget type names that the renderer supports.
pub fn builtin_widget_types() -> &'static [&'static str] {
    &[
        "column",
        "row",
        "container",
        "stack",
        "grid",
        "pin",
        "keyed_column",
        "float",
        "responsive",
        "scrollable",
        "pane_grid",
        "text",
        "rich_text",
        "rich",
        "space",
        "rule",
        "progress_bar",
        "image",
        "svg",
        "markdown",
        "qr_code",
        "text_input",
        "text_editor",
        "checkbox",
        "toggler",
        "radio",
        "slider",
        "vertical_slider",
        "pick_list",
        "combo_box",
        "button",
        "pointer_area",
        "sensor",
        "tooltip",
        "themer",
        "window",
        "overlay",
        "canvas",
        "table",
    ]
}
use super::helpers::*;
use super::{display, input, interactive, layout, table, validate};
use crate::PlushieRenderer;
use crate::extensions::RenderCtx;
use crate::message::Message;
use crate::protocol::TreeNode;

// ---------------------------------------------------------------------------
// Main render dispatch
// ---------------------------------------------------------------------------

/// Map a TreeNode to an iced Element. Unknown types render as an empty container.
///
/// This is the immutable side of the ensure_caches/render split. All mutable
/// cache state (text_editor Content, markdown Items, combo_box State, canvas
/// Cache, etc.) must be pre-populated by [`super::ensure_caches`] before calling
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

    // Dispatch through the WidgetRegistry when available. The registry
    // holds PlushieWidget impls for all built-in types and falls through
    // to the ExtensionDispatcher for custom extension types.
    let element = if let Some(registry) = ctx.registry {
        if let Some(widget) = registry.get_for_type(node.type_name.as_str()) {
            widget.render(node, &ctx)
        } else {
            // Not in the registry: try extension dispatch
            render_via_extension(node, ctx)
        }
    } else {
        // No registry (test paths): use hardcoded match
        render_via_match(node, ctx)
    };

    // Explicit a11y overrides take precedence. When no explicit a11y prop
    // exists, try widget-specific auto-inference.
    //
    // When the registry is active, infer_a11y() on the PlushieWidget is
    // used. Otherwise, fall back to the hardcoded match for legacy paths.
    let overrides = crate::widgets::a11y::A11yOverrides::from_props(&node.props).or_else(|| {
        if let Some(registry) = ctx.registry {
            registry
                .get_for_type(node.type_name.as_str())
                .and_then(|widget| widget.infer_a11y(node))
        } else {
            // Hardcoded path (test contexts without registry).
            // Image and SVG use iced's native .alt()/.description() methods
            // directly, so no A11yOverride wrapping needed for those.
            let props = node.props.as_object();
            match node.type_name.as_str() {
                "text_input" | "text_editor" | "combo_box" => prop_str(props, "placeholder")
                    .map(crate::widgets::a11y::A11yOverrides::with_description),
                _ => None,
            }
        }
    });

    if let Some(overrides) = overrides {
        return crate::widgets::a11y::A11yOverride::wrap(element, overrides).into();
    }

    element
}

// ---------------------------------------------------------------------------
// Match dispatch (used when no registry is available)
// ---------------------------------------------------------------------------

/// Dispatch via the hardcoded match statement. Used when no WidgetRegistry
/// is present (test paths).
fn render_via_match<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    // Stateful widget types whose caches have been migrated to
    // PlushieWidget factories are not listed here. They require
    // the registry path (which provides factory-owned state).
    // This match only covers stateless widgets and those still
    // using WidgetCaches (canvas, qr_code).
    match node.type_name.as_str() {
        // Layout widgets (stateless)
        "column" => layout::render_column(node, ctx),
        "row" => layout::render_row(node, ctx),
        "container" => layout::render_container(node, ctx),
        "stack" => layout::render_stack(node, ctx),
        "grid" => layout::render_grid(node, ctx),
        "pin" => layout::render_pin(node, ctx),
        "keyed_column" => layout::render_keyed_column(node, ctx),
        "float" => layout::render_float(node, ctx),
        "responsive" => layout::render_responsive(node, ctx),
        "scrollable" => layout::render_scrollable(node, ctx),
        // Display widgets (stateless only)
        "text" => display::render_text(node, ctx),
        "rich_text" | "rich" => display::render_rich_text(node, ctx),
        "space" => display::render_space(node, ctx),
        "rule" => display::render_rule(node, ctx),
        "progress_bar" => display::render_progress_bar(node, ctx),
        "image" => display::render_image(node, ctx),
        "svg" => display::render_svg(node, ctx),
        // Input widgets (stateless only)
        "text_input" => input::render_text_input(node, ctx),
        "checkbox" => input::render_checkbox(node, ctx),
        "toggler" => input::render_toggler(node, ctx),
        "radio" => input::render_radio(node, ctx),
        "slider" => input::render_slider(node, ctx),
        "vertical_slider" => input::render_vertical_slider(node, ctx),
        "pick_list" => input::render_pick_list(node, ctx),
        // Interactive widgets (stateless only)
        "button" => interactive::render_button(node, ctx),
        "pointer_area" => interactive::render_mouse_area(node, ctx),
        "sensor" => interactive::render_sensor(node, ctx),
        "tooltip" => interactive::render_tooltip(node, ctx),
        "window" => interactive::render_window(node, ctx),
        "overlay" => interactive::render_overlay(node, ctx),
        // Table (stateless)
        "table" => table::render_table(node, ctx),
        // Extension dispatch / unknown type
        _ => render_via_extension(node, ctx),
    }
}

/// Dispatch to the ExtensionDispatcher for extension types,
/// or render a placeholder for unknown types.
fn render_via_extension<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let type_name = node.type_name.as_str();
    if ctx.extensions.handles_type(type_name) {
        let env = crate::extensions::WidgetEnv {
            caches: &ctx.caches.extension,
            ctx,
        };
        if crate::extensions::catch_unwind_enabled() {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ctx.extensions.render(node, &env)
            })) {
                Ok(Some(element)) => element,
                Ok(None) => container(Space::new()).into(),
                Err(_) => {
                    let at_threshold = ctx.extensions.record_render_panic(type_name);
                    if at_threshold {
                        log::error!(
                            "[id={}] extension for type `{type_name}` hit render panic \
                             threshold, will be poisoned on next prepare cycle",
                            node.id
                        );
                    } else {
                        log::error!("extension panicked in render for node `{}`", node.id);
                    }
                    iced::widget::text(format!(
                        "Extension error: type `{type_name}`, node `{}`",
                        node.id
                    ))
                    .color(iced::Color::from_rgb(1.0, 0.0, 0.0))
                    .into()
                }
            }
        } else {
            match ctx.extensions.render(node, &env) {
                Some(element) => element,
                None => container(Space::new()).into(),
            }
        }
    } else {
        log::warn!(
            "[id={}] unknown node type `{type_name}`, rendering as empty container",
            node.id
        );
        container(Space::new()).into()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::ExtensionDispatcher;
    use crate::image_registry::ImageRegistry;
    use crate::protocol::TreeNode;
    use crate::widgets::WidgetCaches;

    // -- Image registry handle lookup --

    #[test]
    fn image_registry_handle_lookup() {
        let mut registry = ImageRegistry::new();
        // Minimal valid 1x1 RGB PNG.
        let png_bytes: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8-bit RGB
            0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IDAT
            0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE2,
            0x21, 0xBC, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, // IEND
            0x44, 0xAE, 0x42, 0x60, 0x82,
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
    // Render smoke tests -- verify render() doesn't panic for common types
    // -----------------------------------------------------------------------

    use crate::testing::{
        node_with_props as smoke_node, node_with_props_and_children as smoke_node_with_children,
    };

    fn smoke_text_child() -> TreeNode {
        smoke_node("child", "text", serde_json::json!({"content": "hi"}))
    }

    fn smoke_ctx<'a>(
        caches: &'a WidgetCaches,
        images: &'a ImageRegistry,
        theme: &'a iced::Theme,
        dispatcher: &'a ExtensionDispatcher,
    ) -> RenderCtx<'a> {
        RenderCtx {
            caches,
            images,
            theme,
            extensions: dispatcher,
            registry: None,
            default_text_size: None,
            default_font: None,
            window_id: "",
            scale_factor: 1.0,
        }
    }

    #[test]
    fn render_smoke_text() {
        let node = smoke_node("t", "text", serde_json::json!({"content": "hello"}));
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_column_empty() {
        let node = smoke_node("c", "column", serde_json::json!({}));
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_row_empty() {
        let node = smoke_node("r", "row", serde_json::json!({}));
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_container_with_child() {
        let node = smoke_node_with_children(
            "ct",
            "container",
            serde_json::json!({}),
            vec![smoke_text_child()],
        );
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_button_with_child() {
        let node = smoke_node_with_children(
            "btn",
            "button",
            serde_json::json!({}),
            vec![smoke_text_child()],
        );
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_checkbox() {
        let node = smoke_node(
            "cb",
            "checkbox",
            serde_json::json!({"label": "Accept", "checked": true}),
        );
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_space() {
        let node = smoke_node("sp", "space", serde_json::json!({}));
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_rule() {
        let node = smoke_node("rl", "rule", serde_json::json!({"direction": "horizontal"}));
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_progress_bar() {
        let node = smoke_node(
            "pb",
            "progress_bar",
            serde_json::json!({"value": 50.0, "min": 0.0, "max": 100.0}),
        );
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_slider() {
        let node = smoke_node(
            "sl",
            "slider",
            serde_json::json!({"min": 0.0, "max": 100.0, "value": 50.0}),
        );
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_text_input() {
        let node = smoke_node(
            "ti",
            "text_input",
            serde_json::json!({"placeholder": "Type here", "value": ""}),
        );
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_toggler() {
        let node = smoke_node("tg", "toggler", serde_json::json!({"is_toggled": false}));
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_smoke_stack_empty() {
        let node = smoke_node("st", "stack", serde_json::json!({}));
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    // -----------------------------------------------------------------------
    // Error path tests -- unknown type and missing props
    // -----------------------------------------------------------------------

    #[test]
    fn render_unknown_type_returns_element_without_panic() {
        let node = smoke_node("unk", "definitely_not_a_widget", serde_json::json!({}));
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        // Should produce the empty container fallback, not panic.
        let _elem = render(&node, ctx);
    }

    #[test]
    fn render_text_input_missing_props_does_not_panic() {
        let node = smoke_node("ti_empty", "text_input", serde_json::json!({}));
        let caches: WidgetCaches = WidgetCaches::new();
        let images = ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let dispatcher: ExtensionDispatcher = ExtensionDispatcher::default();
        let ctx = smoke_ctx(&caches, &images, &theme, &dispatcher);
        let _elem = render(&node, ctx);
    }

    // -----------------------------------------------------------------------
    // A11y auto-inference tests
    // -----------------------------------------------------------------------

    /// Helper: extract auto-inferred overrides the same way render() does,
    /// without actually rendering (avoids needing image handles etc.).
    fn infer_a11y_overrides(node: &TreeNode) -> Option<crate::widgets::a11y::A11yOverrides> {
        crate::widgets::a11y::A11yOverrides::from_props(&node.props).or_else(|| {
            let props = node.props.as_object();
            match node.type_name.as_str() {
                // Image and SVG use iced's native .alt()/.description() methods
                // directly, so no A11yOverride wrapping needed for those.
                "text_input" | "text_editor" | "combo_box" => prop_str(props, "placeholder")
                    .map(crate::widgets::a11y::A11yOverrides::with_description),
                _ => None,
            }
        })
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
        assert_eq!(overrides.description.as_deref(), Some("Search..."));
        assert!(overrides.label.is_none());
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
        assert_eq!(overrides.label.as_deref(), Some("Explicit label"));
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
        assert_eq!(
            overrides.description.as_deref(),
            Some("Select an option...")
        );
        assert!(overrides.label.is_none());
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
        assert_eq!(overrides.description.as_deref(), Some("Write something..."));
        assert!(overrides.label.is_none());
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
}
