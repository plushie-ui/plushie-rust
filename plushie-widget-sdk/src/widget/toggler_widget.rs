use iced::widget::{container, toggler};
use iced::{Element, Length, Theme, widget};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

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
    let props = &node.props;
    let is_toggled = prop_bool_default(props, "is_toggled", false);
    let label = prop_str(props, "label");
    let spacing = prop_f32(props, "spacing");
    let width = prop_length(props, "width", Length::Shrink);
    let id = node.id.clone();

    let disabled = prop_bool_default(props, "disabled", false);

    let mut t = toggler(is_toggled).width(width);

    if !disabled {
        t = t.on_toggle(move |v| Message::Toggle(ctx.window_id.to_string(), id.clone(), v));
    }

    if let Some(l) = label {
        t = t.label(l);
    }
    if let Some(s) = spacing {
        t = t.spacing(s);
    }
    if let Some(sz) = prop_f32(props, "size") {
        t = t.size(sz);
    }
    if let Some(ts) = prop_f32(props, "text_size").or(ctx.default_text_size) {
        t = t.text_size(ts);
    }
    let font = props
        .get_value("font")
        .as_ref().map(parse_font)
        .or(ctx.default_font);
    if let Some(f) = font {
        t = t.font(f);
    }
    if let Some(lh) = parse_line_height(props) {
        t = t.line_height(lh);
    }
    if let Some(shaping) = parse_shaping(props) {
        t = t.shaping(shaping);
    }
    if let Some(w) = parse_wrapping(props) {
        t = t.wrapping(w);
    }
    if let Some(align) = props
        .get_str("text_alignment")
        .and_then(value_to_horizontal_alignment)
    {
        t = t.alignment(align);
    }

    // Style: string name or style map object
    if let Some(style_val) = props.get_value("style") {
        if let Some(style_name) = style_val.as_str() {
            t = match style_name {
                "default" => t.style(toggler::default),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "toggler"
                    );
                    t
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
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
