use iced::widget::text;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;
use crate::widget::overlay;

pub(crate) struct OverlayWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for OverlayWidget {
    fn type_names(&self) -> &[&str] {
        &["overlay"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
        let position = prop_str(props, "position").unwrap_or_else(|| "below".to_string());
        let gap = prop_f32(props, "gap").unwrap_or(0.0);
        let offset_x = prop_f32(props, "offset_x").unwrap_or(0.0);
        let offset_y = prop_f32(props, "offset_y").unwrap_or(0.0);
        let flip = prop_bool_default(props, "flip", false);
        let align = match prop_str(props, "align").as_deref() {
            Some("start") => overlay::Align::Start,
            Some("end") => overlay::Align::End,
            _ => overlay::Align::Center,
        };

        let children = &node.children;
        if children.len() < 2 {
            return text(format!("overlay requires 2 children (id={})", node.id)).into();
        }

        let anchor = ctx.render_child(&children[0]);
        let content = ctx.render_child(&children[1]);

        let pos = match position.as_str() {
            "above" => overlay::Position::Above,
            "left" => overlay::Position::Left,
            "right" => overlay::Position::Right,
            _ => overlay::Position::Below,
        };

        overlay::OverlayWrapper::new(anchor, content, pos, gap, offset_x, offset_y, flip, align)
            .into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(OverlayWidget)
    }
}
