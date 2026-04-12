use iced::widget::{container, toggler};
use iced::{Element, Theme, widget};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{
    Font, HorizontalAlignment, Length, LineHeight, PlushieType, Shaping, Style as CoreStyle,
    Wrapping,
};

struct TogglerProps {
    label: Option<String>,
    is_toggled: bool,
    disabled: bool,
    spacing: Option<f32>,
    width: Option<Length>,
    size: Option<f32>,
    text_size: Option<f32>,
    font: Option<Font>,
    line_height: Option<LineHeight>,
    shaping: Option<Shaping>,
    wrapping: Option<Wrapping>,
    text_alignment: Option<HorizontalAlignment>,
    style: Option<CoreStyle>,
}

impl TogglerProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            label: String::extract(p, "label"),
            is_toggled: prop_bool_default(p, "is_toggled", false),
            disabled: prop_bool_default(p, "disabled", false),
            spacing: f32::extract(p, "spacing"),
            width: Length::extract(p, "width"),
            size: f32::extract(p, "size"),
            text_size: f32::extract(p, "text_size"),
            font: Font::extract(p, "font"),
            line_height: LineHeight::extract(p, "line_height"),
            shaping: Shaping::extract(p, "shaping"),
            wrapping: Wrapping::extract(p, "wrapping"),
            text_alignment: HorizontalAlignment::extract(p, "text_alignment"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

pub(crate) struct TogglerWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for TogglerWidget {
    fn type_names(&self) -> &[&str] {
        &["toggler"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        render_toggler(node, *ctx)
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(TogglerWidget)
    }
}

fn render_toggler<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let tp = TogglerProps::from_node(node);
    let id = node.id.clone();

    let width = tp
        .width
        .as_ref()
        .map(iced_convert::length)
        .unwrap_or(iced::Length::Shrink);

    let mut t = toggler(tp.is_toggled).width(width);

    if !tp.disabled {
        t = t.on_toggle(move |v| Message::Toggle(ctx.window_id.to_string(), id.clone(), v));
    }

    if let Some(l) = tp.label {
        t = t.label(l);
    }
    if let Some(s) = tp.spacing {
        t = t.spacing(s);
    }
    if let Some(sz) = tp.size {
        t = t.size(sz);
    }
    if let Some(ts) = tp.text_size.or(ctx.default_text_size) {
        t = t.text_size(ts);
    }
    let font = tp.font.map(|f| iced_convert::font(&f)).or(ctx.default_font);
    if let Some(f) = font {
        t = t.font(f);
    }
    if let Some(lh) = tp.line_height {
        t = t.line_height(iced_convert::line_height(lh));
    }
    if let Some(s) = tp.shaping {
        t = t.shaping(iced_convert::shaping(s));
    }
    if let Some(w) = tp.wrapping {
        t = t.wrapping(iced_convert::wrapping(w));
    }
    if let Some(align) = tp.text_alignment {
        t = t.alignment(iced_convert::horizontal_alignment(align));
    }

    // Style: preset name or custom style map
    match &tp.style {
        Some(CoreStyle::Preset(name)) => {
            t = match name.as_str() {
                "default" => t.style(toggler::default),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        name,
                        "toggler"
                    );
                    t
                }
            };
        }
        Some(CoreStyle::Custom(style_map)) => {
            let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
            t = t.style(move |theme: &iced::Theme, status| {
                let mut style = match ov.preset_base.as_deref() {
                    Some("default") => toggler::default(theme, status),
                    _ => toggler::default(theme, status),
                };
                apply_toggler_fields(&mut style, &ov.base);
                match status {
                    toggler::Status::Hovered { .. } => {
                        if let Some(ref f) = ov.hovered {
                            apply_toggler_fields(&mut style, f);
                        } else {
                            style.background = deviate_background(style.background, 0.1);
                        }
                    }
                    toggler::Status::Disabled { .. } => {
                        if let Some(ref f) = ov.disabled {
                            apply_toggler_fields(&mut style, f);
                        } else {
                            style.background = match style.background {
                                iced::Background::Color(c) => {
                                    iced::Background::Color(alpha_color(c, 0.5))
                                }
                                iced::Background::Gradient(g) => {
                                    iced::Background::Gradient(alpha_gradient(g, 0.5))
                                }
                            };
                            if let Some(tc) = style.text_color {
                                style.text_color = Some(alpha_color(tc, 0.5));
                            }
                            style.background_border_color =
                                alpha_color(style.background_border_color, 0.5);
                            style.foreground_border_color =
                                alpha_color(style.foreground_border_color, 0.5);
                        }
                    }
                    _ => {}
                }
                style
            });
        }
        None => {}
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        t = t.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    container(t).id(widget::Id::from(node.id.clone())).into()
}
