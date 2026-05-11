use iced::widget::{Space, sensor};
use iced::{Element, Theme};
use serde_json::Value;

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
    on_resize: Option<String>,
}

impl SensorProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            delay: f64::extract(p, "delay"),
            anticipate: f32::extract(p, "anticipate"),
            on_resize: prop_str(p, "on_resize"),
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

        if let Some(ref tag) = sp.on_resize {
            // on_show: emit as "{tag}:show"
            {
                let wid = window_id.clone();
                let nid = node.id.clone();
                let family = format!("{}:show", tag);
                s = s.on_show(move |size| Message::Event {
                    window_id: wid.clone(),
                    id: nid.clone(),
                    family: family.clone(),
                    value: serde_json::json!({"width": size.width, "height": size.height}),
                });
            }
            // on_resize: emit with the tag directly
            {
                let wid = window_id.clone();
                let nid = node.id.clone();
                let family = tag.clone();
                s = s.on_resize(move |size| Message::Event {
                    window_id: wid.clone(),
                    id: nid.clone(),
                    family: family.clone(),
                    value: serde_json::json!({"width": size.width, "height": size.height}),
                });
            }
            // on_hide: emit as "{tag}:hide"
            {
                let wid = window_id.clone();
                let nid = node.id.clone();
                let family = format!("{}:hide", tag);
                s = s.on_hide(Message::Event {
                    window_id: wid,
                    id: nid,
                    family,
                    value: Value::Null,
                });
            }
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
