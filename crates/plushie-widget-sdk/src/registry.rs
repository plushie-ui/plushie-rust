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
//! use plushie_widget_sdk::prelude::*;
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
use crate::a11y::A11yOverrides;
use crate::message::Message;
use crate::protocol::{OutgoingEvent, TreeNode};
use crate::render_ctx::RenderCtx;

// ---------------------------------------------------------------------------
// InitCtx
// ---------------------------------------------------------------------------

/// Context passed to [`PlushieWidget::init`].
///
/// Provides the widget's config (from the host's Settings message)
/// along with the current theme and text rendering defaults. This
/// allows widgets to do theme-dependent initialization without
/// deferring to the first `prepare()` call.
#[derive(Debug)]
pub struct InitCtx<'a> {
    /// Widget-specific config from `Settings.widget_config[namespace]`.
    /// `Value::Null` if the host didn't provide config for this widget.
    pub config: &'a Value,
    /// The current theme at init time.
    pub theme: &'a Theme,
    /// Global default text size, if set by the host.
    pub default_text_size: Option<f32>,
    /// Global default font, if set by the host.
    pub default_font: Option<iced::Font>,
}

// ---------------------------------------------------------------------------
// GenerationCounter
// ---------------------------------------------------------------------------

/// A monotonically increasing counter for tracking data changes.
///
/// Useful for cache invalidation in widgets that use `canvas::Cache`.
/// Call `bump()` when data changes (in `prepare` or `handle_widget_op`).
/// In your `canvas::Program` implementation, compare the generation
/// against a saved value in your `Program::State` to decide whether
/// to clear and redraw the cache.
///
/// # Example
///
/// ```ignore
/// struct MyState {
///     generation: u64,
///     cache: canvas::Cache,
/// }
///
/// impl canvas::Program<Message> for MyProgram {
///     type State = MyState;
///
///     fn update(&self, state: &mut MyState, ...) -> Option<Action<Message>> {
///         if state.generation != self.current_generation {
///             state.cache.clear();
///             state.generation = self.current_generation;
///         }
///         None
///     }
///
///     fn draw(&self, state: &MyState, ...) -> Vec<Geometry> {
///         vec![state.cache.draw(renderer, bounds.size(), |frame| { ... })]
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct GenerationCounter {
    value: u64,
}

impl GenerationCounter {
    /// Create a new counter starting at zero.
    pub fn new() -> Self {
        Self { value: 0 }
    }

    /// Return the current generation value.
    pub fn get(&self) -> u64 {
        self.value
    }

    /// Increment the generation. Wraps on overflow (u64, effectively never).
    pub fn bump(&mut self) {
        self.value = self.value.wrapping_add(1);
    }
}

impl Default for GenerationCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if panic isolation is disabled via the PLUSHIE_NO_CATCH_UNWIND env var.
/// When true, widget panics propagate normally, preserving stack traces for
/// debugging. Only use during development. In production, catch_unwind
/// prevents one widget from crashing the entire renderer.
fn catch_unwind_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::env::var("PLUSHIE_NO_CATCH_UNWIND").is_err()
        }
        #[cfg(target_arch = "wasm32")]
        {
            true
        }
    })
}

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
    /// When the host sends a Settings message with widget config,
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

    /// Prune per-instance state for nodes no longer in the tree.
    ///
    /// `live_ids` contains every `(window_id, node_id)` pair present
    /// in the current tree. Stateful widgets that key per-instance
    /// state by `(window_id, node_id)` should drop entries whose keys
    /// are not in the set. Stateless widgets can ignore this.
    ///
    /// Called by [`WidgetRegistry::prepare_walk`] after
    /// [`SharedState::prune_shared`], so implementations only need to
    /// worry about their own keyed state. Canonical one-liner:
    ///
    /// ```ignore
    /// self.contents.retain(|k, _| live_ids.contains(k));
    /// ```
    fn cleanup_stale(&mut self, _live_ids: &std::collections::HashSet<(String, String)>) {}

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
    /// For canvas element focus, this is the element's full wire ID.
    fn handle_widget_op(
        &mut self,
        _node_id: &str,
        _op: &str,
        _payload: &Value,
    ) -> Option<Vec<OutgoingEvent>> {
        None
    }

    /// Event payload specs for this widget type.
    ///
    /// Used for runtime validation of emitted event payloads.
    /// Override to declare the expected shape of each event family.
    fn event_specs(&self) -> Vec<plushie_core::EventSpec> {
        vec![]
    }

    /// Command payload specs for this widget type.
    ///
    /// Used for runtime validation of incoming command payloads.
    /// Override to declare the expected shape of each command family.
    fn command_specs(&self) -> Vec<plushie_core::CommandSpec> {
        vec![]
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

/// Shared mutable context threaded through [`WidgetRegistry::prepare_walk_inner`].
///
/// Groups the accumulator state (live ID sets, shared state, theme)
/// so the recursion carries a single context argument instead of a
/// long parameter list.
struct PrepareWalkCtx<'a> {
    shared: &'a mut crate::shared_state::SharedState,
    theme: &'a Theme,
    live_ids: std::collections::HashSet<String>,
    live_keys: std::collections::HashSet<(String, String)>,
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
        ctx: &crate::render_ctx::RenderCtx<'a, R>,
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

        if is_trusted || !catch_unwind_enabled() {
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

    /// Walk the tree depth-first. This is the main mutable phase that
    /// runs after every tree change (snapshot or patch). It:
    ///
    /// 1. Tracks the current `window_id` as it descends through window
    ///    nodes, passing it to each factory's `prepare()`.
    /// 2. Calls `prepare()` on the owning factory for each node.
    /// 3. Populates `node_factory_map` for message and widget-op routing.
    /// 4. Computes style override caches for nodes with a `style` prop.
    /// 5. Prunes stale `SharedState` entries for nodes no longer in the
    ///    tree (prevents unbounded memory growth across tree updates).
    /// 6. Calls `cleanup_stale` on every registered widget so
    ///    factory-owned per-instance state stays in sync with the
    ///    live tree.
    pub fn prepare_walk(
        &mut self,
        root: &TreeNode,
        shared: &mut crate::shared_state::SharedState,
        theme: &Theme,
    ) {
        self.node_factory_map.clear();
        let mut ctx = PrepareWalkCtx {
            shared,
            theme,
            live_ids: std::collections::HashSet::new(),
            live_keys: std::collections::HashSet::new(),
        };
        self.prepare_walk_inner(root, "", &mut ctx, 0);
        ctx.shared.prune_shared(&ctx.live_ids);

        // Dispatch cleanup_stale to every registered widget. This
        // shrinks factory-owned per-instance state keyed by
        // (window_id, node_id) when nodes leave the tree.
        let live_keys = ctx.live_keys;
        for widget in &mut self.impls {
            widget.cleanup_stale(&live_keys);
        }
    }

    fn prepare_walk_inner(
        &mut self,
        node: &TreeNode,
        window_id: &str,
        ctx: &mut PrepareWalkCtx<'_>,
        depth: usize,
    ) {
        // Guard against pathological tree depth. Normalize and render
        // walk also cap at MAX_TREE_DEPTH; matching the cap here keeps
        // all three passes consistent and avoids stack overflow on
        // hostile inputs. The overflow path stops recursing but still
        // records the node itself in live_ids/live_keys so state that
        // belongs to the current node isn't incorrectly evicted.
        if depth > crate::shared_state::MAX_TREE_DEPTH {
            log::error!(
                "[code=tree_too_deep][id={}] prepare_walk depth exceeds {}, truncating subtree",
                node.id,
                crate::shared_state::MAX_TREE_DEPTH
            );
            ctx.live_ids.insert(node.id.clone());
            ctx.live_keys
                .insert((window_id.to_string(), node.id.clone()));
            return;
        }

        ctx.live_ids.insert(node.id.clone());
        ctx.live_keys
            .insert((window_id.to_string(), node.id.clone()));

        // Track which window we're in.
        let current_window_id = if node.type_name == "window" {
            node.id.as_str()
        } else {
            window_id
        };

        // Cross-cutting: populate style overrides for any node with
        // a style prop. Populated for all nodes during prepare_walk.
        crate::shared_state::ensure_style_overrides_cache(node, ctx.shared);

        // Factory-specific prepare.
        if let Some(&idx) = self.type_index.get(node.type_name.as_str()) {
            self.node_factory_map.insert(node.id.clone(), idx);
            self.impls[idx].prepare(node, current_window_id, ctx.theme);
        }

        for child in &node.children {
            self.prepare_walk_inner(child, current_window_id, ctx, depth + 1);
        }
    }

    /// Convert an iced [`Message`] into outgoing protocol events.
    ///
    /// Tries factory dispatch first (registry-aware widgets like sliders,
    /// text editors, pane grids handle their own messages). Falls back to
    /// [`Message::to_outgoing_event`] for simple widget events, and
    /// passes through widget events as generic outgoing events.
    ///
    /// Returns an empty vec for messages that don't produce outgoing
    /// events (subscription events, `NoOp`, `MarkdownUrl`, etc.).
    pub fn process_message(&mut self, msg: &Message) -> Vec<OutgoingEvent> {
        // Try factory dispatch first. If the factory handles the message
        // (returns Some), use that result. Otherwise fall through to the
        // default conversion below.
        if let Some(node_id) = msg.node_id()
            && let Some((idx, _)) = self.get_for_node_id(node_id)
            && let Some(factory) = self.get_mut(idx)
            && let Some(events) = factory.handle_message(msg)
        {
            return events;
        }

        match msg {
            // Diagnostic: stateless conversion.
            Message::Diagnostic { .. } => msg.to_outgoing_event().into_iter().collect(),

            // CanvasElementFocusChanged is handled by CanvasWidget::handle_message
            // (splits into blur + focus events). Fallback returns empty.
            Message::CanvasElementFocusChanged { .. } => vec![],

            // TextEditorAction is handled by its PlushieWidget factory via
            // registry dispatch. This arm is fallback for edge cases where
            // the registry has no mapping.
            Message::TextEditorAction(..) => vec![],

            // Unified widget events: if the registry's handle_message
            // (above) didn't match, pass through as a generic outgoing
            // event.
            Message::Event {
                id, value, family, ..
            } => {
                self.validate_event_payload(id, family, value);
                let value_opt = if value.is_null() {
                    None
                } else {
                    Some(value.clone())
                };
                vec![OutgoingEvent::generic(
                    family.clone(),
                    id.clone(),
                    value_opt,
                )]
            }

            // Pane grid events are handled by PaneGridWidget via registry
            // dispatch. Fallback returns empty.
            Message::PaneFocusCycle(..)
            | Message::PaneResized(..)
            | Message::PaneDragged(..)
            | Message::PaneClicked(..) => vec![],

            // Everything else (subscription events, NoOp, MarkdownUrl,
            // etc.) produces no outgoing events.
            _ => vec![],
        }
    }

    /// Route a widget command to the factory
    /// that owns the target node ID.
    pub fn handle_widget_op(
        &mut self,
        node_id: &str,
        op: &str,
        payload: &Value,
    ) -> Option<Vec<OutgoingEvent>> {
        let (idx, _) = self.get_for_node_id(node_id)?;
        self.validate_command_payload(idx, node_id, op, payload);
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

    // -- Spec validation (debug: panic, release: log warning) ----------------

    /// Validate an event payload against the widget's declared event specs.
    fn validate_event_payload(&self, node_id: &str, family: &str, value: &Value) {
        let Some((idx, _)) = self.get_for_node_id(node_id) else {
            return;
        };
        let specs = self.impls[idx].event_specs();
        if specs.is_empty() {
            return;
        }
        let Some(spec) = specs.iter().find(|s| s.family == family) else {
            return; // unknown family: widget may emit events not in its spec list
        };
        if !spec.payload.validate(value) {
            let msg = format!(
                "event spec mismatch: widget {node_id} emitted \
                 family={family:?} with value that doesn't match spec {:?}",
                spec.payload
            );
            debug_assert!(false, "{msg}");
            #[cfg(not(debug_assertions))]
            log::warn!("{msg}");
        }
    }

    /// Validate a command payload against the widget's declared command specs.
    fn validate_command_payload(
        &self,
        factory_idx: usize,
        node_id: &str,
        family: &str,
        value: &Value,
    ) {
        let specs = self.impls[factory_idx].command_specs();
        if specs.is_empty() {
            return;
        }
        let Some(spec) = specs.iter().find(|s| s.family == family) else {
            return; // unknown family: widget may accept commands not in its spec list
        };
        if !spec.payload.validate(value) {
            let msg = format!(
                "command spec mismatch: widget {node_id} received \
                 family={family:?} with value that doesn't match spec {:?}",
                spec.payload
            );
            debug_assert!(false, "{msg}");
            #[cfg(not(debug_assertions))]
            log::warn!("{msg}");
        }
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

    // -- Spec validation tests -----------------------------------------------

    struct SpecWidget;

    impl PlushieWidget<()> for SpecWidget {
        fn type_names(&self) -> &[&str] {
            &["gauge"]
        }

        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("gauge").into()
        }

        fn clone_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            Box::new(SpecWidget)
        }

        fn event_specs(&self) -> Vec<plushie_core::EventSpec> {
            use plushie_core::spec::*;
            vec![
                EventSpec {
                    family: "slide".into(),
                    payload: PayloadSpec::Value(ValueType::Float),
                },
                EventSpec {
                    family: "calibrated".into(),
                    payload: PayloadSpec::None,
                },
            ]
        }

        fn command_specs(&self) -> Vec<plushie_core::CommandSpec> {
            use plushie_core::spec::*;
            vec![
                CommandSpec {
                    family: "set_value".into(),
                    payload: PayloadSpec::Value(ValueType::Float),
                },
                CommandSpec {
                    family: "reset".into(),
                    payload: PayloadSpec::None,
                },
            ]
        }
    }

    #[test]
    fn event_validation_accepts_correct_payload() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(SpecWidget));
        let idx = registry.index_for_type("gauge").unwrap();
        registry.map_node("g1".into(), idx);

        // Correct float value for "slide" event: no panic
        registry.validate_event_payload("g1", "slide", &serde_json::json!(42.0));
        // Correct null for "calibrated" event: no panic
        registry.validate_event_payload("g1", "calibrated", &serde_json::Value::Null);
    }

    #[test]
    fn command_validation_accepts_correct_payload() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(SpecWidget));
        let idx = registry.index_for_type("gauge").unwrap();
        registry.map_node("g1".into(), idx);

        // Correct float value for "set_value": no panic
        registry.validate_command_payload(idx, "g1", "set_value", &serde_json::json!(72.0));
        // Correct null for "reset": no panic
        registry.validate_command_payload(idx, "g1", "reset", &serde_json::Value::Null);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "event spec mismatch")]
    fn event_validation_panics_on_wrong_type_in_debug() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(SpecWidget));
        let idx = registry.index_for_type("gauge").unwrap();
        registry.map_node("g1".into(), idx);

        // String where float expected: should panic in debug
        registry.validate_event_payload("g1", "slide", &serde_json::json!("not a number"));
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "command spec mismatch")]
    fn command_validation_panics_on_wrong_type_in_debug() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(SpecWidget));
        let idx = registry.index_for_type("gauge").unwrap();
        registry.map_node("g1".into(), idx);

        // String where float expected: should panic in debug
        registry.validate_command_payload(idx, "g1", "set_value", &serde_json::json!("wrong"));
    }

    #[test]
    fn validation_skipped_for_unknown_family() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(SpecWidget));
        let idx = registry.index_for_type("gauge").unwrap();
        registry.map_node("g1".into(), idx);

        // Unknown family: no panic regardless of value
        registry.validate_event_payload("g1", "unknown_event", &serde_json::json!("anything"));
        registry.validate_command_payload(idx, "g1", "unknown_cmd", &serde_json::json!(true));
    }

    #[test]
    fn validation_skipped_for_widgets_without_specs() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(TestWidget::new(&["button"])));
        let idx = registry.index_for_type("button").unwrap();
        registry.map_node("b1".into(), idx);

        // No specs declared: no panic regardless of value
        registry.validate_event_payload("b1", "click", &serde_json::json!("wrong type"));
        registry.validate_command_payload(idx, "b1", "anything", &serde_json::json!(true));
    }

    // ---------------------------------------------------------------------------
    // cleanup_stale dispatch from prepare_walk
    //
    // Factory-owned per-instance HashMaps must shrink as nodes leave
    // the tree; otherwise widget state keyed by (window_id, node_id)
    // leaks forever. cleanup_stale dispatches from prepare_walk
    // after prune_shared, letting factories evict entries whose key
    // is absent from the live set.
    // ---------------------------------------------------------------------------

    fn tree(children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            id: "root".into(),
            type_name: "container".into(),
            props: serde_json::json!({}).into(),
            children,
        }
    }

    fn leaf(id: &str, type_name: &str) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: type_name.to_string(),
            props: serde_json::json!({}).into(),
            children: vec![],
        }
    }

    // Shared counter used to expose live-key set sizes for
    // cleanup_stale tests without smuggling typed references out of
    // the registry.
    use std::sync::Arc;

    #[derive(Default)]
    struct ContentsSpy {
        /// Snapshot of contents size after each cleanup_stale call.
        sizes: std::sync::Mutex<Vec<usize>>,
    }

    struct SpyingWidget {
        contents: std::collections::HashMap<(String, String), String>,
        spy: Arc<ContentsSpy>,
    }

    impl PlushieWidget<()> for SpyingWidget {
        fn type_names(&self) -> &[&str] {
            &["spying"]
        }

        fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &Theme) {
            self.contents
                .insert((window_id.to_string(), node.id.clone()), node.id.clone());
        }

        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("spy").into()
        }

        fn cleanup_stale(&mut self, live_ids: &std::collections::HashSet<(String, String)>) {
            self.contents.retain(|k, _| live_ids.contains(k));
            self.spy.sizes.lock().unwrap().push(self.contents.len());
        }

        fn clone_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            Box::new(SpyingWidget {
                contents: std::collections::HashMap::new(),
                spy: self.spy.clone(),
            })
        }
    }

    #[test]
    fn cleanup_stale_removes_keys_for_nodes_not_in_tree() {
        let mut registry = WidgetRegistry::<()>::new();
        let spy = Arc::new(ContentsSpy::default());
        registry.register(Box::new(SpyingWidget {
            contents: std::collections::HashMap::new(),
            spy: spy.clone(),
        }));

        let mut shared = crate::shared_state::SharedState::new();
        let theme = Theme::Dark;

        // First prepare_walk: two nodes present. cleanup_stale runs
        // after prepare populates contents, so the spy sees size 2.
        let first_tree = tree(vec![leaf("a", "spying"), leaf("b", "spying")]);
        registry.prepare_walk(&first_tree, &mut shared, &theme);

        // Second prepare_walk: only 'a' remains. 'b' must be evicted.
        let second_tree = tree(vec![leaf("a", "spying")]);
        registry.prepare_walk(&second_tree, &mut shared, &theme);

        let sizes = spy.sizes.lock().unwrap().clone();
        assert_eq!(
            sizes,
            vec![2, 1],
            "cleanup_stale should observe 2 contents after first walk and 1 after the second"
        );
    }

    #[test]
    fn prepare_walk_caps_depth_and_logs() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(TestWidget::new(&["stacked"])));

        // Build a tree deeper than MAX_TREE_DEPTH.
        let mut node = leaf("leaf", "stacked");
        for i in 0..(crate::shared_state::MAX_TREE_DEPTH + 20) {
            node = TreeNode {
                id: format!("n{i}"),
                type_name: "stacked".into(),
                props: serde_json::json!({}).into(),
                children: vec![node],
            };
        }

        // prepare_walk must not stack-overflow or panic on a tree
        // that exceeds the depth cap.
        let mut shared = crate::shared_state::SharedState::new();
        registry.prepare_walk(&node, &mut shared, &Theme::Dark);
    }
}
