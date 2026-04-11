use iced::widget::Space;
use iced::{Element, Length, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

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
        let props = &node.props;
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);
        Space::new().width(width).height(height).into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SpaceWidget)
    }
}
