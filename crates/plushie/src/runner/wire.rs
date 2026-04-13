//! Wire mode runner: subprocess renderer via stdin/stdout.
//!
//! Spawns the plushie renderer binary as a child process and
//! communicates over the plushie wire protocol. The app's view
//! tree is diffed and sent as patches. Events arrive from the
//! renderer and are converted to SDK Event types.
//!
//! SDK-local commands (Async, Cancel, SendAfter) are handled
//! in-process using a background tokio runtime. Async task
//! results and delayed events are delivered through an internal
//! channel and processed alongside renderer events.

#[cfg(feature = "wire")]
use serde_json::Value;
#[cfg(feature = "wire")]
use std::collections::HashMap;

#[cfg(feature = "wire")]
use super::bridge::Bridge;
#[cfg(feature = "wire")]
use super::effect_tracker::{self, EffectTracker};
#[cfg(feature = "wire")]
use super::event_bridge::SinkEvent;
#[cfg(feature = "wire")]
use crate::App;
#[cfg(feature = "wire")]
use crate::command::Command;
#[cfg(feature = "wire")]
use crate::event::{EffectEvent, EffectResult, Event};
#[cfg(feature = "wire")]
use crate::runtime::{normalize, tree_diff};

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
    log::info!(
        "renderer hello: {}",
        hello
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    );

    // Initialize the app.
    let (mut model, init_cmd) = A::init();

    let mut sub_manager = crate::runtime::subscriptions::SubscriptionManager::new();
    let mut effect_tracker = EffectTracker::new();
    let mut async_mgr = AsyncTaskManager::new();

    // First render: full snapshot.
    let view = A::view(&model, &mut crate::widget::WidgetRegistrar::new());
    let (normalized, _) = normalize::normalize(&view);
    let mut current_tree = normalized;
    bridge.send_snapshot(&serde_json::to_value(&current_tree).unwrap())?;

    // Execute the initial command (e.g. focus a field, start
    // async work) so apps work from the first frame.
    if let Err(e) = execute_wire_command(&mut bridge, init_cmd, &mut effect_tracker, &mut async_mgr)
    {
        log::error!("initial command execution failed: {e}");
    }

    // Initial subscription sync.
    let new_subs = A::subscribe(&model);
    apply_wire_sub_ops(&mut bridge, sub_manager.sync(new_subs))?;

    // Helper closure: process a single SDK event through the full MVU
    // cycle (update -> view -> normalize -> diff -> patch -> sub sync).
    let process_event = |model: &mut A::Model,
                         event: Event,
                         bridge: &mut Bridge,
                         current_tree: &mut plushie_core::protocol::TreeNode,
                         effect_tracker: &mut EffectTracker,
                         async_mgr: &mut AsyncTaskManager,
                         sub_manager: &mut crate::runtime::subscriptions::SubscriptionManager|
     -> std::io::Result<()> {
        let cmd = A::update(model, event);
        execute_wire_command(bridge, cmd, effect_tracker, async_mgr)?;

        // Re-render and diff.
        let view = A::view(model, &mut crate::widget::WidgetRegistrar::new());
        let (new_tree, warnings) = normalize::normalize(&view);
        for warning in &warnings {
            log::warn!("view normalization: {warning}");
        }

        let patches = tree_diff::diff_tree(current_tree, &new_tree);
        if !patches.is_empty() {
            let ops: Vec<Value> = patches
                .iter()
                .filter_map(|op| serde_json::to_value(op).ok())
                .collect();
            bridge.send_patch(&ops)?;
        }

        *current_tree = new_tree;

        // Sync subscriptions.
        let new_subs = A::subscribe(model);
        apply_wire_sub_ops(bridge, sub_manager.sync(new_subs))?;

        Ok(())
    };

    // Event loop.
    loop {
        // Read next event from renderer.
        let raw = match bridge.receive() {
            Ok(msg) => msg,
            Err(e) => {
                log::error!("renderer connection lost: {e}");

                // Flush all pending effects so the app gets
                // RendererRestarted events for in-flight effects.
                for (tag, _kind) in effect_tracker.flush_all() {
                    let event = Event::Effect(EffectEvent {
                        tag,
                        result: EffectResult::RendererRestarted,
                    });
                    A::update(&mut model, event);
                }

                break;
            }
        };

        // Convert wire event to SDK Event via the shared event bridge.
        // Effect responses are resolved through the tracker to recover
        // the user's tag and the effect kind for typed result parsing.
        let event = wire_to_sdk_event(&raw, &mut effect_tracker);

        if let Some(event) = event {
            if let Err(e) = process_event(
                &mut model,
                event,
                &mut bridge,
                &mut current_tree,
                &mut effect_tracker,
                &mut async_mgr,
                &mut sub_manager,
            ) {
                log::error!("command execution failed: {e}");
            }
        }

        // Drain async results and delayed events that arrived while
        // we were waiting on the bridge. Each result is a full MVU
        // cycle (update -> view -> diff -> patch).
        for sink_event in async_mgr.drain() {
            if let Some(event) = super::event_bridge::sink_event_to_sdk(sink_event) {
                if let Err(e) = process_event(
                    &mut model,
                    event,
                    &mut bridge,
                    &mut current_tree,
                    &mut effect_tracker,
                    &mut async_mgr,
                    &mut sub_manager,
                ) {
                    log::error!("async event processing failed: {e}");
                }
            }
        }

        // Check for timed-out effects after processing each message.
        for (tag, _kind) in effect_tracker.check_timeouts() {
            let event = Event::Effect(EffectEvent {
                tag,
                result: EffectResult::Timeout,
            });
            if let Err(e) = process_event(
                &mut model,
                event,
                &mut bridge,
                &mut current_tree,
                &mut effect_tracker,
                &mut async_mgr,
                &mut sub_manager,
            ) {
                log::error!("timeout command execution failed: {e}");
            }
        }
    }

    Ok(())
}

/// Convert a wire protocol JSON message to an SDK Event via the
/// shared event bridge.
///
/// Constructs a SinkEvent from the raw JSON fields and feeds it
/// through the event bridge for type-safe conversion. Effect
/// responses are resolved through the tracker to recover the user's
/// tag and the effect kind for typed result parsing.
#[cfg(feature = "wire")]
fn wire_to_sdk_event(msg: &Value, effect_tracker: &mut EffectTracker) -> Option<Event> {
    use super::event_bridge::{SinkEvent, sink_event_to_sdk};
    use plushie_core::protocol::{EffectResponse, KeyModifiers, OutgoingEvent};

    let msg_type = msg.get("type")?.as_str()?;

    let sink_event = match msg_type {
        "event" => {
            let event = OutgoingEvent {
                message_type: "event",
                session: String::new(),
                family: msg.get("family")?.as_str()?.to_string(),
                id: msg
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                window_id: msg
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                value: msg.get("value").cloned(),
                tag: msg.get("tag").and_then(|v| v.as_str()).map(String::from),
                modifiers: msg
                    .get("modifiers")
                    .and_then(|v| serde_json::from_value::<KeyModifiers>(v.clone()).ok()),
                data: msg.get("data").cloned(),
                captured: msg.get("captured").and_then(|v| v.as_bool()),
                coalesce: None,
            };
            SinkEvent::Event(event)
        }
        "effect_response" => {
            let wire_id = msg.get("id")?.as_str()?;
            let status = match msg.get("status").and_then(|v| v.as_str()) {
                Some("ok") => "ok",
                Some("cancelled") => "cancelled",
                Some("unsupported") => "unsupported",
                _ => "error",
            };

            // Resolve via the tracker for typed result parsing.
            if let Some((tag, kind)) = effect_tracker.resolve(wire_id) {
                let error_as_value = msg
                    .get("error")
                    .and_then(|v| v.as_str())
                    .map(|e| Value::String(e.to_string()));
                let value = msg.get("result").or(error_as_value.as_ref());
                let result = EffectResult::parse(&kind, status, value);
                return Some(Event::Effect(EffectEvent { tag, result }));
            }

            // Fallback: no tracker entry (e.g. stale response after
            // restart). Use the untyped event bridge conversion.
            let response = EffectResponse {
                message_type: "effect_response",
                session: String::new(),
                id: wire_id.to_string(),
                status,
                result: msg.get("result").cloned(),
                error: msg.get("error").and_then(|v| v.as_str()).map(String::from),
            };
            SinkEvent::EffectResponse(response)
        }
        "query_response" | "op_query_response" => {
            let kind = msg
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tag = msg
                .get("tag")
                .or_else(|| msg.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let data = msg
                .get("result")
                .or_else(|| msg.get("data"))
                .cloned()
                .unwrap_or(Value::Null);
            SinkEvent::QueryResponse { kind, tag, data }
        }
        _ => return None,
    };

    sink_event_to_sdk(sink_event)
}

// ---------------------------------------------------------------------------
// Async task manager: handles Command::Async, Cancel, and SendAfter
// ---------------------------------------------------------------------------

/// Manages SDK-local async tasks and delayed events for wire mode.
///
/// Spawns a background tokio runtime for async work. Results and
/// delayed events are sent through an mpsc channel that the main
/// event loop polls between renderer messages.
#[cfg(feature = "wire")]
struct AsyncTaskManager {
    runtime: tokio::runtime::Runtime,
    tx: std::sync::mpsc::Sender<SinkEvent>,
    rx: std::sync::mpsc::Receiver<SinkEvent>,
    running: HashMap<String, tokio::task::JoinHandle<()>>,
}

#[cfg(feature = "wire")]
impl AsyncTaskManager {
    fn new() -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for wire async");
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            runtime: rt,
            tx,
            rx,
            running: HashMap::new(),
        }
    }

    fn spawn_async(&mut self, tag: String, task_fn: crate::command::AsyncTaskFn) {
        // Cancel any existing task with the same tag.
        if let Some(handle) = self.running.remove(&tag) {
            handle.abort();
        }

        let tx = self.tx.clone();
        let tag_clone = tag.clone();
        let handle = self.runtime.spawn(async move {
            let future = (task_fn)();
            let result = future.await;
            let _ = tx.send(SinkEvent::AsyncResult {
                tag: tag_clone,
                result,
            });
        });
        self.running.insert(tag, handle);
    }

    fn cancel(&mut self, tag: &str) {
        if let Some(handle) = self.running.remove(tag) {
            handle.abort();
        }
    }

    fn send_after(&self, delay: std::time::Duration, event: crate::event::Event) {
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            tokio::time::sleep(delay).await;
            let _ = tx.send(SinkEvent::DelayedEvent(event));
        });
    }

    /// Drain all pending async results and delayed events.
    ///
    /// Also cleans up completed task handles from the running map
    /// to prevent unbounded growth.
    fn drain(&mut self) -> Vec<SinkEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            // Remove completed task handles to free memory.
            if let SinkEvent::AsyncResult { ref tag, .. } = event {
                self.running.remove(tag);
            }
            events.push(event);
        }
        events
    }
}

/// Execute a Command by sending messages through the bridge.
///
/// Renderer operations are sent over the wire. SDK-local commands
/// (Async, Cancel, SendAfter) are handled by the AsyncTaskManager.
#[cfg(feature = "wire")]
fn execute_wire_command(
    bridge: &mut Bridge,
    cmd: Command,
    effect_tracker: &mut EffectTracker,
    async_mgr: &mut AsyncTaskManager,
) -> std::io::Result<()> {
    match cmd {
        Command::None => {}
        Command::Exit => {
            bridge.send_widget_op("exit", &Value::Null)?;
        }
        Command::Batch(cmds) => {
            for c in cmds {
                execute_wire_command(bridge, c, effect_tracker, async_mgr)?;
            }
        }
        Command::Renderer(ref op) => {
            execute_wire_renderer_op(bridge, op, effect_tracker)?;
        }
        Command::Async { tag, task } => {
            async_mgr.spawn_async(tag, task);
        }
        Command::Cancel { tag } => {
            async_mgr.cancel(&tag);
        }
        Command::SendAfter { delay, event } => {
            async_mgr.send_after(delay, *event);
        }
    }
    Ok(())
}

/// Serialize a RendererOp to wire messages and send via the bridge.
#[cfg(feature = "wire")]
fn execute_wire_renderer_op(
    bridge: &mut Bridge,
    op: &plushie_core::ops::RendererOp,
    effect_tracker: &mut EffectTracker,
) -> std::io::Result<()> {
    use plushie_core::ops::{ImageOp, RendererOp, SystemOp, SystemQuery, WindowQuery};
    use serde_json::json;

    match op {
        RendererOp::Command { id, family, value } => bridge.send_command(id, family, value),
        RendererOp::Commands(commands) => {
            for cmd in commands {
                bridge.send_command(&cmd.id, &cmd.family, &cmd.value)?;
            }
            Ok(())
        }
        RendererOp::FocusNext => bridge.send_widget_op("focus_next", &json!({})),
        RendererOp::FocusPrevious => bridge.send_widget_op("focus_previous", &json!({})),
        RendererOp::Window(op) => execute_wire_window_op(bridge, op),
        RendererOp::WindowQuery(query) => {
            let (op_name, window_id, tag) = match query {
                WindowQuery::GetSize { window_id, tag } => ("get_size", window_id, tag),
                WindowQuery::GetPosition { window_id, tag } => ("get_position", window_id, tag),
                WindowQuery::IsMaximized { window_id, tag } => ("is_maximized", window_id, tag),
                WindowQuery::IsMinimized { window_id, tag } => ("is_minimized", window_id, tag),
                WindowQuery::GetMode { window_id, tag } => ("get_mode", window_id, tag),
                WindowQuery::GetScaleFactor { window_id, tag } => {
                    ("get_scale_factor", window_id, tag)
                }
                WindowQuery::MonitorSize { window_id, tag } => ("monitor_size", window_id, tag),
                WindowQuery::RawId { window_id, tag } => ("raw_id", window_id, tag),
            };
            bridge.send_window_op(op_name, window_id, &json!({"tag": tag}))
        }
        RendererOp::SystemOp(SystemOp::AllowAutomaticTabbing(enabled)) => bridge.send(&json!({
            "type": "system_op",
            "session": "",
            "op": "allow_automatic_tabbing",
            "settings": {"enabled": enabled},
        })),
        RendererOp::SystemQuery(query) => {
            let (op_name, tag) = match query {
                SystemQuery::GetTheme { tag } => ("get_system_theme", tag),
                SystemQuery::GetInfo { tag } => ("get_system_info", tag),
            };
            bridge.send(&json!({
                "type": "system_query",
                "session": "",
                "op": op_name,
                "settings": {"tag": tag},
            }))
        }
        RendererOp::Effect {
            tag,
            request,
            timeout,
        } => {
            let kind = request.kind();
            let effective_timeout =
                timeout.unwrap_or_else(|| effect_tracker::default_timeout(kind));
            let wire_id = effect_tracker.track(tag, kind, effective_timeout);
            let (_, payload) = plushie_core::ops::effect_request_to_wire(request);
            bridge.send_effect(&wire_id, kind, &payload)
        }
        RendererOp::Image(image_op) => {
            let msg = match image_op {
                ImageOp::Create { handle, data } => json!({
                    "type": "image_op", "session": "",
                    "op": "create_from_bytes",
                    "handle": handle, "data": base64_encode(data),
                }),
                ImageOp::CreateRaw {
                    handle,
                    width,
                    height,
                    pixels,
                } => json!({
                    "type": "image_op", "session": "",
                    "op": "create_from_rgba",
                    "handle": handle, "pixels": base64_encode(pixels),
                    "width": width, "height": height,
                }),
                ImageOp::Update { handle, data } => json!({
                    "type": "image_op", "session": "",
                    "op": "update",
                    "handle": handle, "data": base64_encode(data),
                }),
                ImageOp::UpdateRaw {
                    handle,
                    width,
                    height,
                    pixels,
                } => json!({
                    "type": "image_op", "session": "",
                    "op": "update_raw",
                    "handle": handle, "pixels": base64_encode(pixels),
                    "width": width, "height": height,
                }),
                ImageOp::Delete(handle) => json!({
                    "type": "image_op", "session": "",
                    "op": "delete", "handle": handle,
                }),
                ImageOp::List { tag } => json!({
                    "type": "image_op", "session": "",
                    "op": "list", "handle": "",
                    "settings": {"tag": tag},
                }),
                ImageOp::Clear => json!({
                    "type": "image_op", "session": "",
                    "op": "clear", "handle": "",
                }),
            };
            bridge.send(&msg)
        }
        RendererOp::Announce(text) => bridge.send_widget_op("announce", &json!({"text": text})),
        RendererOp::LoadFont(data) => {
            bridge.send_widget_op("load_font", &json!({"data": base64_encode(data)}))
        }
        RendererOp::Subscribe {
            kind,
            tag,
            max_rate,
            window_id,
        } => bridge.send_subscribe(kind, tag, *max_rate, window_id.as_deref()),
        RendererOp::Unsubscribe { kind, tag } => bridge.send_unsubscribe(kind, tag),
        RendererOp::TreeHash { tag } => bridge.send_widget_op("tree_hash", &json!({"tag": tag})),
        RendererOp::FindFocused { tag } => {
            bridge.send_widget_op("find_focused", &json!({"tag": tag}))
        }
        RendererOp::AdvanceFrame { timestamp } => {
            bridge.send_widget_op("advance_frame", &json!({"timestamp": timestamp}))
        }
    }
}

/// Execute a window operation via the bridge.
#[cfg(feature = "wire")]
fn execute_wire_window_op(
    bridge: &mut Bridge,
    op: &plushie_core::ops::WindowOp,
) -> std::io::Result<()> {
    use plushie_core::ops::WindowOp;
    use serde_json::json;

    match op {
        WindowOp::Close(id) => bridge.send_widget_op("close_window", &json!({"window_id": id})),
        WindowOp::Resize {
            window_id,
            width,
            height,
        } => bridge.send_window_op(
            "resize",
            window_id,
            &json!({"width": width, "height": height}),
        ),
        WindowOp::Move { window_id, x, y } => {
            bridge.send_window_op("move", window_id, &json!({"x": x, "y": y}))
        }
        WindowOp::Maximize {
            window_id,
            maximized,
        } => bridge.send_window_op("maximize", window_id, &json!({"maximized": maximized})),
        WindowOp::Minimize {
            window_id,
            minimized,
        } => bridge.send_window_op("minimize", window_id, &json!({"minimized": minimized})),
        WindowOp::SetMode { window_id, mode } => {
            bridge.send_window_op("set_mode", window_id, &json!({"mode": mode.to_string()}))
        }
        WindowOp::ToggleMaximize(id) => bridge.send_window_op("toggle_maximize", id, &json!({})),
        WindowOp::ToggleDecorations(id) => {
            bridge.send_window_op("toggle_decorations", id, &json!({}))
        }
        WindowOp::FocusWindow(id) => bridge.send_window_op("gain_focus", id, &json!({})),
        WindowOp::SetLevel { window_id, level } => {
            bridge.send_window_op("set_level", window_id, &json!({"level": level.to_string()}))
        }
        WindowOp::DragWindow(id) => bridge.send_window_op("drag", id, &json!({})),
        WindowOp::DragResize {
            window_id,
            direction,
        } => bridge.send_window_op("drag_resize", window_id, &json!({"direction": direction})),
        WindowOp::RequestAttention { window_id, urgency } => {
            let mut settings = json!({});
            if let Some(u) = urgency {
                settings["urgency"] = json!(u);
            }
            bridge.send_window_op("request_attention", window_id, &settings)
        }
        WindowOp::Screenshot { window_id, tag } => {
            bridge.send_window_op("screenshot", window_id, &json!({"tag": tag}))
        }
        WindowOp::SetResizable {
            window_id,
            resizable,
        } => bridge.send_window_op("set_resizable", window_id, &json!({"resizable": resizable})),
        WindowOp::SetMinSize {
            window_id,
            width,
            height,
        } => bridge.send_window_op(
            "set_min_size",
            window_id,
            &json!({"width": width, "height": height}),
        ),
        WindowOp::SetMaxSize {
            window_id,
            width,
            height,
        } => bridge.send_window_op(
            "set_max_size",
            window_id,
            &json!({"width": width, "height": height}),
        ),
        WindowOp::EnableMousePassthrough(id) => {
            bridge.send_window_op("mouse_passthrough", id, &json!({"enabled": true}))
        }
        WindowOp::DisableMousePassthrough(id) => {
            bridge.send_window_op("mouse_passthrough", id, &json!({"enabled": false}))
        }
        WindowOp::ShowSystemMenu(id) => bridge.send_window_op("show_system_menu", id, &json!({})),
        WindowOp::SetIcon {
            window_id,
            data,
            width,
            height,
        } => bridge.send_window_op(
            "set_icon",
            window_id,
            &json!({
                "data": base64_encode(data), "width": width, "height": height,
            }),
        ),
        WindowOp::SetResizeIncrements {
            window_id,
            width,
            height,
        } => bridge.send_window_op(
            "set_resize_increments",
            window_id,
            &json!({
                "width": width, "height": height,
            }),
        ),
    }
}

/// Base64-encode binary data for JSON wire transport.
#[cfg(feature = "wire")]
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
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
        json["widget_config"] =
            serde_json::to_value(&settings.widget_config).unwrap_or(Value::Null);
    }

    json
}

/// Apply subscription operations by sending wire messages.
#[cfg(feature = "wire")]
fn apply_wire_sub_ops(
    bridge: &mut Bridge,
    ops: Vec<crate::runtime::subscriptions::SubOp>,
) -> std::io::Result<()> {
    use crate::runtime::subscriptions::SubOp;
    for op in ops {
        match op {
            SubOp::Subscribe {
                kind,
                tag,
                max_rate,
                window_id,
            } => {
                bridge.send_subscribe(&kind, &tag, max_rate, window_id.as_deref())?;
            }
            SubOp::Unsubscribe { kind, tag } => {
                bridge.send_unsubscribe(&kind, &tag)?;
            }
            SubOp::StartTimer { tag, .. } | SubOp::StopTimer { tag, .. } => {
                log::debug!("timer subscription not yet implemented in wire mode: {tag}");
            }
        }
    }
    Ok(())
}
