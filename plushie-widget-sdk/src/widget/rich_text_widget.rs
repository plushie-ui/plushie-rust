use iced::widget::text::LineHeight;
use iced::widget::{rich_text, span};
use iced::{Element, Font, Length, Padding, Pixels, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct RichTextWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for RichTextWidget {
    fn type_names(&self) -> &[&str] {
        &["rich_text", "rich"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);

        // spans is an array of objects: {text, size, color, font, link}
        let spans_value = props
            .and_then(|p| p.get("spans"))
            .and_then(|v| v.as_array());

        let span_list: Vec<iced::widget::text::Span<'a, String, Font>> = spans_value
            .map(|arr| {
                arr.iter()
                    .map(|sv| {
                        let content = sv
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned();
                        let mut s = span(content);
                        if let Some(sz) = sv.get("size").and_then(|v| v.as_f64()) {
                            s = s.size(Pixels(sz as f32));
                        }
                        if let Some(c) = sv.get("color").and_then(parse_color) {
                            s = s.color(c);
                        }
                        if let Some(f) = sv.get("font") {
                            s = s.font(parse_font(f));
                        }
                        if let Some(link) = sv.get("link").and_then(|v| v.as_str()) {
                            s = s.link(link.to_owned());
                        }
                        if let Some(true) = sv.get("underline").and_then(|v| v.as_bool()) {
                            s = s.underline(true);
                        }
                        if let Some(true) = sv.get("strikethrough").and_then(|v| v.as_bool()) {
                            s = s.strikethrough(true);
                        }
                        if let Some(lh) = sv.get("line_height").and_then(|v| v.as_f64()) {
                            s = s.line_height(LineHeight::Relative(lh as f32));
                        }
                        if let Some(p) = sv.get("padding") {
                            if let Some(n) = p.as_f64() {
                                s = s.padding(n as f32);
                            } else if let Some(obj) = p.as_object() {
                                let top =
                                    obj.get("top").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let right =
                                    obj.get("right").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let bottom =
                                    obj.get("bottom").and_then(|v| v.as_f64()).unwrap_or(0.0)
                                        as f32;
                                let left =
                                    obj.get("left").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                s = s.padding(Padding {
                                    top,
                                    right,
                                    bottom,
                                    left,
                                });
                            }
                        }
                        if let Some(hl) = sv.get("highlight").and_then(|v| v.as_object()) {
                            if let Some(bg) = hl.get("background").and_then(parse_color) {
                                s = s.background(bg);
                            }
                            if let Some(b) = hl.get("border") {
                                s = s.border(parse_border(b));
                            }
                        }
                        s
                    })
                    .collect()
            })
            .unwrap_or_default();

        let id = node.id.clone();
        let mut rt = rich_text(span_list).width(width).height(height);

        if let Some(sz) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "size")
            .or(ctx.default_text_size)
        {
            rt = rt.size(sz);
        }
        let font = props
            .and_then(|p| p.get("font"))
            .map(parse_font)
            .or(ctx.default_font);
        if let Some(f) = font {
            rt = rt.font(f);
        }
        if let Some(c) = props.and_then(|p| p.get("color")).and_then(parse_color) {
            rt = rt.color(c);
        }
        if let Some(lh) = parse_line_height(props) {
            rt = rt.line_height(lh);
        }
        if let Some(w) = parse_wrapping(props) {
            rt = rt.wrapping(w);
        }
        if let Some(e) = parse_ellipsis(props) {
            rt = rt.ellipsis(e);
        }

        let window_id = ctx.window_id.to_string();
        rt = rt.on_link_click(move |link| {
            Message::Click(window_id.clone(), format!("{}:{}", id, link))
        });

        rt.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(RichTextWidget)
    }
}
