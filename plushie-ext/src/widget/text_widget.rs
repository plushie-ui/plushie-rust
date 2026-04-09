use iced::widget::text;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct TextWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for TextWidget {
    fn type_names(&self) -> &[&str] {
        &["text"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = node.props.as_object();
        let content = prop_str(props, "content").unwrap_or_default();
        let size = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "size")
            .or(ctx.default_text_size);

        let mut t = text(content);
        if let Some(s) = size {
            t = t.size(s);
        }
        let font = props
            .and_then(|p| p.get("font"))
            .map(parse_font)
            .or(ctx.default_font);
        if let Some(f) = font {
            t = t.font(f);
        }
        if let Some(c) = props.and_then(|p| p.get("color")).and_then(parse_color) {
            t = t.color(c);
        }
        if let Some(w) = value_to_length_opt(props.and_then(|p| p.get("width"))) {
            t = t.width(w);
        }
        if let Some(h) = value_to_length_opt(props.and_then(|p| p.get("height"))) {
            t = t.height(h);
        }
        if let Some(lh) = parse_line_height(props) {
            t = t.line_height(lh);
        }
        if let Some(ax) = props
            .and_then(|p| p.get("align_x"))
            .and_then(|v| v.as_str())
            .and_then(value_to_horizontal_alignment)
        {
            t = t.align_x(ax);
        }
        if let Some(ay) = props
            .and_then(|p| p.get("align_y"))
            .and_then(|v| v.as_str())
            .and_then(value_to_vertical_alignment)
        {
            t = t.align_y(ay);
        }
        if let Some(w) = parse_wrapping(props) {
            t = t.wrapping(w);
        }
        if let Some(shaping) = parse_shaping(props) {
            t = t.shaping(shaping);
        }
        if let Some(e) = parse_ellipsis(props) {
            t = t.ellipsis(e);
        }

        // Named style
        if let Some(style_name) = prop_str(props, "style") {
            t = match style_name.as_str() {
                "primary" => t.style(text::primary),
                "secondary" => t.style(text::secondary),
                "success" => t.style(text::success),
                "danger" => t.style(text::danger),
                "warning" => t.style(text::warning),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "text"
                    );
                    t.style(text::default)
                }
            };
        }

        t.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(TextWidget)
    }
}
