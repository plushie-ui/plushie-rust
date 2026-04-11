use iced::widget::text;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{
    Color, Ellipsis, Font, HorizontalAlignment, Length, LineHeight, PlushieType, Shaping,
    Style as CoreStyle, VerticalAlignment, Wrapping,
};

struct TextProps {
    content: Option<String>,
    size: Option<f32>,
    color: Option<Color>,
    font: Option<Font>,
    width: Option<Length>,
    height: Option<Length>,
    line_height: Option<LineHeight>,
    align_x: Option<HorizontalAlignment>,
    align_y: Option<VerticalAlignment>,
    wrapping: Option<Wrapping>,
    shaping: Option<Shaping>,
    ellipsis: Option<Ellipsis>,
    style: Option<CoreStyle>,
}

impl TextProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            content: String::extract(p, "content"),
            size: f32::extract(p, "size"),
            color: Color::extract(p, "color"),
            font: Font::extract(p, "font"),
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            line_height: LineHeight::extract(p, "line_height"),
            align_x: HorizontalAlignment::extract(p, "align_x"),
            align_y: VerticalAlignment::extract(p, "align_y"),
            wrapping: Wrapping::extract(p, "wrapping"),
            shaping: Shaping::extract(p, "shaping"),
            ellipsis: Ellipsis::extract(p, "ellipsis"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

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
        let tp = TextProps::from_node(node);

        let content = tp.content.unwrap_or_default();
        let size = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, &node.props, "size")
            .or(tp.size)
            .or(ctx.default_text_size);

        let mut t = text(content);
        if let Some(s) = size {
            t = t.size(s);
        }
        let font = tp
            .font
            .map(|f| iced_convert::font(&f))
            .or(ctx.default_font);
        if let Some(f) = font {
            t = t.font(f);
        }
        if let Some(ref c) = tp.color {
            t = t.color(iced_convert::color(c));
        }
        if let Some(ref w) = tp.width {
            t = t.width(iced_convert::length(w));
        }
        if let Some(ref h) = tp.height {
            t = t.height(iced_convert::length(h));
        }
        if let Some(ref lh) = tp.line_height {
            t = t.line_height(iced_convert::line_height(*lh));
        }
        if let Some(ax) = tp.align_x {
            t = t.align_x(iced_convert::horizontal_alignment(ax));
        }
        if let Some(ay) = tp.align_y {
            t = t.align_y(iced_convert::vertical_alignment(ay));
        }
        if let Some(w) = tp.wrapping {
            t = t.wrapping(iced_convert::wrapping(w));
        }
        if let Some(s) = tp.shaping {
            t = t.shaping(iced_convert::shaping(s));
        }
        if let Some(e) = tp.ellipsis {
            t = t.ellipsis(iced_convert::ellipsis(e));
        }

        // Named style preset (text widget only supports presets)
        if let Some(CoreStyle::Preset(name)) = &tp.style {
            t = match name.as_str() {
                "primary" => t.style(text::primary),
                "secondary" => t.style(text::secondary),
                "success" => t.style(text::success),
                "danger" => t.style(text::danger),
                "warning" => t.style(text::warning),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        name,
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
