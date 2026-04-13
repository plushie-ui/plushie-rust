use iced::widget::Svg;
use iced::{Element, Radians, Rotation, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{Color, ContentFit, Length, PlushieType};

struct SvgProps {
    width: Option<Length>,
    height: Option<Length>,
    content_fit: Option<ContentFit>,
    color: Option<Color>,
    alt: Option<String>,
    description: Option<String>,
}

impl SvgProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            content_fit: ContentFit::extract(p, "content_fit"),
            color: Color::extract(p, "color"),
            alt: String::extract(p, "alt"),
            description: String::extract(p, "description"),
        }
    }
}

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
        let sp = SvgProps::from_node(node);
        let props = &node.props;

        // source: kept as raw prop access (file path string)
        let source = prop_str(props, "source").unwrap_or_default();
        if source.is_empty() {
            log::warn!("[id={}] svg: no 'source' prop specified", node.id);
        }

        let width = sp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let height = sp
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);

        let mut s = Svg::from_path(source).width(width).height(height);
        if let Some(cf) = sp.content_fit {
            s = s.content_fit(iced_convert::content_fit(cf));
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
        if let Some(alt) = sp.alt {
            s = s.alt(alt);
        }
        if let Some(desc) = sp.description {
            s = s.description(desc);
        }
        if prop_bool_default(props, "decorative", false) {
            s = s.decorative();
        }
        if let Some(ref c) = sp.color {
            let ic = iced_convert::color(c);
            s = s.style(move |_theme, _status| iced::widget::svg::Style { color: Some(ic) });
        }

        s.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SvgWidget)
    }
}
