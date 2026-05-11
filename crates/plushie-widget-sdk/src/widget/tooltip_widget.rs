use iced::widget::{Space, container, text, tooltip};
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{PlushieType, Style as CoreStyle};

struct TooltipProps {
    tip: Option<String>,
    gap: Option<f32>,
    padding: Option<f32>,
    snap_within_viewport: Option<bool>,
    delay: Option<f64>,
    style: Option<CoreStyle>,
}

impl TooltipProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            tip: String::extract(p, "tip"),
            gap: f32::extract(p, "gap"),
            padding: f32::extract(p, "padding"),
            snap_within_viewport: bool::extract(p, "snap_within_viewport"),
            delay: f64::extract(p, "delay"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

pub(crate) struct TooltipWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for TooltipWidget {
    fn type_names(&self) -> &[&str] {
        &["tooltip"]
    }

    /// Render a tooltip widget that shows a popup hint on hover/focus.
    ///
    /// # Accessibility
    ///
    /// Non-empty `tip` text is passed to iced as tooltip text, which
    /// emits a tooltip accessible node and a `described_by` relationship
    /// from the child to that node.
    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let tp = TooltipProps::from_node(node);

        // Position: keep as raw prop access (tooltip::Position includes
        // FollowCursor which doesn't map to plushie-core's Position type)
        let position = prop_str(&node.props, "position")
            .map(|s| match s.to_ascii_lowercase().as_str() {
                "bottom" => tooltip::Position::Bottom,
                "left" => tooltip::Position::Left,
                "right" => tooltip::Position::Right,
                "follow_cursor" | "follow" => tooltip::Position::FollowCursor,
                _ => {
                    log::warn!(
                        "[id={}] tooltip: unknown position {:?}, using top",
                        node.id,
                        s
                    );
                    tooltip::Position::Top
                }
            })
            .unwrap_or(tooltip::Position::Top);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let tip = tp.tip.unwrap_or_default();
        let mut tt = tooltip(child, text(tip.clone()), position);
        if !tip.trim().is_empty() {
            tt = tt.tooltip_text(tip);
        }
        if let Some(g) = tp.gap {
            tt = tt.gap(g);
        }

        // Tooltip padding is a single f32 value (not per-side)
        if let Some(p) = tp.padding {
            tt = tt.padding(p);
        }

        let snap = tp.snap_within_viewport.unwrap_or(true);
        tt = tt.snap_within_viewport(snap);

        if let Some(d) = tp.delay
            && let Some(delay) = duration_from_millis_prop(&node.id, "delay", d)
        {
            tt = tt.delay(delay);
        }

        // Style: preset name or custom style map
        match &tp.style {
            Some(CoreStyle::Preset(name)) => {
                tt = match name.as_str() {
                    "transparent" => tt.style(container::transparent),
                    "rounded_box" => tt.style(container::rounded_box),
                    "bordered_box" => tt.style(container::bordered_box),
                    "dark" => tt.style(container::dark),
                    "primary" => tt.style(container::primary),
                    "secondary" => tt.style(container::secondary),
                    "success" => tt.style(container::success),
                    "danger" => tt.style(container::danger),
                    "warning" => tt.style(container::warning),
                    _ => {
                        log::warn!(
                            "unknown style {:?} for widget type {:?}, using default",
                            name,
                            "tooltip"
                        );
                        tt
                    }
                };
            }
            Some(CoreStyle::Custom(style_map)) => {
                let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
                tt = tt.style(move |_theme| container_style_from_base(&ov.base));
            }
            None => {}
        }

        tt.into()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(TooltipWidget)
    }
}
