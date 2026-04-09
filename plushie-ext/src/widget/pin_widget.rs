use iced::widget::{Space, pin};
use iced::{Element, Length, Point, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

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
        let props = node.props.as_object();
        let x =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "x").unwrap_or(0.0);
        let y =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "y").unwrap_or(0.0);
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);

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

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(PinWidget)
    }
}
