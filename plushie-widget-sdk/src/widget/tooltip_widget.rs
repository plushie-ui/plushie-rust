use std::time::Duration;

use iced::widget::{Space, container, text, tooltip};
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct TooltipWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for TooltipWidget {
    fn type_names(&self) -> &[&str] {
        &["tooltip"]
    }

    /// Render a tooltip widget that shows a popup hint on hover/focus.
    ///
    /// # Accessibility
    ///
    /// The tooltip `tip` text is rendered visually on hover but is not
    /// automatically exposed as the child widget's accessible description.
    /// For AT users to hear the tooltip content, the host should wire the
    /// tooltip text into the child's `a11y.description` prop, or use
    /// `a11y.described_by` to point to a separate text node containing the
    /// same content. iced's tooltip widget itself does not currently emit
    /// an accessible `Tooltip` role.
    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = &node.props;
        let tip = prop_str(props, "tip").unwrap_or_default();
        let gap = prop_f32(props, "gap");
        let position = prop_str(props, "position")
            .map(|s| match s.to_ascii_lowercase().as_str() {
                "bottom" => tooltip::Position::Bottom,
                "left" => tooltip::Position::Left,
                "right" => tooltip::Position::Right,
                "follow_cursor" | "follow" => tooltip::Position::FollowCursor,
                _ => tooltip::Position::Top,
            })
            .unwrap_or(tooltip::Position::Top);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let mut tt = tooltip(child, text(tip), position);
        if let Some(g) = gap {
            tt = tt.gap(g);
        }

        // Tooltip padding is a single f32 value (not per-side)
        if let Some(p) = prop_f32(props, "padding") {
            tt = tt.padding(p);
        }

        let snap = prop_bool_default(props, "snap_within_viewport", true);
        tt = tt.snap_within_viewport(snap);

        if let Some(d) = prop_f64(props, "delay") {
            tt = tt.delay(Duration::from_millis(d as u64));
        }

        // Style: string name or style map (tooltip uses container::Style)
        if let Some(style_val) = props.get_value("style") {
            if let Some(style_name) = style_val.as_str() {
                tt = match style_name {
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
                            style_name,
                            "tooltip"
                        );
                        tt
                    }
                };
            } else if let Some(obj) = style_val.as_object() {
                let ov = get_style_overrides(&node.id, obj, ctx.caches);
                tt = tt.style(move |_theme| container_style_from_base(&ov.base));
            }
        }

        tt.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(TooltipWidget)
    }
}
