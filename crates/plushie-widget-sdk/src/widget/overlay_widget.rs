use iced::widget::text;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;
use crate::widget::overlay;

use plushie_core::types::PlushieType;

struct OverlayProps {
    position: Option<String>,
    gap: Option<f32>,
    offset_x: Option<f32>,
    offset_y: Option<f32>,
    align: Option<String>,
}

impl OverlayProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            position: String::extract(p, "position"),
            gap: f32::extract(p, "gap"),
            offset_x: f32::extract(p, "offset_x"),
            offset_y: f32::extract(p, "offset_y"),
            align: String::extract(p, "align"),
        }
    }
}

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
        let op = OverlayProps::from_node(node);
        let props = &node.props;

        let gap = op.gap.unwrap_or(0.0);
        let offset_x = op.offset_x.unwrap_or(0.0);
        let offset_y = op.offset_y.unwrap_or(0.0);
        let flip = prop_bool_default(props, "flip", false);
        let align = match op.align.as_deref() {
            Some("start") => overlay::Align::Start,
            Some("end") => overlay::Align::End,
            Some("center") | None => overlay::Align::Center,
            Some(other) => {
                log::warn!(
                    "[id={}] overlay: unknown align {:?}, using center",
                    node.id,
                    other
                );
                overlay::Align::Center
            }
        };

        let children = &node.children;
        if children.len() < 2 {
            return text(format!("overlay requires 2 children (id={})", node.id)).into();
        }
        if children.len() > 2 {
            log::warn!(
                "[id={}] overlay: extra children beyond anchor and content are ignored",
                node.id
            );
        }

        let anchor = ctx.render_child(&children[0]);
        let content = ctx.render_child(&children[1]);

        let pos = match op.position.as_deref() {
            Some("above") => overlay::Position::Above,
            Some("left") => overlay::Position::Left,
            Some("right") => overlay::Position::Right,
            Some("below") | None => overlay::Position::Below,
            Some(other) => {
                log::warn!(
                    "[id={}] overlay: unknown position {:?}, using below",
                    node.id,
                    other
                );
                overlay::Position::Below
            }
        };

        overlay::OverlayWrapper::new(anchor, content, pos, gap, offset_x, offset_y, flip, align)
            .into()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(OverlayWidget)
    }
}
