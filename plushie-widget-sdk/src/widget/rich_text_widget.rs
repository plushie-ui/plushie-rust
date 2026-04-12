use iced::widget::text::LineHeight;
use iced::widget::{rich_text, span};
use iced::{Element, Font, Padding, Pixels, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{
    Border as CoreBorder, Color, Ellipsis, Font as CoreFont, Length, PlushieType, Wrapping,
};

struct RichTextProps {
    width: Option<Length>,
    height: Option<Length>,
    font: Option<plushie_core::types::Font>,
    color: Option<Color>,
    line_height: Option<plushie_core::types::LineHeight>,
    wrapping: Option<Wrapping>,
    ellipsis: Option<Ellipsis>,
}

impl RichTextProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            font: plushie_core::types::Font::extract(p, "font"),
            color: Color::extract(p, "color"),
            line_height: plushie_core::types::LineHeight::extract(p, "line_height"),
            wrapping: Wrapping::extract(p, "wrapping"),
            ellipsis: Ellipsis::extract(p, "ellipsis"),
        }
    }
}

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
        let rp = RichTextProps::from_node(node);
        let width = rp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let height = rp
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);

        // Spans: keep as raw prop access (complex array of objects)
        let spans_val = node.props.get_value("spans");
        let spans_value = spans_val.as_ref().and_then(|v| v.as_array());

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
                        if let Some(c) = sv.get("color").and_then(Color::wire_decode) {
                            s = s.color(iced_convert::color(&c));
                        }
                        if let Some(f) = sv.get("font").and_then(CoreFont::wire_decode) {
                            s = s.font(iced_convert::font(&f));
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
                            if let Some(bg) = hl.get("background").and_then(Color::wire_decode) {
                                s = s.background(iced_convert::color(&bg));
                            }
                            if let Some(b) = hl.get("border").and_then(CoreBorder::wire_decode) {
                                s = s.border(iced_convert::border(&b));
                            }
                        }
                        s
                    })
                    .collect()
            })
            .unwrap_or_default();

        let id = node.id.clone();
        let mut rt = rich_text(span_list).width(width).height(height);

        // size: animated prop, keep as raw access for transition support
        if let Some(sz) = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            &node.props,
            "size",
        )
        .or(ctx.default_text_size)
        {
            rt = rt.size(sz);
        }
        let font = rp.font.map(|f| iced_convert::font(&f)).or(ctx.default_font);
        if let Some(f) = font {
            rt = rt.font(f);
        }
        if let Some(ref c) = rp.color {
            rt = rt.color(iced_convert::color(c));
        }
        if let Some(lh) = rp.line_height {
            rt = rt.line_height(iced_convert::line_height(lh));
        }
        if let Some(w) = rp.wrapping {
            rt = rt.wrapping(iced_convert::wrapping(w));
        }
        if let Some(e) = rp.ellipsis {
            rt = rt.ellipsis(iced_convert::ellipsis(e));
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
