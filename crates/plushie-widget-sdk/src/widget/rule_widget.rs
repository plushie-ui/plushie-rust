use iced::widget::rule;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{PlushieType, Style as CoreStyle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuleDirection {
    Horizontal,
    Vertical,
}

fn parse_rule_direction(value: Option<&str>, widget_id: &str) -> RuleDirection {
    match value {
        Some("vertical") => RuleDirection::Vertical,
        Some("horizontal") | None => RuleDirection::Horizontal,
        Some(other) => {
            log::warn!(
                "unknown direction {:?} for rule {:?}, using horizontal",
                other,
                widget_id
            );
            RuleDirection::Horizontal
        }
    }
}

struct RuleProps {
    direction: RuleDirection,
    width: Option<f32>,
    height: Option<f32>,
    thickness: Option<f32>,
    style: Option<CoreStyle>,
}

impl RuleProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            direction: parse_rule_direction(String::extract(p, "direction").as_deref(), &node.id),
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
        let is_vertical = rp.direction == RuleDirection::Vertical;

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
                    let base_fn: fn(&iced::Theme) -> rule::Style = match ov.preset_base.as_deref() {
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

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(RuleWidget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_direction_parser_accepts_known_values() {
        assert_eq!(
            parse_rule_direction(Some("horizontal"), "rule"),
            RuleDirection::Horizontal
        );
        assert_eq!(
            parse_rule_direction(Some("vertical"), "rule"),
            RuleDirection::Vertical
        );
    }

    #[test]
    fn rule_direction_parser_defaults_missing_and_unknown_to_horizontal() {
        assert_eq!(
            parse_rule_direction(None, "rule"),
            RuleDirection::Horizontal
        );
        assert_eq!(
            parse_rule_direction(Some("diagonal"), "rule"),
            RuleDirection::Horizontal
        );
    }
}
