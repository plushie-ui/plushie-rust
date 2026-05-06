use iced::widget::{Space, button, container, text};
use iced::{Element, Theme, widget};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::a11y::A11yOverrides;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{A11y, Length, Padding, PlushieType, Style as CoreStyle};

struct ButtonProps {
    label: Option<String>,
    content: Option<String>,
    padding: Option<Padding>,
    width: Option<Length>,
    height: Option<Length>,
    style: Option<CoreStyle>,
}

impl ButtonProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            label: String::extract(p, "label"),
            content: String::extract(p, "content"),
            padding: Padding::extract(p, "padding"),
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

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
        let bp = ButtonProps::from_node(node);
        let id = node.id.clone();
        let window_id = ctx.window_id.to_string();

        // Button can have either a text label or child content
        let child: Element<'a, Message, Theme, R> = if !node.children.is_empty() {
            node.children
                .first()
                .map(|c| ctx.render_child(c))
                .unwrap_or_else(|| Space::new().into())
        } else {
            let label = bp.label.or(bp.content).unwrap_or_default();
            text(label).into()
        };

        let width = bp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let height = bp
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let clip = prop_bool_default(&node.props, "clip", false);
        let disabled = prop_bool_default(&node.props, "disabled", false)
            || !prop_bool_default(&node.props, "enabled", true);

        let mut b = button(child).width(width).height(height).clip(clip);

        if let Some(ref p) = bp.padding {
            b = b.padding(iced_convert::padding(p));
        }

        if !disabled {
            b = b.on_press(Message::Event {
                window_id,
                id,
                value: Value::Null,
                family: "click".into(),
            });
        }

        // Style: preset name or custom style map
        match &bp.style {
            Some(CoreStyle::Preset(name)) => {
                b = match name.as_str() {
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
                            name,
                            "button"
                        );
                        b.style(button::primary)
                    }
                };
            }
            Some(CoreStyle::Custom(style_map)) => {
                let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
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
            None => {}
        }

        {
            let status_wid = ctx.window_id.to_string();
            let status_id = node.id.clone();
            b = b.on_status_change(move |status| Message::Event {
                window_id: status_wid.clone(),
                id: status_id.clone(),
                value: Value::String(status.to_string()),
                family: "status".into(),
            });
        }

        container(b).id(widget::Id::from(node.id.clone())).into()
    }

    fn infer_a11y(&self, node: &TreeNode) -> Option<A11yOverrides> {
        let mut a11y = A11y::new();
        let mut any = false;
        if let Some(c) = node
            .props
            .get_str("mnemonic")
            .or_else(|| node.props.get_str("access_key"))
            .and_then(|s| s.chars().next())
        {
            a11y = a11y.mnemonic(c);
            any = true;
        }
        let disabled = prop_bool_default(&node.props, "disabled", false)
            || !prop_bool_default(&node.props, "enabled", true);
        if disabled {
            a11y = a11y.disabled(true);
            any = true;
        }
        if any {
            Some(A11yOverrides::from_core(&a11y))
        } else {
            None
        }
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ButtonWidget)
    }
}
