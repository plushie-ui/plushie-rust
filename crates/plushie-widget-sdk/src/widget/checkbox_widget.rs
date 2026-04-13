use iced::widget::text::LineHeight;
use iced::widget::{checkbox, container};
use iced::{Element, Font, Pixels, Theme, widget};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{
    Font as PlushieFont, Length, LineHeight as PlushieLineHeight, PlushieType, Shaping,
    Style as CoreStyle, Wrapping,
};

struct CheckboxProps {
    label: Option<String>,
    checked: bool,
    disabled: bool,
    spacing: Option<f32>,
    size: Option<f32>,
    width: Option<Length>,
    font: Option<PlushieFont>,
    text_size: Option<f32>,
    line_height: Option<PlushieLineHeight>,
    shaping: Option<Shaping>,
    wrapping: Option<Wrapping>,
    style: Option<CoreStyle>,
}

impl CheckboxProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            label: String::extract(p, "label"),
            checked: prop_bool_default(p, "checked", false),
            disabled: prop_bool_default(p, "disabled", false),
            spacing: f32::extract(p, "spacing"),
            size: f32::extract(p, "size"),
            width: Length::extract(p, "width"),
            font: PlushieFont::extract(p, "font"),
            text_size: f32::extract(p, "text_size"),
            line_height: PlushieLineHeight::extract(p, "line_height"),
            shaping: Shaping::extract(p, "shaping"),
            wrapping: Wrapping::extract(p, "wrapping"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

pub(crate) struct CheckboxWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for CheckboxWidget {
    fn type_names(&self) -> &[&str] {
        &["checkbox"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        render_checkbox(node, *ctx)
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(CheckboxWidget)
    }
}

fn render_checkbox<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let cp = CheckboxProps::from_node(node);
    let id = node.id.clone();

    let label = cp.label.unwrap_or_default();
    let width = cp
        .width
        .as_ref()
        .map(iced_convert::length)
        .unwrap_or(iced::Length::Shrink);

    let mut cb = checkbox(cp.checked).label(label).width(width);

    if !cp.disabled {
        cb = cb.on_toggle(move |v| Message::Event {
            window_id: ctx.window_id.to_string(),
            id: id.clone(),
            value: Value::Bool(v),
            family: "toggle".into(),
        });
    }

    if let Some(s) = cp.spacing {
        cb = cb.spacing(s);
    }
    if let Some(sz) = cp.size {
        cb = cb.size(sz);
    }
    if let Some(ts) = cp.text_size.or(ctx.default_text_size) {
        cb = cb.text_size(ts);
    }
    let font = cp.font.map(|f| iced_convert::font(&f)).or(ctx.default_font);
    if let Some(f) = font {
        cb = cb.font(f);
    }
    if let Some(lh) = cp.line_height {
        cb = cb.line_height(iced_convert::line_height(lh));
    }
    if let Some(s) = cp.shaping {
        cb = cb.shaping(iced_convert::shaping(s));
    }
    if let Some(w) = cp.wrapping {
        cb = cb.wrapping(iced_convert::wrapping(w));
    }

    // Icon: complex nested object, kept as raw prop access
    let icon_prop = node.props.get_value("icon");
    if let Some(icon_val) = icon_prop.as_ref().and_then(|v| v.as_object())
        && let Some(cp_str) = icon_val.get("code_point").and_then(|v| v.as_str())
        && let Some(code_point) = cp_str.chars().next()
    {
        let icon_font = icon_val
            .get("font")
            .and_then(plushie_core::types::Font::wire_decode)
            .map(|f| iced_convert::font(&f))
            .unwrap_or(Font::DEFAULT);
        let icon_size = icon_val
            .get("size")
            .and_then(|v| v.as_f64())
            .map(|v| Pixels(v as f32));
        let icon_line_height = icon_val
            .get("line_height")
            .and_then(|v| match v {
                Value::Number(n) => n.as_f64().map(|r| LineHeight::Relative(r as f32)),
                Value::Object(obj) => {
                    if let Some(r) = obj.get("relative").and_then(|v| v.as_f64()) {
                        Some(LineHeight::Relative(r as f32))
                    } else {
                        obj.get("absolute")
                            .and_then(|v| v.as_f64())
                            .map(|a| LineHeight::Absolute(Pixels(a as f32)))
                    }
                }
                _ => None,
            })
            .unwrap_or(LineHeight::default());
        let icon_shaping = icon_val
            .get("shaping")
            .and_then(|v| v.as_str())
            .and_then(|s| match s.to_ascii_lowercase().as_str() {
                "basic" => Some(iced::widget::text::Shaping::Basic),
                "advanced" => Some(iced::widget::text::Shaping::Advanced),
                "auto" => Some(iced::widget::text::Shaping::Auto),
                _ => None,
            })
            .unwrap_or(iced::widget::text::Shaping::Auto);
        let icon_struct = checkbox::Icon {
            font: icon_font,
            code_point,
            size: icon_size,
            line_height: icon_line_height,
            shaping: icon_shaping,
        };
        cb = cb.icon(icon_struct);
    }

    // Style: preset name or custom style map
    match &cp.style {
        Some(CoreStyle::Preset(name)) => {
            cb = match name.as_str() {
                "primary" => cb.style(checkbox::primary),
                "secondary" => cb.style(checkbox::secondary),
                "success" => cb.style(checkbox::success),
                "danger" => cb.style(checkbox::danger),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        name,
                        "checkbox"
                    );
                    cb.style(checkbox::primary)
                }
            };
        }
        Some(CoreStyle::Custom(style_map)) => {
            let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
            cb = cb.style(move |theme: &iced::Theme, status| {
                let mut style = match ov.preset_base.as_deref() {
                    Some("primary") => checkbox::primary(theme, status),
                    Some("secondary") => checkbox::secondary(theme, status),
                    Some("success") => checkbox::success(theme, status),
                    Some("danger") => checkbox::danger(theme, status),
                    _ => checkbox::primary(theme, status),
                };
                apply_checkbox_fields(&mut style, &ov.base);
                match status {
                    checkbox::Status::Hovered { .. } => {
                        if let Some(ref f) = ov.hovered {
                            apply_checkbox_fields(&mut style, f);
                        } else {
                            style.background = deviate_background(style.background, 0.1);
                        }
                    }
                    checkbox::Status::Disabled { .. } => {
                        if let Some(ref f) = ov.disabled {
                            apply_checkbox_fields(&mut style, f);
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
                            style.border = auto_derive_disabled_border(style.border);
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
        cb = cb.on_status_change(move |status| Message::Event {
            window_id: status_wid.clone(),
            id: status_id.clone(),
            value: Value::String(status.to_string()),
            family: "status".into(),
        });
    }

    container(cb).id(widget::Id::from(node.id.clone())).into()
}
