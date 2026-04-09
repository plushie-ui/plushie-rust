//! Unified widget dispatch: [`PlushieWidget`] trait, [`WidgetRegistry`], [`WidgetSet`].
//!
//! Every widget type in plushie (built-in iced wrappers, custom widgets,
//! third-party widgets) implements [`PlushieWidget`] and registers in a
//! [`WidgetRegistry`]. The registry dispatches render, prepare, and message
//! handling uniformly. There is no distinction between built-in and custom
//! widgets.
//!
//! # Widget sets
//!
//! A [`WidgetSet`] is a named collection of widgets. The default "iced" set
//! provides all 36 built-in widget implementations. Third-party sets can
//! add new type names or override existing ones (last-registered wins).
//!
//! # Example
//!
//! ```ignore
//! use plushie_ext::prelude::*;
//!
//! struct Gauge;
//!
//! impl<R: PlushieRenderer> PlushieWidget<R> for Gauge {
//!     fn type_names(&self) -> &[&str] { &["gauge"] }
//!     fn render<'a>(&self, node: &'a TreeNode, ctx: &RenderCtx<'a, R>)
//!         -> Element<'a, Message, Theme, R> { todo!() }
//!     fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
//!         Box::new(Gauge)
//!     }
//! }
//! ```

use std::collections::HashMap;

use iced::{Element, Theme};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::extensions::{InitCtx, RenderCtx};
use crate::message::Message;
use crate::protocol::{OutgoingEvent, TreeNode};
use crate::widgets::a11y::A11yOverrides;

// ---------------------------------------------------------------------------
// PlushieWidget trait
// ---------------------------------------------------------------------------

/// The core trait for all widget type implementations, built-in and custom.
///
/// A `PlushieWidget` handles one or more widget type names (e.g., `["button"]`)
/// and provides rendering, state management, and message handling for nodes
/// of that type.
///
/// Stateless widgets (Button, Text, Space) are zero-sized structs.
/// Stateful widgets (TextEditor, PaneGrid, Canvas) own per-instance state
/// keyed by `(window_id, node_id)` to handle duplicate scoped IDs across
/// windows.
///
/// No `Send` or `Sync` bound: the registry and its widgets are
/// always accessed from a single thread. Multiplexed sessions create
/// their own registries via [`WidgetSet::create_widgets`] rather than
/// cloning across threads.
pub trait PlushieWidget<R: PlushieRenderer> {
    /// Widget type name(s) this implementation handles.
    ///
    /// Most widgets handle a single type (e.g., `["button"]`).
    /// Some handle aliases (e.g., `["rich_text", "rich"]`).
    fn type_names(&self) -> &[&str];

    /// Unique namespace for config routing from Settings.
    ///
    /// When the host sends a Settings message with extension config,
    /// the registry delivers the config slice matching this namespace
    /// to [`init`](Self::init). Empty string means no config.
    fn namespace(&self) -> &str {
        ""
    }

    /// Render a tree node to an iced Element.
    ///
    /// Called during the immutable view phase. The widget reads its
    /// per-instance state from `&self` and shared state from `ctx`.
    ///
    /// The `'a` lifetime binds `&self`, `node`, and `ctx` together so
    /// stateful widgets can return Elements that borrow from their own
    /// fields (e.g., a cached `Theme` or `text_editor::Content`).
    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R>;

    /// Update per-instance state for a node during the mutable phase.
    ///
    /// Called once per tree change for each node matching this widget's
    /// type names. `window_id` identifies which window the node belongs
    /// to. Stateful widgets should key per-instance state by
    /// `(window_id, node_id)`.
    fn prepare(&mut self, _node: &TreeNode, _window_id: &str, _theme: &Theme) {}

    /// Handle a message produced by this widget type.
    ///
    /// Return `Some(events)` to emit custom outgoing events.
    /// Return `None` to use the default message-to-event conversion
    /// (which handles Click, Input, Toggle, Select, etc.).
    ///
    /// Only override this for widgets that need stateful message
    /// processing (TextEditor, PaneGrid, Slider).
    fn handle_message(&mut self, _msg: &Message) -> Option<Vec<OutgoingEvent>> {
        None
    }

    /// Clean up per-instance state when a node leaves the tree.
    fn cleanup(&mut self, _node_id: &str, _window_id: &str) {}

    /// Settings message arrived. Receive per-namespace config.
    fn init(&mut self, _ctx: &InitCtx<'_>) {}

    /// A11y auto-inference for this widget type.
    ///
    /// Called when the node has no explicit `a11y` prop. Return
    /// `Some(overrides)` to inject accessibility annotations
    /// (e.g., using placeholder text as a description).
    fn infer_a11y(&self, _node: &TreeNode) -> Option<A11yOverrides> {
        None
    }

    /// Handle a widget operation (focus, scroll) targeting this widget
    /// or a descendant ID (via prefix-based routing).
    ///
    /// The `node_id` is the full original ID from the operation.
    /// For canvas element focus, this would be "canvas_id/element_id".
    fn handle_widget_op(
        &mut self,
        _node_id: &str,
        _op: &str,
        _payload: &Value,
    ) -> Option<Vec<OutgoingEvent>> {
        None
    }

    /// Create a clone of this widget for multiplexed sessions.
    ///
    /// Each session gets its own widget instance with independent
    /// per-instance state. Stateless widgets can return a fresh
    /// default instance.
    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>>;
}

// ---------------------------------------------------------------------------
// WidgetSet trait
// ---------------------------------------------------------------------------

/// A named collection of [`PlushieWidget`] implementations.
///
/// Widget sets group related widgets (e.g., the "iced" set provides all
/// 36 built-in widgets). Multiple sets can be registered; for type name
/// collisions, the last-registered set wins.
pub trait WidgetSet<R: PlushieRenderer> {
    /// Human-readable name for this set (e.g., "iced", "material").
    ///
    /// Used for logging and introspection (e.g., hello message reports
    /// which set provides each widget type).
    fn name(&self) -> &str;

    /// Create all widget instances for this set.
    fn create_widgets(&self) -> Vec<Box<dyn PlushieWidget<R>>>;
}

// ---------------------------------------------------------------------------
// WidgetRegistry
// ---------------------------------------------------------------------------

/// Central registry mapping widget type names to [`PlushieWidget`] instances.
///
/// The registry owns the full widget lifecycle: prepare (mutable phase),
/// render (immutable phase), message routing, and cleanup.
///
/// # Scoped ID routing
///
/// Messages carry hierarchical IDs separated by `/` (e.g., "form/save",
/// "canvas/element"). The registry routes by exact match first, then
/// walks progressively shorter prefixes until a match is found. This
/// handles both container-scoped IDs and internal composition (e.g.,
/// a gauge widget that internally renders a canvas).
pub struct WidgetRegistry<R: PlushieRenderer = iced::Renderer> {
    /// All registered widget implementations.
    impls: Vec<Box<dyn PlushieWidget<R>>>,

    /// Type name -> index into `impls`.
    type_index: HashMap<String, usize>,

    /// Node ID -> index into `impls`. Populated during prepare_walk.
    node_factory_map: HashMap<String, usize>,

    /// Type name -> set name (for introspection/logging).
    provenance: HashMap<String, String>,
}

impl<R: PlushieRenderer> WidgetRegistry<R> {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            impls: Vec::new(),
            type_index: HashMap::new(),
            node_factory_map: HashMap::new(),
            provenance: HashMap::new(),
        }
    }

    /// Register a single widget. If the type name is already registered,
    /// the new widget replaces it (last-registered wins).
    pub fn register(&mut self, widget: Box<dyn PlushieWidget<R>>) {
        self.register_with_set_name(widget, "");
    }

    /// Register a widget set. All widgets in the set are registered
    /// with the set's name as provenance.
    pub fn register_set(&mut self, set: &dyn WidgetSet<R>) {
        let set_name = set.name().to_string();
        for widget in set.create_widgets() {
            self.register_with_set_name(widget, &set_name);
        }
    }

    fn register_with_set_name(&mut self, widget: Box<dyn PlushieWidget<R>>, set_name: &str) {
        let idx = self.impls.len();
        for &name in widget.type_names() {
            if self.type_index.contains_key(name) {
                let old_provenance = self.provenance.get(name).map(|s| s.as_str()).unwrap_or("");
                let new_provenance = if set_name.is_empty() {
                    "(individual)"
                } else {
                    set_name
                };
                log::info!(
                    "widget type {:?} overridden: {:?} -> {:?}",
                    name,
                    old_provenance,
                    new_provenance,
                );
            }
            self.type_index.insert(name.to_string(), idx);
            if !set_name.is_empty() {
                self.provenance
                    .insert(name.to_string(), set_name.to_string());
            }
        }
        self.impls.push(widget);
    }

    /// Look up the widget implementation for a type name.
    pub fn get_for_type(&self, type_name: &str) -> Option<&dyn PlushieWidget<R>> {
        self.type_index
            .get(type_name)
            .map(|&idx| self.impls[idx].as_ref())
    }

    /// Look up the widget implementation for a node ID, using
    /// prefix-based scoped ID routing.
    ///
    /// Tries exact match first, then walks `/`-separated prefixes:
    /// "a/b/c" -> "a/b" -> "a"
    pub fn get_for_node_id<'a>(&self, node_id: &'a str) -> Option<(usize, &'a str)> {
        // Exact match (most common case)
        if let Some(&idx) = self.node_factory_map.get(node_id) {
            return Some((idx, node_id));
        }
        // Prefix walk
        let mut id = node_id;
        while let Some(slash_pos) = id.rfind('/') {
            id = &id[..slash_pos];
            if let Some(&idx) = self.node_factory_map.get(id) {
                return Some((idx, id));
            }
        }
        None
    }

    /// Return all registered type names.
    pub fn type_names(&self) -> Vec<&str> {
        self.type_index.keys().map(|s| s.as_str()).collect()
    }

    /// Return type names grouped by set/provenance.
    pub fn type_names_by_set(&self) -> HashMap<&str, Vec<&str>> {
        let mut result: HashMap<&str, Vec<&str>> = HashMap::new();
        for type_name in self.type_index.keys() {
            let set_name = self
                .provenance
                .get(type_name)
                .map(|s| s.as_str())
                .unwrap_or("(none)");
            result.entry(set_name).or_default().push(type_name.as_str());
        }
        result
    }

    /// Render a tree node by dispatching to the registered factory.
    ///
    /// Third-party factories (from non-"iced" sets) are wrapped in
    /// `catch_unwind` for panic isolation. A panic produces a red error
    /// placeholder instead of crashing the renderer.
    pub fn render_node<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &crate::extensions::RenderCtx<'a, R>,
    ) -> iced::Element<'a, crate::message::Message, iced::Theme, R> {
        let type_name = node.type_name.as_str();
        let Some(&idx) = self.type_index.get(type_name) else {
            log::warn!(
                "[id={}] unknown node type `{}`, rendering as empty container",
                node.id,
                type_name
            );
            return iced::widget::container(iced::widget::Space::new()).into();
        };

        let is_trusted = self.provenance.get(type_name).is_some_and(|s| s == "iced");

        if is_trusted || !crate::extensions::catch_unwind_enabled() {
            self.impls[idx].render(node, ctx)
        } else {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.impls[idx].render(node, ctx)
            })) {
                Ok(element) => element,
                Err(_) => {
                    log::error!("[id={}] widget `{}` panicked in render", node.id, type_name,);
                    iced::widget::text(format!("Widget error: `{}`", type_name))
                        .color(iced::Color::from_rgb(1.0, 0.0, 0.0))
                        .into()
                }
            }
        }
    }

    /// Whether a type name is registered.
    pub fn handles_type(&self, type_name: &str) -> bool {
        self.type_index.contains_key(type_name)
    }

    /// Clone all widget instances for a new multiplexed session.
    pub fn clone_for_session(&self) -> Self {
        let mut cloned_impls: Vec<Box<dyn PlushieWidget<R>>> = Vec::with_capacity(self.impls.len());
        let mut new_type_index = HashMap::new();

        for (i, widget) in self.impls.iter().enumerate() {
            let cloned = widget.clone_for_session();
            let new_idx = cloned_impls.len();
            cloned_impls.push(cloned);

            // Re-map type_index entries that pointed to old index i
            for (type_name, &old_idx) in &self.type_index {
                if old_idx == i {
                    new_type_index.insert(type_name.clone(), new_idx);
                }
            }
        }

        Self {
            impls: cloned_impls,
            type_index: new_type_index,
            node_factory_map: HashMap::new(),
            provenance: self.provenance.clone(),
        }
    }

    /// Broadcast init to all widgets with matching namespace config.
    pub fn init_all(&mut self, ctx: &InitCtx<'_>) {
        for widget in &mut self.impls {
            let ns = widget.namespace();
            if !ns.is_empty() {
                // Build per-namespace InitCtx with config slice
                let ns_config = ctx
                    .config
                    .as_object()
                    .and_then(|obj| obj.get(ns))
                    .cloned()
                    .unwrap_or(Value::Null);
                let ns_ctx = InitCtx {
                    config: &ns_config,
                    theme: ctx.theme,
                    default_text_size: ctx.default_text_size,
                    default_font: ctx.default_font,
                };
                widget.init(&ns_ctx);
            }
        }
    }

    /// Walk the tree depth-first, calling `prepare()` on each factory
    /// for matching nodes and populating cross-cutting shared state
    /// (style overrides). Also populates `node_factory_map` for message
    /// routing and prunes stale shared state entries.
    pub fn prepare_walk(
        &mut self,
        root: &TreeNode,
        shared: &mut crate::widgets::SharedState,
        theme: &Theme,
    ) {
        self.node_factory_map.clear();
        let mut live_ids = std::collections::HashSet::new();
        self.prepare_walk_inner(root, "", shared, theme, &mut live_ids);
        shared.prune_shared(&live_ids);
    }

    fn prepare_walk_inner(
        &mut self,
        node: &TreeNode,
        window_id: &str,
        shared: &mut crate::widgets::SharedState,
        theme: &Theme,
        live_ids: &mut std::collections::HashSet<String>,
    ) {
        live_ids.insert(node.id.clone());

        // Track which window we're in.
        let current_window_id = if node.type_name == "window" {
            node.id.as_str()
        } else {
            window_id
        };

        // Cross-cutting: populate style overrides for any node with
        // a style prop. This replaces the ensure_caches_walk.
        crate::widgets::caches::ensure_style_overrides_cache(node, shared);

        // Factory-specific prepare.
        if let Some(&idx) = self.type_index.get(node.type_name.as_str()) {
            self.node_factory_map.insert(node.id.clone(), idx);
            self.impls[idx].prepare(node, current_window_id, theme);
        }

        for child in &node.children {
            self.prepare_walk_inner(child, current_window_id, shared, theme, live_ids);
        }
    }

    /// Route a widget operation (extension command) to the factory
    /// that owns the target node ID.
    pub fn handle_widget_op(
        &mut self,
        node_id: &str,
        op: &str,
        payload: &Value,
    ) -> Option<Vec<OutgoingEvent>> {
        let (idx, _) = self.get_for_node_id(node_id)?;
        self.impls[idx].handle_widget_op(node_id, op, payload)
    }

    /// Clear the node-to-factory mapping. Called before a full prepare walk.
    pub fn clear_node_map(&mut self) {
        self.node_factory_map.clear();
    }

    /// Register a node ID -> factory mapping. Called during prepare walk.
    pub fn map_node(&mut self, node_id: String, factory_idx: usize) {
        self.node_factory_map.insert(node_id, factory_idx);
    }

    /// Access a widget by index (mutable). Used during prepare walk.
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut Box<dyn PlushieWidget<R>>> {
        self.impls.get_mut(idx)
    }

    /// Get the factory index for a type name.
    pub fn index_for_type(&self, type_name: &str) -> Option<usize> {
        self.type_index.get(type_name).copied()
    }

    /// Number of registered widgets.
    pub fn len(&self) -> usize {
        self.impls.len()
    }

    /// Whether any widgets are registered.
    pub fn is_empty(&self) -> bool {
        self.impls.is_empty()
    }
}

impl<R: PlushieRenderer> Default for WidgetRegistry<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: PlushieRenderer> std::fmt::Debug for WidgetRegistry<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WidgetRegistry")
            .field("widgets", &self.impls.len())
            .field("type_names", &self.type_index.len())
            .field("node_mappings", &self.node_factory_map.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct TestWidget {
        names: Vec<&'static str>,
    }

    impl TestWidget {
        fn new(names: &[&'static str]) -> Self {
            Self {
                names: names.to_vec(),
            }
        }
    }

    impl PlushieWidget<()> for TestWidget {
        fn type_names(&self) -> &[&str] {
            &self.names
        }

        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("test").into()
        }

        fn clone_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            Box::new(TestWidget::new(&self.names))
        }
    }

    struct TestSet;

    impl WidgetSet<()> for TestSet {
        fn name(&self) -> &str {
            "test"
        }

        fn create_widgets(&self) -> Vec<Box<dyn PlushieWidget<()>>> {
            vec![
                Box::new(TestWidget::new(&["alpha"])),
                Box::new(TestWidget::new(&["beta"])),
            ]
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(TestWidget::new(&["button"])));
        assert!(registry.handles_type("button"));
        assert!(!registry.handles_type("text"));
    }

    #[test]
    fn register_set() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register_set(&TestSet);
        assert!(registry.handles_type("alpha"));
        assert!(registry.handles_type("beta"));
    }

    #[test]
    fn last_registered_wins() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(TestWidget::new(&["button"])));
        let first_idx = registry.index_for_type("button").unwrap();

        registry.register(Box::new(TestWidget::new(&["button"])));
        let second_idx = registry.index_for_type("button").unwrap();

        assert_ne!(first_idx, second_idx);
    }

    #[test]
    fn scoped_id_routing_exact_match() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(TestWidget::new(&["button"])));
        let idx = registry.index_for_type("button").unwrap();
        registry.map_node("form/save".into(), idx);

        let (found_idx, matched_id) = registry.get_for_node_id("form/save").unwrap();
        assert_eq!(found_idx, idx);
        assert_eq!(matched_id, "form/save");
    }

    #[test]
    fn scoped_id_routing_prefix_walk() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(TestWidget::new(&["gauge"])));
        let idx = registry.index_for_type("gauge").unwrap();
        registry.map_node("gauge-1".into(), idx);

        // "gauge-1/canvas/element" should walk to "gauge-1"
        let (found_idx, matched_id) = registry.get_for_node_id("gauge-1/canvas/element").unwrap();
        assert_eq!(found_idx, idx);
        assert_eq!(matched_id, "gauge-1");
    }

    #[test]
    fn scoped_id_routing_no_match() {
        let registry = WidgetRegistry::<()>::new();
        assert!(registry.get_for_node_id("nonexistent/id").is_none());
    }

    #[test]
    fn clone_for_session_preserves_type_index() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register_set(&TestSet);

        let cloned = registry.clone_for_session();
        assert!(cloned.handles_type("alpha"));
        assert!(cloned.handles_type("beta"));
        assert_eq!(cloned.len(), registry.len());
    }

    #[test]
    fn type_names_by_set_groups_correctly() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register_set(&TestSet);
        registry.register(Box::new(TestWidget::new(&["custom"])));

        let by_set = registry.type_names_by_set();
        assert!(by_set.get("test").unwrap().contains(&"alpha"));
        assert!(by_set.get("test").unwrap().contains(&"beta"));
        assert!(by_set.get("(none)").unwrap().contains(&"custom"));
    }
}
