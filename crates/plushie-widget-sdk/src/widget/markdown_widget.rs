use iced::{Element, Theme};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{Color, Length, PlushieType};

struct MarkdownProps {
    content: Option<String>,
    width: Option<Length>,
    link_color: Option<Color>,
    code_theme: Option<String>,
}

impl MarkdownProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            content: String::extract(p, "content"),
            width: Length::extract(p, "width"),
            link_color: Color::extract(p, "link_color"),
            code_theme: String::extract(p, "code_theme"),
        }
    }
}

/// Stateful markdown factory (owns parsed `markdown::Item` lists).
pub(crate) struct MarkdownWidget {
    /// Parsed markdown items per (window_id, node_id), with content hash
    /// for invalidation. Rebuilt when the "content" or "code_theme" prop changes.
    items: std::collections::HashMap<(String, String), (u64, Vec<iced::widget::markdown::Item>)>,
}

impl MarkdownWidget {
    pub(crate) fn new() -> Self {
        Self {
            items: std::collections::HashMap::new(),
        }
    }
}

fn markdown_cache_hash(content: &str, code_theme: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    code_theme.hash(&mut hasher);
    hasher.finish()
}

impl<R: PlushieRenderer> PlushieWidget<R> for MarkdownWidget {
    fn type_names(&self) -> &[&str] {
        &["markdown"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        let key = (window_id.to_string(), node.id.clone());
        let mp = MarkdownProps::from_node(node);
        let content_str = crate::shared_state::enforce_content_cap(
            &node.id,
            "content",
            mp.content.unwrap_or_default(),
            crate::shared_state::MAX_MARKDOWN_BYTES,
        );
        let code_theme_str = mp.code_theme.unwrap_or_default();
        let hash = markdown_cache_hash(&content_str, &code_theme_str);

        if let Some((existing_hash, _)) = self.items.get(&key)
            && *existing_hash == hash
        {
            return;
        }

        let code_theme = match code_theme_str.as_str() {
            "base16_mocha" => Some(iced::highlighter::Theme::Base16Mocha),
            "base16_ocean" => Some(iced::highlighter::Theme::Base16Ocean),
            "base16_eighties" => Some(iced::highlighter::Theme::Base16Eighties),
            "solarized_dark" => Some(iced::highlighter::Theme::SolarizedDark),
            "inspired_github" => Some(iced::highlighter::Theme::InspiredGitHub),
            "" => None,
            other => {
                log::warn!("unknown code_theme {:?}, using default", other);
                None
            }
        };
        let items: Vec<_> = if let Some(theme) = code_theme {
            let mut md = iced::widget::markdown::Content::new().code_theme(theme);
            md.push_str(&content_str);
            md.items().to_vec()
        } else {
            iced::widget::markdown::parse(&content_str).collect()
        };
        self.items.insert(key, (hash, items));
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        let items = match self.items.get(&key) {
            Some((_hash, items)) => items.as_slice(),
            None => {
                log::warn!("markdown cache miss for id={}", node.id);
                return iced::widget::text("(markdown: cache miss)").into();
            }
        };

        let mp = MarkdownProps::from_node(node);

        // text_size, h1_size, h2_size, h3_size, code_size, spacing: keep as
        // animated prop access (these support renderer-side transitions)
        let props = &node.props;
        let mut settings = if let Some(text_size) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "text_size")
                .or(ctx.default_text_size)
        {
            iced::widget::markdown::Settings::with_text_size(
                text_size,
                iced::widget::markdown::Style::from(ctx.theme),
            )
        } else {
            iced::widget::markdown::Settings::from(ctx.theme)
        };
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "h1_size")
        {
            settings.h1_size = iced::Pixels(v);
        }
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "h2_size")
        {
            settings.h2_size = iced::Pixels(v);
        }
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "h3_size")
        {
            settings.h3_size = iced::Pixels(v);
        }
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "code_size")
        {
            settings.code_size = iced::Pixels(v);
        }
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "spacing")
        {
            settings.spacing = iced::Pixels(v);
        }
        if let Some(ref lc) = mp.link_color {
            settings.style.link_color = iced_convert::color(lc);
        }

        let window_id = ctx.window_id.to_string();
        let node_id = node.id.clone();
        let mut md: Element<'a, Message, iced::Theme, R> =
            iced::widget::markdown::view(items, settings).map(move |uri| Message::Event {
                window_id: window_id.clone(),
                id: node_id.clone(),
                value: serde_json::json!({ "link": uri }),
                family: "link_click".into(),
            });

        if let Some(ref w) = mp.width {
            md = iced::widget::container(md)
                .width(iced_convert::length(w))
                .into();
        }

        md
    }

    fn prune_stale(&mut self, live_ids: &std::collections::HashSet<(String, String)>) {
        self.items.retain(|k, _| live_ids.contains(k));
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(MarkdownWidget::new())
    }
}
