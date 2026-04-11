use iced::widget::rule;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct RuleWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for RuleWidget {
    fn type_names(&self) -> &[&str] {
        &["rule"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = &node.props;
        let direction = prop_str(props, "direction").unwrap_or_default();

        // Thickness is the cross-axis dimension:
        // horizontal rule -> height, vertical rule -> width.
        // "thickness" is a universal alias for either.
        let thickness = if direction == "vertical" {
            prop_f32(props, "width")
        } else {
            prop_f32(props, "height")
        }
        .or_else(|| prop_f32(props, "thickness"))
        .unwrap_or(1.0);

        let mut r = if direction == "vertical" {
            rule::vertical(thickness)
        } else {
            rule::horizontal(thickness)
        };
        if let Some(style_val) = props.get_value("style") {
            if let Some(style_name) = style_val.as_str() {
                r = match style_name {
                    "default" => r.style(rule::default),
                    "weak" => r.style(rule::weak),
                    _ => {
                        log::warn!(
                            "unknown style {:?} for widget type {:?}, using default",
                            style_name,
                            "rule"
                        );
                        r
                    }
                };
            } else if let Some(obj) = style_val.as_object() {
                let ov = get_style_overrides(&node.id, obj, ctx.caches);
                r = r.style(move |theme: &iced::Theme| {
                    let base_fn: fn(&iced::Theme) -> rule::Style = match ov.preset_base.as_deref() {
                        Some("default") => rule::default,
                        Some("weak") => rule::weak,
                        _ => rule::default,
                    };
                    apply_rule_style(base_fn(theme), &ov.base)
                });
            }
        }
        r.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(RuleWidget)
    }
}
