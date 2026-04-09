use std::time::Duration;

use iced::widget::{Space, sensor};
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct SensorWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for SensorWidget {
    fn type_names(&self) -> &[&str] {
        &["sensor"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        // Sensor needs a key. Use the node id.
        let id = node.id.clone();
        let show_id = node.id.clone();
        let resize_id = node.id.clone();
        let hide_id = format!("{}:hide", node.id);

        let props = node.props.as_object();

        let mut s = sensor(child)
            .key(id)
            .on_show({
                let window_id = ctx.window_id.to_string();
                move |size| {
                    Message::SensorResize(
                        window_id.clone(),
                        format!("{}:show", show_id),
                        size.width,
                        size.height,
                    )
                }
            })
            .on_resize({
                let window_id = ctx.window_id.to_string();
                move |size| {
                    Message::SensorResize(
                        window_id.clone(),
                        resize_id.clone(),
                        size.width,
                        size.height,
                    )
                }
            })
            .on_hide(Message::Click(ctx.window_id.to_string(), hide_id));

        if let Some(d) = prop_f64(props, "delay") {
            s = s.delay(Duration::from_millis(d as u64));
        }
        if let Some(a) = prop_f32(props, "anticipate") {
            s = s.anticipate(a);
        }

        s.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SensorWidget)
    }
}
