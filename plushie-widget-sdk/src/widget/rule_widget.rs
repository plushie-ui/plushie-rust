use iced::widget::rule;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::PlushieType;

struct RuleProps {
    direction: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
    thickness: Option<f32>,
}

impl RuleProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            direction: String::extract(p, "direction"),
            width: f32::extract(p, "width"),
            height: f32::extract(p, "height"),
            thickness: f32::extract(p, "thickness"),
        }
    }
}

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
        let rp = RuleProps::from_node(node);
        let is_vertical = rp.direction.as_deref() == Some("vertical");

        // Thickness is the cross-axis dimension:
        // horizontal rule -> height, vertical rule -> width.
        // "thickness" is a universal alias for either.
        let thickness = if is_vertical { rp.width } else { rp.height }
            .or(rp.thickness)
            .unwrap_or(1.0);

        let mut r = if is_vertical {
            rule::vertical(thickness)
        } else {
            rule::horizontal(thickness)
        };

        // Style: string name or style map object
        if let Some(style_val) = node.props.get_value("style") {
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
