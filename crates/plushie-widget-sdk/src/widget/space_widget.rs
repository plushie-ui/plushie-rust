use iced::widget::Space;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use plushie_core::types::{Length, PlushieType};

struct SpaceProps {
    width: Option<Length>,
    height: Option<Length>,
}

impl SpaceProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
        }
    }
}

pub(crate) struct SpaceWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for SpaceWidget {
    fn type_names(&self) -> &[&str] {
        &["space"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let _ = ctx;
        let sp = SpaceProps::from_node(node);
        let width = sp
            .width
            .map(|l| iced_convert::length(&l))
            .unwrap_or(iced::Length::Shrink);
        let height = sp
            .height
            .map(|l| iced_convert::length(&l))
            .unwrap_or(iced::Length::Shrink);
        Space::new().width(width).height(height).into()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SpaceWidget)
    }
}
