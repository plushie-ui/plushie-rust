use iced::widget::{Space, container};
use iced::{Element, Fill, Length, Theme, widget};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct ContainerWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for ContainerWidget {
    fn type_names(&self) -> &[&str] {
        &["container"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = node.props.as_object();
        let padding = parse_padding_value(props);
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);
        let center = prop_bool_default(props, "center", false);
        let clip = prop_bool_default(props, "clip", false);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let mut c = container(child).width(width).height(height).clip(clip);

        if let Some(p) = padding {
            c = c.padding(p);
        }

        if let Some(mw) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "max_width")
        {
            c = c.max_width(mw);
        }
        if let Some(mh) = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            props,
            "max_height",
        ) {
            c = c.max_height(mh);
        }

        if center {
            c = c.center(Fill);
        }

        if let Some(ax) = props
            .and_then(|p| p.get("align_x"))
            .and_then(|v| v.as_str())
            .and_then(value_to_horizontal_alignment)
        {
            c = c.align_x(ax);
        }
        if let Some(ay) = props
            .and_then(|p| p.get("align_y"))
            .and_then(|v| v.as_str())
            .and_then(value_to_vertical_alignment)
        {
            c = c.align_y(ay);
        }

        // Inline styling via custom style closure
        let bg = props
            .and_then(|p| p.get("background"))
            .and_then(parse_background);
        let text_color = props.and_then(|p| p.get("color")).and_then(parse_color);
        let border_val = props.and_then(|p| p.get("border")).map(parse_border);
        let shadow_val = props.and_then(|p| p.get("shadow")).map(parse_shadow);
        let has_inline_style =
            bg.is_some() || text_color.is_some() || border_val.is_some() || shadow_val.is_some();

        if has_inline_style {
            c = c.style(move |_theme| {
                let mut style = container::Style {
                    background: bg,
                    text_color,
                    ..Default::default()
                };
                if let Some(b) = border_val {
                    style.border = b;
                }
                if let Some(s) = shadow_val {
                    style.shadow = s;
                }
                style
            });
        }

        // Named style or style map (overrides inline if both present)
        if let Some(style_val) = props.and_then(|p| p.get("style")) {
            if let Some(style_name) = style_val.as_str() {
                c = match style_name {
                    "transparent" => c.style(container::transparent),
                    "rounded_box" => c.style(container::rounded_box),
                    "bordered_box" => c.style(container::bordered_box),
                    "dark" => c.style(container::dark),
                    "primary" => c.style(container::primary),
                    "secondary" => c.style(container::secondary),
                    "success" => c.style(container::success),
                    "danger" => c.style(container::danger),
                    "warning" => c.style(container::warning),
                    _ => {
                        log::warn!(
                            "unknown style {:?} for widget type {:?}, using default",
                            style_name,
                            "container"
                        );
                        c
                    }
                };
            } else if let Some(obj) = style_val.as_object() {
                let ov = get_style_overrides(&node.id, obj, ctx.caches);
                c = c.style(move |theme| {
                    let mut style = match ov.preset_base.as_deref() {
                        Some("transparent") => container::transparent(theme),
                        Some("rounded_box") => container::rounded_box(theme),
                        Some("bordered_box") => container::bordered_box(theme),
                        Some("dark") => container::dark(theme),
                        Some("primary") => container::primary(theme),
                        Some("secondary") => container::secondary(theme),
                        Some("success") => container::success(theme),
                        Some("danger") => container::danger(theme),
                        Some("warning") => container::warning(theme),
                        _ => container::Style::default(),
                    };
                    if let Some(bg) = ov.base.background {
                        style.background = Some(bg);
                    }
                    if let Some(tc) = ov.base.text_color {
                        style.text_color = Some(tc);
                    }
                    if let Some(brd) = ov.base.border {
                        style.border = brd;
                    }
                    if let Some(shd) = ov.base.shadow {
                        style.shadow = shd;
                    }
                    style
                });
            }
        }

        // Widget ID for operations targeting
        c = c.id(widget::Id::from(node.id.clone()));

        c.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ContainerWidget)
    }
}
