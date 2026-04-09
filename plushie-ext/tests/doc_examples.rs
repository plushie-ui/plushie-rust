//! Compilation tests for code examples in the widget development docs.
//!
//! These tests verify that the API patterns shown in the docs actually
//! compile against the real plushie-core and iced APIs. If the API
//! changes, these tests fail, signaling that the docs need updating.
//!
//! The tests don't render pixels -- they exercise the type system and verify
//! that method calls, field access, and trait implementations are correct.

use plushie_ext::prelude::*;
use plushie_ext::testing::*;
use serde_json::json;

// column and row are excluded from the prelude because the function
// forms conflict with the column!/row! macros under glob import.
use plushie_ext::iced::widget::{column, row};

// ============================================================================
// Gauge example (PlushieWidget)
// ============================================================================

struct DocGauge;

impl<R: PlushieRenderer> PlushieWidget<R> for DocGauge {
    fn type_names(&self) -> &[&str] {
        &["doc_gauge"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        _ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let value = node.prop_f32("value").unwrap_or(0.0);
        let label = node.prop_str("label").unwrap_or_default();

        column![
            text(format!("{label}: {value:.0}%")),
            progress_bar(0.0..=100.0, value),
        ]
        .spacing(4)
        .into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(DocGauge)
    }
}

#[test]
fn doc_gauge_renders() {
    let node = node_with_props("g1", "doc_gauge", json!({"value": 50.0, "label": "CPU"}));
    let test = TestEnv::default();
    let ctx = test.render_ctx();
    let gauge = DocGauge;
    let _element: Element<'_, Message, Theme, iced::Renderer> = gauge.render(&node, &ctx);
}

#[test]
fn doc_gauge_no_props() {
    let node = node("g1", "doc_gauge");
    let test = TestEnv::default();
    let ctx = test.render_ctx();
    let gauge = DocGauge;
    let _element: Element<'_, Message, Theme, iced::Renderer> = gauge.render(&node, &ctx);
}

// ============================================================================
// Prop parsing patterns
// ============================================================================

#[test]
fn doc_prop_parsing() {
    let props_val = json!({
        "value": 42.5,
        "label": "test",
        "color": "#3498db",
        "show_label": true,
        "width": "fill",
    });
    let props = props_val.as_object();

    // Free function style
    let _value: Option<f32> = prop_f32(props, "value");
    let _label: Option<String> = prop_str(props, "label");
    let _color: Option<Color> = prop_color(props, "color");
    let _show_label: bool = prop_bool_default(props, "show_label", true);
    let _width: Length = prop_length(props, "width", Length::Fill);

    // TreeNode shorthand style
    let node = node_with_props("n1", "test", props_val.clone());
    let _value: Option<f32> = node.prop_f32("value");
    let _label: Option<String> = node.prop_str("label");
    let _color: Option<Color> = node.prop_color("color");
}

// ============================================================================
// Theme access
// ============================================================================

#[test]
fn doc_theme_access() {
    let test = TestEnv::default();
    let ctx = test.render_ctx();
    let theme = ctx.theme;
    let palette = theme.palette();
    let _primary = palette.primary.base.color;
    let _is_dark = palette.is_dark;
}

// ============================================================================
// Rendering children (container widget)
// ============================================================================

struct DocContainer;

impl<R: PlushieRenderer> PlushieWidget<R> for DocContainer {
    fn type_names(&self) -> &[&str] {
        &["doc_container"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let header = text(node.prop_str("title").unwrap_or_default());
        let children: Vec<Element<'a, Message, Theme, R>> = ctx.render_children(node);
        let mut col = column![header].spacing(8);
        for child in children {
            col = col.push(child);
        }
        col.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(DocContainer)
    }
}

#[test]
fn doc_container_renders() {
    let node = node_with_props_and_children(
        "c1",
        "doc_container",
        json!({"title": "Section"}),
        vec![node("child1", "text")],
    );
    let test = TestEnv::default();
    let ctx = test.render_ctx();
    let widget = DocContainer;
    let _element: Element<'_, Message, Theme, iced::Renderer> = widget.render(&node, &ctx);
}

// ============================================================================
// GenerationCounter pattern
// ============================================================================

#[test]
fn doc_generation_counter() {
    let mut counter = GenerationCounter::new();
    let gen_before = counter.get();
    counter.bump();
    assert_ne!(gen_before, counter.get());
}

// ============================================================================
// CoalesceHint
// ============================================================================

#[test]
fn doc_coalesce_hint() {
    let event =
        OutgoingEvent::extension_event("cursor_pos", "w1", Some(json!({"x": 10.0, "y": 20.0})))
            .with_coalesce(CoalesceHint::Replace);
    assert!(event.coalesce.is_some());
}

// ============================================================================
// Rating widget (PlushieWidget, complete example)
// ============================================================================

struct DocRating;

impl<R: PlushieRenderer> PlushieWidget<R> for DocRating {
    fn type_names(&self) -> &[&str] {
        &["doc_rating"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let value = node.prop_f32("value").unwrap_or(0.0) as usize;
        let max = prop_u32(node.props(), "max").unwrap_or(5) as usize;
        let size = node.prop_f32("size").unwrap_or(24.0);
        let color = node
            .prop_color("color")
            .unwrap_or(ctx.theme.palette().primary.base.color);
        let disabled_color = Color {
            a: color.a * 0.3,
            ..color
        };

        let id = node.id.clone();
        let mut stars = row![].spacing(2);

        for i in 1..=max {
            let filled = i <= value;
            let star_color = if filled { color } else { disabled_color };
            let label = if filled { "\u{2605}" } else { "\u{2606}" };

            let star_text = text(label).size(size).color(star_color);

            let star_button = button(star_text)
                .on_press(Message::widget_event(
                    "",
                    &id,
                    "select",
                    json!({"value": i}),
                ))
                .padding(0)
                .style(button::text);

            stars = stars.push(star_button);
        }

        stars.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(DocRating)
    }
}

#[test]
fn doc_rating_renders() {
    let node = node_with_props(
        "r1",
        "doc_rating",
        json!({"value": 3, "max": 5, "size": 32}),
    );
    let test = TestEnv::default();
    let ctx = test.render_ctx();
    let widget = DocRating;
    let _element: Element<'_, Message, Theme, iced::Renderer> = widget.render(&node, &ctx);
}

#[test]
fn doc_rating_no_props() {
    let node = node("r1", "doc_rating");
    let test = TestEnv::default();
    let ctx = test.render_ctx();
    let widget = DocRating;
    let _element: Element<'_, Message, Theme, iced::Renderer> = widget.render(&node, &ctx);
}

// ============================================================================
// clone_for_session
// ============================================================================

#[test]
fn doc_clone_for_session() {
    let widget = DocRating;
    let cloned: Box<dyn PlushieWidget<iced::Renderer>> = widget.clone_for_session();
    assert_eq!(cloned.type_names(), &["doc_rating"]);
}
