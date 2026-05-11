use iced::widget::{Space, sensor};
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::PlushieType;

struct SensorProps {
    delay: Option<f64>,
    anticipate: Option<f32>,
    on_resize: bool,
}

impl SensorProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            delay: f64::extract(p, "delay"),
            anticipate: f32::extract(p, "anticipate"),
            on_resize: prop_bool_default(p, "on_resize", false),
        }
    }
}

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
        let sp = SensorProps::from_node(node);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let id = node.id.clone();
        let window_id = ctx.window_id.to_string();

        let mut s = sensor(child).key(id);

        if sp.on_resize {
            let wid = window_id.clone();
            let nid = node.id.clone();
            s = s.on_resize(move |size| Message::Event {
                window_id: wid.clone(),
                id: nid.clone(),
                family: "resize".to_string(),
                value: serde_json::json!({"width": size.width, "height": size.height}),
            });
        }

        if let Some(d) = sp.delay
            && let Some(delay) = duration_from_millis_prop(&node.id, "delay", d)
        {
            s = s.delay(delay);
        }
        if let Some(a) = sp.anticipate {
            s = s.anticipate(a);
        }

        s.into()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SensorWidget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn on_resize_is_boolean_enablement() {
        let node = crate::testing::node_with_props("sensor", "sensor", json!({"on_resize": true}));
        let props = SensorProps::from_node(&node);

        assert!(props.on_resize);
    }

    #[test]
    fn string_on_resize_is_not_a_resize_subscription() {
        let node = crate::testing::node_with_props("sensor", "sensor", json!({"on_resize": "tag"}));
        let props = SensorProps::from_node(&node);

        assert!(!props.on_resize);
    }
}
