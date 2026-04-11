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
/// serialized to the wire -- renderer-internal metadata.
///
/// # For widget authors
///
/// Set on events returned from `handle_message()`:
///
/// ```ignore
/// let event = OutgoingEvent::widget_event("cursor_pos", node_id, data)
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
    /// Sum the named `data` fields across coalesced events.
    /// Other fields keep the latest event's values.
    /// Use for: scroll deltas, velocity changes, counters -- anything
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
    /// Source window ID for widget-level events, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    /// Primary value payload (e.g. input text, slider value, selected option).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Subscription tag identifying which subscription requested this event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Keyboard modifier state at the time of the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<KeyModifiers>,
    /// Flexible extra data for events that carry additional fields beyond
    /// the standard id/value/tag/modifiers shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// Whether the event was captured (consumed) by an iced widget before
    /// reaching the subscription listener. Present on keyboard, mouse,
    /// touch, and IME events; absent on widget-level events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured: Option<bool>,
    /// Coalescing hint for rate-limited delivery.
    /// Not serialized to the wire -- renderer-internal metadata.
    #[serde(skip)]
    pub coalesce: Option<CoalesceHint>,
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

    /// Set the primary `value` field on this event.
    ///
    /// For built-in widget events, `value` carries the widget's primary
    /// datum (input text, slider position, selected option). Widget
    /// authors wrapping built-in widgets can use this to emit events
    /// compatible with the built-in shape:
    ///
    /// ```ignore
    /// OutgoingEvent::widget_event("input", id, data)
    ///     .with_value(serde_json::Value::String(text))
    /// ```
    pub fn with_value(mut self, value: Value) -> Self {
        self.value = Some(value);
        self
    }

    /// Set the source window for this event.
    ///
    /// Widget-like events must always carry a real window id on the wire.
    /// Empty ids are a renderer bug and should be fixed at the call site.
    pub fn with_window_id(mut self, window_id: impl Into<String>) -> Self {
        let window_id = window_id.into();
        assert!(
            !window_id.is_empty(),
            "widget-like events must include a non-empty window_id"
        );
        self.window_id = Some(window_id);
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
    pub shift: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub logo: bool,
    #[serde(default)]
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
            window_id: None,
            value: None,
            tag: None,
            modifiers: None,
            data: None,
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
            window_id: None,
            value: None,
            tag: Some(tag),
            modifiers: None,
            data: None,
            captured: None,
            coalesce: None,
        }
    }

    /// Generic widget event with a family string and optional data payload.
    /// Used for on_open, on_close, sort, and other events.
    pub fn generic(family: impl Into<String>, id: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            data,
            ..Self::bare(family, id)
        }
    }

    /// Convenience constructor for widget-emitted events.
    ///
    /// Identical to [`generic`](Self::generic) -- exists for discoverability
    /// so widget authors searching docs for "widget" find it.
    pub fn widget_event(
        family: impl Into<String>,
        id: impl Into<String>,
        data: Option<Value>,
    ) -> Self {
        Self::generic(family, id, data)
    }

    pub fn click(id: String) -> Self {
        Self::bare("click", id)
    }

    pub fn input(id: String, value: String) -> Self {
        Self {
            value: Some(Value::String(value)),
            ..Self::bare("input", id)
        }
    }

    pub fn submit(id: String, value: String) -> Self {
        Self {
            value: Some(Value::String(value)),
            ..Self::bare("submit", id)
        }
    }

    pub fn toggle(id: String, checked: bool) -> Self {
        Self {
            value: Some(Value::Bool(checked)),
            ..Self::bare("toggle", id)
        }
    }

    pub fn slide(id: String, value: f64) -> Self {
        Self {
            value: Some(serde_json::json!(sanitize_f64(value))),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::bare("slide", id)
        }
    }

    pub fn slide_release(id: String, value: f64) -> Self {
        Self {
            value: Some(serde_json::json!(sanitize_f64(value))),
            ..Self::bare("slide_release", id)
        }
    }

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

    pub fn cursor_moved(tag: String, x: f32, y: f32) -> Self {
        Self {
            data: Some(serde_json::json!({"x": sanitize_f32(x), "y": sanitize_f32(y)})),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::tagged("cursor_moved", tag)
        }
    }

    pub fn cursor_entered(tag: String) -> Self {
        Self::tagged("cursor_entered", tag)
    }

    pub fn cursor_left(tag: String) -> Self {
        Self::tagged("cursor_left", tag)
    }

    pub fn button_pressed(tag: String, button: String) -> Self {
        Self {
            value: Some(Value::String(button)),
            ..Self::tagged("button_pressed", tag)
        }
    }

    pub fn button_released(tag: String, button: String) -> Self {
        Self {
            value: Some(Value::String(button)),
            ..Self::tagged("button_released", tag)
        }
    }

    pub fn wheel_scrolled(tag: String, delta_x: f32, delta_y: f32, unit: &str) -> Self {
        Self {
            data: Some(serde_json::json!({
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
            data: Some(serde_json::json!({
                "id": finger_id,
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
            })),
            ..Self::tagged(family, tag)
        }
    }

    pub fn finger_pressed(tag: String, finger_id: u64, x: f32, y: f32) -> Self {
        Self::touch_event("finger_pressed", tag, finger_id, x, y)
    }

    pub fn finger_moved(tag: String, finger_id: u64, x: f32, y: f32) -> Self {
        Self {
            coalesce: Some(CoalesceHint::Replace),
            ..Self::touch_event("finger_moved", tag, finger_id, x, y)
        }
    }

    pub fn finger_lifted(tag: String, finger_id: u64, x: f32, y: f32) -> Self {
        Self::touch_event("finger_lifted", tag, finger_id, x, y)
    }

    pub fn finger_lost(tag: String, finger_id: u64, x: f32, y: f32) -> Self {
        Self::touch_event("finger_lost", tag, finger_id, x, y)
    }

    // -----------------------------------------------------------------------
    // IME events
    // -----------------------------------------------------------------------

    pub fn ime_opened(tag: String) -> Self {
        Self::tagged("ime_opened", tag)
    }

    pub fn ime_preedit(tag: String, text: String, cursor: Option<std::ops::Range<usize>>) -> Self {
        let cursor_val = cursor
            .map(|r| serde_json::json!({"start": r.start, "end": r.end}))
            .unwrap_or(serde_json::Value::Null);
        Self {
            data: Some(serde_json::json!({"text": text, "cursor": cursor_val})),
            ..Self::tagged("ime_preedit", tag)
        }
    }

    pub fn ime_commit(tag: String, text: String) -> Self {
        Self {
            data: Some(serde_json::json!({"text": text})),
            ..Self::tagged("ime_commit", tag)
        }
    }

    pub fn ime_closed(tag: String) -> Self {
        Self::tagged("ime_closed", tag)
    }

    // -----------------------------------------------------------------------
    // Window lifecycle events
    // -----------------------------------------------------------------------

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
            data: Some(serde_json::json!({
                "window_id": window_id,
                "position": pos,
                "width": sanitize_f32(width),
                "height": sanitize_f32(height),
                "scale_factor": sanitize_f32(scale_factor),
            })),
            ..Self::tagged("window_opened", tag)
        }
    }

    /// Window event carrying only a window_id in its data payload.
    fn window_event(family: &str, tag: String, window_id: String) -> Self {
        Self {
            data: Some(serde_json::json!({"window_id": window_id})),
            ..Self::tagged(family, tag)
        }
    }

    pub fn window_closed(tag: String, window_id: String) -> Self {
        Self::window_event("window_closed", tag, window_id)
    }

    pub fn window_close_requested(tag: String, window_id: String) -> Self {
        Self::window_event("window_close_requested", tag, window_id)
    }

    pub fn window_moved(tag: String, window_id: String, x: f32, y: f32) -> Self {
        Self {
            data: Some(serde_json::json!({
                "window_id": window_id,
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
            })),
            ..Self::tagged("window_moved", tag)
        }
    }

    pub fn window_resized(tag: String, window_id: String, width: f32, height: f32) -> Self {
        Self {
            data: Some(serde_json::json!({
                "window_id": window_id,
                "width": sanitize_f32(width),
                "height": sanitize_f32(height),
            })),
            ..Self::tagged("window_resized", tag)
        }
    }

    pub fn window_focused(tag: String, window_id: String) -> Self {
        Self::window_event("window_focused", tag, window_id)
    }

    pub fn window_unfocused(tag: String, window_id: String) -> Self {
        Self::window_event("window_unfocused", tag, window_id)
    }

    pub fn window_rescaled(tag: String, window_id: String, scale_factor: f32) -> Self {
        Self {
            data: Some(serde_json::json!({
                "window_id": window_id,
                "scale_factor": sanitize_f32(scale_factor),
            })),
            ..Self::tagged("window_rescaled", tag)
        }
    }

    pub fn file_hovered(tag: String, window_id: String, path: String) -> Self {
        Self {
            data: Some(serde_json::json!({
                "window_id": window_id,
                "path": path,
            })),
            ..Self::tagged("file_hovered", tag)
        }
    }

    pub fn file_dropped(tag: String, window_id: String, path: String) -> Self {
        Self {
            data: Some(serde_json::json!({
                "window_id": window_id,
                "path": path,
            })),
            ..Self::tagged("file_dropped", tag)
        }
    }

    pub fn files_hovered_left(tag: String, window_id: String) -> Self {
        Self::window_event("files_hovered_left", tag, window_id)
    }

    // -----------------------------------------------------------------------
    // Animation / theme / system events
    // -----------------------------------------------------------------------

    pub fn animation_frame(tag: String, timestamp_millis: u128) -> Self {
        Self {
            data: Some(serde_json::json!({"timestamp": timestamp_millis})),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::tagged("animation_frame", tag)
        }
    }

    pub fn theme_changed(tag: String, mode: String) -> Self {
        Self {
            value: Some(Value::String(mode)),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::tagged("theme_changed", tag)
        }
    }

    // -----------------------------------------------------------------------
    // Canvas element events (interactive group interactions)
    //
    // All canvas element events use scoped IDs: the wire `id` field is
    // `"{canvas_id}/{element_id}"` so the SDK's scoped ID system splits
    // it into `id: element_id, scope: [canvas_id, ...]` automatically.
    // All canvas events use standard families with scoped IDs
    // (`"{canvas_id}/{element_id}"`), making canvas elements look like
    // regular widgets from the SDK's perspective.
    // -----------------------------------------------------------------------

    /// Scoped ID helper: `"{canvas_id}/{element_id}"`.
    fn scoped_element_id(canvas_id: &str, element_id: &str) -> String {
        format!("{canvas_id}/{element_id}")
    }

    pub fn canvas_element_enter(canvas_id: String, element_id: String, x: f32, y: f32) -> Self {
        Self {
            data: Some(serde_json::json!({
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
            })),
            ..Self::bare("enter", Self::scoped_element_id(&canvas_id, &element_id))
        }
    }

    pub fn canvas_element_leave(canvas_id: String, element_id: String) -> Self {
        Self::bare("exit", Self::scoped_element_id(&canvas_id, &element_id))
    }

    pub fn canvas_element_key_press(
        canvas_id: String,
        element_id: String,
        key: String,
        modifiers: KeyModifiers,
    ) -> Self {
        Self {
            data: Some(serde_json::json!({
                "key": key,
                "modifiers": modifiers,
            })),
            ..Self::bare(
                "key_press",
                Self::scoped_element_id(&canvas_id, &element_id),
            )
        }
    }

    pub fn canvas_element_key_release(
        canvas_id: String,
        element_id: String,
        key: String,
        modifiers: KeyModifiers,
    ) -> Self {
        Self {
            data: Some(serde_json::json!({
                "key": key,
                "modifiers": modifiers,
            })),
            ..Self::bare(
                "key_release",
                Self::scoped_element_id(&canvas_id, &element_id),
            )
        }
    }

    /// Canvas element activation.
    pub fn canvas_element_click(
        canvas_id: String,
        element_id: String,
        x: f32,
        y: f32,
        button: String,
    ) -> Self {
        Self {
            data: Some(serde_json::json!({
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
                "button": button,
            })),
            ..Self::bare("click", Self::scoped_element_id(&canvas_id, &element_id))
        }
    }

    pub fn canvas_element_drag(
        canvas_id: String,
        element_id: String,
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
    ) -> Self {
        Self {
            data: Some(serde_json::json!({
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
                "delta_x": sanitize_f32(dx),
                "delta_y": sanitize_f32(dy),
            })),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::bare("drag", Self::scoped_element_id(&canvas_id, &element_id))
        }
    }

    pub fn canvas_element_drag_end(canvas_id: String, element_id: String, x: f32, y: f32) -> Self {
        Self {
            data: Some(serde_json::json!({
                "x": sanitize_f32(x),
                "y": sanitize_f32(y),
            })),
            ..Self::bare("drag_end", Self::scoped_element_id(&canvas_id, &element_id))
        }
    }

    pub fn canvas_element_focused(canvas_id: String, element_id: String) -> Self {
        Self::bare("focused", Self::scoped_element_id(&canvas_id, &element_id))
    }

    pub fn canvas_element_blurred(canvas_id: String, element_id: String) -> Self {
        Self::bare("blurred", Self::scoped_element_id(&canvas_id, &element_id))
    }

    /// The canvas widget itself gained iced-level focus.
    pub fn canvas_focused(canvas_id: String) -> Self {
        Self::bare("focused", canvas_id)
    }

    /// The canvas widget itself lost iced-level focus.
    pub fn canvas_blurred(canvas_id: String) -> Self {
        Self::bare("blurred", canvas_id)
    }

    /// A focusable group gained group-level focus. Uses scoped ID.
    pub fn canvas_group_focused(canvas_id: String, group_id: String) -> Self {
        Self::bare("focused", Self::scoped_element_id(&canvas_id, &group_id))
    }

    /// A focusable group lost group-level focus. Uses scoped ID.
    pub fn canvas_group_blurred(canvas_id: String, group_id: String) -> Self {
        Self::bare("blurred", Self::scoped_element_id(&canvas_id, &group_id))
    }

    /// Renderer-side validation diagnostic.
    ///
    /// The `id` field on the event envelope is set to `canvas_id` for
    /// consistency with other canvas events. The `data` payload carries
    /// the full diagnostic detail including the optional `element_id`.
    pub fn diagnostic(
        canvas_id: String,
        element_id: Option<String>,
        level: &str,
        code: &str,
        message: &str,
    ) -> Self {
        Self {
            data: Some(serde_json::json!({
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

    pub fn pane_resized(id: String, split: String, ratio: f32) -> Self {
        Self {
            data: Some(serde_json::json!({"split": split, "ratio": sanitize_f32(ratio)})),
            coalesce: Some(CoalesceHint::Replace),
            ..Self::bare("pane_resized", id)
        }
    }

    pub fn pane_dragged(
        id: String,
        kind: &str,
        pane: String,
        target: Option<String>,
        region: Option<&str>,
        edge: Option<&str>,
    ) -> Self {
        let mut data = serde_json::json!({"action": kind, "pane": pane});
        if let Some(t) = target {
            data["target"] = serde_json::json!(t);
        }
        if let Some(r) = region {
            data["region"] = serde_json::json!(r);
        }
        if let Some(e) = edge {
            data["edge"] = serde_json::json!(e);
        }
        Self {
            data: Some(data),
            ..Self::bare("pane_dragged", id)
        }
    }

    pub fn pane_clicked(id: String, pane: String) -> Self {
        Self {
            data: Some(serde_json::json!({"pane": pane})),
            ..Self::bare("pane_clicked", id)
        }
    }

    pub fn pane_focus_cycle(id: String, pane: String) -> Self {
        Self {
            data: Some(serde_json::json!({"pane": pane})),
            ..Self::bare("pane_focus_cycle", id)
        }
    }

    // -----------------------------------------------------------------------
    // TextInput paste event
    // -----------------------------------------------------------------------

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
    /// `data.key`, modifiers in the top-level `modifiers` field. Missing
    /// modifier fields default to `false`.
    pub fn scripting_key_press(key: String, modifiers_json: Value) -> Self {
        let mods: KeyModifiers =
            serde_json::from_value(modifiers_json).unwrap_or(KeyModifiers::default());
        Self {
            modifiers: Some(mods),
            data: Some(serde_json::json!({"key": key})),
            ..Self::bare("key_press", String::new())
        }
    }

    /// Key release event from scripting (no full KeyEventData).
    ///
    /// Produces the same event shape as real key_release events: `key` in
    /// `data.key`, modifiers in the top-level `modifiers` field. Missing
    /// modifier fields default to `false`.
    pub fn scripting_key_release(key: String, modifiers_json: Value) -> Self {
        let mods: KeyModifiers =
            serde_json::from_value(modifiers_json).unwrap_or(KeyModifiers::default());
        Self {
            modifiers: Some(mods),
            data: Some(serde_json::json!({"key": key})),
            ..Self::bare("key_release", String::new())
        }
    }

    /// Cursor moved event from scripting.
    pub fn scripting_cursor_moved(x: f64, y: f64) -> Self {
        Self {
            data: Some(serde_json::json!({"x": x, "y": y})),
            ..Self::bare("cursor_moved", String::new())
        }
    }

    /// Scroll event from scripting.
    pub fn scripting_scroll(delta_x: f64, delta_y: f64) -> Self {
        Self {
            data: Some(
                serde_json::json!({"delta_x": delta_x, "delta_y": delta_y, "unit": "pixel"}),
            ),
            ..Self::bare("wheel_scrolled", String::new())
        }
    }

    // -----------------------------------------------------------------------
    // ComboBox option hovered event
    // -----------------------------------------------------------------------

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
            data: Some(serde_json::json!({
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

    /// Build a modifiers data object for inclusion in pointer event data.
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
        let mut data = serde_json::json!({
            "x": sanitize_f32(x),
            "y": sanitize_f32(y),
            "button": button,
            "pointer": pointer_type,
            "modifiers": Self::modifiers_data(&modifiers),
        });
        if let Some(f) = finger {
            data["finger"] = serde_json::json!(f);
        }
        Self {
            data: Some(data),
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
        let mut data = serde_json::json!({
            "x": sanitize_f32(x),
            "y": sanitize_f32(y),
            "button": button,
            "pointer": pointer_type,
            "modifiers": Self::modifiers_data(&modifiers),
        });
        if let Some(f) = finger {
            data["finger"] = serde_json::json!(f);
        }
        Self {
            data: Some(data),
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
        let mut data = serde_json::json!({
            "x": sanitize_f32(x),
            "y": sanitize_f32(y),
            "pointer": pointer_type,
            "modifiers": Self::modifiers_data(&modifiers),
        });
        if let Some(f) = finger {
            data["finger"] = serde_json::json!(f);
        }
        Self {
            data: Some(data),
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
            data: Some(serde_json::json!({
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
            data: Some(serde_json::json!({
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
            data: Some(serde_json::json!({
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

/// Response to an effect request, written to stdout as JSONL.
#[derive(Debug, Serialize)]
pub struct EffectResponse {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub session: String,
    pub id: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    /// Distinct from `error` -- unsupported means the renderer can't
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
    /// Distinct from `error` -- cancellation is a normal user action,
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
    pub message_type: &'static str,
    pub session: String,
    pub kind: String,
}

impl EffectStubAck {
    pub fn registered(kind: String) -> Self {
        Self {
            message_type: "effect_stub_registered",
            session: String::new(),
            kind,
        }
    }

    pub fn unregistered(kind: String) -> Self {
        Self {
            message_type: "effect_stub_unregistered",
            session: String::new(),
            kind,
        }
    }

    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = session.into();
        self
    }
}

/// Response to a Query message.
#[derive(Debug, Serialize)]
pub struct QueryResponse {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub session: String,
    pub id: String,
    pub target: String,
    pub data: Value,
}

impl QueryResponse {
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
    pub message_type: &'static str,
    pub session: String,
    pub id: String,
    pub events: Vec<OutgoingEvent>,
}

impl InteractResponse {
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
    pub message_type: &'static str,
    pub session: String,
    pub id: String,
    pub name: String,
    pub hash: String,
}

#[allow(dead_code)]
impl TreeHashResponse {
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
    pub message_type: &'static str,
    pub session: String,
    pub id: String,
    pub status: &'static str,
}

impl ResetResponse {
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

