//! Outgoing wire messages: events and response types.
//!
//! [`OutgoingEvent`] is the main event struct emitted by the renderer.
//! Response types ([`EffectResponse`], [`QueryResponse`], etc.) are
//! serialized in reply to incoming messages.

use serde::Serialize;
use serde_json::Value;

/// Hint for the renderer's event coalescing system.
///
/// Set by event constructors or widget authors via
/// [`OutgoingEvent::with_coalesce`]. The renderer uses this to decide
/// whether and how to buffer events during rate-limited delivery. Not
/// serialized to the wire (renderer-internal metadata).
///
/// # For widget authors
///
/// Set on events returned from `handle_message()`:
///
/// ```ignore
/// let event = OutgoingEvent::widget_event("cursor_pos", node_id, value)
///     .with_coalesce(CoalesceHint::Replace);
/// ```
///
/// Events without a hint are always delivered immediately (never
/// rate-limited), regardless of `event_rate` or `default_event_rate`.
#[derive(Debug, Clone, PartialEq)]
pub enum CoalesceHint {
    /// Keep the latest event, discard intermediates.
    /// Use for: position reports, state snapshots, progress values --
    /// anything where only the most recent value matters.
    Replace,
    /// Sum the named `value` fields across coalesced events.
    /// Other fields keep the latest event's values.
    /// Use for: scroll deltas, velocity changes, counters, anything
    /// where intermediate values carry magnitude that would be lost
    /// if only the latest were kept.
    Accumulate(Vec<String>),
}

/// An event written to stdout by the renderer.
///
/// All events share a flat struct with optional fields. There are two
/// constructor patterns:
///
/// - **Widget events** (click, input, toggle, etc.) use `id` to identify
///   the source widget. Built via the internal `bare()` constructor.
/// - **Subscription events** (key_press, cursor_moved, window_opened,
///   etc.) use `tag` to identify the subscription that requested them.
///   Built via the internal `tagged()` constructor. The `id` field is empty.
///
/// Widget authors emit custom events via
/// [`widget_event`](Self::widget_event).
#[derive(Debug, Serialize)]
pub struct OutgoingEvent {
    /// Always `"event"`.
    #[serde(rename = "type")]
    pub message_type: &'static str,
    /// Session that produced this event.
    pub session: String,
    /// Event type (e.g. `"click"`, `"key_press"`, `"window_opened"`).
    pub family: String,
    /// Source widget node ID (widget events) or empty (subscription events).
    pub id: String,
    /// Primary value payload (e.g. input text, slider value, selected option,
    /// or structured data for pointer/window/IME events).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Subscription tag identifying which subscription requested this event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Keyboard modifier state at the time of the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<KeyModifiers>,
    /// Whether the event was captured (consumed) by an iced widget before
    /// reaching the subscription listener. Present on keyboard, mouse,
    /// touch, and IME events; absent on widget-level events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured: Option<bool>,
    /// Coalescing hint for rate-limited delivery.
    /// Not serialized to the wire (renderer-internal metadata).
    ///
    /// Kept private so nothing outside this crate can serialize or copy
    /// the field into a typed wire message by accident. Set via
    /// [`with_coalesce`](Self::with_coalesce), observe via
    /// [`coalesce_hint`](Self::coalesce_hint).
    #[serde(skip)]
    pub(crate) coalesce: Option<CoalesceHint>,
}

impl OutgoingEvent {
    /// Mark the event with its capture status.
    pub fn with_captured(mut self, captured: bool) -> Self {
        self.captured = Some(captured);
        self
    }

    /// Set the session ID for this event.
    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = session.into();
        self
    }

    /// Declare that this event can be coalesced during rate-limited
    /// delivery. Without this hint, the event is always delivered
    /// immediately regardless of rate settings.
    pub fn with_coalesce(mut self, hint: CoalesceHint) -> Self {
        self.coalesce = Some(hint);
        self
    }

    /// Current coalesce hint, if any.
    ///
    /// Exposed for the renderer's event buffering pipeline; not part of
    /// the wire protocol.
    pub fn coalesce_hint(&self) -> Option<&CoalesceHint> {
        self.coalesce.as_ref()
    }

    /// Consume the coalesce hint (renderer-internal).
    pub fn take_coalesce(&mut self) -> Option<CoalesceHint> {
        self.coalesce.take()
    }

    /// Set the primary `value` field on this event.
    ///
    /// For built-in widget events, `value` carries the widget's primary
    /// datum (input text, slider position, selected option). Widget
    /// authors wrapping built-in widgets can use this to emit events
    /// compatible with the built-in shape:
    ///
    /// ```ignore
    /// OutgoingEvent::widget_event("input", id, value)
    ///     .with_value(serde_json::Value::String(text))
    /// ```
    pub fn with_value(mut self, value: Value) -> Self {
        self.value = Some(value);
        self
    }
}

/// Serializable representation of keyboard modifiers.
///
/// All fields default to `false`, so partial JSON like `{"shift": true}`
/// deserializes correctly with unset modifiers left as `false`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct KeyModifiers {
    #[serde(default)]
    /// Whether Shift is held.
    pub shift: bool,
    #[serde(default)]
    /// Whether Control is held.
    pub ctrl: bool,
    #[serde(default)]
    /// Whether Alt is held.
    pub alt: bool,
    #[serde(default)]
    /// Whether the Super/Command key is held.
    pub logo: bool,
    #[serde(default)]
    /// Whether the Command key is held (macOS).
    pub command: bool,
}

// ---------------------------------------------------------------------------
// Widget events (click, input, toggle, slide, select, submit)
// ---------------------------------------------------------------------------

impl OutgoingEvent {
    /// Helper to build a bare event with only the common fields.
    fn bare(family: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            message_type: "event",
            session: String::new(),
            family: family.into(),
            id: id.into(),
            value: None,
            tag: None,
            modifiers: None,
            captured: None,
            coalesce: None,
        }
    }

    /// Helper to build a subscription-tagged event with no widget id.
    pub fn tagged(family: impl Into<String>, tag: String) -> Self {
        Self {
            message_type: "event",
            session: String::new(),
            family: family.into(),
            id: String::new(),
            value: None,
            tag: Some(tag),
            modifiers: None,
            captured: None,
            coalesce: None,
        }
    }

    /// Generic widget event with a family string and optional value payload.
    /// Used for on_open, on_close, sort, and other events.
    pub fn generic(family: impl Into<String>, id: impl Into<String>, value: Option<Value>) -> Self {
        Self {
            value,
            ..Self::bare(family, id)
        }
    }

    /// Convenience constructor for widget-emitted events.
    ///
    /// Identical to [`generic`](Self::generic); exists for discoverability
    /// so widget authors searching docs for "widget" find it.
    pub fn widget_event(
        family: impl Into<String>,
        id: impl Into<String>,
        value: Option<Value>,
    ) -> Self {
        Self::generic(family, id, value)
    }

    /// Set or construct `click`.
    pub fn click(id: String) -> Self {
        Self::bare("click", id)
    }

    /// Set or construct `input`.
    pub fn input(id: String, value: String) -> Self {
        Self {
            value: Some(Value::String(value)),
            ..Self::bare("input", id)
        }
    }

    /// Set or construct `submit`.
    pub fn submit(id: String, value: String) -> Self {
        Self {
            value: Some(Value::String(value)),
            ..Self::bare("submit", id)
        }
    }

    /// Set or construct `toggle`.
    pub fn toggle(id: String, checked: bool) -> Self {
        Self {
            value: Some(Value::Bool(checked)),
            ..Self::bare("toggle", id)
        }
    }

    /// Set or construct `slide`.
    pub fn slide(id: String, value: f64) -> Self {
        Self {
            value: Some(serde_json::json!(sanitize_f64(value))),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::bare("slide", id)
        }
    }

    /// Set or construct `slide_release`.
    pub fn slide_release(id: String, value: f64) -> Self {
        Self {
            value: Some(serde_json::json!(sanitize_f64(value))),
            ..Self::bare("slide_release", id)
        }
    }

    /// Set or construct `select`.
    pub fn select(id: String, value: String) -> Self {
        Self {
            value: Some(Value::String(value)),
            ..Self::bare("select", id)
        }
    }

    // -----------------------------------------------------------------------
    // Keyboard events
    //
    // key_press and key_release constructors that depend on iced types
    // (KeyEventData) are defined in plushie-widget-sdk, not here.
    // -----------------------------------------------------------------------

    /// Set or construct `modifiers_changed`.
    pub fn modifiers_changed(tag: String, modifiers: KeyModifiers) -> Self {
        Self {
            modifiers: Some(modifiers),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::tagged("modifiers_changed", tag)
        }
    }

    // -----------------------------------------------------------------------
    // Mouse events
    // -----------------------------------------------------------------------

    /// Set or construct `cursor_moved`.
    pub fn cursor_moved(tag: String, x: f32, y: f32) -> Self {
        Self {
            value: Some(serde_json::json!({"x": sanitize_f32(x), "y": sanitize_f32(y)})),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::tagged("cursor_moved", tag)
        }
    }

    /// Set or construct `cursor_entered`.
    pub fn cursor_entered(tag: String) -> Self {
        Self::tagged("cursor_entered", tag)
    }

    /// Set or construct `cursor_left`.
    pub fn cursor_left(tag: String) -> Self {
        Self::tagged("cursor_left", tag)
    }

    /// Set or construct `button_pressed`.
    pub fn button_pressed(tag: String, button: String) -> Self {
        Self {
            value: Some(Value::String(button)),
            ..Self::tagged("button_pressed", tag)
        }
    }

    /// Set or construct `button_released`.
    pub fn button_released(tag: String, button: String) -> Self {
        Self {
            value: Some(Value::String(button)),
            ..Self::tagged("button_released", tag)
        }
    }

    /// Set or construct `wheel_scrolled`.
    pub fn wheel_scrolled(tag: String, delta_x: f32, delta_y: f32, unit: &str) -> Self {
        Self {
            value: Some(serde_json::json!({
                "delta_x": sanitize_f32(delta_x),
                "delta_y": sanitize_f32(delta_y),
                "unit": unit,
            })),
            coalesce: Some(CoalesceHint::Accumulate(vec![
                "delta_x".into(),
                "delta_y".into(),
            ])),
            ..Self::tagged("wheel_scrolled", tag)
        }
    }

    // -----------------------------------------------------------------------
    // Touch events
    // -----------------------------------------------------------------------

    fn touch_event(family: &str, tag: String, finger_id: u64, x: f32, y: f32) -> Self {
        Self {
            value: Some(serde_json::json!({
                "id": finger_id,
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
            })),
            ..Self::tagged(family, tag)
        }
    }

    /// Set or construct `finger_pressed`.
    pub fn finger_pressed(tag: String, finger_id: u64, x: f32, y: f32) -> Self {
        Self::touch_event("finger_pressed", tag, finger_id, x, y)
    }

    /// Set or construct `finger_moved`.
    pub fn finger_moved(tag: String, finger_id: u64, x: f32, y: f32) -> Self {
        Self {
            coalesce: Some(CoalesceHint::Replace),
            ..Self::touch_event("finger_moved", tag, finger_id, x, y)
        }
    }

    /// Set or construct `finger_lifted`.
    pub fn finger_lifted(tag: String, finger_id: u64, x: f32, y: f32) -> Self {
        Self::touch_event("finger_lifted", tag, finger_id, x, y)
    }

    /// Set or construct `finger_lost`.
    pub fn finger_lost(tag: String, finger_id: u64, x: f32, y: f32) -> Self {
        Self::touch_event("finger_lost", tag, finger_id, x, y)
    }

    // -----------------------------------------------------------------------
    // IME events
    // -----------------------------------------------------------------------

    /// Set or construct `ime_opened`.
    pub fn ime_opened(tag: String) -> Self {
        Self::tagged("ime_opened", tag)
    }

    /// Set or construct `ime_preedit`.
    pub fn ime_preedit(tag: String, text: String, cursor: Option<std::ops::Range<usize>>) -> Self {
        let cursor_val = cursor
            .map(|r| serde_json::json!({"start": r.start, "end": r.end}))
            .unwrap_or(serde_json::Value::Null);
        Self {
            value: Some(serde_json::json!({"text": text, "cursor": cursor_val})),
            ..Self::tagged("ime_preedit", tag)
        }
    }

    /// Set or construct `ime_commit`.
    pub fn ime_commit(tag: String, text: String) -> Self {
        Self {
            value: Some(serde_json::json!({"text": text})),
            ..Self::tagged("ime_commit", tag)
        }
    }

    /// Set or construct `ime_closed`.
    pub fn ime_closed(tag: String) -> Self {
        Self::tagged("ime_closed", tag)
    }

    // -----------------------------------------------------------------------
    // Window lifecycle events
    // -----------------------------------------------------------------------

    /// Set or construct `window_opened`.
    pub fn window_opened(
        tag: String,
        window_id: String,
        position: Option<(f32, f32)>,
        width: f32,
        height: f32,
        scale_factor: f32,
    ) -> Self {
        let pos =
            position.map(|(x, y)| serde_json::json!({"x": sanitize_f32(x), "y": sanitize_f32(y)}));
        Self {
            value: Some(serde_json::json!({
                "window_id": window_id,
                "position": pos,
                "width": sanitize_f32(width),
                "height": sanitize_f32(height),
                "scale_factor": sanitize_f32(scale_factor),
            })),
            ..Self::tagged("window_opened", tag)
        }
    }

    /// Window event carrying only a window_id in its value payload.
    fn window_event(family: &str, tag: String, window_id: String) -> Self {
        Self {
            value: Some(serde_json::json!({"window_id": window_id})),
            ..Self::tagged(family, tag)
        }
    }

    /// Set or construct `window_closed`.
    pub fn window_closed(tag: String, window_id: String) -> Self {
        Self::window_event("window_closed", tag, window_id)
    }

    /// Set or construct `window_close_requested`.
    pub fn window_close_requested(tag: String, window_id: String) -> Self {
        Self::window_event("window_close_requested", tag, window_id)
    }

    /// Set or construct `window_moved`.
    pub fn window_moved(tag: String, window_id: String, x: f32, y: f32) -> Self {
        Self {
            value: Some(serde_json::json!({
                "window_id": window_id,
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
            })),
            ..Self::tagged("window_moved", tag)
        }
    }

    /// Set or construct `window_resized`.
    pub fn window_resized(tag: String, window_id: String, width: f32, height: f32) -> Self {
        Self {
            value: Some(serde_json::json!({
                "window_id": window_id,
                "width": sanitize_f32(width),
                "height": sanitize_f32(height),
            })),
            ..Self::tagged("window_resized", tag)
        }
    }

    /// Set or construct `window_focused`.
    pub fn window_focused(tag: String, window_id: String) -> Self {
        Self::window_event("window_focused", tag, window_id)
    }

    /// Set or construct `window_unfocused`.
    pub fn window_unfocused(tag: String, window_id: String) -> Self {
        Self::window_event("window_unfocused", tag, window_id)
    }

    /// Set or construct `window_rescaled`.
    pub fn window_rescaled(tag: String, window_id: String, scale_factor: f32) -> Self {
        Self {
            value: Some(serde_json::json!({
                "window_id": window_id,
                "scale_factor": sanitize_f32(scale_factor),
            })),
            ..Self::tagged("window_rescaled", tag)
        }
    }

    /// Set or construct `file_hovered`.
    pub fn file_hovered(tag: String, window_id: String, path: String) -> Self {
        Self {
            value: Some(serde_json::json!({
                "window_id": window_id,
                "path": path,
            })),
            ..Self::tagged("file_hovered", tag)
        }
    }

    /// Set or construct `file_dropped`.
    pub fn file_dropped(tag: String, window_id: String, path: String) -> Self {
        Self {
            value: Some(serde_json::json!({
                "window_id": window_id,
                "path": path,
            })),
            ..Self::tagged("file_dropped", tag)
        }
    }

    /// Set or construct `files_hovered_left`.
    pub fn files_hovered_left(tag: String, window_id: String) -> Self {
        Self::window_event("files_hovered_left", tag, window_id)
    }

    // -----------------------------------------------------------------------
    // Animation / theme / system events
    // -----------------------------------------------------------------------

    /// Set or construct `animation_frame`.
    pub fn animation_frame(tag: String, timestamp_millis: u128) -> Self {
        Self {
            value: Some(serde_json::json!({"timestamp": timestamp_millis})),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::tagged("animation_frame", tag)
        }
    }

    /// Set or construct `theme_changed`.
    pub fn theme_changed(tag: String, mode: String) -> Self {
        Self {
            value: Some(Value::String(mode)),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::tagged("theme_changed", tag)
        }
    }

    /// Renderer-side validation diagnostic.
    ///
    /// The `id` field on the event envelope is set to `canvas_id` for
    /// consistency with other canvas events. The `value` payload carries
    /// the full diagnostic detail including the optional `element_id`.
    pub fn diagnostic(
        canvas_id: String,
        element_id: Option<String>,
        level: &str,
        code: &str,
        message: &str,
    ) -> Self {
        Self {
            value: Some(serde_json::json!({
                "level": level,
                "element_id": element_id,
                "code": code,
                "message": message,
            })),
            ..Self::bare("diagnostic", canvas_id)
        }
    }

    // -----------------------------------------------------------------------
    // PaneGrid events
    // -----------------------------------------------------------------------

    /// Set or construct `pane_resized`.
    pub fn pane_resized(id: String, split: String, ratio: f32) -> Self {
        Self {
            value: Some(serde_json::json!({"split": split, "ratio": sanitize_f32(ratio)})),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::bare("pane_resized", id)
        }
    }

    /// Set or construct `pane_dragged`.
    pub fn pane_dragged(
        id: String,
        kind: &str,
        pane: String,
        target: Option<String>,
        region: Option<&str>,
        edge: Option<&str>,
    ) -> Self {
        let mut val = serde_json::json!({"action": kind, "pane": pane});
        if let Some(t) = target {
            val["target"] = serde_json::json!(t);
        }
        if let Some(r) = region {
            val["region"] = serde_json::json!(r);
        }
        if let Some(e) = edge {
            val["edge"] = serde_json::json!(e);
        }
        Self {
            value: Some(val),
            ..Self::bare("pane_dragged", id)
        }
    }

    /// Set or construct `pane_clicked`.
    pub fn pane_clicked(id: String, pane: String) -> Self {
        Self {
            value: Some(serde_json::json!({"pane": pane})),
            ..Self::bare("pane_clicked", id)
        }
    }

    /// Set or construct `pane_focus_cycle`.
    pub fn pane_focus_cycle(id: String, pane: String) -> Self {
        Self {
            value: Some(serde_json::json!({"pane": pane})),
            ..Self::bare("pane_focus_cycle", id)
        }
    }

    // -----------------------------------------------------------------------
    // TextInput paste event
    // -----------------------------------------------------------------------

    /// Set or construct `paste`.
    pub fn paste(id: String, text: String) -> Self {
        Self {
            value: Some(Value::String(text)),
            ..Self::bare("paste", id)
        }
    }

    // -----------------------------------------------------------------------
    // Scripting key events (no full KeyEventData available)
    // -----------------------------------------------------------------------

    /// Key press event from scripting (no full KeyEventData).
    ///
    /// Produces the same event shape as real key_press events: `key` in
    /// `value.key`, modifiers in the top-level `modifiers` field. Missing
    /// modifier fields default to `false`.
    pub fn scripting_key_press(key: String, modifiers_json: Value) -> Self {
        let mods: KeyModifiers =
            serde_json::from_value(modifiers_json).unwrap_or(KeyModifiers::default());
        Self {
            modifiers: Some(mods),
            value: Some(serde_json::json!({"key": key})),
            ..Self::bare("key_press", String::new())
        }
    }

    /// Key release event from scripting (no full KeyEventData).
    ///
    /// Produces the same event shape as real key_release events: `key` in
    /// `value.key`, modifiers in the top-level `modifiers` field. Missing
    /// modifier fields default to `false`.
    pub fn scripting_key_release(key: String, modifiers_json: Value) -> Self {
        let mods: KeyModifiers =
            serde_json::from_value(modifiers_json).unwrap_or(KeyModifiers::default());
        Self {
            modifiers: Some(mods),
            value: Some(serde_json::json!({"key": key})),
            ..Self::bare("key_release", String::new())
        }
    }

    /// Cursor moved event from scripting.
    ///
    /// Uses `f32` to match the real `cursor_moved` event shape (see
    /// [`Self::cursor_moved`]). Scripting has no precision requirement f64
    /// meets but f32 doesn't.
    pub fn scripting_cursor_moved(x: f32, y: f32) -> Self {
        Self {
            value: Some(serde_json::json!({
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
            })),
            ..Self::bare("cursor_moved", String::new())
        }
    }

    /// Scroll event from scripting.
    ///
    /// Uses `f32` to match the real `wheel_scrolled` event shape (see
    /// [`Self::wheel_scrolled`]).
    pub fn scripting_scroll(delta_x: f32, delta_y: f32) -> Self {
        Self {
            value: Some(serde_json::json!({
                "delta_x": sanitize_f32(delta_x),
                "delta_y": sanitize_f32(delta_y),
                "unit": "pixel",
            })),
            ..Self::bare("wheel_scrolled", String::new())
        }
    }

    // -----------------------------------------------------------------------
    // ComboBox option hovered event
    // -----------------------------------------------------------------------

    /// Set or construct `option_hovered`.
    pub fn option_hovered(id: String, value: String) -> Self {
        Self {
            value: Some(Value::String(value)),
            ..Self::bare("option_hovered", id)
        }
    }

    // -----------------------------------------------------------------------
    // Scrollable events
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    /// Set or construct `scroll`.
    pub fn scroll(
        id: String,
        abs_x: f32,
        abs_y: f32,
        rel_x: f32,
        rel_y: f32,
        bounds_w: f32,
        bounds_h: f32,
        content_w: f32,
        content_h: f32,
    ) -> Self {
        Self {
            value: Some(serde_json::json!({
                "absolute_x": sanitize_f32(abs_x), "absolute_y": sanitize_f32(abs_y),
                "relative_x": sanitize_f32(rel_x), "relative_y": sanitize_f32(rel_y),
                "bounds_width": sanitize_f32(bounds_w), "bounds_height": sanitize_f32(bounds_h),
                "content_width": sanitize_f32(content_w), "content_height": sanitize_f32(content_h),
            })),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::bare("scrolled", id)
        }
    }

    // -----------------------------------------------------------------------
    // Unified pointer events
    //
    // These constructors produce events with unified families ("press",
    // "release", "move", "scroll", "enter", "exit", "double_click",
    // "resize") that carry pointer_type, finger ID, coordinates, and
    // modifier state.
    // -----------------------------------------------------------------------

    /// Build a modifiers object for inclusion in pointer event values.
    fn modifiers_data(modifiers: &KeyModifiers) -> serde_json::Value {
        serde_json::json!({
            "shift": modifiers.shift,
            "ctrl": modifiers.ctrl,
            "alt": modifiers.alt,
            "logo": modifiers.logo,
            "command": modifiers.command,
        })
    }

    /// Unified pointer press event.
    ///
    /// `pointer_type`: `"mouse"`, `"touch"`, or `"pen"`.
    /// `finger`: finger ID when `pointer_type` is `"touch"`, `None` otherwise.
    pub fn pointer_press(
        id: String,
        x: f32,
        y: f32,
        button: &str,
        pointer_type: &str,
        finger: Option<u64>,
        modifiers: KeyModifiers,
    ) -> Self {
        let mut val = serde_json::json!({
            "x": sanitize_f32(x),
            "y": sanitize_f32(y),
            "button": button,
            "pointer": pointer_type,
            "modifiers": Self::modifiers_data(&modifiers),
        });
        if let Some(f) = finger {
            val["finger"] = serde_json::json!(f);
        }
        Self {
            value: Some(val),
            ..Self::bare("press", id)
        }
    }

    /// Unified pointer release event.
    pub fn pointer_release(
        id: String,
        x: f32,
        y: f32,
        button: &str,
        pointer_type: &str,
        finger: Option<u64>,
        modifiers: KeyModifiers,
    ) -> Self {
        let mut val = serde_json::json!({
            "x": sanitize_f32(x),
            "y": sanitize_f32(y),
            "button": button,
            "pointer": pointer_type,
            "modifiers": Self::modifiers_data(&modifiers),
        });
        if let Some(f) = finger {
            val["finger"] = serde_json::json!(f);
        }
        Self {
            value: Some(val),
            ..Self::bare("release", id)
        }
    }

    /// Unified pointer move event (coalesceable).
    pub fn pointer_move(
        id: String,
        x: f32,
        y: f32,
        pointer_type: &str,
        finger: Option<u64>,
        modifiers: KeyModifiers,
    ) -> Self {
        let mut val = serde_json::json!({
            "x": sanitize_f32(x),
            "y": sanitize_f32(y),
            "pointer": pointer_type,
            "modifiers": Self::modifiers_data(&modifiers),
        });
        if let Some(f) = finger {
            val["finger"] = serde_json::json!(f);
        }
        Self {
            value: Some(val),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::bare("move", id)
        }
    }

    /// Unified pointer scroll event (coalesceable, accumulates deltas).
    pub fn pointer_scroll(
        id: String,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        pointer_type: &str,
        modifiers: KeyModifiers,
    ) -> Self {
        Self {
            value: Some(serde_json::json!({
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
                "delta_x": sanitize_f32(delta_x),
                "delta_y": sanitize_f32(delta_y),
                "pointer": pointer_type,
                "modifiers": Self::modifiers_data(&modifiers),
            })),
            coalesce: Some(CoalesceHint::Accumulate(vec![
                "delta_x".into(),
                "delta_y".into(),
            ])),
            ..Self::bare("scroll", id)
        }
    }

    /// Unified pointer enter event (no data payload).
    pub fn pointer_enter(id: String) -> Self {
        Self::bare("enter", id)
    }

    /// Unified pointer exit event (no data payload).
    pub fn pointer_exit(id: String) -> Self {
        Self::bare("exit", id)
    }

    /// Unified pointer double-click event.
    pub fn pointer_double_click(
        id: String,
        x: f32,
        y: f32,
        pointer_type: &str,
        modifiers: KeyModifiers,
    ) -> Self {
        Self {
            value: Some(serde_json::json!({
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
                "pointer": pointer_type,
                "modifiers": Self::modifiers_data(&modifiers),
            })),
            ..Self::bare("double_click", id)
        }
    }

    /// Unified resize event (for sensor widgets).
    pub fn resize(id: String, width: f32, height: f32) -> Self {
        Self {
            value: Some(serde_json::json!({
                "width": sanitize_f32(width),
                "height": sanitize_f32(height),
            })),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::bare("resize", id)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Replace non-finite f32 with 0.0 for safe JSON serialization.
fn sanitize_f32(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        log::warn!("non-finite f32 ({v}) replaced with 0.0 in outgoing event");
        0.0
    }
}

/// Replace non-finite f64 with 0.0 for safe JSON serialization.
fn sanitize_f64(v: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        log::warn!("non-finite f64 ({v}) replaced with 0.0 in outgoing event");
        0.0
    }
}

// ---------------------------------------------------------------------------
// Response types (serialized to stdout in reply to incoming messages)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Diagnostic (renderer -> host)
// ---------------------------------------------------------------------------

/// Severity level for an outgoing diagnostic.
///
/// Wire form is a snake_case string (`"info"`, `"warn"`, `"error"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    /// Informational message; not a problem.
    Info,
    /// Something irregular but recoverable.
    Warn,
    /// Unrecoverable by the renderer; host intervention may be needed.
    Error,
}

impl std::fmt::Display for DiagnosticLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => f.write_str("info"),
            Self::Warn => f.write_str("warn"),
            Self::Error => f.write_str("error"),
        }
    }
}

/// A structured diagnostic sent from the renderer to the host.
///
/// Wire form:
///
/// ```json
/// {
///   "type": "diagnostic",
///   "session": "s1",
///   "level": "warn",
///   "diagnostic": {"kind": "font_family_not_found", "family": "Inter"}
/// }
/// ```
///
/// The `diagnostic` field is a [`plushie_core::Diagnostic`]
/// serialised via its existing `#[serde(tag = "kind")]` representation,
/// so hosts can decode directly into the typed enum (or pattern-match
/// on the `kind` discriminant) without bespoke parsing.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticMessage {
    /// Always `"diagnostic"`.
    #[serde(rename = "type")]
    pub message_type: &'static str,
    /// Session that produced this diagnostic.
    pub session: String,
    /// Severity.
    pub level: DiagnosticLevel,
    /// The typed diagnostic payload.
    pub diagnostic: crate::Diagnostic,
}

impl DiagnosticMessage {
    /// Construct a new value with empty session.
    pub fn new(level: DiagnosticLevel, diagnostic: crate::Diagnostic) -> Self {
        Self {
            message_type: "diagnostic",
            session: String::new(),
            level,
            diagnostic,
        }
    }

    /// Set the session ID for this diagnostic.
    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = session.into();
        self
    }
}

/// Response to an effect request, written to stdout as JSONL.
#[derive(Debug, Serialize)]
pub struct EffectResponse {
    #[serde(rename = "type")]
    /// Message type.
    pub message_type: &'static str,
    /// Session.
    pub session: String,
    /// Target widget ID.
    pub id: String,
    /// Status.
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Operation result payload.
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Error payload.
    pub error: Option<String>,
}

impl EffectResponse {
    /// The effect completed successfully with the given result.
    pub fn ok(id: String, result: Value) -> Self {
        Self {
            message_type: "effect_response",
            session: String::new(),
            id,
            status: "ok",
            result: Some(result),
            error: None,
        }
    }

    /// The effect failed with the given reason.
    pub fn error(id: String, reason: String) -> Self {
        Self {
            message_type: "effect_response",
            session: String::new(),
            id,
            status: "error",
            result: None,
            error: Some(reason),
        }
    }

    /// The requested effect kind is not supported by this backend.
    /// Distinct from `error`: unsupported means the renderer can't
    /// handle this effect at all (e.g. file dialogs in headless mode),
    /// not that it tried and failed. The SDK uses this to trigger
    /// registered effect stubs or propagate to the app.
    pub fn unsupported(id: String) -> Self {
        Self {
            message_type: "effect_response",
            session: String::new(),
            id,
            status: "unsupported",
            result: None,
            error: None,
        }
    }

    /// The user cancelled the operation (e.g. closed a file dialog).
    /// Distinct from `error`: cancellation is a normal user action,
    /// not a failure.
    pub fn cancelled(id: String) -> Self {
        Self {
            message_type: "effect_response",
            session: String::new(),
            id,
            status: "cancelled",
            result: None,
            error: None,
        }
    }

    /// Set the session ID for this response.
    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = session.into();
        self
    }
}

/// Acknowledgement that an effect stub was registered or unregistered.
/// Sent back to the SDK so it can wait for confirmation before
/// proceeding (no timing assumptions about message ordering).
#[derive(Debug, Serialize)]
pub struct EffectStubAck {
    #[serde(rename = "type")]
    /// Message type.
    pub message_type: &'static str,
    /// Session.
    pub session: String,
    /// Event kind string used on the wire.
    pub kind: String,
}

impl EffectStubAck {
    /// Set or construct `registered`.
    pub fn registered(kind: String) -> Self {
        Self {
            message_type: "effect_stub_register_ack",
            session: String::new(),
            kind,
        }
    }

    /// Set or construct `unregistered`.
    pub fn unregistered(kind: String) -> Self {
        Self {
            message_type: "effect_stub_unregister_ack",
            session: String::new(),
            kind,
        }
    }

    /// Return a new value with the session set.
    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = session.into();
        self
    }
}

/// Response to a Query message.
#[derive(Debug, Serialize)]
pub struct QueryResponse {
    #[serde(rename = "type")]
    /// Message type.
    pub message_type: &'static str,
    /// Session.
    pub session: String,
    /// Target widget ID.
    pub id: String,
    /// Target identifier.
    pub target: String,
    /// Raw bytes (pixels, font, etc.).
    pub data: Value,
}

impl QueryResponse {
    /// Construct a new value.
    pub fn new(id: String, target: String, data: Value) -> Self {
        Self {
            message_type: "query_response",
            session: String::new(),
            id,
            target,
            data,
        }
    }

    /// Set the session ID for this response.
    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = session.into();
        self
    }
}

/// Response to an Interact message.
#[derive(Debug, Serialize)]
pub struct InteractResponse {
    #[serde(rename = "type")]
    /// Message type.
    pub message_type: &'static str,
    /// Session.
    pub session: String,
    /// Target widget ID.
    pub id: String,
    /// Events.
    pub events: Vec<OutgoingEvent>,
}

impl InteractResponse {
    /// Construct a new value.
    pub fn new(id: String, events: Vec<OutgoingEvent>) -> Self {
        Self {
            message_type: "interact_response",
            session: String::new(),
            id,
            events,
        }
    }

    /// Set the session ID for this response and all contained events.
    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        let session = session.into();
        for event in &mut self.events {
            event.session.clone_from(&session);
        }
        self.session = session;
        self
    }
}

/// Response to a TreeHash message.
///
/// Tree hashes capture structural tree data (hash of JSON tree). No pixel data.
/// For pixel data, see the `screenshot_response` message type.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct TreeHashResponse {
    #[serde(rename = "type")]
    /// Message type.
    pub message_type: &'static str,
    /// Session.
    pub session: String,
    /// Target widget ID.
    pub id: String,
    /// Identifier string.
    pub name: String,
    /// Hash.
    pub hash: String,
}

#[allow(dead_code)]
impl TreeHashResponse {
    /// Construct a new value.
    pub fn new(id: String, name: String, hash: String) -> Self {
        Self {
            message_type: "tree_hash_response",
            session: String::new(),
            id,
            name,
            hash,
        }
    }

    /// Set the session ID for this response.
    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = session.into();
        self
    }
}

/// Response to a Reset message.
#[derive(Debug, Serialize)]
pub struct ResetResponse {
    #[serde(rename = "type")]
    /// Message type.
    pub message_type: &'static str,
    /// Session.
    pub session: String,
    /// Target widget ID.
    pub id: String,
    /// Status.
    pub status: &'static str,
}

impl ResetResponse {
    /// Set or construct `ok`.
    pub fn ok(id: String) -> Self {
        Self {
            message_type: "reset_response",
            session: String::new(),
            id,
            status: "ok",
        }
    }

    /// Set the session ID for this response.
    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = session.into();
        self
    }
}
