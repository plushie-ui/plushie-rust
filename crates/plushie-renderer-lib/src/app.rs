//! Application struct and core utility methods.
//!
//! Defines the [`App`] struct (the iced daemon's state) and the methods
//! that the rest of the renderer uses to query window titles, themes,
//! scale factors, and emit subscription events.

use std::sync::Arc;

use iced::{Task, Theme, keyboard, window};

use plushie_widget_sdk::message::Message;
use plushie_widget_sdk::protocol::OutgoingEvent;
use plushie_widget_sdk::registry::WidgetRegistry;

use crate::constants::*;
use crate::effects::EffectHandler;
use crate::emitter::{CoalesceKey, EventEmitter};
use crate::emitters::SinkMutex;
use crate::window_map;

/// Validate and clamp a scale factor. Returns 1.0 for invalid values
/// (zero, negative, NaN, infinity).
pub fn validate_scale_factor(sf: f32) -> f32 {
    if sf <= 0.0 || !sf.is_finite() {
        log::warn!("invalid scale_factor {sf}, using 1.0");
        1.0
    } else {
        sf
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

/// The iced daemon application. Owns the rendering engine, window
/// state, widget registry, and all runtime state needed to translate
/// between the wire protocol and iced's update/view cycle.
pub struct App {
    pub core: plushie_widget_sdk::engine::Core,
    pub theme: Theme,
    /// Widget ops and effects return iced Tasks, but `apply()` doesn't
    /// return them. They accumulate here and are drained via `Task::batch`
    /// in `update()` after `apply()` returns.
    pub pending_tasks: Vec<Task<Message>>,
    /// Bidirectional plushie ID <-> iced window ID mapping with per-window state.
    pub windows: window_map::WindowMap,
    /// In-memory image handles for use by Image widgets and canvas draw.
    pub image_registry: plushie_widget_sdk::image_registry::ImageRegistry,
    /// Current system theme, tracked via ThemeChanged subscription.
    pub system_theme: Theme,
    /// True when the app-level theme is "system" (follow OS preference).
    pub theme_follows_system: bool,
    /// Global scale factor multiplier (1.0 = follow OS DPI).
    pub scale_factor: f32,
    /// Unified widget registry. All widget types are dispatched through
    /// this registry.
    pub registry: WidgetRegistry,
    /// Epoch for animation_frame timestamp calculation.
    pub animation_epoch: Option<iced::time::Instant>,
    /// Rate-limited event emitter with coalescing.
    pub emitter: EventEmitter,
    /// Platform-specific effect handler injected at construction.
    /// Native and WASM crates each provide their own [`EffectHandler`]
    /// implementation.
    pub effect_handler: Box<dyn EffectHandler>,
    /// Renderer-side animation manager. Tracks transitions, springs,
    /// and exit ghosts. Advances on frame ticks and writes interpolated
    /// values to SharedState.interpolated_props.
    pub transition_manager: plushie_widget_sdk::animation::TransitionManager,
    /// Current keyboard modifier state, updated on every ModifiersChanged
    /// event. Included on all outgoing pointer events.
    pub current_modifiers: keyboard::Modifiers,
    /// Wire protocol codec. Used for encoding stub acks and scripting
    /// responses. Stored here so these paths don't need the global.
    pub codec: plushie_widget_sdk::codec::Codec,
}

impl App {
    pub fn new(
        registry: WidgetRegistry,
        effect_handler: Box<dyn EffectHandler>,
        sink: Arc<SinkMutex>,
    ) -> Self {
        Self {
            core: plushie_widget_sdk::engine::Core::new(),
            theme: DEFAULT_THEME,
            pending_tasks: Vec::new(),
            windows: window_map::WindowMap::new(),
            image_registry: plushie_widget_sdk::image_registry::ImageRegistry::new(),
            system_theme: DEFAULT_THEME,
            theme_follows_system: false,
            scale_factor: 1.0,
            registry,
            animation_epoch: None,
            emitter: EventEmitter::new(sink),
            effect_handler,
            transition_manager: plushie_widget_sdk::animation::TransitionManager::new(),
            current_modifiers: keyboard::Modifiers::default(),
            codec: plushie_widget_sdk::codec::Codec::MsgPack,
        }
    }

    /// Set the wire protocol codec. Called during startup after
    /// codec negotiation. Defaults to MsgPack.
    pub fn set_codec(&mut self, codec: plushie_widget_sdk::codec::Codec) {
        self.codec = codec;
    }

    pub fn title_for_window(&self, iced_id: window::Id) -> String {
        if let Some(window_id) = self.windows.get_window_id(&iced_id)
            && let Some(node) = self.core.tree.find_window(window_id)
            && let Some(title) = node.props.get("title").and_then(|v| v.as_str())
        {
            return title.chars().filter(|c| !c.is_control()).collect();
        }
        DEFAULT_WINDOW_TITLE.to_string()
    }

    pub fn theme_for_window(&self, iced_id: window::Id) -> Theme {
        self.theme_ref_for_window(iced_id).clone()
    }

    pub fn theme_ref_for_window(&self, iced_id: window::Id) -> &Theme {
        if let Some(window_id) = self.windows.get_window_id(&iced_id)
            && let Some(cached) = self.windows.cached_theme(window_id)
        {
            return cached;
        }
        if self.theme_follows_system {
            &self.system_theme
        } else {
            &self.theme
        }
    }

    pub fn scale_factor_for_window(&self, iced_id: window::Id) -> f32 {
        let window_id = self.windows.get_window_id(&iced_id);

        // Per-window override from WindowState (set via window open/update ops).
        if let Some(sf) = window_id.and_then(|jid| self.windows.scale_factor(jid)) {
            return validate_scale_factor(sf);
        }

        // Fall back to the tree node's scale_factor prop.
        let sf = window_id
            .and_then(|jid| self.core.tree.find_window(jid))
            .and_then(|node| node.props.get("scale_factor"))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(self.scale_factor);
        validate_scale_factor(sf)
    }

    /// Emit a subscription event to all matching entries (specific kind +
    /// catch-all SUB_EVENT), filtered by window_id. The event_fn is called
    /// once per matching entry with the entry's tag.
    pub fn emit_subscription(
        &self,
        key: &str,
        captured: bool,
        event_fn: impl Fn(String) -> OutgoingEvent,
    ) -> Task<Message> {
        self.emit_subscription_for_window(key, None, captured, event_fn)
    }

    /// Emit a subscription event scoped to a specific window.
    pub fn emit_subscription_for_window(
        &self,
        key: &str,
        window_id: Option<&str>,
        captured: bool,
        event_fn: impl Fn(String) -> OutgoingEvent,
    ) -> Task<Message> {
        let entries = self
            .core
            .matching_entries_with_catchall(key, SUB_EVENT, window_id);
        // Fast paths for the common 0- and 1-entry cases avoid
        // allocating a `Vec` and a `Task::batch` per event.
        match entries.len() {
            0 => Task::none(),
            1 => {
                let entry = &entries[0];
                self.emitter
                    .emit_direct(event_fn(entry.tag.clone()).with_captured(captured))
            }
            _ => {
                let tasks: Vec<_> = entries
                    .into_iter()
                    .map(|entry| {
                        self.emitter
                            .emit_direct(event_fn(entry.tag.clone()).with_captured(captured))
                    })
                    .collect();
                Task::batch(tasks)
            }
        }
    }

    pub fn lookup_widget_event_rate(&self, widget_id: &str) -> Option<u32> {
        let node = self.core.tree.find_by_id(widget_id)?;
        node.props
            .get("event_rate")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
    }

    /// Coalesce a subscription event for all matching entries.
    pub fn coalesce_subscription(
        &mut self,
        key: &str,
        captured: bool,
        event_fn: impl Fn(String) -> OutgoingEvent,
    ) -> Task<Message> {
        self.coalesce_subscription_for_window(key, None, captured, event_fn)
    }

    /// Coalesce a subscription event scoped to a specific window.
    /// Each matching entry gets its own coalesce buffer (keyed by tag)
    /// so rate limiting is isolated per subscription entry.
    pub fn coalesce_subscription_for_window(
        &mut self,
        key: &str,
        window_id: Option<&str>,
        captured: bool,
        event_fn: impl Fn(String) -> OutgoingEvent,
    ) -> Task<Message> {
        let entries = self
            .core
            .matching_entries_with_catchall(key, SUB_EVENT, window_id);
        // Fast paths for the common 0- and 1-entry cases avoid
        // allocating a `Vec` and a `Task::batch` per high-frequency
        // event (cursor move, scroll, etc.).
        match entries.len() {
            0 => Task::none(),
            1 => {
                let entry = &entries[0];
                let event = event_fn(entry.tag.clone()).with_captured(captured);
                self.emitter
                    .coalesce(CoalesceKey::Subscription(entry.tag.clone()), event)
            }
            _ => {
                let tasks: Vec<_> = entries
                    .into_iter()
                    .map(|entry| {
                        let event = event_fn(entry.tag.clone()).with_captured(captured);
                        self.emitter
                            .coalesce(CoalesceKey::Subscription(entry.tag.clone()), event)
                    })
                    .collect();
                Task::batch(tasks)
            }
        }
    }

    /// Route a [`Message`] to every widget with an active subscription
    /// for `kind` (optionally scoped to `window_id`) and emit the
    /// resulting outgoing events.
    ///
    /// Fast path: returns [`Task::none`] when no widget cares about
    /// `kind`, so handlers can call this unconditionally without
    /// cloning message payloads in the common case.
    pub fn dispatch_widget_subscription(
        &mut self,
        kind: &str,
        window_id: Option<&str>,
        msg: &Message,
    ) -> Task<Message> {
        if !self.registry.has_widget_subscription(kind) {
            return Task::none();
        }
        let events = self
            .registry
            .dispatch_widget_subscription(kind, window_id, msg);
        if events.is_empty() {
            return Task::none();
        }
        let tasks: Vec<_> = events
            .into_iter()
            .map(|event| self.emitter.emit_immediate(event))
            .collect();
        Task::batch(tasks)
    }
}
