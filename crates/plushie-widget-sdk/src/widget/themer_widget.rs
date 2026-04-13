use std::collections::HashMap;

use iced::widget::Space;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;

use plushie_core::types::PlushieType;

pub(crate) struct ThemerWidget {
    /// Resolved themes per (window_id, node_id). Populated during prepare,
    /// borrowed during render for child context theming.
    themes: HashMap<(String, String), Theme>,
}

impl ThemerWidget {
    pub(crate) fn new() -> Self {
        Self {
            themes: HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for ThemerWidget {
    fn type_names(&self) -> &[&str] {
        &["themer"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &Theme) {
        let key = (window_id.to_string(), node.id.clone());
        let theme_val = plushie_core::types::Theme::extract(&node.props, "theme");
        if let Some(ref t) = theme_val {
            let wire = serde_json::Value::from(t.wire_encode());
            if let Some(resolved) = crate::theming::resolve_theme_only(&wire) {
                self.themes.insert(key, resolved);
            } else {
                self.themes.remove(&key);
            }
        } else {
            self.themes.remove(&key);
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        // Render from factory-owned state. prepare() populates self.themes
        // via the registry prepare_walk (wired into App::apply and headless).
        let key = (ctx.window_id.to_string(), node.id.clone());
        let cached_theme = self.themes.get(&key);
        let child_theme = cached_theme.unwrap_or(ctx.theme);
        let child_ctx = ctx.with_theme(child_theme);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| child_ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let themer_theme = cached_theme.cloned();
        iced::widget::Themer::new(themer_theme, child).into()
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        let key = (window_id.to_string(), node_id.to_string());
        self.themes.remove(&key);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ThemerWidget::new())
    }
}
