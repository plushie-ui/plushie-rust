use std::collections::HashMap;

use iced::widget::Space;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::theming::ThemeChrome;

use plushie_core::types::PlushieType;

pub(crate) struct ThemerWidget {
    /// Resolved themes per (window_id, node_id). Populated during prepare,
    /// borrowed during render for child context theming.
    themes: HashMap<(String, String), Theme>,
    chromes: HashMap<(String, String), ThemeChrome>,
}

impl ThemerWidget {
    pub(crate) fn new() -> Self {
        Self {
            themes: HashMap::new(),
            chromes: HashMap::new(),
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
            match crate::theming::resolve_theme_resolution(&wire) {
                crate::theming::ThemeResolution::Theme(resolved, chrome) => {
                    self.themes.insert(key.clone(), resolved);
                    self.chromes.insert(key, chrome);
                }
                crate::theming::ThemeResolution::System
                | crate::theming::ThemeResolution::Invalid => {
                    self.themes.remove(&key);
                    self.chromes.remove(&key);
                }
            }
        } else {
            self.themes.remove(&key);
            self.chromes.remove(&key);
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
        let child_chrome = self.chromes.get(&key).copied().unwrap_or(ctx.theme_chrome);
        let child_ctx = ctx.with_theme_and_chrome(child_theme, child_chrome);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| child_ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let themer_theme = cached_theme.cloned();
        iced::widget::Themer::new(themer_theme, child).into()
    }

    fn prune_stale(&mut self, live_ids: &std::collections::HashSet<(String, String)>) {
        self.themes.retain(|k, _| live_ids.contains(k));
        self.chromes.retain(|k, _| live_ids.contains(k));
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ThemerWidget::new())
    }
}
