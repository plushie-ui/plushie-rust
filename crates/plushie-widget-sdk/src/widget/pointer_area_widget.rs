use iced::widget::{mouse_area, Space};
use iced::{mouse, Element, Theme};
use serde_json::Value;

use crate::iced_convert;
use crate::message::Message;
use crate::protocol::{KeyModifiers, OutgoingEvent, TreeNode};
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;
use crate::PlushieRenderer;

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

fn pointer_press_message(
    window_id: String,
    id: String,
    position: iced::Point,
    button: &'static str,
) -> Message {
    pointer_message(
        window_id,
        id,
        OutgoingEvent::pointer_press(
            "",
            position.x,
            position.y,
            button,
            "mouse",
            None,
            KeyModifiers::default(),
        ),
    )
}

fn pointer_release_message(
    window_id: String,
    id: String,
    position: iced::Point,
    button: &'static str,
) -> Message {
    pointer_message(
        window_id,
        id,
        OutgoingEvent::pointer_release(
            "",
            position.x,
            position.y,
            button,
            "mouse",
            None,
            KeyModifiers::default(),
        ),
    )
}

fn pointer_double_click_message(window_id: String, id: String, position: iced::Point) -> Message {
    pointer_message(
        window_id,
        id,
        OutgoingEvent::pointer_double_click(
            "",
            position.x,
            position.y,
            "mouse",
            KeyModifiers::default(),
        ),
    )
}

fn pointer_message(window_id: String, id: String, event: OutgoingEvent) -> Message {
    Message::Event {
        window_id,
        id,
        family: event.family,
        value: event.value.unwrap_or(Value::Null),
    }
}

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
            ma = ma.on_middle_press(move |p| {
                pointer_press_message(wid.clone(), ev_id.clone(), p, "middle")
            });
        }
        if prop_bool_default(props, "on_right_press", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_right_press(move |p| {
                pointer_press_message(wid.clone(), ev_id.clone(), p, "right")
            });
        }
        if prop_bool_default(props, "on_right_release", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_right_release(move |p| {
                pointer_release_message(wid.clone(), ev_id.clone(), p, "right")
            });
        }
        if prop_bool_default(props, "on_middle_release", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_middle_release(move |p| {
                pointer_release_message(wid.clone(), ev_id.clone(), p, "middle")
            });
        }
        if prop_bool_default(props, "on_double_click", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_double_click(move |p| {
                pointer_double_click_message(wid.clone(), ev_id.clone(), p)
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

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(PointerAreaWidget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assert_event(message: Message, family: &str, value: Value) {
        match message {
            Message::Event {
                window_id,
                id,
                family: actual_family,
                value: actual_value,
            } => {
                assert_eq!(window_id, "main");
                assert_eq!(id, "area");
                assert_eq!(actual_family, family);
                assert_eq!(actual_value, value);
            }
            other => panic!("expected pointer event message, got {other:?}"),
        }
    }

    fn pointer_modifiers() -> Value {
        json!({
            "shift": false,
            "ctrl": false,
            "alt": false,
            "logo": false,
            "command": false,
        })
    }

    #[test]
    fn right_press_uses_canonical_pointer_press_payload() {
        let message = pointer_press_message(
            "main".to_string(),
            "area".to_string(),
            iced::Point::new(12.5, 7.0),
            "right",
        );

        assert_event(
            message,
            "press",
            json!({
                "x": 12.5,
                "y": 7.0,
                "button": "right",
                "pointer": "mouse",
                "modifiers": pointer_modifiers(),
            }),
        );
    }

    #[test]
    fn middle_release_uses_canonical_pointer_release_payload() {
        let message = pointer_release_message(
            "main".to_string(),
            "area".to_string(),
            iced::Point::new(2.0, 3.5),
            "middle",
        );

        assert_event(
            message,
            "release",
            json!({
                "x": 2.0,
                "y": 3.5,
                "button": "middle",
                "pointer": "mouse",
                "modifiers": pointer_modifiers(),
            }),
        );
    }

    #[test]
    fn double_click_uses_canonical_pointer_payload() {
        let message = pointer_double_click_message(
            "main".to_string(),
            "area".to_string(),
            iced::Point::new(4.0, 8.0),
        );

        assert_event(
            message,
            "double_click",
            json!({
                "x": 4.0,
                "y": 8.0,
                "pointer": "mouse",
                "modifiers": pointer_modifiers(),
            }),
        );
    }
}
