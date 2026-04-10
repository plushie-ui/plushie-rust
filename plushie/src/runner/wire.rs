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
use crate::event::{
    self, AsyncEvent, EffectEvent, EffectResult, Event, EventType, SystemEvent,
    SystemEventType, TimerEvent, WidgetCommandError, WidgetEvent, WindowEvent,
    WindowEventType,
};
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
    use std::time::Duration;

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
    let (mut model, _init_cmd) = A::init();

    // First render: full snapshot.
    let view = A::view(&model);
    let (mut current_tree, _) = normalize::normalize(&view.0);
    bridge.send_snapshot(&current_tree)?;

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

        // Convert wire event to SDK Event.
        if let Some(event) = wire_event_to_sdk_event(&raw) {
            let cmd = A::update(&mut model, event);
            if let Err(e) = execute_wire_command(&mut bridge, &cmd) {
                log::error!("command execution failed: {e}");
            }

            // Re-render and diff.
            let view = A::view(&model);
            let (new_tree, warnings) = normalize::normalize(&view.0);
            for warning in &warnings {
                log::warn!("view normalization: {warning}");
            }

            let patches = tree_diff::diff(&current_tree, &new_tree);
            if !patches.is_empty() {
                let ops: Vec<Value> = patches
                    .iter()
                    .filter_map(|op| serde_json::to_value(op).ok())
                    .collect();
                bridge.send_patch(&ops)?;
            }

            current_tree = new_tree;
        }
    }

    Ok(())
}

/// Convert a wire protocol event message to an SDK Event.
#[cfg(feature = "wire")]
fn wire_event_to_sdk_event(msg: &Value) -> Option<Event> {
    let msg_type = msg.get("type")?.as_str()?;

    match msg_type {
        "event" => {
            let family = msg.get("family")?.as_str()?;
            let id = msg.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let window_id = msg.get("window_id").and_then(|v| v.as_str()).unwrap_or_default();
            let value = msg.get("value").cloned().unwrap_or(Value::Null);
            let data = msg.get("data").cloned().unwrap_or(Value::Null);
            let tag = msg.get("tag").and_then(|v| v.as_str());

            // Check if this is a subscription event (has tag, no id).
            if let Some(tag) = tag {
                return match family {
                    "key_press" | "key_release" => {
                        // TODO: parse key event data
                        None
                    }
                    "animation_frame" => Some(Event::System(SystemEvent {
                        event_type: SystemEventType::AnimationFrame,
                        tag: Some(tag.to_string()),
                        value: Some(value),
                        id: None,
                        window_id: if window_id.is_empty() { None } else { Some(window_id.to_string()) },
                    })),
                    "theme_changed" => Some(Event::System(SystemEvent {
                        event_type: SystemEventType::ThemeChanged,
                        tag: Some(tag.to_string()),
                        value: Some(value),
                        id: None,
                        window_id: None,
                    })),
                    _ => None,
                };
            }

            // Widget event.
            let event_type = family_to_event_type(family);
            let (local_id, scope) = split_scoped_id(id);
            let primary_value = if !data.is_null() { data } else { value };

            Some(Event::Widget(WidgetEvent {
                event_type,
                id: local_id,
                window_id: window_id.to_string(),
                scope,
                value: primary_value,
            }))
        }

        "effect_response" => {
            let id = msg.get("id")?.as_str()?;
            let status = msg.get("status").and_then(|v| v.as_str()).unwrap_or("error");
            let result_value = msg.get("result").cloned().unwrap_or(Value::Null);

            let result = match status {
                "ok" => EffectResult::Ok(result_value),
                "cancelled" => EffectResult::Cancelled,
                _ => EffectResult::Error(result_value),
            };

            Some(Event::Effect(EffectEvent {
                tag: id.to_string(),
                result,
            }))
        }

        "query_response" => {
            let id = msg.get("id")?.as_str()?;
            let result = msg.get("result").cloned().unwrap_or(Value::Null);

            Some(Event::System(SystemEvent {
                event_type: SystemEventType::TreeHash, // generic query
                tag: Some(id.to_string()),
                value: Some(result),
                id: None,
                window_id: None,
            }))
        }

        _ => None,
    }
}

/// Split a scoped ID into local ID + reversed scope.
#[cfg(feature = "wire")]
fn split_scoped_id(scoped: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = scoped.split('/').collect();
    if parts.len() <= 1 {
        (scoped.to_string(), vec![])
    } else {
        let local = parts.last().unwrap().to_string();
        let scope = parts[..parts.len() - 1]
            .iter()
            .rev()
            .map(|s| s.to_string())
            .collect();
        (local, scope)
    }
}

/// Convert event family string to EventType.
#[cfg(feature = "wire")]
fn family_to_event_type(family: &str) -> EventType {
    crate::event::family_to_event_type(family)
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
        Command::Focus(id) => {
            bridge.send_widget_op("focus", &serde_json::json!({"target": id}))?;
        }
        Command::FocusNext => {
            bridge.send_widget_op("focus_next", &Value::Object(Default::default()))?;
        }
        Command::FocusPrevious => {
            bridge.send_widget_op("focus_previous", &Value::Object(Default::default()))?;
        }
        Command::ScrollTo { target, x, y } => {
            bridge.send_widget_op("scroll_to", &serde_json::json!({
                "target": target, "offset_x": x, "offset_y": y
            }))?;
        }
        Command::ScrollBy { target, x, y } => {
            bridge.send_widget_op("scroll_by", &serde_json::json!({
                "target": target, "offset_x": x, "offset_y": y
            }))?;
        }
        Command::SnapTo { target, x, y } => {
            bridge.send_widget_op("snap_to", &serde_json::json!({
                "target": target, "x": x, "y": y
            }))?;
        }
        Command::SnapToEnd(target) => {
            bridge.send_widget_op("snap_to_end", &serde_json::json!({"target": target}))?;
        }
        Command::Window(op) => {
            execute_wire_window_op(bridge, op)?;
        }
        Command::WidgetCommand { node_id, op, payload } => {
            bridge.send_widget_command(node_id, op, payload)?;
        }
        Command::Announce(text) => {
            bridge.send_widget_op("announce", &serde_json::json!({"text": text}))?;
        }
        Command::SelectAll(target) => {
            bridge.send_widget_op("select_all", &serde_json::json!({"target": target}))?;
        }
        Command::MoveCursorToFront(target) => {
            bridge.send_widget_op("move_cursor_to_front", &serde_json::json!({"target": target}))?;
        }
        Command::MoveCursorToEnd(target) => {
            bridge.send_widget_op("move_cursor_to_end", &serde_json::json!({"target": target}))?;
        }
        Command::MoveCursorTo { target, position } => {
            bridge.send_widget_op("move_cursor_to", &serde_json::json!({
                "target": target, "position": position
            }))?;
        }
        Command::SelectRange { target, start, end } => {
            bridge.send_widget_op("select_range", &serde_json::json!({
                "target": target, "start": start, "end": end
            }))?;
        }
        _ => {
            log::debug!("unhandled wire command: {cmd:?}");
        }
    }
    Ok(())
}

/// Execute a window command via the bridge.
#[cfg(feature = "wire")]
fn execute_wire_window_op(bridge: &mut Bridge, op: &crate::command::WindowOp) -> std::io::Result<()> {
    use crate::command::WindowOp;
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
