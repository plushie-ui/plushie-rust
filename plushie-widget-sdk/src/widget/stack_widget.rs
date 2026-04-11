use iced::widget::Stack;
use iced::{Element, Length, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct StackWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for StackWidget {
    fn type_names(&self) -> &[&str] {
        &["stack"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);
        let clip = prop_bool_default(props, "clip", false);

        let children = ctx.render_children(node);

        Stack::with_children(children)
            .width(width)
            .height(height)
            .clip(clip)
            .into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(StackWidget)
    }
}
