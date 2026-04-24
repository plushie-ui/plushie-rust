//! Reusable canvas rendering engine for PlushieWidget composition.
//!
//! [`CanvasEngine`] provides the full canvas infrastructure (layer caching,
//! interactive elements, hit testing, focus management, drag tracking,
//! keyboard navigation) as a composable building block. Any PlushieWidget
//! can embed a CanvasEngine to get canvas-based rendering with all
//! interactive features, the same way Elixir widgets use the canvas DSL.
//!
//! # Example
//!
//! ```ignore
//! use plushie_widget_sdk::prelude::*;
//! use plushie_widget_sdk::canvas_engine::CanvasEngine;
//!
//! struct GaugeWidget<R: PlushieRenderer> {
//!     canvas: CanvasEngine<R>,
//! }
//!
//! impl<R: PlushieRenderer> PlushieWidget<R> for GaugeWidget<R> {
//!     fn type_names(&self) -> &[&str] { &["gauge"] }
//!
//!     fn prepare(&mut self, node: &TreeNode, window_id: &str, theme: &Theme) {
//!         self.canvas.prepare(node, window_id);
//!     }
//!
//!     fn render<'a>(&'a self, node: &'a TreeNode, ctx: &RenderCtx<'a, R>)
//!         -> Element<'a, Message, Theme, R>
//!     {
//!         self.canvas.render(node, ctx, None)
//!     }
//!
//!     fn handle_message(&mut self, msg: &Message) -> HandleResult {
//!         self.canvas.handle_message(msg)
//!     }
//!
//!     fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
//!         Box::new(GaugeWidget { canvas: CanvasEngine::new() })
//!     }
//! }
//! ```

use std::cell::Cell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use iced::widget::canvas;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::OutgoingEvent;
use crate::protocol::TreeNode;
use crate::render_ctx::RenderCtx;
use crate::widget::canvas as canvas_widgets;

pub(crate) type CanvasLayerCaches<R> = HashMap<String, CanvasLayerCache<R>>;

pub(crate) struct CanvasLayerCache<R: PlushieRenderer> {
    content_hash: u64,
    theme_hash: Cell<u64>,
    pub(crate) cache: canvas::Cache<R>,
}

impl<R: PlushieRenderer> CanvasLayerCache<R> {
    fn new(content_hash: u64) -> Self {
        Self {
            content_hash,
            theme_hash: Cell::new(0),
            cache: canvas::Cache::new(),
        }
    }

    fn update_content_hash(&mut self, content_hash: u64) {
        if self.content_hash != content_hash {
            self.cache.clear();
            self.content_hash = content_hash;
        }
    }

    pub(crate) fn ensure_theme_hash(&self, theme_hash: u64) {
        if self.theme_hash.get() != theme_hash {
            self.cache.clear();
            self.theme_hash.set(theme_hash);
        }
    }
}

pub(crate) fn canvas_theme_hash(theme: &Theme) -> u64 {
    fn hash_color(color: iced::Color, state: &mut DefaultHasher) {
        color.r.to_bits().hash(state);
        color.g.to_bits().hash(state);
        color.b.to_bits().hash(state);
        color.a.to_bits().hash(state);
    }

    let palette = theme.palette();
    let mut hasher = DefaultHasher::new();
    palette.is_dark.hash(&mut hasher);
    hash_color(palette.primary.base.color, &mut hasher);
    hash_color(palette.background.base.color, &mut hasher);
    hash_color(palette.background.base.text, &mut hasher);
    hash_color(palette.success.base.color, &mut hasher);
    hash_color(palette.danger.base.color, &mut hasher);
    hash_color(palette.warning.base.color, &mut hasher);
    hasher.finish()
}

/// Reusable canvas rendering engine.
///
/// Owns per-instance canvas state (layer tessellation caches, interactive
/// elements, pending focus) keyed by `(window_id, node_id)`. Provides
/// prepare, render, and message handling that PlushieWidget implementations
/// delegate to.
pub struct CanvasEngine<R: PlushieRenderer> {
    /// Per-canvas, per-layer tessellation caches with content hashing.
    layer_caches: HashMap<(String, String), CanvasLayerCaches<R>>,
    /// Pre-parsed interactive elements per (window_id, canvas_id).
    interactions: HashMap<(String, String), Vec<canvas_widgets::InteractiveElement>>,
    /// Pending programmatic focus per (window_id, canvas_id).
    pending_focus: HashMap<(String, String), String>,
}

impl<R: PlushieRenderer> CanvasEngine<R> {
    /// Create a new empty engine.
    pub fn new() -> Self {
        Self {
            layer_caches: HashMap::new(),
            interactions: HashMap::new(),
            pending_focus: HashMap::new(),
        }
    }

    /// Update layer caches and interactive elements from the tree node.
    ///
    /// Call this from your PlushieWidget::prepare() implementation.
    /// Parses interactive elements, validates a11y annotations, and
    /// manages per-layer tessellation cache invalidation using content
    /// hashing.
    pub fn prepare(&mut self, node: &TreeNode, window_id: &str) {
        use crate::widget::canvas::canvas_layers_from_node;

        let key = (window_id.to_string(), node.id.clone());
        let layer_map = canvas_layers_from_node(node);

        if crate::validate::is_validate_props_enabled() {
            for warning in canvas_widgets::validate_canvas_shape_tree(node) {
                log::warn!("[canvas {}] {}", node.id, warning);
            }
        }

        // Parse interactive elements from all layers.
        let mut interactive_elements = Vec::new();
        for (layer_name, shapes) in &layer_map {
            canvas_widgets::collect_interactive_elements(
                shapes,
                layer_name,
                canvas_widgets::TransformMatrix::identity(),
                None,
                None,
                "",
                &mut interactive_elements,
            );
        }

        let diags = canvas_widgets::validate_interactive_elements(&node.id, &interactive_elements);
        for diag in &diags {
            if let Some(msg) = diag
                .value
                .as_ref()
                .and_then(|d| d.get("message"))
                .and_then(|m| m.as_str())
            {
                log::warn!("[canvas {}] {}", node.id, msg);
            }
        }
        self.interactions.insert(key.clone(), interactive_elements);

        // Update or create per-layer tessellation caches.
        // Direct Hash impl on CanvasShape (f32 fields via to_bits) avoids
        // materialising a Debug string per layer per prepare.
        let node_caches = self.layer_caches.entry(key).or_default();
        for (layer_name, shapes) in &layer_map {
            let hash = {
                let mut hasher = DefaultHasher::new();
                shapes.hash(&mut hasher);
                hasher.finish()
            };
            match node_caches.get_mut(layer_name) {
                Some(record) => record.update_content_hash(hash),
                None => {
                    node_caches.insert(layer_name.clone(), CanvasLayerCache::new(hash));
                }
            }
        }
        node_caches.retain(|name, _| layer_map.contains_key(name));
    }

    /// Render the canvas node into an iced Element.
    ///
    /// Call this from your PlushieWidget::render() implementation.
    /// The `extra_pending_focus` parameter allows merging focus from
    /// external sources (e.g., SharedState for widget_ops compatibility).
    pub fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
        extra_pending_focus: Option<String>,
    ) -> Element<'a, Message, Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        let pending = self
            .pending_focus
            .get(&key)
            .cloned()
            .or(extra_pending_focus);
        canvas_widgets::render_canvas_with_state(
            node,
            *ctx,
            self.layer_caches.get(&key),
            self.interactions
                .get(&key)
                .map(|v| v.as_slice())
                .unwrap_or(&[]),
            pending,
        )
    }

    /// Process a canvas message.
    ///
    /// Handles CanvasElementFocusChanged by splitting into blur + focus
    /// events. Returns [`crate::registry::HandleResult::Fallthrough`] for all other
    /// message types so the registry's default message-to-event
    /// conversion runs.
    pub fn handle_message(&mut self, msg: &Message) -> crate::registry::HandleResult {
        use crate::registry::HandleResult;
        match msg {
            Message::CanvasElementFocusChanged {
                old_element_id,
                new_element_id,
                ..
            } => {
                let mut events = Vec::with_capacity(2);
                if let Some(old_id) = old_element_id {
                    events.push(OutgoingEvent::generic("blurred", old_id.clone(), None));
                }
                if let Some(new_id) = new_element_id {
                    events.push(OutgoingEvent::generic("focused", new_id.clone(), None));
                }
                HandleResult::emit(events)
            }
            _ => HandleResult::Fallthrough,
        }
    }

    /// Set pending programmatic focus for a canvas element.
    ///
    /// `element_id` is the element's full wire ID. The canvas is found by
    /// matching the element_id as a prefix of existing interaction keys.
    pub fn set_pending_focus(&mut self, element_id: &str) {
        // Find the interaction key whose canvas node_id is a prefix of the element_id.
        if let Some(key) = self
            .interactions
            .keys()
            .find(|(_, nid)| element_id.starts_with(nid.as_str()))
            .cloned()
        {
            self.pending_focus.insert(key, element_id.to_string());
        }
    }

    /// Remove all state for a canvas instance.
    #[allow(dead_code)]
    pub fn cleanup(&mut self, node_id: &str, window_id: &str) {
        let key = (window_id.to_string(), node_id.to_string());
        self.layer_caches.remove(&key);
        self.interactions.remove(&key);
        self.pending_focus.remove(&key);
    }

    /// Prune per-instance state for canvas nodes that have left the tree.
    ///
    /// Paired with [`crate::registry::PlushieWidget::cleanup_stale`].
    /// `live_ids` contains every `(window_id, node_id)` still present;
    /// drop entries whose keys aren't in the set.
    pub fn cleanup_stale(&mut self, live_ids: &std::collections::HashSet<(String, String)>) {
        self.layer_caches.retain(|k, _| live_ids.contains(k));
        self.interactions.retain(|k, _| live_ids.contains(k));
        self.pending_focus.retain(|k, _| live_ids.contains(k));
    }
}

impl<R: PlushieRenderer> Default for CanvasEngine<R> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_theme_hash_changes_with_palette_colors() {
        let light = iced::Theme::Light;
        let dark = iced::Theme::Dark;

        assert_ne!(
            light.palette().background.base.color,
            dark.palette().background.base.color
        );
        assert_ne!(canvas_theme_hash(&light), canvas_theme_hash(&dark));
    }

    #[test]
    fn layer_cache_tracks_theme_hash_separately_from_content_hash() {
        let cache = CanvasLayerCache::<iced::Renderer>::new(10);
        assert_eq!(cache.theme_hash.get(), 0);

        cache.ensure_theme_hash(20);
        assert_eq!(cache.theme_hash.get(), 20);

        cache.ensure_theme_hash(20);
        assert_eq!(cache.theme_hash.get(), 20);
    }
}
