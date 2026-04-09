use iced::widget::progress_bar;
use iced::{Element, Length, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widgets::helpers::*;

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
        let props = node.props.as_object();
        let range = prop_range_f32(props);
        let value = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "value")
            .unwrap_or(0.0)
            .clamp(*range.start(), *range.end());
        let width = prop_length(props, "width", Length::Fill);
        let height = prop_length(props, "height", Length::Shrink);

        let mut pb = progress_bar(range, value).length(width).girth(height);

        if prop_bool_default(props, "vertical", false) {
            pb = pb.vertical();
        }
        if let Some(label) = prop_str(props, "label") {
            pb = pb.label(label);
        }

        // Style: string name or style map object
        if let Some(style_val) = props.and_then(|p| p.get("style")) {
            if let Some(style_name) = style_val.as_str() {
                pb = match style_name {
                    "primary" => pb.style(progress_bar::primary),
                    "secondary" => pb.style(progress_bar::secondary),
                    "success" => pb.style(progress_bar::success),
                    "danger" => pb.style(progress_bar::danger),
                    "warning" => pb.style(progress_bar::warning),
                    _ => {
                        log::warn!(
                            "unknown style {:?} for widget type {:?}, using default",
                            style_name,
                            "progress_bar"
                        );
                        pb.style(progress_bar::primary)
                    }
                };
            } else if let Some(obj) = style_val.as_object() {
                let ov = get_style_overrides(&node.id, obj, ctx.caches);
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
        }

        pb.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ProgressBarWidget)
    }
}
