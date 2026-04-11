use iced::widget::container;
use iced::{Element, Theme, widget};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{
    Font, Length, LineHeight, PlushieType, Shaping, Style as CoreStyle, Wrapping,
};

struct RadioProps {
    label: Option<String>,
    value: Option<String>,
    selected: Option<String>,
    group: Option<String>,
    spacing: Option<f32>,
    size: Option<f32>,
    width: Option<Length>,
    font: Option<Font>,
    text_size: Option<f32>,
    line_height: Option<LineHeight>,
    shaping: Option<Shaping>,
    wrapping: Option<Wrapping>,
    style: Option<CoreStyle>,
}

impl RadioProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            label: String::extract(p, "label"),
            value: String::extract(p, "value"),
            selected: String::extract(p, "selected"),
            group: String::extract(p, "group"),
            spacing: f32::extract(p, "spacing"),
            size: f32::extract(p, "size"),
            width: Length::extract(p, "width"),
            font: Font::extract(p, "font"),
            text_size: f32::extract(p, "text_size"),
            line_height: LineHeight::extract(p, "line_height"),
            shaping: Shaping::extract(p, "shaping"),
            wrapping: Wrapping::extract(p, "wrapping"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

pub(crate) struct RadioWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for RadioWidget {
    fn type_names(&self) -> &[&str] {
        &["radio"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        render_radio(node, *ctx)
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(RadioWidget)
    }
}

fn render_radio<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let rp = RadioProps::from_node(node);

    let value = rp.value.unwrap_or_default();
    let selected_str = rp.selected.unwrap_or_default();
    let label = rp.label.unwrap_or_else(|| value.clone());
    // Use "group" prop as the event ID so all radios in a group emit the same ID.
    let event_id = rp.group.unwrap_or_else(|| node.id.clone());

    let is_selected = if value == selected_str {
        Some(0u8)
    } else {
        None
    };
    let select_value = value;

    let mut r = iced::widget::Radio::new(label, 0u8, is_selected, move |_| {
        Message::Select(
            ctx.window_id.to_string(),
            event_id.clone(),
            select_value.clone(),
        )
    });

    if let Some(s) = rp.spacing {
        r = r.spacing(s);
    }
    if let Some(ref w) = rp.width {
        r = r.width(iced_convert::length(w));
    }
    if let Some(sz) = rp.size {
        r = r.size(sz);
    }
    if let Some(ts) = rp.text_size.or(ctx.default_text_size) {
        r = r.text_size(ts);
    }
    let font = rp
        .font
        .map(|f| iced_convert::font(&f))
        .or(ctx.default_font);
    if let Some(f) = font {
        r = r.font(f);
    }
    if let Some(lh) = rp.line_height {
        r = r.line_height(iced_convert::line_height(lh));
    }
    if let Some(s) = rp.shaping {
        r = r.shaping(iced_convert::shaping(s));
    }
    if let Some(w) = rp.wrapping {
        r = r.wrapping(iced_convert::wrapping(w));
    }

    // Style: preset name or custom style map
    match &rp.style {
        Some(CoreStyle::Preset(name)) => {
            r = match name.as_str() {
                "default" => r.style(iced::widget::radio::default),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        name,
                        "radio"
                    );
                    r
                }
            };
        }
        Some(CoreStyle::Custom(style_map)) => {
            let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
            r = r.style(move |theme: &iced::Theme, status| {
                let mut style = match ov.preset_base.as_deref() {
                    Some("default") => iced::widget::radio::default(theme, status),
                    _ => iced::widget::radio::default(theme, status),
                };
                apply_radio_fields(&mut style, &ov.base);
                if matches!(status, iced::widget::radio::Status::Hovered { .. }) {
                    if let Some(ref f) = ov.hovered {
                        apply_radio_fields(&mut style, f);
                    } else {
                        style.background = deviate_background(style.background, 0.1);
                    }
                }
                style
            });
        }
        None => {}
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        r = r.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    container(r).id(widget::Id::from(node.id.clone())).into()
}
