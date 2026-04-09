use iced::widget::{Space, button, container, text};
use iced::{Element, Length, Theme, widget};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct ButtonWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for ButtonWidget {
    fn type_names(&self) -> &[&str] {
        &["button"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = node.props.as_object();
        let id = node.id.clone();
        let window_id = ctx.window_id.to_string();

        // Button can have either a text label or child content
        let child: Element<'a, Message, Theme, R> = if !node.children.is_empty() {
            node.children
                .first()
                .map(|c| ctx.render_child(c))
                .unwrap_or_else(|| Space::new().into())
        } else {
            let label = prop_str(props, "label")
                .or_else(|| prop_str(props, "content"))
                .unwrap_or_default();
            text(label).into()
        };

        let padding = parse_padding_value(props);
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);
        let clip = prop_bool_default(props, "clip", false);
        let disabled = prop_bool_default(props, "disabled", false)
            || !prop_bool_default(props, "enabled", true);

        let mut b = button(child).width(width).height(height).clip(clip);

        if let Some(p) = padding {
            b = b.padding(p);
        }

        if !disabled {
            b = b.on_press(Message::Click(window_id, id));
        }

        // Style: string name or style map object
        if let Some(style_val) = props.and_then(|p| p.get("style")) {
            if let Some(style_name) = style_val.as_str() {
                b = match style_name {
                    "primary" => b.style(button::primary),
                    "secondary" => b.style(button::secondary),
                    "success" => b.style(button::success),
                    "warning" => b.style(button::warning),
                    "danger" => b.style(button::danger),
                    "text" => b.style(button::text),
                    "background" => b.style(button::background),
                    "subtle" => b.style(button::subtle),
                    _ => {
                        log::warn!(
                            "unknown style {:?} for widget type {:?}, using default",
                            style_name,
                            "button"
                        );
                        b.style(button::primary)
                    }
                };
            } else if let Some(obj) = style_val.as_object() {
                let ov = get_style_overrides(&node.id, obj, ctx.caches);
                b = b.style(move |theme: &iced::Theme, status| {
                    let mut style = match ov.preset_base.as_deref() {
                        Some("primary") => button::primary(theme, status),
                        Some("secondary") => button::secondary(theme, status),
                        Some("success") => button::success(theme, status),
                        Some("danger") => button::danger(theme, status),
                        Some("warning") => button::warning(theme, status),
                        Some("text") => button::text(theme, status),
                        Some("background") => button::background(theme, status),
                        Some("subtle") => button::subtle(theme, status),
                        _ => button::primary(theme, status),
                    };
                    apply_button_fields(&mut style, &ov.base);
                    match status {
                        button::Status::Hovered => {
                            if let Some(ref f) = ov.hovered {
                                apply_button_fields(&mut style, f);
                            } else {
                                style.background = auto_derive_hover_bg(style.background);
                            }
                        }
                        button::Status::Pressed => {
                            if let Some(ref f) = ov.pressed {
                                apply_button_fields(&mut style, f);
                            }
                        }
                        button::Status::Disabled => {
                            if let Some(ref f) = ov.disabled {
                                apply_button_fields(&mut style, f);
                            } else {
                                style.background = auto_derive_disabled_bg(style.background);
                                style.text_color = auto_derive_disabled_text(style.text_color);
                                style.border = auto_derive_disabled_border(style.border);
                                style.shadow = auto_derive_disabled_shadow(style.shadow);
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
            b = b.on_status_change(move |status| {
                Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
            });
        }

        container(b).id(widget::Id::from(node.id.clone())).into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ButtonWidget)
    }
}
