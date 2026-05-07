//! Pure state engine, decoupled from the iced runtime.
//!
//! [`Core`] owns the UI tree, widget caches, and subscription state.
//! It processes [`IncomingMessage`]s and returns [`CoreEffect`]s that
//! the host (the iced `App` or the headless runner) must execute.
//! Core never touches iced directly; it's pure state management.

use std::collections::HashMap;

use iced::Font;
use serde_json::Value;

use plushie_core::protocol::{IncomingMessage, OutgoingEvent};
use plushie_widget_sdk::runtime::{self as runtime, SharedState};

use crate::tree::Tree;

/// Side effects produced by [`Core::apply`] that the host must handle.
///
/// Core is zero-I/O: it never writes to stdout, opens windows, or runs
/// platform operations. Instead it returns these effects as commands for
/// the host (the iced daemon or headless runner) to execute. This keeps
/// Core testable and mode-agnostic.
///
/// Effects are returned in a `Vec` and should be processed in order.
/// Some variants (e.g. `StateChange::SyncWindows`) may depend on prior
/// tree mutations from the same `apply` call.
///
/// Variants are grouped by conceptual category so hosts can dispatch
/// on the outer variant first (emit vs dispatch vs state change) and
/// then on the inner typed sub-variant.
#[derive(Debug)]
pub enum CoreEffect {
    /// Write something to the outgoing wire stream.
    Emit(Emit),
    /// Run a platform or widget operation against the renderer.
    Dispatch(Dispatch),
    /// Update host-owned state that lives outside Core.
    StateChange(StateChange),
}

/// Outgoing wire payloads produced by Core. Every variant is a
/// fully-formed message the host can encode and write without any
/// further parsing.
#[derive(Debug)]
pub enum Emit {
    /// Widget or subscription event.
    Event(OutgoingEvent),
    /// Response to an effect request (stub or synthetic).
    EffectResponse(plushie_core::protocol::EffectResponse),
    /// Acknowledgement that an effect stub registration changed.
    StubAck(plushie_core::protocol::EffectStubAck),
}

/// Platform or widget operations the host must execute on Core's
/// behalf. Core doesn't touch iced, stdout, or the filesystem; it
/// produces these typed commands and the host dispatches them.
#[derive(Debug)]
pub enum Dispatch {
    /// Handle a platform effect (file dialog, clipboard, notification).
    ///
    /// Core does not execute effects; it passes the raw request through
    /// for the host to dispatch. The host decides whether to run the
    /// effect synchronously, asynchronously (via Task::perform), or
    /// return unsupported (e.g. in headless mode where file dialogs
    /// are unavailable).
    ///
    /// # Known effect kinds
    ///
    /// **Async (file dialogs):** `file_open`, `file_open_multiple`,
    /// `file_save`, `directory_select`, `directory_select_multiple`
    ///
    /// **Sync (clipboard):** `clipboard_read`, `clipboard_write`,
    /// `clipboard_read_html`, `clipboard_write_html`, `clipboard_clear`,
    /// `clipboard_read_primary`, `clipboard_write_primary`
    ///
    /// **Sync (notification):** `notification`
    Effect {
        request_id: String,
        kind: String,
        payload: Value,
    },

    /// Renderer-internal widget-targeted operation by op string.
    ///
    /// Covers focus, scroll, cursor, pane-grid ops, tree_hash queries,
    /// list_images, load_font, announce, exit, find_focused.
    WidgetOp { op: String, payload: Value },

    /// Typed window operation (open, close, resize, move, ...).
    Window(plushie_core::ops::WindowOp),

    /// Typed window query (get_size, get_position, ...).
    WindowQuery(plushie_core::ops::WindowQuery),

    /// Typed system-wide operation.
    System(plushie_core::ops::SystemOp),

    /// Typed system-wide query.
    SystemQuery(plushie_core::ops::SystemQuery),

    /// Image registry operation that targets a specific handle.
    ///
    /// # Known ops
    ///
    /// `create_image`, `update_image`, `delete_image`. Registry-level
    /// ops without per-image fields (`list`, `clear`) re-emit as
    /// `WidgetOp` and share the existing handlers.
    Image {
        op: String,
        handle: String,
        data: Option<Vec<u8>>,
        pixels: Option<Vec<u8>>,
        width: Option<u32>,
        height: Option<u32>,
    },
}

/// Changes to host-owned state that lives outside Core.
#[derive(Debug)]
pub enum StateChange {
    /// The window set may have changed; re-sync with renderer.
    ///
    /// Produced after every Snapshot and Patch that succeeds. The host
    /// should compare `tree.window_ids()` against its open window set
    /// and open/close as needed.
    SyncWindows,

    /// The global/root theme changed to an explicit value.
    ///
    /// The host should update its cached theme and set
    /// `theme_follows_system = false`.
    ThemeChanged(iced::Theme, runtime::ThemeChrome),

    /// The root theme was set to `"system"`: the app-level theme
    /// should follow the OS preference.
    ThemeFollowsSystem,

    /// Nodes removed during a patch that had "exit" props.
    /// The host should promote these to ghost nodes for exit animations.
    ExitNodes(Vec<(String, usize, plushie_core::protocol::TreeNode)>),

    /// Widget configuration received from the host's Settings message.
    ///
    /// The host should call `dispatcher.init_all(&config, &theme, ...)`
    /// to pass configuration and context to registered widgets.
    WidgetConfig(Value),
}

/// A single subscription entry within a kind. Multiple entries per kind
/// allow window-scoped subscriptions alongside global ones.
#[derive(Debug, Clone)]
pub struct SubscriptionEntry {
    pub tag: String,
    /// When set, only events from this window match. None = all windows.
    pub window_id: Option<String>,
    pub max_rate: Option<u32>,
}

/// Pure state core, decoupled from the iced runtime.
///
/// Owns the retained UI tree, widget caches, active subscriptions, and
/// global rendering defaults. The host calls [`apply`](Self::apply) with
/// each incoming message and executes the returned [`CoreEffect`]s.
pub struct Core {
    /// The retained UI tree (snapshots replace it, patches update it).
    pub tree: Tree,
    /// Caches for stateful widgets (text_editor content, markdown items, etc.).
    pub caches: SharedState,
    /// Active event subscriptions: kind -> list of entries.
    /// Each kind can have multiple entries with different tags and
    /// optional window scoping.
    pub active_subscriptions: HashMap<String, Vec<SubscriptionEntry>>,
    /// Global default event rate from Settings (events per second).
    /// None = no limit (full speed).
    pub default_event_rate: Option<u32>,
    /// Global default text size from Settings.
    pub default_text_size: Option<f32>,
    /// Global default font from Settings.
    pub default_font: Option<Font>,
    /// Cached resolved theme from the root node's `theme` prop.
    /// Only re-resolved when the raw JSON value changes.
    pub cached_theme: Option<iced::Theme>,
    pub cached_theme_chrome: runtime::ThemeChrome,
    /// Content hash of the last resolved theme prop, used for change
    /// detection. Replaces the previous `to_string()` approach which
    /// allocated and compared a full JSON string on every check.
    cached_theme_hash: Option<u64>,
    /// True after the first Settings message has been applied. Used to
    /// suppress warnings about startup-only fields on the initial Settings.
    settings_applied: bool,
    /// Registered effect stubs: kind -> response value. When an effect
    /// request matches a stub, the renderer returns the stubbed response
    /// immediately without executing the real effect. Used for testing
    /// and scripting.
    pub effect_stubs: HashMap<String, Value>,
    /// Per-session prop-validation override.
    ///
    /// `Some(true)` enables validation for this session.
    /// `None` falls back to the process-wide
    /// [`is_validate_props_enabled`](runtime::is_validate_props_enabled)
    /// check (which itself defaults to `cfg(debug_assertions)` when
    /// no global value has been set). `validate_props: false` in
    /// Settings does not disable validation; it leaves the fallback in
    /// control so hosts cannot turn off debug/default validation for a
    /// session.
    pub validate_props: Option<bool>,
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

impl Core {
    pub fn new() -> Self {
        Self {
            tree: Tree::new(),
            caches: SharedState::new(),
            active_subscriptions: HashMap::new(),
            default_event_rate: None,
            default_text_size: None,
            default_font: None,
            cached_theme: None,
            cached_theme_chrome: runtime::ThemeChrome::default(),
            cached_theme_hash: None,
            settings_applied: false,
            effect_stubs: HashMap::new(),
            validate_props: None,
        }
    }

    /// Resolve whether prop validation should run for this session.
    ///
    /// A per-session `true` forces validation on. Otherwise the
    /// process-wide flag decides.
    pub fn is_validate_props_enabled(&self) -> bool {
        match self.validate_props {
            Some(true) => true,
            None => runtime::is_validate_props_enabled(),
            Some(false) => runtime::is_validate_props_enabled(),
        }
    }

    /// Check whether at least one entry is registered for the given kind.
    pub fn has_subscription(&self, kind: &str) -> bool {
        self.active_subscriptions
            .get(kind)
            .is_some_and(|entries| !entries.is_empty())
    }

    /// Return all entries matching a kind, filtered by window_id.
    /// An entry matches if its window_id is None (global) or equals
    /// the event's window_id.
    pub fn matching_entries(&self, kind: &str, window_id: Option<&str>) -> Vec<&SubscriptionEntry> {
        match self.active_subscriptions.get(kind) {
            Some(entries) => entries
                .iter()
                .filter(|e| match (&e.window_id, window_id) {
                    (None, _) => true,
                    (Some(sub_wid), Some(evt_wid)) => sub_wid == evt_wid,
                    (Some(_), None) => false,
                })
                .collect(),
            None => Vec::new(),
        }
    }

    /// Return all entries matching a specific kind plus the catch-all
    /// SUB_EVENT kind, filtered by window_id. Useful for event emission
    /// where both specific and catch-all subscriptions should fire.
    pub fn matching_entries_with_catchall(
        &self,
        kind: &str,
        catchall_kind: &str,
        window_id: Option<&str>,
    ) -> Vec<&SubscriptionEntry> {
        let mut entries = self.matching_entries(kind, window_id);
        if kind != catchall_kind {
            entries.extend(self.matching_entries(catchall_kind, window_id));
        }
        entries
    }

    /// Collect all max_rate values from subscription entries, keyed by tag.
    /// Returns (tag, max_rate) pairs for entries that have a max_rate set.
    /// The tag includes window scope when present, so rate limiting is
    /// isolated per subscription entry.
    pub fn subscription_rates(&self) -> impl Iterator<Item = (&str, u32)> {
        self.active_subscriptions.values().flat_map(|entries| {
            entries
                .iter()
                .filter_map(|e| e.max_rate.map(|r| (e.tag.as_str(), r)))
        })
    }

    /// Collect all tags that have a max_rate set.
    pub fn subscription_rate_tags(&self) -> impl Iterator<Item = &str> {
        self.active_subscriptions.values().flat_map(|entries| {
            entries
                .iter()
                .filter(|e| e.max_rate.is_some())
                .map(|e| e.tag.as_str())
        })
    }

    /// Compute the canonical SHA-256 hash of the current tree.
    /// Returns the hex-encoded hash string, or an empty string if no tree.
    pub fn tree_hash(&self) -> String {
        match plushie_core::protocol::canonical_tree_hash(self.tree.root()) {
            Ok(hash) => hash,
            Err(e) => {
                log::error!("tree_hash: serialization failed: {e}");
                "SERIALIZATION_ERROR".to_string()
            }
        }
    }

    /// Resolve and cache a theme from a JSON prop value. Only re-resolves
    /// when the serialized JSON differs from the cached version.
    fn resolve_and_cache_theme(
        &mut self,
        theme_val: &serde_json::Value,
        effects: &mut Vec<CoreEffect>,
    ) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        plushie_widget_sdk::shared_state::hash_json_value(theme_val, &mut hasher);
        let hash = hasher.finish();

        if self.cached_theme_hash == Some(hash) {
            // Theme prop unchanged, skip resolution.
            return;
        }
        match runtime::resolve_theme_resolution(theme_val) {
            runtime::ThemeResolution::Theme(theme, chrome) => {
                self.cached_theme_hash = Some(hash);
                self.cached_theme = Some(theme.clone());
                self.cached_theme_chrome = chrome;
                effects.push(CoreEffect::StateChange(StateChange::ThemeChanged(
                    theme, chrome,
                )));
            }
            runtime::ThemeResolution::System => {
                self.cached_theme_hash = Some(hash);
                self.cached_theme = None;
                self.cached_theme_chrome = runtime::ThemeChrome::default();
                effects.push(CoreEffect::StateChange(StateChange::ThemeFollowsSystem));
            }
            runtime::ThemeResolution::Invalid => self.clear_cached_theme(effects),
        }
    }

    fn clear_cached_theme(&mut self, effects: &mut Vec<CoreEffect>) {
        if self.cached_theme_hash.is_none() {
            return;
        }

        self.cached_theme = None;
        self.cached_theme_chrome = runtime::ThemeChrome::default();
        self.cached_theme_hash = None;
        effects.push(CoreEffect::StateChange(StateChange::ThemeFollowsSystem));
    }

    /// Process an incoming message, mutate state, return effects.
    pub fn apply(&mut self, message: IncomingMessage) -> Vec<CoreEffect> {
        let mut effects = Vec::new();

        match message {
            IncomingMessage::Snapshot { tree } => {
                log::debug!("snapshot received (root id={})", tree.id);
                if let Some(theme_val) = tree.props.get_value("theme") {
                    self.resolve_and_cache_theme(&theme_val, &mut effects);
                } else {
                    self.clear_cached_theme(&mut effects);
                }
                if let Err(duplicates) = self.tree.snapshot(tree) {
                    let dup_list = duplicates.join(", ");
                    log::error!("snapshot contains duplicate node IDs: {dup_list}");
                    effects.push(CoreEffect::Emit(Emit::Event(OutgoingEvent::generic(
                        "error".to_string(),
                        "duplicate_node_ids".to_string(),
                        Some(serde_json::json!({
                            "error": "snapshot contains duplicate node IDs",
                            "duplicates": duplicates,
                        })),
                    ))));
                }
                self.caches.clear();
                if let Some(root) = self.tree.root()
                    && self.is_validate_props_enabled()
                {
                    Self::emit_prop_validation_warnings(root, &mut effects);
                }
                effects.push(CoreEffect::StateChange(StateChange::SyncWindows));
            }
            IncomingMessage::Patch { ops } => {
                log::debug!("patch received ({} ops)", ops.len());
                if let Err(error) = Tree::validate_patch_order(&ops) {
                    log::error!("invalid patch order: {error}");
                    effects.push(CoreEffect::Emit(Emit::Event(OutgoingEvent::generic(
                        "error",
                        "patch_order",
                        Some(serde_json::json!({
                            "error": error,
                        })),
                    ))));
                    return effects;
                }
                let exit_nodes = self.tree.apply_patch(ops);
                if !exit_nodes.is_empty() {
                    effects.push(CoreEffect::StateChange(StateChange::ExitNodes(exit_nodes)));
                }
                // Re-check root theme prop in case a patch changed it.
                if let Some(root) = self.tree.root() {
                    if let Some(theme_val) = root.props.get_value("theme") {
                        self.resolve_and_cache_theme(&theme_val, &mut effects);
                    } else {
                        self.clear_cached_theme(&mut effects);
                    }
                }
                if let Some(root) = self.tree.root()
                    && self.is_validate_props_enabled()
                {
                    Self::emit_prop_validation_warnings(root, &mut effects);
                }
                effects.push(CoreEffect::StateChange(StateChange::SyncWindows));
            }
            IncomingMessage::Effect { id, kind, payload } => {
                log::debug!("effect request: {kind} ({id})");
                if id.is_empty() {
                    log::warn!("effect request missing response id: {kind}");
                    effects.push(CoreEffect::Emit(Emit::Event(OutgoingEvent::generic(
                        "error",
                        "effect",
                        Some(serde_json::json!({
                            "error": "effect request missing response id",
                            "kind": kind,
                        })),
                    ))));
                } else if let Err(err) =
                    plushie_core::ops::validate_effect_request_from_wire(&kind, &payload)
                {
                    log::warn!("invalid effect request: {err}");
                    effects.push(CoreEffect::Emit(Emit::EffectResponse(
                        plushie_core::protocol::EffectResponse::error(id, err.to_string()),
                    )));
                } else if let Some(stub_response) = self.effect_stubs.get(&kind) {
                    log::debug!("effect stub hit: {kind} ({id})");
                    effects.push(CoreEffect::Emit(Emit::EffectResponse(
                        plushie_core::protocol::EffectResponse::ok(id, stub_response.clone()),
                    )));
                } else {
                    effects.push(CoreEffect::Dispatch(Dispatch::Effect {
                        request_id: id,
                        kind,
                        payload,
                    }));
                }
            }
            IncomingMessage::WidgetOp { op, payload } => {
                log::debug!("widget_op: {op}");
                effects.push(CoreEffect::Dispatch(Dispatch::WidgetOp { op, payload }));
            }
            IncomingMessage::Subscribe {
                kind,
                tag,
                window_id,
                max_rate,
            } => {
                log::debug!("subscription register: {kind} -> {tag} (window: {window_id:?})");
                let entries = self.active_subscriptions.entry(kind.clone()).or_default();
                // Update existing entry with same tag, or add a new one.
                if let Some(existing) = entries.iter_mut().find(|e| e.tag == tag) {
                    existing.window_id = window_id;
                    existing.max_rate = max_rate;
                } else {
                    entries.push(SubscriptionEntry {
                        tag,
                        window_id,
                        max_rate,
                    });
                }
            }
            IncomingMessage::Unsubscribe { kind, tag } => {
                if let Some(tag) = tag {
                    log::debug!("subscription unregister: {kind} tag={tag}");
                    if let Some(entries) = self.active_subscriptions.get_mut(&kind) {
                        entries.retain(|e| e.tag != tag);
                        if entries.is_empty() {
                            self.active_subscriptions.remove(&kind);
                        }
                    }
                } else {
                    log::debug!("subscription unregister: {kind} (all)");
                    self.active_subscriptions.remove(&kind);
                }
            }
            IncomingMessage::WindowOp {
                op,
                window_id,
                payload,
            } => {
                log::debug!("window_op: {op} ({window_id})");
                if let Some(typed) =
                    plushie_core::ops::WindowOp::from_wire(&op, &window_id, &payload)
                {
                    effects.push(CoreEffect::Dispatch(Dispatch::Window(typed)));
                } else if let Some(typed) =
                    plushie_core::ops::WindowQuery::from_wire(&op, &window_id, &payload)
                {
                    effects.push(CoreEffect::Dispatch(Dispatch::WindowQuery(typed)));
                } else {
                    log::warn!("unknown window_op: {op}");
                }
            }
            IncomingMessage::SystemOp { op, payload } => {
                log::debug!("system_op: {op}");
                if let Some(typed) = plushie_core::ops::SystemOp::from_wire(&op, &payload) {
                    effects.push(CoreEffect::Dispatch(Dispatch::System(typed)));
                } else {
                    log::warn!("unknown system_op: {op}");
                }
            }
            IncomingMessage::SystemQuery { op, payload } => {
                log::debug!("system_query: {op}");
                if let Some(typed) = plushie_core::ops::SystemQuery::from_wire(&op, &payload) {
                    effects.push(CoreEffect::Dispatch(Dispatch::SystemQuery(typed)));
                } else {
                    log::warn!("unknown system_query: {op}");
                }
            }
            IncomingMessage::Settings { settings } => {
                log::debug!("settings received");

                // Protocol version was already validated by
                // renderer::startup::perform_handshake before we got
                // here; no second check needed.

                // Typed deny_unknown_fields pass: logs per-field
                // diagnostics for unknown keys and type mismatches
                // without failing the whole parse.
                validate_wire_settings(&settings);

                // Startup-only fields are extracted by run.rs before the
                // daemon starts. Subsequent Settings messages can't change
                // them, so warn if the host sends them again.
                if self.settings_applied {
                    for field in &["antialiasing", "vsync", "fonts", "scale_factor"] {
                        if settings.get(*field).is_some() {
                            log::warn!(
                                "Settings field `{field}` is startup-only; \
                                 ignored after the daemon has started"
                            );
                        }
                    }
                }
                self.settings_applied = true;

                self.default_event_rate = settings
                    .get("default_event_rate")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);
                self.default_text_size = settings
                    .get("default_text_size")
                    .and_then(|v| v.as_f64())
                    .map(plushie_widget_sdk::prop_helpers::f64_to_f32);
                self.default_font = settings.get("default_font").map(resolve_font_with_fallback);
                // Per-session validate_props override. Only `true`
                // forces validation on for this session. `false`
                // leaves the process/debug default in control.
                if settings
                    .get("validate_props")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    self.validate_props = Some(true);
                }
                let ext_config = settings
                    .get("widget_config")
                    .cloned()
                    .unwrap_or(Value::Null);
                effects.push(CoreEffect::StateChange(StateChange::WidgetConfig(
                    ext_config,
                )));
            }
            IncomingMessage::ImageOp { op, payload } => {
                log::debug!("image_op: {op} ({handle})", handle = payload.handle);
                match op.as_str() {
                    // `list` and `clear` are registry-level ops with no
                    // per-image fields. Re-emit through the existing
                    // widget-op handlers so the shared logic stays in
                    // one place; the typed wire shape replaces the old
                    // `widget_op` envelope on the wire.
                    "list" => {
                        let payload_value = match payload.tag {
                            Some(tag) => serde_json::json!({"tag": tag}),
                            None => Value::Null,
                        };
                        effects.push(CoreEffect::Dispatch(Dispatch::WidgetOp {
                            op: "list_images".to_string(),
                            payload: payload_value,
                        }));
                    }
                    "clear" => {
                        effects.push(CoreEffect::Dispatch(Dispatch::WidgetOp {
                            op: "clear_images".to_string(),
                            payload: Value::Null,
                        }));
                    }
                    _ => {
                        effects.push(CoreEffect::Dispatch(Dispatch::Image {
                            op,
                            handle: payload.handle,
                            data: payload.data,
                            pixels: payload.pixels,
                            width: payload.width,
                            height: payload.height,
                        }));
                    }
                }
            }
            IncomingMessage::LoadFont { payload } => {
                log::debug!("load_font: family={}", payload.family);
                // Re-emit as the existing WidgetOp dispatch path so the
                // shared "load_font" handler (renderer-lib widget_ops
                // and headless's `load_font_from_payload`) keeps a
                // single applied site. The typed message's win is a
                // clean wire shape and native msgpack binary; the
                // internal dispatch shape is unchanged.
                let data_json = match payload.data {
                    Some(bytes) => {
                        use base64::Engine;
                        Value::String(base64::engine::general_purpose::STANDARD.encode(&bytes))
                    }
                    None => Value::Null,
                };
                let payload_value = serde_json::json!({
                    "family": payload.family,
                    "data": data_json,
                });
                effects.push(CoreEffect::Dispatch(Dispatch::WidgetOp {
                    op: "load_font".to_string(),
                    payload: payload_value,
                }));
            }
            // Scripting messages handled by the renderer binary (daemon /
            // headless), not by Core. Listed explicitly so adding a new
            // IncomingMessage variant produces a compile error here instead
            // of silently falling through a catch-all `_` arm.
            IncomingMessage::Query { .. } => {
                log::debug!("Query message ignored by Core (handled by scripting layer)");
            }
            IncomingMessage::Interact { .. } => {
                log::debug!("Interact message ignored by Core (handled by scripting layer)");
            }
            IncomingMessage::TreeHash { .. } => {
                log::debug!("TreeHash message ignored by Core (handled by scripting layer)");
            }
            IncomingMessage::Screenshot { .. } => {
                log::debug!("Screenshot message ignored by Core (handled by scripting layer)");
            }
            IncomingMessage::Reset { .. } => {
                log::debug!("Reset message ignored by Core (handled by scripting layer)");
            }
            IncomingMessage::Command { .. } => {
                log::debug!("Command message ignored by Core (handled by renderer App)");
            }
            IncomingMessage::Commands { .. } => {
                log::debug!("Commands message ignored by Core (handled by renderer App)");
            }
            IncomingMessage::AdvanceFrame { .. } => {
                log::warn!(
                    "AdvanceFrame is only supported in headless/test mode; ignored in daemon mode"
                );
            }
            IncomingMessage::RegisterEffectStub { kind, response } => {
                if plushie_core::ops::is_known_effect_kind(&kind) {
                    log::info!("effect stub registered: {kind}");
                    self.effect_stubs.insert(kind.clone(), response);
                    effects.push(CoreEffect::Emit(Emit::StubAck(
                        plushie_core::protocol::EffectStubAck::registered(kind),
                    )));
                } else {
                    log::warn!("unknown effect stub kind: {kind}");
                    effects.push(CoreEffect::Emit(Emit::StubAck(
                        plushie_core::protocol::EffectStubAck::register_error(kind),
                    )));
                }
            }
            IncomingMessage::UnregisterEffectStub { kind } => {
                if plushie_core::ops::is_known_effect_kind(&kind) {
                    log::info!("effect stub unregistered: {kind}");
                    self.effect_stubs.remove(&kind);
                    effects.push(CoreEffect::Emit(Emit::StubAck(
                        plushie_core::protocol::EffectStubAck::unregistered(kind),
                    )));
                } else {
                    log::warn!("unknown effect stub kind: {kind}");
                    effects.push(CoreEffect::Emit(Emit::StubAck(
                        plushie_core::protocol::EffectStubAck::unregister_error(kind),
                    )));
                }
            }
        }

        effects
    }

    /// Walk the tree and emit prop validation warnings as wire events.
    /// Called after Snapshot and Patch when validate_props is enabled.
    fn emit_prop_validation_warnings(
        root: &plushie_core::protocol::TreeNode,
        effects: &mut Vec<CoreEffect>,
    ) {
        Self::validate_node_recursive(root, effects);
    }

    fn validate_node_recursive(
        node: &plushie_core::protocol::TreeNode,
        effects: &mut Vec<CoreEffect>,
    ) {
        let warnings = runtime::collect_prop_warnings(node);
        if !warnings.is_empty() {
            effects.push(CoreEffect::Emit(Emit::Event(OutgoingEvent::generic(
                "prop_validation",
                node.id.clone(),
                Some(serde_json::json!({
                    "node_id": node.id,
                    "node_type": node.type_name,
                    "warnings": warnings,
                })),
            ))));
        }
        for child in &node.children {
            Self::validate_node_recursive(child, effects);
        }
    }
}

/// Resolve a font family from a `default_font` settings entry,
/// walking the optional fallback chain. Emits a
/// `font_family_not_found` diagnostic on each unresolved family.
///
/// Resolution order per name:
/// 1. Built-in shortcut: `monospace` -> `Font::MONOSPACE`.
/// 2. Runtime-loaded family via [`plushie_widget_sdk::fonts::is_loaded`] (populated
///    by `Command::load_font` at execution time).
/// 3. Fall through to the next name in the chain and emit a
///    `font_family_not_found` diagnostic.
///
/// If every name falls through, returns `Font::DEFAULT`.
fn resolve_font_with_fallback(v: &Value) -> Font {
    let primary = v.get("family").and_then(|f| f.as_str());
    let fallback_iter = v.get("fallback").and_then(|a| a.as_array());
    let mut chain: Vec<&str> = Vec::new();
    if let Some(p) = primary {
        chain.push(p);
    }
    if let Some(arr) = fallback_iter {
        for entry in arr {
            if let Some(s) = entry.as_str() {
                chain.push(s);
            }
        }
    }
    for name in &chain {
        if matches!(*name, "monospace") {
            return Font::MONOSPACE;
        }
        if plushie_widget_sdk::fonts::is_loaded(name)
            && let Some(interned) = runtime::intern_font_family_public(name)
        {
            return Font {
                family: iced::font::Family::Name(interned),
                ..Font::DEFAULT
            };
        }
        plushie_core::diagnostics::warn(plushie_core::Diagnostic::FontFamilyNotFound {
            family: (*name).to_string(),
        });
    }
    Font::DEFAULT
}

/// Typed shape of the Settings payload, for `deny_unknown_fields`
/// validation. Field-level decode failures emit diagnostics but do
/// not fail the whole parse; the caller continues extracting fields
/// via the existing `get`-and-coerce pattern so partial settings
/// still take effect.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // fields observed via Debug only; real extraction is field-by-field
struct WireSettings {
    #[serde(default)]
    protocol_version: Option<u64>,
    #[serde(default)]
    default_event_rate: Option<u64>,
    #[serde(default)]
    default_text_size: Option<f64>,
    #[serde(default)]
    default_font: Option<serde_json::Value>,
    #[serde(default)]
    antialiasing: Option<bool>,
    #[serde(default)]
    vsync: Option<bool>,
    #[serde(default)]
    fonts: Option<Vec<String>>,
    #[serde(default)]
    scale_factor: Option<f64>,
    #[serde(default)]
    theme: Option<serde_json::Value>,
    #[serde(default)]
    widget_config: Option<serde_json::Value>,
    #[serde(default)]
    validate_props: Option<bool>,
    #[serde(default)]
    log_level: Option<String>,
}

/// Run the typed `deny_unknown_fields` validation. Unknown keys
/// and type mismatches produce an error diagnostic but do not fail
/// the parse: the caller proceeds with per-field extraction.
fn validate_wire_settings(settings: &Value) {
    match serde_json::from_value::<WireSettings>(settings.clone()) {
        Ok(_) => {}
        Err(e) => {
            plushie_core::diagnostics::error(plushie_core::Diagnostic::InvalidSettings {
                detail: e.to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie_core::protocol::{IncomingMessage, PatchOp, TreeNode};
    use plushie_widget_sdk::testing::{
        node as make_node, node_with_children as make_node_with_children,
        node_with_props as make_node_with_props,
    };

    fn make_patch_op(op: &str, path: Vec<usize>, rest: serde_json::Value) -> PatchOp {
        let mut obj = serde_json::Map::new();
        obj.insert("op".to_string(), serde_json::json!(op));
        obj.insert("path".to_string(), serde_json::json!(path));
        if let Some(map) = rest.as_object() {
            for (key, value) in map {
                obj.insert(key.clone(), value.clone());
            }
        }
        serde_json::from_value(serde_json::Value::Object(obj)).unwrap()
    }

    fn child_ids(core: &Core) -> Vec<String> {
        core.tree
            .root()
            .unwrap()
            .children
            .iter()
            .map(|child| child.id.clone())
            .collect()
    }

    fn has_sync_windows(effects: &[CoreEffect]) -> bool {
        effects
            .iter()
            .any(|effect| matches!(effect, CoreEffect::StateChange(StateChange::SyncWindows)))
    }

    fn has_patch_order_error(effects: &[CoreEffect]) -> bool {
        effects.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::Emit(Emit::Event(event))
                    if event.family == "error" && event.id == "patch_order"
            )
        })
    }

    fn has_theme_follows_system(effects: &[CoreEffect]) -> bool {
        effects.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::StateChange(StateChange::ThemeFollowsSystem)
            )
        })
    }

    fn has_prop_validation(effects: &[CoreEffect], node_id: &str) -> bool {
        effects.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::Emit(Emit::Event(event))
                    if event.family == "prop_validation" && event.id == node_id
            )
        })
    }

    // -- Core::new() --

    #[test]
    fn new_returns_empty_tree() {
        let core: Core = Core::new();
        assert!(core.tree.root().is_none());
    }

    #[test]
    fn new_has_empty_active_subscriptions() {
        let core: Core = Core::new();
        assert!(core.active_subscriptions.is_empty());
    }

    #[test]
    fn new_has_no_default_text_size() {
        let core: Core = Core::new();
        assert!(core.default_text_size.is_none());
    }

    #[test]
    fn new_has_no_default_font() {
        let core: Core = Core::new();
        assert!(core.default_font.is_none());
    }

    // -- Snapshot --

    #[test]
    fn snapshot_sets_tree_and_returns_sync_windows() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Snapshot {
            tree: make_node("root", "column"),
        };
        let effects = core.apply(msg);
        // Tree should be populated
        assert!(core.tree.root().is_some());
        assert_eq!(core.tree.root().unwrap().id, "root");
        // Must include SyncWindows
        let has_sync = effects
            .iter()
            .any(|e| matches!(e, CoreEffect::StateChange(StateChange::SyncWindows)));
        assert!(has_sync);
    }

    #[test]
    fn snapshot_with_theme_prop_returns_theme_changed() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Snapshot {
            tree: make_node_with_props("root", "column", serde_json::json!({"theme": "dark"})),
        };
        let effects = core.apply(msg);
        let has_theme = effects
            .iter()
            .any(|e| matches!(e, CoreEffect::StateChange(StateChange::ThemeChanged(_, _))));
        assert!(has_theme);
    }

    #[test]
    fn snapshot_with_unknown_theme_does_not_apply_dark_or_system() {
        let mut core: Core = Core::new();
        let effects = core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_props("root", "column", serde_json::json!({"theme": "neon_pink"})),
        });

        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, CoreEffect::StateChange(StateChange::ThemeChanged(_, _))))
        );
        assert!(!has_theme_follows_system(&effects));
        assert!(core.cached_theme.is_none());
    }

    #[test]
    fn unknown_theme_clears_previous_resolved_theme() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_props("root", "column", serde_json::json!({"theme": "nord"})),
        });
        assert!(matches!(
            core.cached_theme.as_ref(),
            Some(iced::Theme::Nord)
        ));

        let effects = core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_props("root", "column", serde_json::json!({"theme": "neon_pink"})),
        });

        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, CoreEffect::StateChange(StateChange::ThemeChanged(_, _))))
        );
        assert!(has_theme_follows_system(&effects));
        assert!(core.cached_theme.is_none());
    }

    #[test]
    fn removing_unknown_theme_after_clear_does_not_emit_again() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_props("root", "column", serde_json::json!({"theme": "nord"})),
        });
        let effects = core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_props("root", "column", serde_json::json!({"theme": "neon_pink"})),
        });
        assert!(has_theme_follows_system(&effects));

        let effects = core.apply(IncomingMessage::Snapshot {
            tree: make_node("root", "column"),
        });

        assert!(!has_theme_follows_system(&effects));
        assert!(core.cached_theme_hash.is_none());
    }

    #[test]
    fn snapshot_without_theme_prop_has_no_theme_changed() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Snapshot {
            tree: make_node("root", "column"),
        };
        let effects = core.apply(msg);
        let has_theme = effects
            .iter()
            .any(|e| matches!(e, CoreEffect::StateChange(StateChange::ThemeChanged(_, _))));
        assert!(!has_theme);
    }

    #[test]
    fn snapshot_without_theme_prop_clears_previous_theme_chrome() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_props(
                "root",
                "column",
                serde_json::json!({
                    "theme": {
                        "name": "chrome",
                        "scrollbar_color": "#112233"
                    }
                }),
            ),
        });
        assert!(core.cached_theme_chrome.scrollbar_color.is_some());

        let effects = core.apply(IncomingMessage::Snapshot {
            tree: make_node("root", "column"),
        });

        assert!(has_theme_follows_system(&effects));
        assert!(core.cached_theme.is_none());
        assert!(core.cached_theme_chrome.is_empty());
    }

    // -- Patch --

    #[test]
    fn patch_with_no_ops_returns_sync_windows() {
        let mut core: Core = Core::new();
        // First put a tree in place so patch has something to work with
        let snapshot_msg = IncomingMessage::Snapshot {
            tree: make_node("root", "column"),
        };
        core.apply(snapshot_msg);

        let patch_msg = IncomingMessage::Patch { ops: vec![] };
        let effects = core.apply(patch_msg);
        assert!(has_sync_windows(&effects));
    }

    #[test]
    fn patch_removing_root_theme_clears_previous_theme_chrome() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_props(
                "root",
                "column",
                serde_json::json!({
                    "theme": {
                        "name": "chrome",
                        "cursor_color": "#112233",
                        "scrollbar_color": "#445566",
                        "scroller_color": "#778899"
                    }
                }),
            ),
        });
        assert!(!core.cached_theme_chrome.is_empty());

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![make_patch_op(
                "update_props",
                vec![],
                serde_json::json!({
                    "props": {"theme": null}
                }),
            )],
        });

        assert!(has_theme_follows_system(&effects));
        assert!(core.cached_theme.is_none());
        assert!(core.cached_theme_chrome.is_empty());
    }

    #[test]
    fn patch_rejects_insert_before_remove_without_mutating_tree() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_children(
                "root",
                "column",
                vec![
                    make_node("a", "text"),
                    make_node("b", "text"),
                    make_node("c", "text"),
                ],
            ),
        });

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![
                make_patch_op(
                    "insert_child",
                    vec![],
                    serde_json::json!({
                        "index": 3,
                        "node": {"id": "d", "type": "text", "props": {}, "children": []}
                    }),
                ),
                make_patch_op("remove_child", vec![], serde_json::json!({"index": 0})),
            ],
        });

        assert_eq!(child_ids(&core), vec!["a", "b", "c"]);
        assert!(has_patch_order_error(&effects));
        assert!(!has_sync_windows(&effects));
    }

    #[test]
    fn patch_rejects_remove_same_parent_ascending_without_mutating_tree() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_children(
                "root",
                "column",
                vec![
                    make_node("a", "text"),
                    make_node("b", "text"),
                    make_node("c", "text"),
                ],
            ),
        });

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![
                make_patch_op("remove_child", vec![], serde_json::json!({"index": 0})),
                make_patch_op("remove_child", vec![], serde_json::json!({"index": 1})),
            ],
        });

        assert_eq!(child_ids(&core), vec!["a", "b", "c"]);
        assert!(has_patch_order_error(&effects));
        assert!(!has_sync_windows(&effects));
    }

    #[test]
    fn patch_rejects_insert_same_parent_descending_without_mutating_tree() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_children("root", "column", vec![make_node("a", "text")]),
        });

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![
                make_patch_op(
                    "insert_child",
                    vec![],
                    serde_json::json!({
                        "index": 1,
                        "node": {"id": "b", "type": "text", "props": {}, "children": []}
                    }),
                ),
                make_patch_op(
                    "insert_child",
                    vec![],
                    serde_json::json!({
                        "index": 0,
                        "node": {"id": "c", "type": "text", "props": {}, "children": []}
                    }),
                ),
            ],
        });

        assert_eq!(child_ids(&core), vec!["a"]);
        assert!(has_patch_order_error(&effects));
        assert!(!has_sync_windows(&effects));
    }

    #[test]
    fn patch_valid_remove_update_insert_sequence_applies() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_children(
                "root",
                "column",
                vec![
                    make_node_with_props("a", "text", serde_json::json!({"content": "old"})),
                    make_node("b", "text"),
                    make_node("c", "text"),
                ],
            ),
        });

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![
                make_patch_op("remove_child", vec![], serde_json::json!({"index": 2})),
                make_patch_op(
                    "update_props",
                    vec![0],
                    serde_json::json!({"props": {"content": "new"}}),
                ),
                make_patch_op(
                    "insert_child",
                    vec![],
                    serde_json::json!({
                        "index": 1,
                        "node": {"id": "d", "type": "text", "props": {}, "children": []}
                    }),
                ),
            ],
        });

        assert_eq!(child_ids(&core), vec!["a", "d", "b"]);
        assert_eq!(
            core.tree.root().unwrap().children[0].props.to_value()["content"],
            "new"
        );
        assert!(!has_patch_order_error(&effects));
        assert!(has_sync_windows(&effects));
    }

    #[test]
    fn patch_allows_parent_update_before_child_remove() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_children(
                "root",
                "column",
                vec![make_node("a", "text"), make_node("b", "text")],
            ),
        });

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![
                make_patch_op(
                    "update_props",
                    vec![],
                    serde_json::json!({"props": {"spacing": 8}}),
                ),
                make_patch_op("remove_child", vec![], serde_json::json!({"index": 1})),
            ],
        });

        assert_eq!(child_ids(&core), vec!["a"]);
        assert_eq!(core.tree.root().unwrap().props.to_value()["spacing"], 8);
        assert!(!has_patch_order_error(&effects));
        assert!(has_sync_windows(&effects));
    }

    #[test]
    fn patch_allows_insert_in_one_subtree_before_update_in_another() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_children(
                "root",
                "column",
                vec![
                    make_node_with_children("left", "column", vec![]),
                    make_node_with_props("right", "text", serde_json::json!({"content": "old"})),
                ],
            ),
        });

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![
                make_patch_op(
                    "insert_child",
                    vec![0],
                    serde_json::json!({
                        "index": 0,
                        "node": {"id": "left-child", "type": "text", "props": {}, "children": []}
                    }),
                ),
                make_patch_op(
                    "update_props",
                    vec![1],
                    serde_json::json!({"props": {"content": "new"}}),
                ),
            ],
        });

        let root = core.tree.root().unwrap();
        assert_eq!(root.children[0].children[0].id, "left-child");
        assert_eq!(root.children[1].props.to_value()["content"], "new");
        assert!(!has_patch_order_error(&effects));
        assert!(has_sync_windows(&effects));
    }

    #[test]
    fn malformed_insert_still_uses_existing_per_op_error_handling() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_children(
                "root",
                "column",
                vec![make_node_with_props(
                    "a",
                    "text",
                    serde_json::json!({"content": "old"}),
                )],
            ),
        });

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![
                make_patch_op("insert_child", vec![], serde_json::json!({"index": 0})),
                make_patch_op(
                    "update_props",
                    vec![0],
                    serde_json::json!({"props": {"content": "new"}}),
                ),
            ],
        });

        assert_eq!(child_ids(&core), vec!["a"]);
        assert_eq!(
            core.tree.root().unwrap().children[0].props.to_value()["content"],
            "new"
        );
        assert!(!has_patch_order_error(&effects));
        assert!(has_sync_windows(&effects));
    }

    #[test]
    fn invalid_insert_node_still_uses_existing_per_op_error_handling() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_children(
                "root",
                "column",
                vec![make_node_with_props(
                    "a",
                    "text",
                    serde_json::json!({"content": "old"}),
                )],
            ),
        });

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![
                make_patch_op(
                    "insert_child",
                    vec![],
                    serde_json::json!({"index": 0, "node": {"garbage": true}}),
                ),
                make_patch_op(
                    "update_props",
                    vec![0],
                    serde_json::json!({"props": {"content": "new"}}),
                ),
            ],
        });

        assert_eq!(child_ids(&core), vec!["a"]);
        assert_eq!(
            core.tree.root().unwrap().children[0].props.to_value()["content"],
            "new"
        );
        assert!(!has_patch_order_error(&effects));
        assert!(has_sync_windows(&effects));
    }

    #[test]
    fn non_object_update_props_still_uses_existing_per_op_error_handling() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_children(
                "root",
                "column",
                vec![make_node_with_props(
                    "a",
                    "text",
                    serde_json::json!({"content": "old"}),
                )],
            ),
        });

        let effects = core.apply(IncomingMessage::Patch {
            ops: vec![
                make_patch_op(
                    "insert_child",
                    vec![],
                    serde_json::json!({
                        "index": 1,
                        "node": {"id": "b", "type": "text", "props": {}, "children": []}
                    }),
                ),
                make_patch_op("update_props", vec![0], serde_json::json!({"props": false})),
            ],
        });

        assert_eq!(child_ids(&core), vec!["a", "b"]);
        assert_eq!(
            core.tree.root().unwrap().children[0].props.to_value()["content"],
            "old"
        );
        assert!(!has_patch_order_error(&effects));
        assert!(has_sync_windows(&effects));
    }

    // -- Settings --

    #[test]
    fn settings_sets_default_text_size() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({"default_text_size": 18.0}),
        };
        core.apply(msg);
        assert_eq!(core.default_text_size, Some(18.0_f32));
    }

    #[test]
    fn settings_sets_default_font_monospace() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({"default_font": {"family": "monospace"}}),
        };
        core.apply(msg);
        assert_eq!(core.default_font, Some(iced::Font::MONOSPACE));
    }

    #[test]
    fn settings_sets_default_font_default_for_unknown_family() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({"default_font": {"family": "sans_serif"}}),
        };
        core.apply(msg);
        assert_eq!(core.default_font, Some(iced::Font::DEFAULT));
    }

    #[test]
    fn settings_sets_default_event_rate() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({"default_event_rate": 60}),
        };
        core.apply(msg);
        assert_eq!(core.default_event_rate, Some(60));
    }

    #[test]
    fn settings_validate_props_false_does_not_store_local_override() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({"validate_props": false}),
        };
        core.apply(msg);
        assert_eq!(core.validate_props, None);
        assert_eq!(
            core.is_validate_props_enabled(),
            runtime::is_validate_props_enabled()
        );
        if cfg!(debug_assertions) {
            assert!(core.is_validate_props_enabled());
            let effects = core.apply(IncomingMessage::Snapshot {
                tree: make_node_with_props("bad", "text", serde_json::json!({"content": 42})),
            });
            assert!(
                has_prop_validation(&effects, "bad"),
                "validate_props false must not suppress debug/default validation"
            );
        }
    }

    #[test]
    fn settings_validate_props_true_stores_local_override() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({"validate_props": true}),
        };
        core.apply(msg);
        assert_eq!(core.validate_props, Some(true));
        assert!(core.is_validate_props_enabled());
        let effects = core.apply(IncomingMessage::Snapshot {
            tree: make_node_with_props("bad", "text", serde_json::json!({"content": 42})),
        });
        assert!(
            has_prop_validation(&effects, "bad"),
            "validate_props true should enable validation for the session"
        );
    }

    #[test]
    fn settings_without_default_event_rate_leaves_none() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({"default_text_size": 14.0}),
        };
        core.apply(msg);
        assert_eq!(core.default_event_rate, None);
    }

    #[test]
    fn subscribe_with_max_rate_stores_rate_in_entry() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Subscribe {
            kind: "on_pointer_move".to_string(),
            tag: "mouse".to_string(),
            window_id: None,
            max_rate: Some(30),
        };
        core.apply(msg);
        let entries = &core.active_subscriptions["on_pointer_move"];
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].max_rate, Some(30));
    }

    #[test]
    fn subscribe_without_max_rate_has_none_rate() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Subscribe {
            kind: "on_key_press".to_string(),
            tag: "keys".to_string(),
            window_id: None,
            max_rate: None,
        };
        core.apply(msg);
        let entries = &core.active_subscriptions["on_key_press"];
        assert_eq!(entries[0].max_rate, None);
    }

    #[test]
    fn unsubscribe_removes_all_entries_for_kind() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Subscribe {
            kind: "on_pointer_move".to_string(),
            tag: "mouse".to_string(),
            window_id: None,
            max_rate: Some(30),
        });
        core.apply(IncomingMessage::Unsubscribe {
            kind: "on_pointer_move".to_string(),
            tag: None,
        });
        assert!(!core.active_subscriptions.contains_key("on_pointer_move"));
    }

    #[test]
    fn unsubscribe_by_tag_removes_specific_entry() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Subscribe {
            kind: "on_key_press".to_string(),
            tag: "global".to_string(),
            window_id: None,
            max_rate: None,
        });
        core.apply(IncomingMessage::Subscribe {
            kind: "on_key_press".to_string(),
            tag: "main_keys".to_string(),
            window_id: Some("main".to_string()),
            max_rate: None,
        });
        assert_eq!(core.active_subscriptions["on_key_press"].len(), 2);
        core.apply(IncomingMessage::Unsubscribe {
            kind: "on_key_press".to_string(),
            tag: Some("main_keys".to_string()),
        });
        let entries = &core.active_subscriptions["on_key_press"];
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tag, "global");
    }

    #[test]
    fn subscribe_with_window_id_stores_scope() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Subscribe {
            kind: "on_key_press".to_string(),
            tag: "main_keys".to_string(),
            window_id: Some("main".to_string()),
            max_rate: None,
        });
        let entries = &core.active_subscriptions["on_key_press"];
        assert_eq!(entries[0].window_id, Some("main".to_string()));
    }

    #[test]
    fn matching_entries_filters_by_window_id() {
        let mut core: Core = Core::new();
        core.apply(IncomingMessage::Subscribe {
            kind: "on_key_press".to_string(),
            tag: "global".to_string(),
            window_id: None,
            max_rate: None,
        });
        core.apply(IncomingMessage::Subscribe {
            kind: "on_key_press".to_string(),
            tag: "main_keys".to_string(),
            window_id: Some("main".to_string()),
            max_rate: None,
        });
        // Event from "main" window matches both global and main-scoped
        let main_entries = core.matching_entries("on_key_press", Some("main"));
        assert_eq!(main_entries.len(), 2);
        // Event from "popup" window matches only global
        let popup_entries = core.matching_entries("on_key_press", Some("popup"));
        assert_eq!(popup_entries.len(), 1);
        assert_eq!(popup_entries[0].tag, "global");
    }

    #[test]
    fn settings_without_widget_config_emits_null_config() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({"default_text_size": 14.0}),
        };
        let effects = core.apply(msg);
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            CoreEffect::StateChange(StateChange::WidgetConfig(serde_json::Value::Null))
        ));
    }

    #[test]
    fn settings_with_widget_config_emits_effect() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({
                "default_text_size": 14.0,
                "widget_config": {
                    "terminal": {"shell": "/bin/bash"}
                }
            }),
        };
        let effects = core.apply(msg);
        let has_ext_config = effects
            .iter()
            .any(|e| matches!(e, CoreEffect::StateChange(StateChange::WidgetConfig(_))));
        assert!(has_ext_config);
    }

    #[test]
    fn settings_with_widget_config_contains_correct_value() {
        let mut core: Core = Core::new();
        let config_val = serde_json::json!({"terminal": {"shell": "/bin/zsh"}});
        let msg = IncomingMessage::Settings {
            settings: serde_json::json!({
                "widget_config": config_val,
            }),
        };
        let effects = core.apply(msg);
        let ext_config = effects.iter().find_map(|e| match e {
            CoreEffect::StateChange(StateChange::WidgetConfig(v)) => Some(v),
            _ => None,
        });
        assert_eq!(
            ext_config.unwrap(),
            &serde_json::json!({"terminal": {"shell": "/bin/zsh"}})
        );
    }

    // -- Subscribe / Unsubscribe --

    #[test]
    fn subscription_register_adds_to_active_subscriptions() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Subscribe {
            kind: "time".to_string(),
            tag: "tick".to_string(),
            window_id: None,
            max_rate: None,
        };
        core.apply(msg);
        let entries = &core.active_subscriptions["time"];
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tag, "tick");
    }

    #[test]
    fn subscription_register_returns_no_effects() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Subscribe {
            kind: "keyboard".to_string(),
            tag: "key".to_string(),
            window_id: None,
            max_rate: None,
        };
        let effects = core.apply(msg);
        assert!(effects.is_empty());
    }

    #[test]
    fn subscription_unregister_removes_from_active_subscriptions() {
        let mut core: Core = Core::new();
        core.active_subscriptions
            .entry("time".to_string())
            .or_default()
            .push(SubscriptionEntry {
                tag: "tick".to_string(),
                window_id: None,
                max_rate: None,
            });
        let msg = IncomingMessage::Unsubscribe {
            kind: "time".to_string(),
            tag: None,
        };
        core.apply(msg);
        assert!(!core.active_subscriptions.contains_key("time"));
    }

    #[test]
    fn subscription_unregister_returns_no_effects() {
        let mut core: Core = Core::new();
        let msg = IncomingMessage::Unsubscribe {
            kind: "time".to_string(),
            tag: None,
        };
        let effects = core.apply(msg);
        assert!(effects.is_empty());
    }

    // -- Unhandled message types --

    #[test]
    fn unhandled_message_returns_empty_effects() {
        let mut core: Core = Core::new();
        // Query is handled by the scripting layer, not Core; hits the catch-all
        let msg = IncomingMessage::Query {
            id: "q1".to_string(),
            target: "tree".to_string(),
            selector: Value::Null,
        };
        let effects = core.apply(msg);
        assert!(effects.is_empty());
    }

    #[test]
    fn snapshot_clears_shared_state() {
        let mut core: Core = Core::new();

        // Test that shared state is cleared on snapshot.
        // Manually insert a value and verify it's cleared.
        core.caches
            .interpolated_props
            .insert("w1".into(), serde_json::Map::new());
        core.apply(IncomingMessage::Snapshot {
            tree: make_node("root", "column"),
        });
        assert!(core.caches.interpolated_props.is_empty());
    }

    // -- Multi-window sequence -----------------------------------------------

    fn make_window_node(id: &str) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: "window".to_string(),
            props: plushie_core::protocol::Props::default(),
            children: vec![],
        }
    }

    #[test]
    fn multi_window_snapshot_two_windows_produces_sync_windows() {
        let mut core: Core = Core::new();
        let mut root = make_node("root", "column");
        root.children.push(make_window_node("win-a"));
        root.children.push(make_window_node("win-b"));

        let effects = core.apply(IncomingMessage::Snapshot { tree: root });

        let has_sync = effects
            .iter()
            .any(|e| matches!(e, CoreEffect::StateChange(StateChange::SyncWindows)));
        assert!(has_sync, "Snapshot with windows should produce SyncWindows");

        // Verify the tree has both windows.
        let ids = core.tree.window_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"win-a".to_string()));
        assert!(ids.contains(&"win-b".to_string()));
    }

    #[test]
    fn multi_window_second_snapshot_removes_window() {
        let mut core: Core = Core::new();

        // First snapshot: two windows.
        let mut root1 = make_node("root", "column");
        root1.children.push(make_window_node("win-a"));
        root1.children.push(make_window_node("win-b"));
        core.apply(IncomingMessage::Snapshot { tree: root1 });
        assert_eq!(core.tree.window_ids().len(), 2);

        // Second snapshot: only one window.
        let mut root2 = make_node("root", "column");
        root2.children.push(make_window_node("win-a"));
        let effects = core.apply(IncomingMessage::Snapshot { tree: root2 });

        let has_sync = effects
            .iter()
            .any(|e| matches!(e, CoreEffect::StateChange(StateChange::SyncWindows)));
        assert!(has_sync, "Second Snapshot should produce SyncWindows");

        let ids = core.tree.window_ids();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], "win-a");
    }

    #[test]
    fn multi_window_snapshot_then_add_window_via_second_snapshot() {
        let mut core: Core = Core::new();

        // First: one window.
        let mut root1 = make_node("root", "column");
        root1.children.push(make_window_node("win-a"));
        core.apply(IncomingMessage::Snapshot { tree: root1 });
        assert_eq!(core.tree.window_ids().len(), 1);

        // Second: three windows.
        let mut root2 = make_node("root", "column");
        root2.children.push(make_window_node("win-a"));
        root2.children.push(make_window_node("win-b"));
        root2.children.push(make_window_node("win-c"));
        let effects = core.apply(IncomingMessage::Snapshot { tree: root2 });

        let has_sync = effects
            .iter()
            .any(|e| matches!(e, CoreEffect::StateChange(StateChange::SyncWindows)));
        assert!(has_sync);

        let ids = core.tree.window_ids();
        assert_eq!(ids.len(), 3);
    }

    // -- Duplicate node ID detection --

    #[test]
    fn snapshot_with_duplicate_ids_emits_error_event() {
        let mut core: Core = Core::new();
        let mut root = make_node("root", "column");
        root.children.push(make_node("dupe", "text"));
        root.children.push(make_node("dupe", "button"));

        let effects = core.apply(IncomingMessage::Snapshot { tree: root });
        let has_error = effects.iter().any(|e| match e {
            CoreEffect::Emit(Emit::Event(ev)) => ev.family == "error",
            _ => false,
        });
        assert!(has_error, "duplicate IDs should produce an error event");
        // Tree should still be accepted
        assert!(core.tree.root().is_some());
    }

    #[test]
    fn snapshot_without_duplicates_has_no_error_event() {
        let mut core: Core = Core::new();
        let mut root = make_node("root", "column");
        root.children.push(make_node("a", "text"));
        root.children.push(make_node("b", "button"));

        let effects = core.apply(IncomingMessage::Snapshot { tree: root });
        let has_error = effects.iter().any(|e| match e {
            CoreEffect::Emit(Emit::Event(ev)) => ev.family == "error",
            _ => false,
        });
        assert!(!has_error, "unique IDs should not produce an error event");
    }

    #[test]
    fn invalid_effect_payload_returns_error_without_dispatch() {
        let mut core = Core::new();

        let effects = core.apply(IncomingMessage::Effect {
            id: "req-1".to_string(),
            kind: "clipboard_write".to_string(),
            payload: serde_json::json!({}),
        });

        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::Dispatch(Dispatch::Effect {
                    request_id,
                    kind,
                    ..
                }) if request_id == "req-1" && kind == "clipboard_write"
            )
        }));
        let response = effects.iter().find_map(|effect| match effect {
            CoreEffect::Emit(Emit::EffectResponse(response)) => Some(response),
            _ => None,
        });
        assert!(matches!(
            response,
            Some(response)
                if response.id == "req-1"
                    && response.status == "error"
                    && response.error.as_deref()
                        == Some("missing required field for clipboard_write: text")
        ));
    }

    #[test]
    fn unknown_effect_kind_returns_error_without_dispatch() {
        let mut core = Core::new();

        let effects = core.apply(IncomingMessage::Effect {
            id: "req-1".to_string(),
            kind: "not_real".to_string(),
            payload: serde_json::json!({}),
        });

        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, CoreEffect::Dispatch(Dispatch::Effect { .. })))
        );
        let response = effects.iter().find_map(|effect| match effect {
            CoreEffect::Emit(Emit::EffectResponse(response)) => Some(response),
            _ => None,
        });
        assert!(matches!(
            response,
            Some(response)
                if response.id == "req-1"
                    && response.status == "error"
                    && response.error.as_deref() == Some("unknown effect kind: not_real")
        ));
    }

    #[test]
    fn effect_with_empty_id_emits_error_event_without_dispatch() {
        let mut core = Core::new();

        let effects = core.apply(IncomingMessage::Effect {
            id: String::new(),
            kind: "clipboard_write".to_string(),
            payload: serde_json::json!({"text": "hello"}),
        });

        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, CoreEffect::Dispatch(Dispatch::Effect { .. })))
        );
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::Emit(Emit::Event(event))
                    if event.family == "error" && event.id == "effect"
            )
        }));
    }

    #[test]
    fn unknown_effect_stub_kind_is_rejected_without_inserting() {
        let mut core = Core::new();

        let effects = core.apply(IncomingMessage::RegisterEffectStub {
            kind: "not_real".to_string(),
            response: serde_json::json!({"ok": true}),
        });

        assert!(!core.effect_stubs.contains_key("not_real"));
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::Emit(Emit::StubAck(ack))
                    if ack.kind == "not_real" && ack.status == "error"
            )
        }));
    }

    #[test]
    fn valid_effect_stub_registration_still_works() {
        let mut core = Core::new();

        let effects = core.apply(IncomingMessage::RegisterEffectStub {
            kind: "clipboard_read".to_string(),
            response: serde_json::json!({"text": "stubbed"}),
        });

        assert_eq!(
            core.effect_stubs.get("clipboard_read"),
            Some(&serde_json::json!({"text": "stubbed"}))
        );
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::Emit(Emit::StubAck(ack))
                    if ack.kind == "clipboard_read" && ack.status == "registered"
            )
        }));
    }

    #[test]
    fn valid_effect_stub_intercepts_valid_effect_request() {
        let mut core = Core::new();
        core.apply(IncomingMessage::RegisterEffectStub {
            kind: "clipboard_write".to_string(),
            response: serde_json::json!({"stubbed": true}),
        });

        let effects = core.apply(IncomingMessage::Effect {
            id: "req-1".to_string(),
            kind: "clipboard_write".to_string(),
            payload: serde_json::json!({"text": "hello"}),
        });

        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, CoreEffect::Dispatch(Dispatch::Effect { .. })))
        );
        let response = effects.iter().find_map(|effect| match effect {
            CoreEffect::Emit(Emit::EffectResponse(response)) => Some(response),
            _ => None,
        });
        assert!(matches!(
            response,
            Some(response)
                if response.id == "req-1"
                    && response.status == "ok"
                    && response.result.as_ref() == Some(&serde_json::json!({"stubbed": true}))
        ));
    }

    #[test]
    fn unknown_effect_stub_unregister_is_rejected_without_mutating_stubs() {
        let mut core = Core::new();
        core.effect_stubs.insert(
            "clipboard_read".to_string(),
            serde_json::json!({"text": "stubbed"}),
        );

        let effects = core.apply(IncomingMessage::UnregisterEffectStub {
            kind: "not_real".to_string(),
        });

        assert!(core.effect_stubs.contains_key("clipboard_read"));
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                CoreEffect::Emit(Emit::StubAck(ack))
                    if ack.kind == "not_real" && ack.status == "error"
            )
        }));
    }
}
