use iced::widget::{Space, mouse_area};
use iced::{Element, Theme, mouse};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widgets::helpers::*;

pub(crate) struct PointerAreaWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for PointerAreaWidget {
    fn type_names(&self) -> &[&str] {
        &["mouse_area"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = node.props.as_object();
        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let id = node.id.clone();
        let release_id = format!("{}:release", node.id);
        let window_id = ctx.window_id.to_string();

        let mut ma = mouse_area(child)
            .on_press({
                let wid = window_id.clone();
                let nid = id.clone();
                move |_p| Message::Click(wid.clone(), nid.clone())
            })
            .on_release({
                let wid = window_id.clone();
                let rid = release_id.clone();
                move |_p| Message::Click(wid.clone(), rid.clone())
            });

        // Conditional event handlers (opt-in via boolean props)
        if prop_bool_default(props, "on_middle_press", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_middle_press(move |p| {
                Message::MouseAreaEvent(wid.clone(), ev_id.clone(), "middle_press".into(), p.x, p.y)
            });
        }
        if prop_bool_default(props, "on_right_press", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_right_press(move |p| {
                Message::MouseAreaEvent(wid.clone(), ev_id.clone(), "right_press".into(), p.x, p.y)
            });
        }
        if prop_bool_default(props, "on_right_release", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_right_release(move |p| {
                Message::MouseAreaEvent(wid.clone(), ev_id.clone(), "right_release".into(), p.x, p.y)
            });
        }
        if prop_bool_default(props, "on_middle_release", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_middle_release(move |p| {
                Message::MouseAreaEvent(
                    wid.clone(),
                    ev_id.clone(),
                    "middle_release".into(),
                    p.x,
                    p.y,
                )
            });
        }
        if prop_bool_default(props, "on_double_click", false) {
            let ev_id = node.id.clone();
            let wid = window_id.clone();
            ma = ma.on_double_click(move |p| {
                Message::MouseAreaEvent(wid.clone(), ev_id.clone(), "double_click".into(), p.x, p.y)
            });
        }
        if prop_bool_default(props, "on_enter", false) {
            let ev_id = node.id.clone();
            ma = ma.on_enter(Message::MouseAreaEvent(
                window_id.clone(),
                ev_id,
                "enter".into(),
                0.0,
                0.0,
            ));
        }
        if prop_bool_default(props, "on_exit", false) {
            let ev_id = node.id.clone();
            ma = ma.on_exit(Message::MouseAreaEvent(
                window_id.clone(),
                ev_id,
                "exit".into(),
                0.0,
                0.0,
            ));
        }
        if prop_bool_default(props, "on_move", false) {
            let ev_id = node.id.clone();
            let move_window_id = window_id.clone();
            ma = ma.on_move(move |p| {
                Message::MouseAreaMove(move_window_id.clone(), ev_id.clone(), p.x, p.y)
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
                Message::MouseAreaScroll(
                    scroll_window_id.clone(),
                    ev_id.clone(),
                    dx,
                    dy,
                    position.x,
                    position.y,
                )
            });
        }

        if let Some(cursor) = prop_str(props, "cursor")
            && let Some(interaction) = parse_interaction(&cursor)
        {
            ma = ma.interaction(interaction);
        }

        ma.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(PointerAreaWidget)
    }
}
