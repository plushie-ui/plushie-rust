use iced::widget::progress_bar;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::a11y::A11yOverrides;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{A11y, Length, PlushieType, Style as CoreStyle, ValueRange};

struct ProgressBarProps {
    range: Option<ValueRange>,
    value: Option<f32>,
    width: Option<Length>,
    height: Option<Length>,
    vertical: bool,
    label: Option<String>,
    style: Option<CoreStyle>,
}

impl ProgressBarProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            range: ValueRange::extract(p, "range"),
            value: f32::extract(p, "value"),
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            vertical: prop_bool_default(p, "vertical", false),
            label: String::extract(p, "label"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

pub(crate) struct ProgressBarWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for ProgressBarWidget {
    fn type_names(&self) -> &[&str] {
        &["progress_bar"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let pbp = ProgressBarProps::from_node(node);

        let range = pbp.range.unwrap_or(ValueRange::new(0.0, 100.0));
        let range_inclusive = range.min..=range.max;
        let value = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            &node.props,
            "value",
        )
        .or(pbp.value)
        .unwrap_or(0.0)
        .clamp(range.min, range.max);
        let width = pbp
            .width
            .map(|l| iced_convert::length(&l))
            .unwrap_or(iced::Length::Fill);
        let height = pbp
            .height
            .map(|l| iced_convert::length(&l))
            .unwrap_or(iced::Length::Shrink);

        let mut pb = progress_bar(range_inclusive, value)
            .length(width)
            .girth(height);

        if pbp.vertical {
            pb = pb.vertical();
        }
        if let Some(label) = pbp.label {
            pb = pb.label(label);
        }

        // Style: preset name or custom style map
        match &pbp.style {
            Some(CoreStyle::Preset(name)) => {
                pb = match name.as_str() {
                    "primary" => pb.style(progress_bar::primary),
                    "secondary" => pb.style(progress_bar::secondary),
                    "success" => pb.style(progress_bar::success),
                    "danger" => pb.style(progress_bar::danger),
                    "warning" => pb.style(progress_bar::warning),
                    _ => {
                        log::warn!(
                            "unknown style {:?} for widget type {:?}, using default",
                            name,
                            "progress_bar"
                        );
                        pb.style(progress_bar::primary)
                    }
                };
            }
            Some(CoreStyle::Custom(style_map)) => {
                let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
                pb = pb.style(move |theme: &iced::Theme| {
                    let mut style = match ov.preset_base.as_deref() {
                        Some("primary") => progress_bar::primary(theme),
                        Some("secondary") => progress_bar::secondary(theme),
                        Some("success") => progress_bar::success(theme),
                        Some("danger") => progress_bar::danger(theme),
                        Some("warning") => progress_bar::warning(theme),
                        _ => progress_bar::primary(theme),
                    };
                    apply_progress_bar_fields(&mut style, &ov.base);
                    style
                });
            }
            None => {}
        }

        pb.into()
    }

    fn infer_a11y(&self, node: &TreeNode) -> Option<A11yOverrides> {
        Some(A11yOverrides::from_core(
            &A11y::new().value(format_progress_value(node)),
        ))
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ProgressBarWidget)
    }
}

/// Format the progress value as a screen-reader-friendly string.
///
/// Returns `"45%"` when the range is the default 0-100, otherwise
/// `"45/100"` with the configured maximum.
fn format_progress_value(node: &TreeNode) -> String {
    let pbp = ProgressBarProps::from_node(node);
    let range = pbp.range.unwrap_or(ValueRange::new(0.0, 100.0));
    let raw = pbp.value.unwrap_or(range.min).clamp(range.min, range.max);
    if (range.min - 0.0).abs() < f32::EPSILON && (range.max - 100.0).abs() < f32::EPSILON {
        format!("{}%", raw.round() as i64)
    } else {
        format!("{}/{}", raw.round() as i64, range.max.round() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn infer(props: serde_json::Value) -> Option<A11yOverrides> {
        let node = crate::testing::node_with_props("p", "progress_bar", props);
        <ProgressBarWidget as PlushieWidget<iced::Renderer>>::infer_a11y(&ProgressBarWidget, &node)
    }

    #[test]
    fn percent_format_for_default_range() {
        let o = infer(json!({"value": 45.0})).expect("progress_bar always infers a value");
        assert_eq!(o.core().value.as_deref(), Some("45%"));
    }

    #[test]
    fn fraction_format_for_non_default_range() {
        let o = infer(json!({
            "value": 7.0,
            "range": [0.0, 10.0]
        }))
        .expect("progress_bar always infers a value");
        assert_eq!(o.core().value.as_deref(), Some("7/10"));
    }

    #[test]
    fn missing_value_defaults_to_range_min() {
        let o = infer(json!({})).expect("progress_bar always infers a value");
        assert_eq!(o.core().value.as_deref(), Some("0%"));
    }

    #[test]
    fn over_range_value_is_clamped() {
        let o = infer(json!({"value": 250.0})).expect("progress_bar always infers a value");
        assert_eq!(o.core().value.as_deref(), Some("100%"));
    }
}
