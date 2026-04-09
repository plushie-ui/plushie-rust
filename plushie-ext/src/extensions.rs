//! Widget extension trait and supporting types.
//!
//! [`WidgetExtension`] lets Rust crates add custom widget types to the
//! plushie renderer. Each extension is registered at startup via
//! [`PlushieAppBuilder`](crate::app::PlushieAppBuilder) and wrapped
//! by [`ExtensionAdapter`](crate::extension_adapter::ExtensionAdapter)
//! to integrate with the unified
//! [`WidgetRegistry`](crate::registry::WidgetRegistry).
//!
//! Supporting types:
//! - [`ExtensionCaches`] -- type-erased key-value store namespaced by
//!   extension, for per-node state
//! - [`WidgetEnv`] -- immutable render context passed to extension
//!   `render()` methods
//! - [`EventResult`] -- return type for extension event handling
//! - [`InitCtx`] -- context for extension initialization
//! - [`GenerationCounter`] -- helper for cache invalidation
//!
//! State is managed through [`ExtensionCaches`]. Mutation happens in
//! `prepare()` / `handle_event()` / `handle_command()` (mutable
//! phase), reads happen in `render()` (immutable phase), matching
//! iced's `update()`/`view()` split.

use std::any::Any;
use std::collections::HashMap;

use iced::{Element, Theme};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::image_registry::ImageRegistry;
use crate::message::Message;
use crate::protocol::{OutgoingEvent, TreeNode};

// ---------------------------------------------------------------------------
// WidgetExtension trait
// ---------------------------------------------------------------------------

/// Trait for native Rust widget extensions.
///
/// Extensions handle custom node types that the built-in renderer doesn't
/// know about. The trait scales from trivial render-only widgets (implement
/// `type_names`, `config_key`, `render`) to full custom iced widgets with
/// autonomous state (implement all methods).
///
/// # Lifecycle
///
/// Methods are called in this order:
///
/// 1. **Registration** -- `type_names()` and `config_key()` are queried once
///    at startup to build the dispatch index. `config_key()` must be unique
///    and must not contain `':'` (reserved as the cache namespace separator).
///
/// 2. **`init(config)`** -- called when a Settings message arrives from the
///    host. Receives the value from `extension_config[config_key]`, or
///    `Value::Null` if absent. Called before any `prepare()`.
///
/// 3. **`prepare(node, caches, theme)`** -- called in the mutable phase
///    (during `update()`) after every tree change (Snapshot or Patch), for
///    each node whose type matches this extension. Use this to create or
///    update per-node state in `ExtensionCaches`. Guaranteed to run before
///    `render()` for the same tree state.
///
/// 4. **`render(node, env)`** -- called in the immutable phase (`view()`)
///    to produce an iced `Element`. Receives read-only access to caches
///    via `WidgetEnv`. May be called multiple times per frame. Must not
///    block or perform I/O.
///
/// 5. **`handle_event(node_id, family, data, caches)`** -- called when a
///    widget event is emitted for a node owned by this extension. Return
///    `EventResult::PassThrough` to forward the event to the host,
///    `Consumed(events)` to suppress it, or `Observed(events)` to forward
///    the original AND emit additional events.
///
/// 6. **`handle_command(node_id, op, payload, caches)`** -- called when the
///    host sends an `ExtensionCommand` targeting a node owned by this
///    extension. Return any events to emit back to the host.
///
/// 7. **`cleanup(node_id, caches)`** -- called when a node is removed from
///    the tree (detected during `prepare_all()`). Use this to release
///    per-node resources from `ExtensionCaches`. Not called on process
///    exit or panic.
///
/// # Panic isolation
///
/// All mutable methods (`init`, `prepare`, `handle_event`,
/// `handle_command`, `cleanup`) are wrapped in `catch_unwind`. A panic
/// poisons the extension -- subsequent calls are skipped and a red
/// placeholder is rendered. Three consecutive `render()` panics also
/// trigger poisoning. Poison state is cleared on the next Snapshot.
///
/// # Cache access
///
/// `prepare()`, `handle_event()`, `handle_command()`, and `cleanup()`
/// receive `&mut ExtensionCaches` for read-write access. `render()`
/// receives read-only access via `WidgetEnv.caches`. This split matches
/// iced's `update()`/`view()` separation -- mutation happens in `update`,
/// reads in `view`.
///
/// # Prop helpers
///
/// The prelude re-exports typed prop extraction functions from
/// [`crate::prop_helpers`] for reading values from `TreeNode.props`:
///
/// - `prop_str(node, "key") -> Option<String>`
/// - `prop_f32(node, "key") -> Option<f32>`
/// - `prop_f64(node, "key") -> Option<f64>`
/// - `prop_i32(node, "key") -> Option<i32>`
/// - `prop_i64(node, "key") -> Option<i64>`
/// - `prop_u32(node, "key") -> Option<u32>`
/// - `prop_u64(node, "key") -> Option<u64>`
/// - `prop_usize(node, "key") -> Option<usize>`
/// - `prop_bool(node, "key") -> Option<bool>`
/// - `prop_bool_default(node, "key", default) -> bool`
/// - `prop_length(node, "key", default) -> Length`
/// - `prop_color(node, "key") -> Option<Color>` (parses `#RRGGBB` / `#RRGGBBAA`)
/// - `prop_str_array(node, "key") -> Option<Vec<String>>`
/// - `prop_f32_array(node, "key") -> Option<Vec<f32>>`
/// - `prop_f64_array(node, "key") -> Option<Vec<f64>>`
/// - `prop_range_f32(node) -> RangeInclusive<f32>` (reads `"range"` prop)
/// - `prop_range_f64(node) -> RangeInclusive<f64>` (reads `"range"` prop)
/// - `prop_object(node, "key") -> Option<&Map<String, Value>>`
/// - `prop_value(node, "key") -> Option<&Value>` (raw JSON access)
/// - `prop_horizontal_alignment(node, "key") -> alignment::Horizontal`
/// - `prop_vertical_alignment(node, "key") -> alignment::Vertical`
/// - `prop_content_fit(node) -> Option<ContentFit>`
/// - `value_to_length(val) -> Option<Length>` (lower-level conversion)
///
/// # Panic safety
///
/// All mutable trait methods (`init`, `prepare`, `handle_event`,
/// `handle_command`, `cleanup`) are wrapped in `catch_unwind`. If your
/// extension panics, the renderer logs the error, poisons the extension
/// (disabling further calls), and renders a red placeholder in its place.
///
/// Because `catch_unwind` uses `AssertUnwindSafe`, the compiler's unwind
/// safety checks are bypassed. This means your `&mut self` state could be
/// observed in a partially-mutated state if a panic interrupts a
/// multi-step mutation. The poisoning mechanism prevents further calls, but
/// if your extension shares state across nodes via `ExtensionCaches`, keep
/// each mutation atomic -- don't leave cache entries in an intermediate
/// state where a panic between two writes would be visible to other nodes.
///
/// `render()` panics are caught by the widget dispatch layer. Three
/// consecutive render panics trigger automatic poisoning.
///
/// # Accessibility
///
/// Extension widgets automatically get `A11yOverride` wrapping from the
/// renderer's a11y layer, so hosts can set a11y props (role, label, etc.)
/// on extension nodes the same way as built-in widgets. However:
///
/// - **Auto-inference does not apply** to extension types. The host must
///   set explicit `a11y` props for accessible labels and descriptions.
/// - **Focus cycling (Tab)** only visits widgets that implement the
///   `focusable` operation. If your extension renders focusable widgets
///   (e.g. text inputs), they participate automatically. If it renders
///   custom interactive content without iced's built-in focusable
///   widgets, Tab navigation will skip it.
///
/// # Examples
///
/// A minimal render-only extension that displays a greeting:
///
/// ```rust,ignore
/// use plushie_ext::prelude::*;
///
/// struct GreetingExtension;
///
/// impl<R: PlushieRenderer> WidgetExtension<R> for GreetingExtension {
///     fn type_names(&self) -> &[&str] {
///         &["greeting"]
///     }
///
///     fn config_key(&self) -> &str {
///         "greeting"
///     }
///
///     fn render<'a>(&self, node: &'a TreeNode, _env: &WidgetEnv<'a, R>) -> Element<'a, Message, Theme, R> {
///         use plushie_ext::iced::widget::text;
///         let name = node.props.get("name")
///             .and_then(|v| v.as_str())
///             .unwrap_or("world");
///         text(format!("Hello, {name}!")).into()
///     }
/// }
/// ```
pub trait WidgetExtension<R: PlushieRenderer = iced::Renderer>: Send + Sync + 'static {
    /// Node type names this extension handles (e.g. ["sparkline", "heatmap"]).
    fn type_names(&self) -> &[&str];

    /// Key used to route configuration from the Settings wire message's
    /// `extension_config` object. Must be unique across all extensions.
    fn config_key(&self) -> &str;

    /// Receive configuration and context from the host.
    ///
    /// Called on startup and renderer restart. The `ctx` provides the
    /// extension's config (from `Settings.extension_config[config_key]`),
    /// the current theme, and text rendering defaults. Extensions that
    /// need theme-dependent one-time setup can do it here instead of
    /// deferring to the first `prepare()` call.
    fn init(&mut self, _ctx: &InitCtx<'_>) {}

    /// Initialize or synchronize state for a node.
    ///
    /// Called in the mutable phase (after `Core::apply`, before `view()`)
    /// every time the tree changes (Snapshot or Patch). Nodes are visited
    /// in **depth-first pre-order** (parent before children) -- this is
    /// deterministic for a given tree structure. If an extension has
    /// multiple nodes, they're visited in tree order.
    ///
    /// Use this to populate [`ExtensionCaches`] entries that `render()`
    /// reads. The prepare/render split avoids the need for
    /// `RefCell` or interior mutability in the view phase.
    fn prepare(&mut self, _node: &TreeNode, _caches: &mut ExtensionCaches, _theme: &Theme) {}

    /// Build an iced Element for a node. Called in the immutable phase (view).
    fn render<'a>(
        &self,
        node: &'a TreeNode,
        env: &WidgetEnv<'a, R>,
    ) -> Element<'a, Message, Theme, R>;

    /// Handle an event emitted by this extension's widgets. Called before
    /// the event reaches the wire.
    fn handle_event(
        &mut self,
        _node_id: &str,
        _family: &str,
        _data: &Value,
        _caches: &mut ExtensionCaches,
    ) -> EventResult {
        EventResult::PassThrough
    }

    /// Handle a command sent from the host directly to this extension.
    ///
    /// The host sends `ExtensionCommand` messages with an `op` string and a
    /// JSON `payload`. By convention, `op` names use `snake_case` and are
    /// scoped to the extension (e.g. `"reset_zoom"`, `"set_data"`). The
    /// extension decides what ops it supports; unrecognized ops should be
    /// logged and ignored (return an empty vec).
    ///
    /// Return a vec of `OutgoingEvent`s to emit back to the host. Errors
    /// should be reported as built-in `"error"` events with
    /// `id = "extension_command"` and relevant details in the data payload,
    /// rather than panicking.
    fn handle_command(
        &mut self,
        _node_id: &str,
        _op: &str,
        _payload: &Value,
        _caches: &mut ExtensionCaches,
    ) -> Vec<OutgoingEvent> {
        vec![]
    }

    /// Called when a node is removed from the tree. Use this for
    /// external resource cleanup (file handles, connections, etc.).
    ///
    /// Cache entries for the removed node are automatically removed
    /// after this method returns -- you do not need to call
    /// `caches.remove()` yourself unless you have entries under
    /// non-standard keys.
    fn cleanup(&mut self, _node_id: &str, _caches: &mut ExtensionCaches) {}

    /// Create a fresh instance for a new session. Required for
    /// multiplexed mode (`--max-sessions > 1`). Each session gets its
    /// own extension instances so mutable state is fully isolated.
    ///
    /// The default implementation panics. Extensions that support
    /// multiplexed sessions must override this.
    fn new_instance(&self) -> Box<dyn WidgetExtension<R>> {
        unimplemented!(
            "extension `{}` does not support multiplexed sessions; \
             implement new_instance() to enable --max-sessions > 1",
            self.config_key()
        );
    }
}

// ---------------------------------------------------------------------------
// EventResult
// ---------------------------------------------------------------------------

/// Result of extension event handling.
///
/// Returned from [`WidgetExtension::handle_event`] to control whether the
/// original event reaches the host and whether additional events are emitted.
#[derive(Debug)]
#[must_use = "an EventResult should not be silently discarded"]
pub enum EventResult {
    /// Don't handle -- forward the original event to the host as-is.
    PassThrough,
    /// The extension consumed the event. The original event is suppressed and
    /// will NOT be forwarded to the host. The contained events (if any) are
    /// emitted instead. Note: `Consumed(vec![])` suppresses the original
    /// event without emitting any replacement -- use this intentionally, as
    /// the host will never see the event.
    Consumed(Vec<OutgoingEvent>),
    /// The extension observed the event. The original event IS forwarded to
    /// the host, and the contained additional events are also emitted.
    Observed(Vec<OutgoingEvent>),
}

// ---------------------------------------------------------------------------
// ExtensionCaches
// ---------------------------------------------------------------------------

/// Type-erased cache storage for extensions.
///
/// Keys are namespaced by extension `config_key()` to prevent collisions
/// between extensions that happen to use the same cache key string. All
/// public methods accept a `namespace` parameter (the extension's
/// `config_key()`) which is prefixed onto the raw key internally.
///
/// # Thread-safety invariant
///
/// `ExtensionCaches` is `Send + Sync` because all stored values are
/// `Any + Send + Sync`. However, the struct itself is only ever accessed
/// from a single thread at a time: mutation happens during `update()`
/// (the mutable phase), and reads happen during `view()` (the immutable
/// phase). In `--max-sessions` mode each session gets its own
/// `ExtensionCaches` instance, so there is no cross-thread sharing. No
/// internal locking is needed.
pub struct ExtensionCaches {
    inner: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl std::fmt::Debug for ExtensionCaches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionCaches")
            .field("entries", &self.inner.len())
            .field("keys", &self.inner.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ExtensionCaches {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Build the internal namespaced key: `"config_key:raw_key"`.
    fn namespaced_key(namespace: &str, key: &str) -> String {
        format!("{namespace}:{key}")
    }

    /// Look up a cached value by namespace and key.
    ///
    /// Returns `None` if the key doesn't exist **or** if the stored type
    /// doesn't match `T`. A type mismatch logs a warning so extension
    /// authors can spot accidental type changes during development.
    pub fn get<T: 'static>(&self, namespace: &str, key: &str) -> Option<&T> {
        let full_key = Self::namespaced_key(namespace, key);
        let entry = self.inner.get(&full_key)?;
        let result = entry.downcast_ref();
        if result.is_none() {
            log::warn!(
                "extension cache type mismatch for `{full_key}`: \
                 stored type does not match requested type"
            );
        }
        result
    }

    /// Look up a cached value mutably by namespace and key.
    ///
    /// Returns `None` if the key doesn't exist **or** if the stored type
    /// doesn't match `T`. A type mismatch logs a warning.
    pub fn get_mut<T: 'static>(&mut self, namespace: &str, key: &str) -> Option<&mut T> {
        let full_key = Self::namespaced_key(namespace, key);
        let entry = self.inner.get_mut(&full_key)?;
        let result = entry.downcast_mut();
        if result.is_none() {
            log::warn!(
                "extension cache type mismatch for `{full_key}`: \
                 stored type does not match requested type"
            );
        }
        result
    }

    pub fn get_or_insert<T: Send + Sync + 'static>(
        &mut self,
        namespace: &str,
        key: &str,
        default: impl FnOnce() -> T,
    ) -> &mut T {
        let ns_key = Self::namespaced_key(namespace, key);

        // Check for type mismatch on an existing entry *before* consuming
        // the default closure, so we can replace the stale value with a
        // fresh default of the correct type.
        let needs_replace = self
            .inner
            .get(&ns_key)
            .is_some_and(|v| v.downcast_ref::<T>().is_none());

        if needs_replace {
            log::warn!(
                "extension cache type mismatch for key `{ns_key}`: \
                 replacing existing entry with new default"
            );
            self.inner.remove(&ns_key);
        }

        self.inner
            .entry(ns_key)
            .or_insert_with(|| Box::new(default()))
            .downcast_mut()
            .expect("downcast must succeed: entry was just inserted with correct type")
    }

    pub fn insert<T: Send + Sync + 'static>(&mut self, namespace: &str, key: &str, value: T) {
        self.inner
            .insert(Self::namespaced_key(namespace, key), Box::new(value));
    }

    pub fn remove(&mut self, namespace: &str, key: &str) -> bool {
        self.inner
            .remove(&Self::namespaced_key(namespace, key))
            .is_some()
    }

    pub fn contains(&self, namespace: &str, key: &str) -> bool {
        self.inner
            .contains_key(&Self::namespaced_key(namespace, key))
    }

    /// Remove all entries for a given namespace prefix.
    pub fn remove_namespace(&mut self, namespace: &str) {
        let prefix = format!("{namespace}:");
        self.inner.retain(|k, _| !k.starts_with(&prefix));
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl Default for ExtensionCaches {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WidgetEnv
// ---------------------------------------------------------------------------

// Re-export RenderCtx from its own module for backward compat.
pub use crate::render_ctx::RenderCtx;

/// Context provided to extension `render()` methods.
///
/// All fields are immutable references -- mutation happens in `prepare()`,
/// reads happen here. This mirrors iced's `update()`/`view()` split.
///
/// # Available data
///
/// - `caches` -- extension caches (read-only). Use
///   `caches.get::<T>(config_key, node_id)` to read per-node state
///   populated in `prepare()`.
/// - `ctx` -- the shared [`RenderCtx`] carrying images, theme,
///   defaults, and child rendering. Convenience methods below
///   delegate to it: `images()`, `theme()`, `default_text_size()`,
///   `default_font()`, `render_child()`.
pub struct WidgetEnv<'a, R: PlushieRenderer = iced::Renderer> {
    pub caches: &'a ExtensionCaches,
    pub ctx: RenderCtx<'a, R>,
}

impl<R: PlushieRenderer> std::fmt::Debug for WidgetEnv<'_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WidgetEnv")
            .field("caches", self.caches)
            .field("ctx", &self.ctx)
            .finish()
    }
}

impl<'a, R: PlushieRenderer> WidgetEnv<'a, R> {
    pub fn images(&self) -> &'a ImageRegistry {
        self.ctx.images
    }
    pub fn theme(&self) -> &'a Theme {
        self.ctx.theme
    }
    pub fn default_text_size(&self) -> Option<f32> {
        self.ctx.default_text_size
    }
    pub fn default_font(&self) -> Option<iced::Font> {
        self.ctx.default_font
    }
    pub fn render_child(&self, node: &'a TreeNode) -> Element<'a, Message, Theme, R> {
        self.ctx.render_child(node)
    }
    /// The plushie window ID this render is for, or `""` in headless/test.
    pub fn window_id(&self) -> &'a str {
        self.ctx.window_id
    }
    /// Display scale factor for this window (1.0 = no scaling).
    pub fn scale_factor(&self) -> f32 {
        self.ctx.scale_factor
    }
}

/// Context passed to [`WidgetExtension::init`].
///
/// Provides the extension's config (from the host's Settings message)
/// along with the current theme and text rendering defaults. This
/// allows extensions to do theme-dependent initialization without
/// deferring to the first `prepare()` call.
#[derive(Debug)]
pub struct InitCtx<'a> {
    /// Extension-specific config from `Settings.extension_config[config_key]`.
    /// `Value::Null` if the host didn't provide config for this extension.
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
/// Store in `ExtensionCaches` alongside your data. Call `bump()` when data
/// changes (in `handle_command` or `prepare`). In your `canvas::Program`
/// implementation, compare the generation against a saved value in your
/// `Program::State` to decide whether to clear and redraw the cache.
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
///     // update() has &mut State -- clear the cache here when data changes.
///     fn update(&self, state: &mut MyState, ...) -> Option<Action<Message>> {
///         if state.generation != self.current_generation {
///             state.cache.clear();
///             state.generation = self.current_generation;
///         }
///         None
///     }
///
///     // draw() has &State -- the cache handles re-tessellation automatically
///     // when cleared above.
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

    /// Increment the generation. Wraps on overflow (u64 -- effectively never).
    pub fn bump(&mut self) {
        self.value = self.value.wrapping_add(1);
    }
}

impl Default for GenerationCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Test extension implementations --------------------------------------

    /// Minimal test extension that renders a text widget.
    struct TestExtension {
        type_names: Vec<&'static str>,
        config_key: &'static str,
    }

    impl TestExtension {
        fn new(type_names: Vec<&'static str>, config_key: &'static str) -> Self {
            Self {
                type_names,
                config_key,
            }
        }
    }

    impl WidgetExtension for TestExtension {
        fn type_names(&self) -> &[&str] {
            &self.type_names
        }

        fn config_key(&self) -> &str {
            self.config_key
        }

        fn render<'a>(&self, node: &'a TreeNode, _env: &WidgetEnv<'a>) -> Element<'a, Message> {
            use iced::widget::text;
            text(format!("test:{}", node.id)).into()
        }
    }

    // -- ExtensionCaches: get/insert/get_or_insert ---------------------------

    #[test]
    fn cache_insert_and_get() {
        let mut caches = ExtensionCaches::new();
        caches.insert("charts", "node1", 42u32);

        assert_eq!(caches.get::<u32>("charts", "node1"), Some(&42));
        assert_eq!(caches.get::<u32>("charts", "node2"), None);
    }

    #[test]
    fn cache_get_mut() {
        let mut caches = ExtensionCaches::new();
        caches.insert("ns", "key", vec![1, 2, 3]);

        if let Some(v) = caches.get_mut::<Vec<i32>>("ns", "key") {
            v.push(4);
        }
        assert_eq!(caches.get::<Vec<i32>>("ns", "key"), Some(&vec![1, 2, 3, 4]));
    }

    #[test]
    fn cache_get_or_insert_creates_default() {
        let mut caches = ExtensionCaches::new();
        let val = caches.get_or_insert::<String>("ns", "key", || "hello".to_string());
        assert_eq!(val, "hello");

        // Second call returns existing value, doesn't overwrite.
        let val = caches.get_or_insert::<String>("ns", "key", || "world".to_string());
        assert_eq!(val, "hello");
    }

    #[test]
    fn cache_get_or_insert_type_mismatch_replaces_with_default() {
        let mut caches = ExtensionCaches::new();
        caches.insert("ns", "key", 42u32);
        // Previously this panicked. Now it logs a warning, replaces the
        // stale entry, and returns a fresh default of the requested type.
        let val = caches.get_or_insert::<String>("ns", "key", || "replaced".to_string());
        assert_eq!(val, "replaced");
    }

    #[test]
    fn cache_wrong_type_returns_none() {
        let mut caches = ExtensionCaches::new();
        caches.insert("ns", "key", 42u32);

        // Asking for a different type returns None (not a panic for get).
        assert_eq!(caches.get::<String>("ns", "key"), None);
    }

    #[test]
    fn cache_remove_and_contains() {
        let mut caches = ExtensionCaches::new();
        caches.insert("ns", "key", 1u8);

        assert!(caches.contains("ns", "key"));
        assert!(caches.remove("ns", "key"));
        assert!(!caches.contains("ns", "key"));
        assert!(!caches.remove("ns", "key"));
    }

    #[test]
    fn cache_clear_removes_everything() {
        let mut caches = ExtensionCaches::new();
        caches.insert("a", "k1", 1u32);
        caches.insert("b", "k2", 2u32);

        caches.clear();
        assert!(!caches.contains("a", "k1"));
        assert!(!caches.contains("b", "k2"));
    }

    // -- Cache namespace isolation -------------------------------------------

    #[test]
    fn cache_namespace_isolation() {
        let mut caches = ExtensionCaches::new();

        // Two extensions use the same raw key "data" -- they shouldn't collide.
        caches.insert("charts", "data", vec![1.0f64, 2.0, 3.0]);
        caches.insert("gauges", "data", 42u32);

        assert_eq!(
            caches.get::<Vec<f64>>("charts", "data"),
            Some(&vec![1.0, 2.0, 3.0])
        );
        assert_eq!(caches.get::<u32>("gauges", "data"), Some(&42));
    }

    #[test]
    fn cache_remove_namespace() {
        let mut caches = ExtensionCaches::new();
        caches.insert("charts", "a", 1u32);
        caches.insert("charts", "b", 2u32);
        caches.insert("gauges", "a", 3u32);

        caches.remove_namespace("charts");

        assert!(!caches.contains("charts", "a"));
        assert!(!caches.contains("charts", "b"));
        assert!(caches.contains("gauges", "a"));
    }

    // -- EventResult variants ------------------------------------------------

    #[test]
    fn event_result_pass_through() {
        let result = EventResult::PassThrough;
        assert!(matches!(result, EventResult::PassThrough));
    }

    #[test]
    fn event_result_consumed_with_events() {
        let events = vec![OutgoingEvent::generic("test", "n1".to_string(), None)];
        let result = EventResult::Consumed(events);
        match result {
            EventResult::Consumed(e) => assert_eq!(e.len(), 1),
            _ => panic!("expected Consumed"),
        }
    }

    #[test]
    fn event_result_observed_with_events() {
        let events = vec![OutgoingEvent::generic("test", "n1".to_string(), None)];
        let result = EventResult::Observed(events);
        match result {
            EventResult::Observed(e) => assert_eq!(e.len(), 1),
            _ => panic!("expected Observed"),
        }
    }

    // -- GenerationCounter ---------------------------------------------------

    #[test]
    fn generation_counter_starts_at_zero() {
        let counter = GenerationCounter::new();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn generation_counter_bumps() {
        let mut counter = GenerationCounter::new();
        counter.bump();
        assert_eq!(counter.get(), 1);
        counter.bump();
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn generation_counter_default() {
        let counter = GenerationCounter::default();
        assert_eq!(counter.get(), 0);
    }

    // -- new_instance ---------------------------------------------------------

    /// Extension that implements new_instance() for session cloning.
    struct CloneableExtension {
        label: &'static str,
    }

    impl CloneableExtension {
        fn new(label: &'static str) -> Self {
            Self { label }
        }
    }

    impl WidgetExtension for CloneableExtension {
        fn type_names(&self) -> &[&str] {
            &["cloneable_widget"]
        }
        fn config_key(&self) -> &str {
            "cloneable"
        }
        fn render<'a>(&self, _node: &'a TreeNode, _env: &WidgetEnv<'a>) -> Element<'a, Message> {
            use iced::widget::text;
            text(self.label).into()
        }
        fn new_instance(&self) -> Box<dyn WidgetExtension> {
            Box::new(CloneableExtension::new(self.label))
        }
    }

    #[test]
    fn new_instance_default_panics() {
        let ext = TestExtension::new(vec!["sparkline"], "charts");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ext.new_instance();
        }));
        assert!(result.is_err(), "default new_instance() should panic");
    }

    #[test]
    fn new_instance_custom_returns_fresh_instance() {
        let ext = CloneableExtension::new("original");
        let fresh = ext.new_instance();
        assert_eq!(fresh.type_names(), &["cloneable_widget"]);
        assert_eq!(fresh.config_key(), "cloneable");
    }
}
