use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::canvas_engine::CanvasEngine;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;

/// Thin wrapper over [`CanvasEngine`]. Delegates all canvas logic to the
/// reusable engine.
pub(crate) struct CanvasWidget<R: PlushieRenderer> {
    engine: CanvasEngine<R>,
}

impl<R: PlushieRenderer> CanvasWidget<R> {
    pub(crate) fn new() -> Self {
        Self {
            engine: CanvasEngine::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for CanvasWidget<R> {
    fn type_names(&self) -> &[&str] {
        &["canvas"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &Theme) {
        self.engine.prepare(node, window_id);
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        self.engine.render(node, ctx, None)
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        self.engine.handle_message(msg)
    }

    fn handle_widget_op(
        &mut self,
        node_id: &str,
        op: &str,
        _payload: &serde_json::Value,
    ) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        if op == "focus" {
            self.engine.set_pending_focus(node_id);
            Some(vec![])
        } else {
            None
        }
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        self.engine.cleanup(node_id, window_id);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(CanvasWidget::new())
    }
}
