use iced::widget::{Space, container};
use iced::{Element, Fill, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;

use plushie_core::types::{Length, Padding, PlushieType};

struct WindowProps {
    padding: Option<Padding>,
    width: Option<Length>,
    height: Option<Length>,
}

impl WindowProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            padding: Padding::extract(p, "padding"),
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
        }
    }
}

pub(crate) struct WindowWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for WindowWidget {
    fn type_names(&self) -> &[&str] {
        &["window"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let wp = WindowProps::from_node(node);

        let width = wp.width.as_ref().map(iced_convert::length).unwrap_or(Fill);
        let height = wp.height.as_ref().map(iced_convert::length).unwrap_or(Fill);

        let child_ctx = ctx.with_window_id(&node.id);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| child_ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let mut c = container(child).width(width).height(height);

        if let Some(ref p) = wp.padding {
            c = c.padding(iced_convert::padding(p));
        }

        c.into()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(WindowWidget)
    }
}
