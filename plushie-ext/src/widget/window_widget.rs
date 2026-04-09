use iced::widget::{Space, container};
use iced::{Element, Fill, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

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
        let props = node.props.as_object();
        let padding = parse_padding_value(props);
        let width = prop_length(props, "width", Fill);
        let height = prop_length(props, "height", Fill);

        let child_ctx = ctx.with_window_id(&node.id);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| child_ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let mut c = container(child).width(width).height(height);

        if let Some(p) = padding {
            c = c.padding(p);
        }

        c.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(WindowWidget)
    }
}
