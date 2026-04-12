use iced::widget::Stack;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;

use plushie_core::types::{Length, PlushieType};

struct StackProps {
    width: Option<Length>,
    height: Option<Length>,
    clip: Option<bool>,
}

impl StackProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            clip: bool::extract(p, "clip"),
        }
    }
}

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
        let sp = StackProps::from_node(node);

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

        let children = ctx.render_children(node);

        Stack::with_children(children)
            .width(width)
            .height(height)
            .clip(sp.clip.unwrap_or(false))
            .into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(StackWidget)
    }
}
