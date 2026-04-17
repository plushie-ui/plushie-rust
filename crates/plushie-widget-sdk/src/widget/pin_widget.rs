use iced::widget::{Space, pin};
use iced::{Element, Point, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{Length, PlushieType};

struct PinProps {
    x: Option<f32>,
    y: Option<f32>,
    width: Option<Length>,
    height: Option<Length>,
}

impl PinProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            x: f32::extract(p, "x"),
            y: f32::extract(p, "y"),
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
        }
    }
}

pub(crate) struct PinWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for PinWidget {
    fn type_names(&self) -> &[&str] {
        &["pin"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let pp = PinProps::from_node(node);
        let props = &node.props;

        let x = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "x")
            .or(pp.x)
            .unwrap_or(0.0);
        let y = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "y")
            .or(pp.y)
            .unwrap_or(0.0);
        let width = pp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let height = pp
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        pin(child)
            .position(Point::new(x, y))
            .width(width)
            .height(height)
            .into()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(PinWidget)
    }
}
