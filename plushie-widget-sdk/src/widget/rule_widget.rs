use iced::widget::rule;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{PlushieType, Style as CoreStyle};

struct RuleProps {
    direction: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
    thickness: Option<f32>,
    style: Option<CoreStyle>,
}

impl RuleProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            direction: String::extract(p, "direction"),
            width: f32::extract(p, "width"),
            height: f32::extract(p, "height"),
            thickness: f32::extract(p, "thickness"),
            style: CoreStyle::extract(p, "style"),
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

        // Style: preset name or custom style map
        match &rp.style {
            Some(CoreStyle::Preset(name)) => {
                r = match name.as_str() {
                    "default" => r.style(rule::default),
                    "weak" => r.style(rule::weak),
                    _ => {
                        log::warn!(
                            "unknown style {:?} for widget type {:?}, using default",
                            name,
                            "rule"
                        );
                        r
                    }
                };
            }
            Some(CoreStyle::Custom(style_map)) => {
                let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
                r = r.style(move |theme: &iced::Theme| {
                    let base_fn: fn(&iced::Theme) -> rule::Style =
                        match ov.preset_base.as_deref() {
                            Some("default") => rule::default,
                            Some("weak") => rule::weak,
                            _ => rule::default,
                        };
                    apply_rule_style(base_fn(theme), &ov.base)
                });
            }
            None => {}
        }
        r.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(RuleWidget)
    }
}
