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

use std::collections::HashMap;

use iced::widget::canvas;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::OutgoingEvent;
use crate::protocol::TreeNode;
use crate::render_ctx::RenderCtx;
use crate::widget::canvas as canvas_widgets;

/// Reusable canvas rendering engine.
///
/// Owns per-instance canvas state (layer tessellation caches, interactive
/// elements, pending focus) keyed by `(window_id, node_id)`. Provides
/// prepare, render, and message handling that PlushieWidget implementations
/// delegate to.
#[allow(clippy::type_complexity)]
pub struct CanvasEngine<R: PlushieRenderer> {
    /// Per-canvas, per-layer tessellation caches with content hashing.
    layer_caches: HashMap<(String, String), HashMap<String, (u64, canvas::Cache<R>)>>,
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
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        use crate::widget::canvas::canvas_layers_from_node;

        let key = (window_id.to_string(), node.id.clone());
        let layer_map = canvas_layers_from_node(node);

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
                Some((existing_hash, cache)) => {
                    if *existing_hash != hash {
                        cache.clear();
                        *existing_hash = hash;
                    }
                }
                None => {
                    node_caches.insert(layer_name.clone(), (hash, canvas::Cache::new()));
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
    /// events. Returns [`HandleResult::Fallthrough`] for all other
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
