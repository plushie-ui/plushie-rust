use iced::widget::{Space, container, sensor};
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;

use plushie_core::types::{Length, PlushieType};

struct ResponsiveProps {
    width: Option<Length>,
    height: Option<Length>,
}

impl ResponsiveProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
        }
    }
}

pub(crate) struct ResponsiveWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for ResponsiveWidget {
    fn type_names(&self) -> &[&str] {
        &["responsive"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        // iced's Responsive widget takes a closure that receives Size and returns
        // an Element. Since we can't call back to the host within a single frame,
        // we render the children as-is and wrap in a sensor so the host receives
        // resize events with the actual measured size.
        let rp = ResponsiveProps::from_node(node);

        let width = rp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Fill);
        let height = rp
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Fill);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let window_id = ctx.window_id.to_string();
        let resize_id = node.id.clone();

        sensor(container(child).width(width).height(height))
            .key(node.id.clone())
            .on_resize(move |size| {
                Message::SensorResize(
                    window_id.clone(),
                    resize_id.clone(),
                    size.width,
                    size.height,
                )
            })
            .into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ResponsiveWidget)
    }
}
