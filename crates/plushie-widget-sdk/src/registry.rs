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
//! Simple stateless widget via the derive:
//!
//! ```ignore
//! use plushie_widget_sdk::prelude::*;
//!
//! #[derive(PlushieWidget)]
//! #[plushie_widget(type_name = "gauge")]
//! struct Gauge;
//!
//! impl<R: PlushieRenderer> PlushieWidgetRender<R> for Gauge {
//!     fn render<'a>(
//!         &'a self,
//!         node: &'a TreeNode,
//!         ctx: &RenderCtx<'a, R>,
//!     ) -> PlushieElement<'a, R> {
//!         todo!()
//!     }
//! }
//! ```
//!
//! Manual impl (stateful widgets, multiple type names, etc.):
//!
//! ```ignore
//! impl<R: PlushieRenderer> PlushieWidget<R> for Gauge {
//!     fn type_names(&self) -> &[&str] { &["gauge"] }
//!     fn render<'a>(&'a self, node: &'a TreeNode, ctx: &RenderCtx<'a, R>)
//!         -> PlushieElement<'a, R> { todo!() }
//!     fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
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
///
/// `init` is called for every registered widget on every Settings
/// message, regardless of whether the widget declares a namespace.
/// Widgets with a namespace receive the matching config slice from
/// `widget_config[namespace]`; widgets without a namespace receive
/// `Value::Null`.
///
/// # Typed config
///
/// Widgets that store config in a struct should declare
/// `#[derive(Default, Deserialize)]` and read via
/// [`config_or_default`](Self::config_or_default) so missing or
/// malformed config falls back to sane defaults without boilerplate:
///
/// ```ignore
/// use serde::Deserialize;
///
/// #[derive(Default, Deserialize)]
/// struct GaugeConfig {
///     #[serde(default)]
///     warn_threshold: f32,
/// }
///
/// impl<R: PlushieRenderer> PlushieWidget<R> for Gauge {
///     fn namespace(&self) -> &str { "gauge" }
///     fn init(&mut self, ctx: &InitCtx<'_>) {
///         let cfg = ctx.config_or_default::<GaugeConfig>();
///         self.warn_threshold = cfg.warn_threshold;
///     }
///     // ...
/// }
/// ```
#[derive(Debug)]
pub struct InitCtx<'a> {
    /// Widget-specific config from `Settings.widget_config[namespace]`.
    /// `Value::Null` if the host didn't provide config for this
    /// widget, or if the widget has no namespace.
    pub config: &'a Value,
    /// The current theme at init time.
    pub theme: &'a Theme,
    /// Global default text size, if set by the host.
    pub default_text_size: Option<f32>,
    /// Global default font, if set by the host.
    pub default_font: Option<iced::Font>,
}

impl InitCtx<'_> {
    /// Deserialize [`config`](Self::config) into a typed value.
    ///
    /// Returns the deserialization error on malformed input so the
    /// caller can decide how to handle it (log, fail fast, fall back
    /// to defaults).
    pub fn config_as<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        T::deserialize(self.config.clone())
    }

    /// Deserialize [`config`](Self::config), falling back to
    /// [`Default::default`] on malformed or missing config.
    ///
    /// Prefer this helper when the widget has sensible defaults and
    /// partial/absent config should not take the renderer down. The
    /// fallback path also covers the "widget has no namespace" case,
    /// where `config` is [`Value::Null`].
    pub fn config_or_default<T: serde::de::DeserializeOwned + Default>(&self) -> T {
        self.config_as::<T>().unwrap_or_default()
    }
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
    /// Return [`HandleResult::Fallthrough`] to let the registry run
    /// its default message-to-event conversion (Click, Input, Toggle,
    /// Select, and other generic widget events).
    ///
    /// Return [`HandleResult::Handled`] (usually constructed via
    /// [`HandleResult::consume`] or [`HandleResult::emit`]) to take
    /// responsibility for the message. The registry emits the
    /// returned events and skips the generic fallback for this call.
    ///
    /// Only override this for widgets that need stateful message
    /// processing (TextEditor, PaneGrid, Slider).
    fn handle_message(&mut self, _msg: &Message) -> HandleResult {
        HandleResult::Fallthrough
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

    /// Subscriptions this widget wants active while `node` is in
    /// the tree.
    ///
    /// The registry calls this during [`prepare_walk`](WidgetRegistry::prepare_walk)
    /// each cycle. Each returned [`WidgetSubscription`] carries an
    /// inner tag that the registry namespaces to
    /// `widget:{window_id}#{scope}/{node_id}:{inner_tag}` before
    /// handing to the subscription manager. Subscriptions disappear
    /// automatically when the owning node leaves the tree.
    ///
    /// Default impl returns an empty `Vec`. Timers, animation frames,
    /// and per-widget event listeners are the primary use case.
    fn subscriptions(
        &self,
        _node: &TreeNode,
        _ctx: &SubscribeCtx<'_>,
    ) -> Vec<WidgetSubscription> {
        vec![]
    }

    /// Produce a fresh widget instance for a new multiplexed session.
    ///
    /// Each session gets its own widget with independent per-instance
    /// state. Return a widget with *no per-instance state* carried
    /// over. Shared, read-only configuration can be wrapped in `Arc`
    /// and cloned cheaply.
    ///
    /// For stateless widgets, `Box::new(Self::default())` (or
    /// `Box::new(Self)` for unit structs) is the canonical impl.
    ///
    /// See the "Session multiplexing" section in
    /// `docs/core-widget-guide.md` for the full contract and a worked
    /// stateful-widget example.
    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>>;
}

/// Return value from [`PlushieWidget::handle_message`].
///
/// Makes the "did the widget handle this message" signal explicit.
/// `Fallthrough` lets the registry run its default conversion;
/// `Handled` means the widget took responsibility and the registry
/// should emit the returned events without running the fallback.
#[derive(Debug)]
pub enum HandleResult {
    /// The widget did not consume the message. The registry should
    /// run its default message-to-event conversion.
    Fallthrough,
    /// The widget consumed the message. The registry should emit
    /// the contained events and skip the generic fallback.
    ///
    /// An empty `Vec` is valid: it means "handled, emit nothing",
    /// distinct from `Fallthrough`.
    Handled(Vec<OutgoingEvent>),
}

impl HandleResult {
    /// Shorthand for "consumed the message, emit nothing".
    pub fn consume() -> Self {
        HandleResult::Handled(Vec::new())
    }

    /// Shorthand for "consumed the message, emit these events".
    pub fn emit(events: Vec<OutgoingEvent>) -> Self {
        HandleResult::Handled(events)
    }
}

/// Context passed to [`PlushieWidget::subscriptions`].
///
/// Carries everything the widget needs to decide what to subscribe
/// to: the window id the node lives under, the current theme (for
/// theme-conditional subs), and a short "scope" string the registry
/// uses when namespacing the tag.
#[derive(Debug)]
pub struct SubscribeCtx<'a> {
    /// Window id the node belongs to. Empty for unwindowed nodes.
    pub window_id: &'a str,
    /// Current theme, for theme-conditional subscription choices.
    pub theme: &'a Theme,
    /// Short scope identifier included in the namespaced tag. Set
    /// by the registry to the widget's primary type name.
    pub scope: &'a str,
}

/// A subscription request returned from [`PlushieWidget::subscriptions`].
///
/// The registry namespaces `tag` into the global subscription key
/// space as `widget:{window_id}#{scope}/{node_id}:{tag}`. `kind`
/// names the event source (e.g. `"animation_frame"`, `"on_key_press"`)
/// the widget wants to listen to. `max_rate` optionally caps delivery
/// rate in events per second.
#[derive(Debug, Clone)]
pub struct WidgetSubscription {
    /// Inner tag chosen by the widget. Human-readable; namespaced by
    /// the registry before use.
    pub tag: String,
    /// Event source kind (e.g. `"animation_frame"`, `"on_key_press"`).
    pub kind: String,
    /// Optional rate cap in events per second. None = uncapped.
    pub max_rate: Option<u32>,
}

impl WidgetSubscription {
    /// Short-hand constructor.
    pub fn new(kind: impl Into<String>, tag: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            tag: tag.into(),
            max_rate: None,
        }
    }

    /// Attach a delivery-rate cap (events per second).
    pub fn with_max_rate(mut self, rate: u32) -> Self {
        self.max_rate = Some(rate);
        self
    }
}

/// Helper trait used by `#[derive(PlushieWidget)]`.
///
/// The derive generates an `impl PlushieWidget<R>` that delegates its
/// `render` method to `PlushieWidgetRender::render`. Widget authors
/// using the derive implement this trait to supply only the render
/// body; `type_names` and `fresh_for_session` are generated.
///
/// Manually-implemented widgets do not need this trait.
pub trait PlushieWidgetRender<R: PlushieRenderer> {
    /// Render a tree node to an iced Element. Same contract as
    /// [`PlushieWidget::render`].
    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R>;
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

    /// Active widget subscriptions collected during the most recent
    /// [`prepare_walk`](Self::prepare_walk). Tag is the fully
    /// namespaced key (`widget:{window_id}#{scope}/{node_id}:{inner}`);
    /// value carries the kind, max_rate, and originating node_id so
    /// the engine can route events back to the owning widget's
    /// [`PlushieWidget::handle_message`].
    active_widget_subs: HashMap<String, CollectedSubscription>,
}

/// A collected widget subscription, keyed by its namespaced tag.
#[derive(Debug, Clone)]
pub struct CollectedSubscription {
    /// Event source kind (e.g. "animation_frame", "on_key_press").
    pub kind: String,
    /// Rate cap in events per second, if any.
    pub max_rate: Option<u32>,
    /// Node id that produced this subscription (routing target).
    pub node_id: String,
    /// Window id the node belongs to, for window-scoped delivery.
    pub window_id: String,
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
    widget_subs: HashMap<String, CollectedSubscription>,
}

impl<R: PlushieRenderer> WidgetRegistry<R> {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            impls: Vec::new(),
            type_index: HashMap::new(),
            node_factory_map: HashMap::new(),
            provenance: HashMap::new(),
            active_widget_subs: HashMap::new(),
        }
    }

    /// Register a single widget. If the type name is already registered,
    /// the new widget replaces it (last-registered wins).
    ///
    /// Prefer [`register_strict`](Self::register_strict) for app-level
    /// registration: it fails loud on accidental type-name collisions
    /// with already-registered widgets. This looser variant stays
    /// available for intentional overrides.
    pub fn register(&mut self, widget: Box<dyn PlushieWidget<R>>) {
        self.register_with_set_name(widget, "");
    }

    /// Register a single widget, panicking if any of its type names
    /// already has an implementation registered.
    ///
    /// # Panics
    ///
    /// Panics with a message listing the colliding type names. The
    /// panic covers both same-set collisions (two built-in widgets
    /// claim the same name, which is a programmer bug) and
    /// cross-set collisions (a `.widget()` call shadows a built-in
    /// type, which is usually a typo).
    pub fn register_strict(&mut self, widget: Box<dyn PlushieWidget<R>>) {
        let collisions: Vec<String> = widget
            .type_names()
            .iter()
            .filter(|name| self.type_index.contains_key(**name))
            .map(|s| (*s).to_string())
            .collect();
        if !collisions.is_empty() {
            panic!(
                "widget registration collides with existing type name(s): [{}]. \
                 Rename the widget, or use `widget_override` (on the builder) / \
                 `register` (on the registry) if the override is intentional.",
                collisions.join(", "),
            );
        }
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
            if set_name.is_empty() {
                // Individual `.widget()` registration. Clear any
                // inherited provenance so a widget that shadows a
                // built-in type name no longer counts as trusted for
                // panic isolation.
                self.provenance.remove(name);
            } else {
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

    /// Whether the widget at `type_name` is trusted (bypasses catch_unwind).
    /// Only the iced set is trusted; every other provenance (empty, a
    /// third-party set) goes through panic isolation.
    fn is_trusted(&self, type_name: &str) -> bool {
        self.provenance.get(type_name).is_some_and(|s| s == "iced")
    }

    /// Dispatch a widget call with panic isolation (immutable receiver).
    ///
    /// Trusted widgets (the built-in iced set) run `f` directly.
    /// Untrusted widgets run `f` inside `catch_unwind`. If the call
    /// panics, a diagnostic is logged and `fallback` is returned.
    ///
    /// `PLUSHIE_NO_CATCH_UNWIND=1` disables the wrapping for debugging
    /// so panics propagate with full stack traces.
    fn call_widget<T>(
        &self,
        type_name: &str,
        label: &str,
        node_id: &str,
        f: impl FnOnce(&dyn PlushieWidget<R>) -> T,
        fallback: impl FnOnce() -> T,
    ) -> T {
        let Some(&idx) = self.type_index.get(type_name) else {
            return fallback();
        };
        let widget = self.impls[idx].as_ref();

        if self.is_trusted(type_name) || !catch_unwind_enabled() {
            return f(widget);
        }

        // AssertUnwindSafe: widget panics at arbitrary entry points
        // may leave the widget's internal state inconsistent. That is
        // acceptable here because this dispatch is the firewall; the
        // renderer ignores the widget's contribution for this call and
        // continues driving the rest of the tree.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(widget))) {
            Ok(v) => v,
            Err(_) => {
                log::error!(
                    "[code=widget_panic][id={node_id}] widget `{type_name}` panicked in {label}",
                );
                fallback()
            }
        }
    }

    /// Mutable counterpart to [`call_widget`]. Dispatches to a widget's
    /// mutable method with the same trusted/untrusted policy.
    fn call_widget_mut<T>(
        &mut self,
        type_name: &str,
        label: &str,
        node_id: &str,
        f: impl FnOnce(&mut Box<dyn PlushieWidget<R>>) -> T,
        fallback: impl FnOnce() -> T,
    ) -> T {
        let trusted = self.is_trusted(type_name);
        let Some(&idx) = self.type_index.get(type_name) else {
            return fallback();
        };

        if trusted || !catch_unwind_enabled() {
            return f(&mut self.impls[idx]);
        }

        let widget = &mut self.impls[idx];
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(widget))) {
            Ok(v) => v,
            Err(_) => {
                log::error!(
                    "[code=widget_panic][id={node_id}] widget `{type_name}` panicked in {label}",
                );
                fallback()
            }
        }
    }

    /// Render a tree node by dispatching to the registered factory.
    ///
    /// Untrusted widgets (anything not provided by the built-in iced
    /// set) run inside `catch_unwind`. A panic produces a red error
    /// placeholder instead of crashing the renderer.
    ///
    /// Render has its own inline isolation because the returned
    /// `Element` borrows the widget (`&'a self`), which the generic
    /// `call_widget` helper's return type can't express.
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

        if self.is_trusted(type_name) || !catch_unwind_enabled() {
            self.impls[idx].render(node, ctx)
        } else {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.impls[idx].render(node, ctx)
            })) {
                Ok(element) => element,
                Err(_) => {
                    log::error!(
                        "[code=widget_panic][id={}] widget `{type_name}` panicked in render",
                        node.id,
                    );
                    iced::widget::text(format!("Widget error: `{type_name}`"))
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
    pub fn fresh_for_session(&self) -> Self {
        let mut cloned_impls: Vec<Box<dyn PlushieWidget<R>>> = Vec::with_capacity(self.impls.len());
        let mut new_type_index = HashMap::new();

        for (i, widget) in self.impls.iter().enumerate() {
            let cloned = widget.fresh_for_session();
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
            active_widget_subs: HashMap::new(),
        }
    }

    /// Collect diagnostics for widgets that declare the same
    /// event/command family with mismatched payload specs.
    ///
    /// Widgets that declare identical specs for a shared family name
    /// (the common "click", "submit" taxonomy) are silent. Only
    /// genuine shape collisions produce a diagnostic, so a host SDK
    /// can surface them as a widget contract issue.
    pub fn family_collision_diagnostics(&self) -> Vec<OutgoingEvent> {
        use plushie_core::spec::{CommandSpec, EventSpec, PayloadSpec};

        let mut out = Vec::new();
        let mut events: HashMap<String, (String, PayloadSpec)> = HashMap::new();
        let mut commands: HashMap<String, (String, PayloadSpec)> = HashMap::new();

        for widget in &self.impls {
            let type_name = widget
                .type_names()
                .first()
                .map(|s| s.to_string())
                .unwrap_or_default();

            for EventSpec { family, payload } in widget.event_specs() {
                match events.get(&family) {
                    Some((prev_type, prev_payload)) => {
                        // PayloadSpec doesn't derive PartialEq upstream; compare via Debug format.
                        if format!("{prev_payload:?}") != format!("{payload:?}") {
                            out.push(OutgoingEvent::generic(
                                "widget_family_collision",
                                "",
                                Some(serde_json::json!({
                                    "kind": "event",
                                    "type_a": prev_type,
                                    "type_b": type_name,
                                    "family": family,
                                    "spec_a": format!("{:?}", prev_payload),
                                    "spec_b": format!("{:?}", payload),
                                })),
                            ));
                        }
                    }
                    None => {
                        events.insert(family, (type_name.clone(), payload));
                    }
                }
            }

            for CommandSpec { family, payload } in widget.command_specs() {
                match commands.get(&family) {
                    Some((prev_type, prev_payload)) => {
                        // PayloadSpec doesn't derive PartialEq upstream; compare via Debug format.
                        if format!("{prev_payload:?}") != format!("{payload:?}") {
                            out.push(OutgoingEvent::generic(
                                "widget_family_collision",
                                "",
                                Some(serde_json::json!({
                                    "kind": "command",
                                    "type_a": prev_type,
                                    "type_b": type_name,
                                    "family": family,
                                    "spec_a": format!("{:?}", prev_payload),
                                    "spec_b": format!("{:?}", payload),
                                })),
                            ));
                        }
                    }
                    None => {
                        commands.insert(family, (type_name.clone(), payload));
                    }
                }
            }
        }

        out
    }

    /// Broadcast init to every registered widget.
    ///
    /// `init` is called for every widget regardless of namespace. A
    /// widget that declares a non-empty namespace receives the
    /// matching config slice from `ctx.config` (or `Value::Null` if
    /// the host didn't provide one). A widget with no namespace
    /// receives `Value::Null`. Every call runs under panic isolation
    /// for untrusted widgets.
    pub fn init_all(&mut self, ctx: &InitCtx<'_>) {
        // Collect (type_name, namespace) pairs to drive the panic-
        // isolated dispatch. Iterating over `impls` directly would
        // borrow `self` for the whole loop and block call_widget_mut.
        let per_widget: Vec<(String, String)> = self
            .impls
            .iter()
            .enumerate()
            .map(|(idx, widget)| {
                let type_name = widget
                    .type_names()
                    .first()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("widget#{idx}"));
                (type_name, widget.namespace().to_string())
            })
            .collect();

        for (type_name, ns) in per_widget {
            let ns_config = if ns.is_empty() {
                Value::Null
            } else {
                ctx.config
                    .as_object()
                    .and_then(|obj| obj.get(&ns))
                    .cloned()
                    .unwrap_or(Value::Null)
            };
            self.call_widget_mut(
                &type_name,
                "init",
                &type_name,
                |widget| {
                    let ns_ctx = InitCtx {
                        config: &ns_config,
                        theme: ctx.theme,
                        default_text_size: ctx.default_text_size,
                        default_font: ctx.default_font,
                    };
                    widget.init(&ns_ctx);
                },
                || {},
            );
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
            widget_subs: HashMap::new(),
        };
        self.prepare_walk_inner(root, "", &mut ctx, 0);
        ctx.shared.prune_shared(&ctx.live_ids);

        // Dispatch cleanup_stale to every registered widget. This
        // shrinks factory-owned per-instance state keyed by
        // (window_id, node_id) when nodes leave the tree.
        let live_keys = ctx.live_keys;
        let type_names: Vec<String> = self
            .impls
            .iter()
            .enumerate()
            .map(|(idx, widget)| {
                widget
                    .type_names()
                    .first()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("widget#{idx}"))
            })
            .collect();
        for type_name in type_names {
            let live_keys_ref = &live_keys;
            self.call_widget_mut(
                &type_name,
                "cleanup_stale",
                &type_name,
                |widget| widget.cleanup_stale(live_keys_ref),
                || {},
            );
        }

        // Replace the active subscription set with the one gathered
        // this walk. Subscriptions whose owning nodes left the tree
        // are dropped automatically.
        self.active_widget_subs = ctx.widget_subs;
    }

    /// Active widget subscriptions collected during the most recent
    /// [`prepare_walk`](Self::prepare_walk). Keyed by namespaced tag.
    pub fn active_widget_subscriptions(&self) -> &HashMap<String, CollectedSubscription> {
        &self.active_widget_subs
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
            let type_name = node.type_name.clone();
            let window_id_owned = current_window_id.to_string();
            let node_ref = node;
            let theme_ref = ctx.theme;
            self.call_widget_mut(
                &type_name,
                "prepare",
                &node.id,
                |widget| widget.prepare(node_ref, &window_id_owned, theme_ref),
                || {},
            );

            // Collect widget-scoped subscriptions. Immutable call so
            // we drop through call_widget (panic isolation still
            // active via the helper).
            let scope = type_name.clone();
            let sub_node_id = node.id.clone();
            let sub_window_id = current_window_id.to_string();
            let subs = self.call_widget(
                &type_name,
                "subscriptions",
                &node.id,
                |widget| {
                    widget.subscriptions(
                        node_ref,
                        &SubscribeCtx {
                            window_id: &sub_window_id,
                            theme: theme_ref,
                            scope: &scope,
                        },
                    )
                },
                Vec::new,
            );
            for sub in subs {
                let full_tag = format!(
                    "widget:{}#{}/{}:{}",
                    sub_window_id, scope, sub_node_id, sub.tag,
                );
                ctx.widget_subs.insert(
                    full_tag,
                    CollectedSubscription {
                        kind: sub.kind,
                        max_rate: sub.max_rate,
                        node_id: sub_node_id.clone(),
                        window_id: sub_window_id.clone(),
                    },
                );
            }
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
        {
            let type_name = self.impls[idx]
                .type_names()
                .first()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let result = self.call_widget_mut(
                &type_name,
                "handle_message",
                node_id,
                |factory| factory.handle_message(msg),
                || HandleResult::Fallthrough,
            );
            if let HandleResult::Handled(events) = result {
                return events;
            }
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
        let type_name = self.impls[idx]
            .type_names()
            .first()
            .map(|s| s.to_string())
            .unwrap_or_default();
        self.call_widget_mut(
            &type_name,
            "handle_widget_op",
            node_id,
            |widget| widget.handle_widget_op(node_id, op, payload),
            || None,
        )
    }

    /// Panic-isolated `infer_a11y` dispatch for the widget that owns `node`.
    ///
    /// Returns `None` if no widget is registered for the node type, if
    /// the widget declines to infer (returns `None`), or if the call
    /// panics (untrusted widget only; a trusted widget's panic still
    /// propagates).
    pub fn infer_a11y_for_node(&self, node: &TreeNode) -> Option<crate::a11y::A11yOverrides> {
        let type_name = node.type_name.as_str();
        if !self.type_index.contains_key(type_name) {
            return None;
        }
        self.call_widget(
            type_name,
            "infer_a11y",
            &node.id,
            |widget| widget.infer_a11y(node),
            || None,
        )
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

        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
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

    struct CountingWidget {
        calls: std::cell::Cell<u32>,
    }

    impl Default for CountingWidget {
        fn default() -> Self {
            Self {
                calls: std::cell::Cell::new(0),
            }
        }
    }

    impl PlushieWidget<()> for CountingWidget {
        fn type_names(&self) -> &[&str] {
            &["counter"]
        }

        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("count").into()
        }

        fn prepare(&mut self, _node: &TreeNode, _window_id: &str, _theme: &Theme) {
            self.calls.set(self.calls.get() + 1);
        }

        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            // New session -> fresh zero counter.
            Box::new(CountingWidget::default())
        }
    }

    // -- init ergonomics -----------------------------------------------------

    struct InitSpy {
        inits: std::rc::Rc<std::cell::RefCell<Vec<(String, Value)>>>,
        ns: &'static str,
    }

    impl PlushieWidget<()> for InitSpy {
        fn type_names(&self) -> &[&str] {
            &["init_spy"]
        }

        fn namespace(&self) -> &str {
            self.ns
        }

        fn init(&mut self, ctx: &InitCtx<'_>) {
            self.inits
                .borrow_mut()
                .push((self.ns.to_string(), ctx.config.clone()));
        }

        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("spy").into()
        }

        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            Box::new(InitSpy {
                inits: self.inits.clone(),
                ns: self.ns,
            })
        }
    }

    #[test]
    fn init_runs_for_every_widget_including_empty_namespace() {
        let inits = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut registry = WidgetRegistry::<()>::new();
        // Two widgets: one with a namespace, one without. Both must
        // receive init.
        registry.register(Box::new(InitSpy {
            inits: inits.clone(),
            ns: "",
        }));
        let theme = Theme::Dark;
        let config = serde_json::json!({ "other_ns": { "x": 1 } });
        let ctx = InitCtx {
            config: &config,
            theme: &theme,
            default_text_size: None,
            default_font: None,
        };
        registry.init_all(&ctx);

        let calls = inits.borrow().clone();
        assert_eq!(calls.len(), 1, "init should fire once for the widget");
        assert_eq!(calls[0].0, "", "namespace-less widget should still init");
        assert_eq!(
            calls[0].1,
            Value::Null,
            "namespace-less widget receives Value::Null"
        );
    }

    #[test]
    fn init_delivers_namespaced_config_slice() {
        let inits = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(InitSpy {
            inits: inits.clone(),
            ns: "gauge",
        }));
        let theme = Theme::Dark;
        let config = serde_json::json!({ "gauge": { "threshold": 42 } });
        let ctx = InitCtx {
            config: &config,
            theme: &theme,
            default_text_size: None,
            default_font: None,
        };
        registry.init_all(&ctx);

        let calls = inits.borrow().clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "gauge");
        assert_eq!(calls[0].1, serde_json::json!({ "threshold": 42 }));
    }

    #[test]
    fn init_ctx_config_or_default_parses_typed_struct() {
        #[derive(Default, serde::Deserialize)]
        struct Cfg {
            #[serde(default)]
            threshold: f32,
        }

        let theme = Theme::Dark;
        let config = serde_json::json!({ "threshold": 42.0 });
        let ctx = InitCtx {
            config: &config,
            theme: &theme,
            default_text_size: None,
            default_font: None,
        };
        let cfg = ctx.config_or_default::<Cfg>();
        assert!((cfg.threshold - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn init_ctx_config_or_default_falls_back_on_null() {
        #[derive(Default, serde::Deserialize)]
        struct Cfg {
            #[serde(default)]
            threshold: f32,
        }

        let theme = Theme::Dark;
        let null = Value::Null;
        let ctx = InitCtx {
            config: &null,
            theme: &theme,
            default_text_size: None,
            default_font: None,
        };
        let cfg = ctx.config_or_default::<Cfg>();
        assert_eq!(cfg.threshold, 0.0);
    }

    #[test]
    fn init_ctx_config_as_reports_errors() {
        #[derive(serde::Deserialize)]
        struct Cfg {
            #[allow(dead_code)]
            threshold: f32,
        }

        let theme = Theme::Dark;
        let bad = serde_json::json!({ "threshold": "not a number" });
        let ctx = InitCtx {
            config: &bad,
            theme: &theme,
            default_text_size: None,
            default_font: None,
        };
        let result = ctx.config_as::<Cfg>();
        assert!(result.is_err());
    }

    #[test]
    fn fresh_for_session_isolates_state_between_sessions() {
        // Drive registry A. The counter widget records prepare calls
        // indirectly through the node-factory map.
        let mut a = WidgetRegistry::<()>::new();
        a.register(Box::new(CountingWidget::default()));
        let tree_a = tree(vec![leaf("n1", "counter"), leaf("n2", "counter")]);
        let mut shared_a = crate::shared_state::SharedState::new();
        a.prepare_walk(&tree_a, &mut shared_a, &Theme::Dark);

        // A fresh session registry must not carry node mappings or
        // per-widget state from `a`.
        let mut b = a.fresh_for_session();
        let tree_b = tree(vec![leaf("n3", "counter")]);
        let mut shared_b = crate::shared_state::SharedState::new();
        b.prepare_walk(&tree_b, &mut shared_b, &Theme::Dark);

        assert!(a.get_for_node_id("n1").is_some());
        assert!(a.get_for_node_id("n2").is_some());
        assert!(b.get_for_node_id("n3").is_some());
        assert!(
            b.get_for_node_id("n1").is_none(),
            "session registry must not inherit node->factory map"
        );
        assert!(
            b.get_for_node_id("n2").is_none(),
            "session registry must not inherit node->factory map"
        );
    }

    #[test]
    fn fresh_for_session_preserves_type_index() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register_set(&TestSet);

        let cloned = registry.fresh_for_session();
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

        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
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

        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
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

    // ---------------------------------------------------------------------------
    // Provenance / panic isolation on `.widget()` overrides
    //
    // An individual `.widget()` registration that happens to use a type name
    // already claimed by a trusted widget set (e.g. "button" from the iced
    // set) must NOT inherit that set's trusted provenance. Otherwise the
    // override is treated as trusted and a panic inside it skips
    // `catch_unwind` and takes the renderer down.
    // ---------------------------------------------------------------------------

    struct PanickingButton;

    impl PlushieWidget<iced::Renderer> for PanickingButton {
        fn type_names(&self) -> &[&str] {
            &["button"]
        }

        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, iced::Renderer>,
        ) -> Element<'a, Message, Theme, iced::Renderer> {
            panic!("intentional render panic");
        }

        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<iced::Renderer>> {
            Box::new(PanickingButton)
        }
    }

    // ---------------------------------------------------------------------------
    // Widget subscriptions
    // ---------------------------------------------------------------------------

    struct TimerWidget;

    impl PlushieWidget<()> for TimerWidget {
        fn type_names(&self) -> &[&str] {
            &["timer"]
        }

        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("timer").into()
        }

        fn subscriptions(
            &self,
            _node: &TreeNode,
            _ctx: &SubscribeCtx<'_>,
        ) -> Vec<WidgetSubscription> {
            vec![WidgetSubscription::new("animation_frame", "tick")]
        }

        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            Box::new(TimerWidget)
        }
    }

    #[test]
    fn widget_subscriptions_collected_and_namespaced_during_prepare_walk() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(TimerWidget));

        let tree = tree(vec![leaf("t1", "timer")]);
        let mut shared = crate::shared_state::SharedState::new();
        registry.prepare_walk(&tree, &mut shared, &Theme::Dark);

        let subs = registry.active_widget_subscriptions();
        // Namespace shape: widget:{window_id}#{scope}/{node_id}:{inner}
        let expected_key = "widget:#timer/t1:tick";
        assert!(
            subs.contains_key(expected_key),
            "expected namespaced subscription key {expected_key}, got keys: {:?}",
            subs.keys().collect::<Vec<_>>()
        );
        let collected = subs.get(expected_key).unwrap();
        assert_eq!(collected.kind, "animation_frame");
        assert_eq!(collected.node_id, "t1");
    }

    #[test]
    fn widget_subscriptions_dropped_when_node_leaves_tree() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(TimerWidget));

        let mut shared = crate::shared_state::SharedState::new();

        let first = tree(vec![leaf("t1", "timer")]);
        registry.prepare_walk(&first, &mut shared, &Theme::Dark);
        assert!(!registry.active_widget_subscriptions().is_empty());

        let empty_tree = tree(vec![]);
        registry.prepare_walk(&empty_tree, &mut shared, &Theme::Dark);
        assert!(
            registry.active_widget_subscriptions().is_empty(),
            "subscription must go away once the owning node leaves the tree"
        );
    }

    // ---------------------------------------------------------------------------
    // Registration policy
    // ---------------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "collides with existing type name")]
    fn register_strict_panics_on_type_name_collision() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(TestWidget::new(&["button"])));
        // register_strict must fail loud here; it's almost certainly a typo.
        registry.register_strict(Box::new(TestWidget::new(&["button"])));
    }

    #[test]
    fn register_strict_succeeds_without_collision() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register_strict(Box::new(TestWidget::new(&["one"])));
        registry.register_strict(Box::new(TestWidget::new(&["two"])));
        assert!(registry.handles_type("one"));
        assert!(registry.handles_type("two"));
    }

    // Family collision diagnostics: identical specs on the same family
    // (shared taxonomy) are silent; mismatched specs emit a diagnostic.

    struct SpecFamilyA;
    impl PlushieWidget<()> for SpecFamilyA {
        fn type_names(&self) -> &[&str] {
            &["a"]
        }
        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("a").into()
        }
        fn event_specs(&self) -> Vec<plushie_core::EventSpec> {
            use plushie_core::spec::*;
            vec![EventSpec {
                family: "select".into(),
                payload: PayloadSpec::Value(ValueType::Integer),
            }]
        }
        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            Box::new(SpecFamilyA)
        }
    }

    struct SpecFamilyBMatching;
    impl PlushieWidget<()> for SpecFamilyBMatching {
        fn type_names(&self) -> &[&str] {
            &["b"]
        }
        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("b").into()
        }
        fn event_specs(&self) -> Vec<plushie_core::EventSpec> {
            use plushie_core::spec::*;
            // Same family, same spec: intentional shared taxonomy.
            vec![EventSpec {
                family: "select".into(),
                payload: PayloadSpec::Value(ValueType::Integer),
            }]
        }
        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            Box::new(SpecFamilyBMatching)
        }
    }

    struct SpecFamilyBConflicting;
    impl PlushieWidget<()> for SpecFamilyBConflicting {
        fn type_names(&self) -> &[&str] {
            &["b"]
        }
        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("b").into()
        }
        fn event_specs(&self) -> Vec<plushie_core::EventSpec> {
            use plushie_core::spec::*;
            // Same family, different spec: genuine collision.
            vec![EventSpec {
                family: "select".into(),
                payload: PayloadSpec::Value(ValueType::String),
            }]
        }
        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            Box::new(SpecFamilyBConflicting)
        }
    }

    #[test]
    fn family_collision_silent_for_matching_specs() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(SpecFamilyA));
        registry.register(Box::new(SpecFamilyBMatching));
        assert!(registry.family_collision_diagnostics().is_empty());
    }

    #[test]
    fn family_collision_emitted_for_mismatched_specs() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(SpecFamilyA));
        registry.register(Box::new(SpecFamilyBConflicting));
        let diags = registry.family_collision_diagnostics();
        assert_eq!(diags.len(), 1, "mismatched spec must produce one diagnostic");
    }

    #[test]
    fn widget_override_of_iced_type_clears_trusted_provenance() {
        let mut registry = WidgetRegistry::<iced::Renderer>::new();
        registry.register_set(&crate::widget::widget_set::iced_widget_set());
        assert_eq!(
            registry.provenance.get("button").map(|s| s.as_str()),
            Some("iced"),
            "iced set should install provenance for `button`"
        );

        registry.register(Box::new(PanickingButton));
        assert!(
            registry.provenance.get("button").is_none(),
            "`.widget()` override must drop inherited provenance so panic \
             isolation wraps the new widget"
        );
    }

    struct PanickingInPrepare;

    impl PlushieWidget<()> for PanickingInPrepare {
        fn type_names(&self) -> &[&str] {
            &["prepare_panic"]
        }

        fn prepare(&mut self, _node: &TreeNode, _window_id: &str, _theme: &Theme) {
            panic!("intentional prepare panic");
        }

        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, ()>,
        ) -> Element<'a, Message, Theme, ()> {
            iced::widget::text("noop").into()
        }

        fn fresh_for_session(&self) -> Box<dyn PlushieWidget<()>> {
            Box::new(PanickingInPrepare)
        }
    }

    #[test]
    fn prepare_panic_does_not_crash_registry() {
        let mut registry = WidgetRegistry::<()>::new();
        registry.register(Box::new(PanickingInPrepare));

        let tree = tree(vec![leaf("p1", "prepare_panic")]);
        let mut shared = crate::shared_state::SharedState::new();
        // Must not panic out of prepare_walk.
        registry.prepare_walk(&tree, &mut shared, &Theme::Dark);
    }

    #[test]
    fn widget_override_panic_is_contained_in_render() {
        let mut registry = WidgetRegistry::<iced::Renderer>::new();
        registry.register_set(&crate::widget::widget_set::iced_widget_set());
        registry.register(Box::new(PanickingButton));

        let node = leaf("b1", "button");
        let caches = crate::shared_state::SharedState::new();
        let images = crate::image_registry::ImageRegistry::new();
        let theme = iced::Theme::Dark;
        let ctx = RenderCtx {
            caches: &caches,
            images: &images,
            theme: &theme,
            registry: &registry,
            default_text_size: None,
            default_font: None,
            window_id: "",
            scale_factor: 1.0,
        };

        // The panicking override must produce the error placeholder
        // instead of unwinding out of render_node.
        let _elem = registry.render_node(&node, &ctx);
    }
}
