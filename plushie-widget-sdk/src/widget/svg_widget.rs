use iced::widget::Svg;
use iced::{Element, Length, Radians, Rotation, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct SvgWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for SvgWidget {
    fn type_names(&self) -> &[&str] {
        &["svg"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = &node.props;
        let source = prop_str(props, "source").unwrap_or_default();
        if source.is_empty() {
            log::warn!("[id={}] svg: no 'source' prop specified", node.id);
        }
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);
        let content_fit = prop_content_fit(props);

        let mut s = Svg::from_path(source).width(width).height(height);
        if let Some(cf) = content_fit {
            s = s.content_fit(cf);
        }
        if let Some(r) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "rotation")
        {
            s = s.rotation(Rotation::from(Radians(r.to_radians())));
        }
        if let Some(o) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "opacity")
        {
            s = s.opacity(o);
        }
        if let Some(alt) = prop_str(props, "alt") {
            s = s.alt(alt);
        }
        if let Some(desc) = prop_str(props, "description") {
            s = s.description(desc);
        }
        if prop_bool_default(props, "decorative", false) {
            s = s.decorative();
        }
        if let Some(color_str) = prop_str(props, "color")
            && let Some(c) = crate::theming::parse_hex_color(&color_str)
        {
            s = s.style(move |_theme, _status| iced::widget::svg::Style { color: Some(c) });
        }

        s.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SvgWidget)
    }
}
