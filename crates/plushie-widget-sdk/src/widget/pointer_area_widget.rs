use iced::widget::{Space, mouse_area};
use iced::{Element, Theme, mouse};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{CursorStyle, PlushieType};

struct PointerAreaProps {
    cursor: Option<CursorStyle>,
    on_press: Option<String>,
    on_release: Option<String>,
}

impl PointerAreaProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            cursor: CursorStyle::extract(p, "cursor"),
            on_press: prop_str(p, "on_press"),
            on_release: prop_str(p, "on_release"),
        }
    }
}

pub(crate) struct PointerAreaWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for PointerAreaWidget {
    fn type_names(&self) -> &[&str] {
        &["pointer_area"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let pap = PointerAreaProps::from_node(node);
        let props = &node.props;

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let window_id = ctx.window_id.to_string();

        let mut ma = mouse_area(child);

        if let Some(ref tag) = pap.on_press {
            let wid = window_id.clone();
            let nid = node.id.clone();
            let family = tag.clone();
            ma = ma.on_press(move |_p| Message::Event {
                window_id: wid.clone(),
                id: nid.clone(),
                family: family.clone(),
                value: Value::Null,
            });
        }
        if let Some(ref tag) = pap.on_release {
            let wid = window_id.clone();
            let nid = node.id.clone();
            let family = tag.clone();
            ma = ma.on_release(move |_p| Message::Event {
                window_id: wid.clone(),
                id: nid.clone(),
                family: family.clone(),
                value: Value::Null,
            });
        }

        // Conditional event handlers (opt-in via boolean props)
        if prop_bool_default(props, "on_middle_press", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_middle_press(move |p| Message::Event {
                window_id: wid.clone(),
                id: ev_id.clone(),
                value: serde_json::json!({"x": p.x, "y": p.y}),
                family: "middle_press".into(),
            });
        }
        if prop_bool_default(props, "on_right_press", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_right_press(move |p| Message::Event {
                window_id: wid.clone(),
                id: ev_id.clone(),
                value: serde_json::json!({"x": p.x, "y": p.y}),
                family: "right_press".into(),
            });
        }
        if prop_bool_default(props, "on_right_release", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_right_release(move |p| Message::Event {
                window_id: wid.clone(),
                id: ev_id.clone(),
                value: serde_json::json!({"x": p.x, "y": p.y}),
                family: "right_release".into(),
            });
        }
        if prop_bool_default(props, "on_middle_release", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_middle_release(move |p| Message::Event {
                window_id: wid.clone(),
                id: ev_id.clone(),
                value: serde_json::json!({"x": p.x, "y": p.y}),
                family: "middle_release".into(),
            });
        }
        if prop_bool_default(props, "on_double_click", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_double_click(move |p| Message::Event {
                window_id: wid.clone(),
                id: ev_id.clone(),
                value: serde_json::json!({"x": p.x, "y": p.y}),
                family: "double_click".into(),
            });
        }
        if prop_bool_default(props, "on_enter", false) {
            let ev_id = node.id.clone();
            ma = ma.on_enter(Message::Event {
                window_id: window_id.clone(),
                id: ev_id,
                value: Value::Null,
                family: "enter".into(),
            });
        }
        if prop_bool_default(props, "on_exit", false) {
            let ev_id = node.id.clone();
            ma = ma.on_exit(Message::Event {
                window_id: window_id.clone(),
                id: ev_id,
                value: Value::Null,
                family: "exit".into(),
            });
        }
        if prop_bool_default(props, "on_move", false) {
            let ev_id = node.id.clone();
            let move_window_id = window_id.clone();
            ma = ma.on_move(move |p| Message::Event {
                window_id: move_window_id.clone(),
                id: ev_id.clone(),
                value: serde_json::json!({"x": p.x, "y": p.y}),
                family: "move".into(),
            });
        }
        if prop_bool_default(props, "on_scroll", false) {
            let ev_id = node.id.clone();
            let scroll_window_id = window_id.clone();
            ma = ma.on_scroll(move |delta, position| {
                let (dx, dy) = match delta {
                    mouse::ScrollDelta::Lines { x, y } => (x, y),
                    mouse::ScrollDelta::Pixels { x, y } => (x, y),
                };
                Message::Event {
                    window_id: scroll_window_id.clone(),
                    id: ev_id.clone(),
                    value: serde_json::json!({"delta_x": dx, "delta_y": dy, "x": position.x, "y": position.y}),
                    family: "scroll".into(),
                }
            });
        }

        if let Some(cursor) = pap.cursor {
            ma = ma.interaction(iced_convert::cursor_style(cursor));
        }

        ma.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(PointerAreaWidget)
    }
}
