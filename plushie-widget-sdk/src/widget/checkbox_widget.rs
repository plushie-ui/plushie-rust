use iced::widget::text::LineHeight;
use iced::widget::{checkbox, container};
use iced::{Element, Font, Length, Pixels, Theme, widget};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

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
    let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
    let label = prop_str(props, "label").unwrap_or_default();
    let checked = prop_bool_default(props, "checked", false);
    let spacing = prop_f32(props, "spacing");
    let width = prop_length(props, "width", Length::Shrink);
    let id = node.id.clone();

    let disabled = prop_bool_default(props, "disabled", false);

    let mut cb = checkbox(checked).label(label).width(width);

    if !disabled {
        cb = cb.on_toggle(move |v| Message::Toggle(ctx.window_id.to_string(), id.clone(), v));
    }

    if let Some(s) = spacing {
        cb = cb.spacing(s);
    }
    if let Some(sz) = prop_f32(props, "size") {
        cb = cb.size(sz);
    }
    if let Some(ts) = prop_f32(props, "text_size").or(ctx.default_text_size) {
        cb = cb.text_size(ts);
    }
    let font = props
        .and_then(|p| p.get("font"))
        .map(parse_font)
        .or(ctx.default_font);
    if let Some(f) = font {
        cb = cb.font(f);
    }
    if let Some(lh) = parse_line_height(props) {
        cb = cb.line_height(lh);
    }
    if let Some(shaping) = parse_shaping(props) {
        cb = cb.shaping(shaping);
    }
    if let Some(w) = parse_wrapping(props) {
        cb = cb.wrapping(w);
    }
    if let Some(icon_val) = props
        .and_then(|p| p.get("icon"))
        .and_then(|v| v.as_object())
        && let Some(cp_str) = icon_val.get("code_point").and_then(|v| v.as_str())
        && let Some(code_point) = cp_str.chars().next()
    {
        let icon_font = icon_val
            .get("font")
            .map(parse_font)
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
    // Style: string name or style map object
    if let Some(style_val) = props.and_then(|p| p.get("style")) {
        if let Some(style_name) = style_val.as_str() {
            cb = match style_name {
                "primary" => cb.style(checkbox::primary),
                "secondary" => cb.style(checkbox::secondary),
                "success" => cb.style(checkbox::success),
                "danger" => cb.style(checkbox::danger),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "checkbox"
                    );
                    cb.style(checkbox::primary)
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
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
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        cb = cb.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    container(cb).id(widget::Id::from(node.id.clone())).into()
}
