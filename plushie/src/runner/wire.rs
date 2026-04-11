//! Wire mode runner: subprocess renderer via stdin/stdout.
//!
//! Spawns the plushie renderer binary as a child process and
//! communicates over the plushie wire protocol. The app's view
//! tree is diffed and sent as patches. Events arrive from the
//! renderer and are converted to SDK Event types.

#[cfg(feature = "wire")]
use serde_json::Value;

#[cfg(feature = "wire")]
use crate::App;
#[cfg(feature = "wire")]
use crate::command::Command;
#[cfg(feature = "wire")]
use crate::event::Event;
#[cfg(feature = "wire")]
use crate::runtime::{normalize, tree_diff};
#[cfg(feature = "wire")]
use super::bridge::Bridge;

/// Run the app in wire mode.
///
/// Spawns the renderer binary at `binary_path` and communicates
/// over stdin/stdout using the plushie wire protocol.
#[cfg(feature = "wire")]
pub fn run_wire<A: App>(binary_path: &str) -> crate::Result {
    // Build settings from the app.
    let settings = build_settings::<A>();

    // Spawn the renderer.
    let mut bridge = Bridge::spawn(binary_path)?;

    // Send initial settings.
    bridge.send_settings(&settings)?;

    // Read the hello message.
    let hello = bridge.receive()?;
    log::info!("renderer hello: {}", hello.get("name").and_then(|v| v.as_str()).unwrap_or("unknown"));

    // Initialize the app.
    let (mut model, init_cmd) = A::init();

    // Track active subscriptions for diffing.
    let mut active_subs: Vec<crate::subscription::Subscription> = Vec::new();

    // First render: full snapshot.
    let view = A::view(&model);
    let (normalized, _) = normalize::normalize(&view);
    let mut current_tree = normalized;
    bridge.send_snapshot(&serde_json::to_value(&current_tree).unwrap())?;

    // Execute the initial command (e.g. focus a field, start
    // async work) so apps work from the first frame.
    if let Err(e) = execute_wire_command(&mut bridge, &init_cmd) {
        log::error!("initial command execution failed: {e}");
    }

    // Initial subscription sync.
    let new_subs = A::subscribe(&model);
    sync_subscriptions(&mut bridge, &active_subs, &new_subs)?;
    active_subs = new_subs;

    // Event loop.
    loop {
        // Read next event from renderer.
        let raw = match bridge.receive() {
            Ok(msg) => msg,
            Err(e) => {
                log::error!("renderer connection lost: {e}");
                break;
            }
        };

        // Convert wire event to SDK Event via the shared event bridge.
        if let Some(event) = wire_to_sdk_event(&raw) {
            let cmd = A::update(&mut model, event);
            if let Err(e) = execute_wire_command(&mut bridge, &cmd) {
                log::error!("command execution failed: {e}");
            }

            // Re-render and diff.
            let view = A::view(&model);
            let (new_tree, warnings) = normalize::normalize(&view);
            for warning in &warnings {
                log::warn!("view normalization: {warning}");
            }

            let patches = tree_diff::diff_tree(&current_tree, &new_tree);
            if !patches.is_empty() {
                let ops: Vec<Value> = patches
                    .iter()
                    .filter_map(|op| serde_json::to_value(op).ok())
                    .collect();
                bridge.send_patch(&ops)?;
            }

            current_tree = new_tree;

            // Sync subscriptions.
            let new_subs = A::subscribe(&model);
            if let Err(e) = sync_subscriptions(&mut bridge, &active_subs, &new_subs) {
                log::error!("subscription sync failed: {e}");
            }
            active_subs = new_subs;
        }
    }

    Ok(())
}

/// Convert a wire protocol JSON message to an SDK Event via the
/// shared event bridge.
///
/// Constructs a SinkEvent from the raw JSON fields and feeds it
/// through the event bridge for type-safe conversion.
#[cfg(feature = "wire")]
fn wire_to_sdk_event(msg: &Value) -> Option<Event> {
    use plushie_core::protocol::{OutgoingEvent, EffectResponse, KeyModifiers};
    use super::event_bridge::{SinkEvent, sink_event_to_sdk};

    let msg_type = msg.get("type")?.as_str()?;

    let sink_event = match msg_type {
        "event" => {
            let event = OutgoingEvent {
                message_type: "event",
                session: String::new(),
                family: msg.get("family")?.as_str()?.to_string(),
                id: msg.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                window_id: msg.get("window_id").and_then(|v| v.as_str()).map(String::from),
                value: msg.get("value").cloned(),
                tag: msg.get("tag").and_then(|v| v.as_str()).map(String::from),
                modifiers: msg.get("modifiers").and_then(|v| serde_json::from_value::<KeyModifiers>(v.clone()).ok()),
                data: msg.get("data").cloned(),
                captured: msg.get("captured").and_then(|v| v.as_bool()),
                coalesce: None,
            };
            SinkEvent::Event(event)
        }
        "effect_response" => {
            let response = EffectResponse {
                message_type: "effect_response",
                session: String::new(),
                id: msg.get("id")?.as_str()?.to_string(),
                status: match msg.get("status").and_then(|v| v.as_str()) {
                    Some("ok") => "ok",
                    Some("cancelled") => "cancelled",
                    _ => "error",
                },
                result: msg.get("result").cloned(),
                error: msg.get("error").and_then(|v| v.as_str()).map(String::from),
            };
            SinkEvent::EffectResponse(response)
        }
        "query_response" | "op_query_response" => {
            let kind = msg.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let tag = msg.get("tag")
                .or_else(|| msg.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let data = msg.get("result")
                .or_else(|| msg.get("data"))
                .cloned()
                .unwrap_or(Value::Null);
            SinkEvent::QueryResponse { kind, tag, data }
        }
        _ => return None,
    };

    sink_event_to_sdk(sink_event)
}

/// Execute a Command by sending messages through the bridge.
#[cfg(feature = "wire")]
fn execute_wire_command(bridge: &mut Bridge, cmd: &Command) -> std::io::Result<()> {
    match cmd {
        Command::None => {}
        Command::Exit => {
            bridge.send_widget_op("exit", &Value::Null)?;
        }
        Command::Batch(cmds) => {
            for c in cmds {
                execute_wire_command(bridge, c)?;
            }
        }
        Command::Renderer(op) => {
            execute_wire_renderer_op(bridge, op)?;
        }
        Command::Async { .. } | Command::Cancel { .. } | Command::SendAfter { .. } => {
            log::warn!("SDK-local command not yet supported in wire mode: {cmd:?}");
        }
    }
    Ok(())
}

/// Serialize a RendererOp to wire messages and send via the bridge.
#[cfg(feature = "wire")]
fn execute_wire_renderer_op(bridge: &mut Bridge, op: &plushie_core::ops::RendererOp) -> std::io::Result<()> {
    use plushie_core::ops::RendererOp;
    match op {
        RendererOp::Focus(id) => bridge.send_widget_op("focus", &serde_json::json!({"target": id})),
        RendererOp::FocusNext => bridge.send_widget_op("focus_next", &Value::Object(Default::default())),
        RendererOp::FocusPrevious => bridge.send_widget_op("focus_previous", &Value::Object(Default::default())),
        RendererOp::SelectAll(target) => bridge.send_widget_op("select_all", &serde_json::json!({"target": target})),
        RendererOp::MoveCursorToFront(target) => bridge.send_widget_op("move_cursor_to_front", &serde_json::json!({"target": target})),
        RendererOp::MoveCursorToEnd(target) => bridge.send_widget_op("move_cursor_to_end", &serde_json::json!({"target": target})),
        RendererOp::MoveCursorTo { target, position } => bridge.send_widget_op("move_cursor_to", &serde_json::json!({"target": target, "position": position})),
        RendererOp::SelectRange { target, start, end } => bridge.send_widget_op("select_range", &serde_json::json!({"target": target, "start": start, "end": end})),
        RendererOp::ScrollTo { target, x, y } => bridge.send_widget_op("scroll_to", &serde_json::json!({"target": target, "offset_x": x, "offset_y": y})),
        RendererOp::ScrollBy { target, x, y } => bridge.send_widget_op("scroll_by", &serde_json::json!({"target": target, "offset_x": x, "offset_y": y})),
        RendererOp::SnapTo { target, x, y } => bridge.send_widget_op("snap_to", &serde_json::json!({"target": target, "x": x, "y": y})),
        RendererOp::SnapToEnd(target) => bridge.send_widget_op("snap_to_end", &serde_json::json!({"target": target})),
        RendererOp::Window(op) => execute_wire_window_op(bridge, op),
        RendererOp::WidgetCommand { node_id, op, payload } => bridge.send_widget_command(node_id, op, payload),
        RendererOp::Announce(text) => bridge.send_widget_op("announce", &serde_json::json!({"text": text})),
        RendererOp::Effect { tag, request } => {
            let (kind, payload) = plushie_core::ops::effect_request_to_wire(request);
            bridge.send_effect(tag, kind, &payload)
        }
        RendererOp::Subscribe { kind, tag, max_rate, window_id } => {
            bridge.send_subscribe(&kind, &tag, *max_rate, window_id.as_deref())
        }
        RendererOp::Unsubscribe { kind, tag } => {
            bridge.send_unsubscribe(&kind, &tag)
        }
        _ => {
            log::debug!("unhandled wire renderer op: {op:?}");
            Ok(())
        }
    }
}

/// Execute a window command via the bridge.
#[cfg(feature = "wire")]
fn execute_wire_window_op(bridge: &mut Bridge, op: &plushie_core::ops::WindowOp) -> std::io::Result<()> {
    use plushie_core::ops::WindowOp;
    match op {
        WindowOp::Close(id) => {
            bridge.send_widget_op("close_window", &serde_json::json!({"window_id": id}))?;
        }
        WindowOp::Resize { window_id, width, height } => {
            bridge.send_window_op("resize", window_id, &serde_json::json!({
                "width": width, "height": height
            }))?;
        }
        WindowOp::Move { window_id, x, y } => {
            bridge.send_window_op("move", window_id, &serde_json::json!({
                "x": x, "y": y
            }))?;
        }
        WindowOp::Maximize { window_id, maximized } => {
            bridge.send_window_op("maximize", window_id, &serde_json::json!({
                "maximized": maximized
            }))?;
        }
        WindowOp::Minimize { window_id, minimized } => {
            bridge.send_window_op("minimize", window_id, &serde_json::json!({
                "minimized": minimized
            }))?;
        }
        _ => {
            log::debug!("unhandled wire window op: {op:?}");
        }
    }
    Ok(())
}

/// Build settings JSON from the App trait.
#[cfg(feature = "wire")]
fn build_settings<A: App>() -> Value {
    let settings = A::settings();
    let mut json = serde_json::json!({
        "protocol_version": 1,
    });

    if let Some(size) = settings.default_text_size {
        json["default_text_size"] = serde_json::json!(size);
    }
    if let Some(antialiasing) = settings.antialiasing {
        json["antialiasing"] = serde_json::json!(antialiasing);
    }
    if let Some(vsync) = settings.vsync {
        json["vsync"] = serde_json::json!(vsync);
    }
    if let Some(scale) = settings.scale_factor {
        json["scale_factor"] = serde_json::json!(scale);
    }
    if let Some(rate) = settings.default_event_rate {
        json["default_event_rate"] = serde_json::json!(rate);
    }
    if !settings.widget_config.is_empty() {
        json["widget_config"] = serde_json::to_value(&settings.widget_config)
            .unwrap_or(Value::Null);
    }

    json
}

/// Diff old and new subscription lists and send Subscribe/Unsubscribe
/// messages for the differences.
///
/// Timer subscriptions (kind "every") are filtered out since they're
/// handled SDK-side, not by the renderer.
#[cfg(feature = "wire")]
fn sync_subscriptions(
    bridge: &mut Bridge,
    old: &[crate::subscription::Subscription],
    new: &[crate::subscription::Subscription],
) -> std::io::Result<()> {
    use std::collections::HashSet;

    // Filter to renderer-side subscriptions only (skip timers).
    let is_renderer_sub = |s: &&crate::subscription::Subscription| s.wire_kind() != "every";

    let old_keys: HashSet<(&str, &str)> = old.iter()
        .filter(is_renderer_sub)
        .map(|s| s.diff_key())
        .collect();
    let new_keys: HashSet<(&str, &str)> = new.iter()
        .filter(is_renderer_sub)
        .map(|s| s.diff_key())
        .collect();

    // Unsubscribe removed subscriptions.
    for sub in old.iter().filter(is_renderer_sub) {
        let key = sub.diff_key();
        if !new_keys.contains(&key) {
            bridge.send_unsubscribe(sub.wire_kind(), &sub.tag)?;
        }
    }

    // Subscribe new subscriptions.
    for sub in new.iter().filter(is_renderer_sub) {
        let key = sub.diff_key();
        if !old_keys.contains(&key) {
            bridge.send_subscribe(
                sub.wire_kind(),
                &sub.tag,
                sub.max_rate,
                sub.window_id.as_deref(),
            )?;
        }
    }

    Ok(())
}
