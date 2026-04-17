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
use plushie_core::outgoing_message::OutgoingMessage;
#[cfg(feature = "wire")]
use serde_json::Value;
#[cfg(feature = "wire")]
use std::collections::HashMap;
#[cfg(feature = "wire")]
use std::io;

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
use crate::runtime::tree_diff;
#[cfg(feature = "wire")]
use crate::settings::ExitReason;

/// Run the app in wire mode.
///
/// Spawns the renderer binary at `binary_path` and communicates
/// over stdin/stdout using the plushie wire protocol.
///
/// Auto-restart is governed by [`App::restart_policy`]. On every
/// unexpected renderer exit the app's [`App::handle_renderer_exit`]
/// hook is called with the matching [`ExitReason`]; if the policy
/// allows, the runner then respawns the subprocess and resends
/// Settings + tree snapshot + subscription state. After
/// `max_restarts` exhaustion the hook fires once more with
/// [`ExitReason::MaxRestartsReached`] and the function returns
/// [`crate::Error::RendererExit`].
///
/// Uses a private 2-worker tokio runtime for SDK-local async work
/// (`Command::Async`, `Command::Stream`, `Command::SendAfter`,
/// effect-timeout scheduling). Apps that already have a tokio
/// runtime should prefer [`run_wire_with_runtime`] to avoid a
/// second runtime living alongside theirs.
#[cfg(feature = "wire")]
pub fn run_wire<A: App>(binary_path: &str) -> crate::Result {
    run_wire_inner::<A>(binary_path, None)
}

/// Run the app in wire mode using a caller-provided tokio runtime.
///
/// Equivalent to [`run_wire`] except SDK-local async tasks are
/// spawned on the supplied [`tokio::runtime::Handle`]. Integration
/// point for apps that already drive their own tokio runtime and
/// want to avoid a second one being created implicitly.
///
/// The handle is only used to spawn tasks; the runtime itself is
/// owned by the caller and must outlive the returned [`crate::Result`].
#[cfg(feature = "wire")]
pub fn run_wire_with_runtime<A: App>(
    binary_path: &str,
    runtime: tokio::runtime::Handle,
) -> crate::Result {
    run_wire_inner::<A>(binary_path, Some(runtime))
}

#[cfg(feature = "wire")]
fn run_wire_inner<A: App>(
    binary_path: &str,
    runtime: Option<tokio::runtime::Handle>,
) -> crate::Result {
    let settings = build_settings::<A>();
    let policy = A::restart_policy();

    // Initialize the app once. The model persists across restarts.
    let (mut model, init_cmd) = A::init();

    let mut sub_manager = crate::runtime::subscriptions::SubscriptionManager::new();
    let mut effect_tracker = EffectTracker::new();
    let mut async_mgr = AsyncTaskManager::new(runtime);
    let mut view_errors = crate::runtime::view_errors::ViewErrors::default();

    // Initial view; shared across restarts as the "current tree". If
    // the first view call panics there is no last-good tree to fall
    // back to, so we use an empty container as a seed.
    let seed = plushie_core::protocol::TreeNode {
        id: String::new(),
        type_name: "container".to_string(),
        props: plushie_core::protocol::Props::Typed(plushie_core::protocol::PropMap::new()),
        children: vec![],
    };
    let mut current_tree = match crate::runtime::view_errors::run_guarded_view_wire::<A>(
        &mut view_errors,
        &model,
        &seed,
    ) {
        crate::runtime::view_errors::ViewOutcome::Ok(tree, _) => tree,
        crate::runtime::view_errors::ViewOutcome::Panicked { last_good, .. } => last_good,
    };

    let mut restart_count: u32 = 0;
    let mut pending_init: Option<Command> = Some(init_cmd);

    loop {
        // Bring up (or respawn) the renderer and establish the
        // reader thread. On respawn we resend settings + snapshot +
        // subscription state so the renderer catches up.
        let mut bridge = Bridge::spawn(binary_path)
            .map_err(|e| crate::Error::spawn(binary_path.to_string(), e))?;

        bridge.send_settings(&settings)?;

        // Synchronous hello read (reader thread not started yet).
        let hello = bridge.receive()?;
        log::info!(
            "renderer hello: {}",
            hello
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );

        bridge.start_reader()?;

        // Send snapshot so the renderer has the current tree.
        let snapshot_value = serde_json::to_value(&current_tree)
            .map_err(|e| crate::Error::WireEncode(format!("snapshot: {e}")))?;
        bridge.send_snapshot(&snapshot_value)?;

        // Execute the initial command once (only on the first spawn).
        // Subscriptions and in-flight commands are replayed below.
        if let Some(cmd) = pending_init.take()
            && let Err(e) =
                execute_wire_command(&mut bridge, cmd, &mut effect_tracker, &mut async_mgr)
        {
            log::error!("initial command execution failed: {e}");
        }

        // Subscription sync. On restart this replays the full set.
        let new_subs = A::subscribe(&model);
        // Force a full resync by clearing the manager state so every
        // current subscription is re-emitted as a Subscribe op.
        if restart_count > 0 {
            sub_manager = crate::runtime::subscriptions::SubscriptionManager::new();
        }
        apply_wire_sub_ops(&mut bridge, &mut async_mgr, sub_manager.sync(new_subs))?;

        // After a restart, flush all in-flight effects with
        // RendererRestarted so the app can react (image re-upload,
        // etc.). On first spawn this is a no-op.
        if restart_count > 0 {
            for (tag, _kind) in effect_tracker.flush_all() {
                let event = Event::Effect(EffectEvent {
                    tag,
                    result: EffectResult::RendererRestarted,
                });
                A::update(&mut model, event);
            }
        }

        // Run the main event loop until the renderer exits. The
        // inner function returns the classified ExitReason on break.
        let reason = run_session::<A>(
            &mut bridge,
            &mut model,
            &mut current_tree,
            &mut effect_tracker,
            &mut async_mgr,
            &mut sub_manager,
            &mut view_errors,
            policy.heartbeat_interval,
        );

        log::warn!(
            "plushie wire: renderer exited ({}); restart count = {}",
            reason.label(),
            restart_count
        );

        // Always call the app's exit hook; this lets apps save state
        // or log before the (potentially final) restart attempt.
        A::handle_renderer_exit(&mut model, reason.clone());

        // Shutdown: do not restart. Drain any in-flight effects with
        // EffectResult::Shutdown so the app can release associated
        // resources (progress bars, loading flags) instead of
        // waiting forever on a response that will never come.
        if matches!(reason, ExitReason::Shutdown) {
            flush_effects_on_shutdown::<A>(&mut model, &mut effect_tracker);
            return Ok(());
        }

        // Restart policy: if we're out of attempts, fire a final
        // hook call and return the typed error.
        if restart_count >= policy.max_restarts {
            let final_reason = ExitReason::MaxRestartsReached {
                last_reason: Box::new(reason.clone()),
            };
            A::handle_renderer_exit(&mut model, final_reason.clone());
            // Max-restarts is a terminal state; flush pending effects
            // as Shutdown too so the app sees the same drained state
            // as on a clean shutdown.
            flush_effects_on_shutdown::<A>(&mut model, &mut effect_tracker);
            return Err(crate::Error::RendererExit(final_reason));
        }

        // Exponential backoff before respawning.
        let delay = policy
            .restart_delay
            .saturating_mul(2u32.saturating_pow(restart_count));
        log::info!(
            "plushie wire: restarting renderer in {}ms (attempt {}/{})",
            delay.as_millis(),
            restart_count + 1,
            policy.max_restarts
        );
        std::thread::sleep(delay);
        restart_count += 1;

        // Bridge is dropped here; its Drop kills + reaps the old
        // child. We rebuild cleanly next iteration.
        drop(bridge);
    }
}

/// Drain pending effects with [`EffectResult::Shutdown`].
///
/// Called on runner teardown (clean shutdown or max-restarts
/// exhaustion) so the app observes a terminal event for each
/// in-flight effect rather than a silent drop.
#[cfg(feature = "wire")]
fn flush_effects_on_shutdown<A: App>(model: &mut A::Model, effect_tracker: &mut EffectTracker) {
    let pending = effect_tracker.pending_count();
    if pending == 0 {
        return;
    }
    log::info!("wire shutdown: flushing {pending} in-flight effect(s) as Shutdown");
    for (tag, _kind) in effect_tracker.flush_all() {
        let event = Event::Effect(EffectEvent {
            tag,
            result: EffectResult::Shutdown,
        });
        // Fire-and-forget: commands returned from update() are
        // discarded on teardown since the wire is already closing.
        let _ = A::update(model, event);
    }
}

/// Run one session against an already-spawned renderer. Returns the
/// classified [`ExitReason`] when the session ends (renderer
/// disconnect, crash, heartbeat timeout, or explicit shutdown).
#[allow(clippy::too_many_arguments)]
#[cfg(feature = "wire")]
fn run_session<A: App>(
    bridge: &mut Bridge,
    model: &mut A::Model,
    current_tree: &mut plushie_core::protocol::TreeNode,
    effect_tracker: &mut EffectTracker,
    async_mgr: &mut AsyncTaskManager,
    sub_manager: &mut crate::runtime::subscriptions::SubscriptionManager,
    view_errors: &mut crate::runtime::view_errors::ViewErrors,
    heartbeat_interval: Option<std::time::Duration>,
) -> ExitReason {
    loop {
        let incoming = bridge.recv_timeout(heartbeat_interval);
        match incoming {
            super::bridge::Incoming::Message(raw) => {
                let events = wire_to_sdk_events(&raw, effect_tracker, async_mgr);
                for event in events {
                    if let Err(e) = process_event::<A>(
                        model,
                        event,
                        bridge,
                        current_tree,
                        effect_tracker,
                        async_mgr,
                        sub_manager,
                        view_errors,
                    ) {
                        log::error!("command execution failed: {e}");
                    }
                }
            }
            super::bridge::Incoming::Error(e) => {
                log::error!("renderer connection lost: {e}");
                return classify_exit(bridge, &e);
            }
            super::bridge::Incoming::Timeout => {
                log::warn!(
                    "plushie wire: no message in {:?}, triggering restart",
                    heartbeat_interval
                );
                return ExitReason::HeartbeatTimeout;
            }
        }

        // Drain async results, delayed events, and effect timeouts
        // that arrived while we were waiting on the bridge.
        for sink_event in async_mgr.drain() {
            let event = match sink_event {
                SinkEvent::EffectTimeout { wire_id } => {
                    // Resolve to the user tag via the tracker; emit
                    // Timeout. Skip silently if already resolved
                    // (response-raced-the-deadline).
                    effect_tracker.resolve(&wire_id).map(|(tag, _kind)| {
                        Event::Effect(EffectEvent {
                            tag,
                            result: EffectResult::Timeout,
                        })
                    })
                }
                other => super::event_bridge::sink_event_to_sdk(other),
            };
            if let Some(event) = event
                && let Err(e) = process_event::<A>(
                    model,
                    event,
                    bridge,
                    current_tree,
                    effect_tracker,
                    async_mgr,
                    sub_manager,
                    view_errors,
                )
            {
                log::error!("async event processing failed: {e}");
            }
        }
    }
}

/// Process a single SDK event through the full MVU cycle:
/// update -> view -> normalize -> diff -> patch -> sub sync.
///
/// Wraps `A::view()` in the view-errors guard so a panic falls
/// back to the last-good tree and increments the consecutive
/// counter (frozen-UI overlay at threshold).
#[allow(clippy::too_many_arguments)]
#[cfg(feature = "wire")]
fn process_event<A: App>(
    model: &mut A::Model,
    event: Event,
    bridge: &mut Bridge,
    current_tree: &mut plushie_core::protocol::TreeNode,
    effect_tracker: &mut EffectTracker,
    async_mgr: &mut AsyncTaskManager,
    sub_manager: &mut crate::runtime::subscriptions::SubscriptionManager,
    view_errors: &mut crate::runtime::view_errors::ViewErrors,
) -> crate::Result {
    let cmd = A::update(model, event);
    execute_wire_command(bridge, cmd, effect_tracker, async_mgr)?;

    // Re-render and diff under panic guard.
    let outcome =
        crate::runtime::view_errors::run_guarded_view_wire::<A>(view_errors, model, current_tree);
    let new_tree = match outcome {
        crate::runtime::view_errors::ViewOutcome::Ok(tree, warnings) => {
            for warning in &warnings {
                log::warn!("view normalization: {warning}");
            }
            tree
        }
        crate::runtime::view_errors::ViewOutcome::Panicked { last_good, .. } => last_good,
    };

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
    apply_wire_sub_ops(bridge, async_mgr, sub_manager.sync(new_subs))?;

    Ok(())
}

/// Classify a bridge receive error into a typed [`ExitReason`].
///
/// `UnexpectedEof` indicates the renderer closed stdout cleanly but
/// without sending a proper shutdown marker; everything else is
/// treated as a crash. Reaps the child (non-blocking) to capture the
/// exit code for `Crash`.
#[cfg(feature = "wire")]
fn classify_exit(bridge: &mut Bridge, err: &io::Error) -> ExitReason {
    match err.kind() {
        io::ErrorKind::UnexpectedEof => ExitReason::ConnectionLost,
        _ => {
            let code = bridge.try_reap();
            ExitReason::Crash {
                message: err.to_string(),
                code,
            }
        }
    }
}

/// Typed representation of a single renderer -> SDK message.
///
/// Mirrors the outgoing shapes in plushie-core's OutgoingEvent /
/// response families. Unknown messages decode into
/// [`IncomingRendererMessage::Unknown`] so a SDK/renderer version
/// skew produces a diagnostic rather than a silent drop.
#[cfg(feature = "wire")]
#[derive(Debug)]
enum IncomingRendererMessage {
    Event {
        family: String,
        id: String,
        value: Option<Value>,
        tag: Option<String>,
        modifiers: Option<plushie_core::protocol::KeyModifiers>,
        captured: Option<bool>,
    },
    EffectResponse {
        id: String,
        status: &'static str,
        result: Option<Value>,
        error: Option<String>,
    },
    QueryResponse {
        kind: String,
        tag: String,
        data: Value,
    },
    InteractResponse {
        events: Vec<Value>,
    },
    /// Message type the SDK doesn't recognise. Preserves the type
    /// string and raw payload so diagnostics can surface version
    /// skew cleanly.
    Unknown {
        msg_type: String,
        #[allow(dead_code)] // Retained for log-only diagnostic use.
        raw: Value,
    },
}

/// Decode a top-level wire message into a typed variant.
#[cfg(feature = "wire")]
fn decode_incoming(msg: &Value) -> Option<IncomingRendererMessage> {
    let msg_type = msg.get("type").and_then(|v| v.as_str())?;
    let decoded = match msg_type {
        "event" => {
            let family = msg.get("family").and_then(|v| v.as_str())?.to_string();
            let id = msg
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            IncomingRendererMessage::Event {
                family,
                id,
                value: msg.get("value").cloned(),
                tag: msg
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                modifiers: msg
                    .get("modifiers")
                    .and_then(|v| serde_json::from_value(v.clone()).ok()),
                captured: msg.get("captured").and_then(|v| v.as_bool()),
            }
        }
        "effect_response" => {
            let id = msg.get("id").and_then(|v| v.as_str())?.to_string();
            let status = match msg.get("status").and_then(|v| v.as_str()) {
                Some("ok") => "ok",
                Some("cancelled") => "cancelled",
                Some("unsupported") => "unsupported",
                _ => "error",
            };
            IncomingRendererMessage::EffectResponse {
                id,
                status,
                result: msg.get("result").cloned(),
                error: msg.get("error").and_then(|v| v.as_str()).map(String::from),
            }
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
            IncomingRendererMessage::QueryResponse { kind, tag, data }
        }
        "interact_response" => {
            let events = msg
                .get("events")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            IncomingRendererMessage::InteractResponse { events }
        }
        other => IncomingRendererMessage::Unknown {
            msg_type: other.to_string(),
            raw: msg.clone(),
        },
    };
    Some(decoded)
}

/// Convert a wire protocol JSON message to SDK Events.
///
/// Decodes into [`IncomingRendererMessage`] first so unknown
/// message types are preserved as `Unknown` and surface through the
/// diagnostic channel instead of being silently dropped.
#[cfg(feature = "wire")]
fn wire_to_sdk_events(
    msg: &Value,
    effect_tracker: &mut EffectTracker,
    async_mgr: &mut AsyncTaskManager,
) -> Vec<Event> {
    use super::event_bridge::{SinkEvent, sink_event_to_sdk};
    use plushie_core::protocol::{EffectResponse, OutgoingEvent};

    let Some(decoded) = decode_incoming(msg) else {
        // No `type` field at all: not our message shape.
        log::warn!("[code=unknown_message_type] wire message without `type` field: {msg}");
        return vec![];
    };

    let sink_event = match decoded {
        IncomingRendererMessage::InteractResponse { events } => {
            // Interact responses contain multiple events that each
            // need a full MVU cycle. Recursively convert each one.
            return events
                .iter()
                .flat_map(|e| wire_to_sdk_events(e, effect_tracker, async_mgr))
                .collect();
        }
        IncomingRendererMessage::Unknown { msg_type, raw: _ } => {
            // TODO(M-6): Replace with structured diagnostic event
            // once the M-6 inbound diagnostic stream is wired. For
            // now a tagged log line is the observable path.
            log::error!(
                "[code=unknown_message_type] unrecognised renderer message type `{msg_type}`; \
                     likely a host/renderer version skew"
            );
            return vec![];
        }
        IncomingRendererMessage::Event {
            family,
            id,
            value,
            tag,
            modifiers,
            captured,
        } => {
            let mut event = OutgoingEvent::widget_event(family, id, value);
            event.tag = tag;
            event.modifiers = modifiers;
            event.captured = captured;
            SinkEvent::Event(event)
        }
        IncomingRendererMessage::EffectResponse {
            id: wire_id,
            status,
            result,
            error,
        } => {
            // Cancel the tokio-scheduled deadline task now that a
            // real response has arrived. Cheap if the task already
            // completed (rare).
            async_mgr.cancel_effect_timeout(&wire_id);
            // Resolve via the tracker for typed result parsing.
            if let Some((tag, kind)) = effect_tracker.resolve(&wire_id) {
                let error_as_value = error.as_ref().map(|e| Value::String(e.clone()));
                let value = result.as_ref().or(error_as_value.as_ref());
                let result = EffectResult::parse(&kind, status, value);
                return vec![Event::Effect(EffectEvent { tag, result })];
            }

            let response = EffectResponse {
                message_type: "effect_response",
                session: String::new(),
                id: wire_id,
                status,
                result,
                error,
            };
            SinkEvent::EffectResponse(response)
        }
        IncomingRendererMessage::QueryResponse { kind, tag, data } => {
            SinkEvent::QueryResponse { kind, tag, data }
        }
    };

    sink_event_to_sdk(sink_event).into_iter().collect()
}

// ---------------------------------------------------------------------------
// Async task manager: handles Command::Async, Cancel, and SendAfter
// ---------------------------------------------------------------------------

/// Manages SDK-local async tasks and delayed events for wire mode.
///
/// Spawns a background tokio runtime for async work. Results and
/// delayed events are sent through a bounded mpsc channel that the
/// main event loop polls between renderer messages. The 1024 slot
/// capacity matches the backpressure pattern used by the headless
/// multiplex writer.
/// Tokio runtime backing: either a caller-supplied handle or a
/// privately owned 2-worker runtime.
#[cfg(feature = "wire")]
enum RuntimeBacking {
    Handle(tokio::runtime::Handle),
    Owned(tokio::runtime::Runtime),
}

#[cfg(feature = "wire")]
impl RuntimeBacking {
    fn handle(&self) -> tokio::runtime::Handle {
        match self {
            Self::Handle(h) => h.clone(),
            Self::Owned(rt) => rt.handle().clone(),
        }
    }
}

#[cfg(feature = "wire")]
struct AsyncTaskManager {
    runtime: RuntimeBacking,
    tx: std::sync::mpsc::SyncSender<SinkEvent>,
    rx: std::sync::mpsc::Receiver<SinkEvent>,
    running: HashMap<String, tokio::task::JoinHandle<()>>,
    /// Pending effect timeout tasks keyed by tracker wire ID.
    ///
    /// Aborted when a response arrives so the deadline task does
    /// not fire for a completed effect.
    effect_timeouts: HashMap<String, tokio::task::JoinHandle<()>>,
    /// Recurring-timer tasks keyed by subscription tag.
    ///
    /// Each tagged `Subscription::every` spawns a tokio interval
    /// task that pushes a `SinkEvent::DelayedEvent` carrying a
    /// timer-tick event on each fire. Aborted on `stop_timer` or
    /// AsyncTaskManager drop.
    timers: HashMap<String, tokio::task::JoinHandle<()>>,
}

#[cfg(feature = "wire")]
impl AsyncTaskManager {
    /// Bounded capacity for async-result delivery. Matches the
    /// headless multiplex writer pattern; generous for typical
    /// workloads while preventing runaway growth.
    const CHANNEL_CAPACITY: usize = 1024;

    fn new(external: Option<tokio::runtime::Handle>) -> Self {
        let runtime = match external {
            Some(handle) => RuntimeBacking::Handle(handle),
            None => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .expect("failed to create tokio runtime for wire async");
                RuntimeBacking::Owned(rt)
            }
        };
        let (tx, rx) = std::sync::mpsc::sync_channel(Self::CHANNEL_CAPACITY);
        Self {
            runtime,
            tx,
            rx,
            running: HashMap::new(),
            effect_timeouts: HashMap::new(),
            timers: HashMap::new(),
        }
    }

    /// Spawn a tokio sleep task that posts an [`SinkEvent::EffectTimeout`]
    /// once the deadline elapses.
    ///
    /// The task is keyed by the tracker's wire ID so a response can
    /// cancel it via [`Self::cancel_effect_timeout`]. The timeout
    /// fires regardless of whether the wire reader is currently
    /// blocked, closing the "deadline only checked on next incoming
    /// message" gap in the old polling design.
    fn schedule_effect_timeout(&mut self, wire_id: String, duration: std::time::Duration) {
        // Replace any prior timeout task for this wire ID.
        if let Some(handle) = self.effect_timeouts.remove(&wire_id) {
            handle.abort();
        }
        let tx = self.tx.clone();
        let wire_id_for_task = wire_id.clone();
        let handle = self.runtime.handle().spawn(async move {
            tokio::time::sleep(duration).await;
            let _ = tx.send(SinkEvent::EffectTimeout {
                wire_id: wire_id_for_task,
            });
        });
        self.effect_timeouts.insert(wire_id, handle);
    }

    /// Cancel a pending effect-timeout task by wire ID.
    ///
    /// Called when a response arrives before the deadline so the
    /// scheduled SinkEvent::EffectTimeout is never emitted.
    fn cancel_effect_timeout(&mut self, wire_id: &str) {
        if let Some(handle) = self.effect_timeouts.remove(wire_id) {
            handle.abort();
        }
    }

    /// Start a recurring SDK-side timer for [`Subscription::every`].
    ///
    /// Spawns a tokio interval task on the runtime that pushes a
    /// `SinkEvent::DelayedEvent(Event::Timer(...))` on each tick. The
    /// main event loop picks those up via [`Self::drain`] and routes
    /// them through the same path as async results.
    ///
    /// Replaces any existing timer with the same tag (matches the
    /// direct mode behaviour where `active_timers.insert` replaces).
    fn start_timer(&mut self, tag: String, interval: std::time::Duration) {
        if let Some(handle) = self.timers.remove(&tag) {
            handle.abort();
        }
        let tx = self.tx.clone();
        let tag_for_task = tag.clone();
        let handle = self.runtime.handle().spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // Skip the immediate tick (tokio::interval fires once at
            // start); users expect the first fire to land after
            // `interval` has elapsed, matching iced's `time::every`.
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let event = Event::Timer(crate::event::TimerEvent {
                    tag: tag_for_task.clone(),
                    timestamp,
                });
                if tx.send(SinkEvent::DelayedEvent(event)).is_err() {
                    // Main loop tore down; stop ticking.
                    break;
                }
            }
        });
        self.timers.insert(tag, handle);
    }

    /// Stop a recurring timer by tag.
    fn stop_timer(&mut self, tag: &str) {
        if let Some(handle) = self.timers.remove(tag) {
            handle.abort();
        }
    }

    fn spawn_async(&mut self, tag: String, task_fn: crate::command::AsyncTaskFn) {
        // Cancel any existing task with the same tag.
        if let Some(handle) = self.running.remove(&tag) {
            handle.abort();
        }

        let tx = self.tx.clone();
        let tag_clone = tag.clone();
        let handle = self.runtime.handle().spawn(async move {
            let future = (task_fn)();
            // Guard against user-future panics so the app sees an
            // AsyncEvent(Err(..)) instead of silently hanging.
            let result = super::run_task_with_panic_guard(&tag_clone, future).await;
            let _ = tx.send(SinkEvent::AsyncResult {
                tag: tag_clone,
                result,
            });
        });
        self.running.insert(tag, handle);
    }

    fn spawn_stream(&mut self, tag: String, task_fn: crate::command::StreamTaskFn) {
        if let Some(handle) = self.running.remove(&tag) {
            handle.abort();
        }

        let tx_stream = self.tx.clone();
        let tx_final = self.tx.clone();
        let tag_for_sink = tag.clone();
        let tag_for_result = tag.clone();

        let emitter = crate::command::StreamEmitter::buffered(&tag);
        emitter.attach_sink(Box::new(move |t, value| {
            let _ = tx_stream.send(SinkEvent::StreamValue { tag: t, value });
            let _ = &tag_for_sink;
        }));

        let handle = self.runtime.handle().spawn(async move {
            let future = (task_fn)(emitter);
            let result = super::run_task_with_panic_guard(&tag_for_result, future).await;
            let _ = tx_final.send(SinkEvent::AsyncResult {
                tag: tag_for_result,
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
        self.runtime.handle().spawn(async move {
            // tokio::time::sleep doesn't panic in practice, but we
            // route through the panic guard for consistency with the
            // other spawn paths. SendAfter carries a delivery-only
            // future, so the result slot is unused.
            use futures::FutureExt;
            let fut = async move { tokio::time::sleep(delay).await };
            let _ = std::panic::AssertUnwindSafe(fut).catch_unwind().await;
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
            match &event {
                SinkEvent::AsyncResult { tag, .. } => {
                    self.running.remove(tag);
                }
                SinkEvent::EffectTimeout { wire_id } => {
                    self.effect_timeouts.remove(wire_id);
                }
                _ => {}
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
) -> crate::Result {
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
            execute_wire_renderer_op(bridge, op, effect_tracker, async_mgr)?;
        }
        Command::Async { tag, task } => {
            async_mgr.spawn_async(tag, task);
        }
        Command::Stream { tag, task } => {
            async_mgr.spawn_stream(tag, task);
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
    async_mgr: &mut AsyncTaskManager,
) -> crate::Result {
    use plushie_core::ops::{ImageOp, RendererOp, SystemOp, SystemQuery, WindowQuery};
    use serde_json::json;

    match op {
        RendererOp::Command { id, family, value } => bridge.send_command(id, family, value),
        RendererOp::Commands(commands) => bridge.send_commands(commands.clone()),
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
                _ => {
                    log::warn!("wire mode: unhandled WindowQuery variant; query skipped");
                    return Ok(());
                }
            };
            bridge.send_window_op(op_name, window_id, &json!({"tag": tag}))
        }
        RendererOp::SystemOp(SystemOp::AllowAutomaticTabbing(enabled)) => {
            bridge.send(&OutgoingMessage::SystemOp {
                session: String::new(),
                op: "allow_automatic_tabbing".to_string(),
                payload: json!({"enabled": enabled}),
            })
        }
        RendererOp::SystemQuery(query) => {
            let (op_name, tag) = match query {
                SystemQuery::GetTheme { tag } => ("get_system_theme", tag),
                SystemQuery::GetInfo { tag } => ("get_system_info", tag),
                _ => {
                    log::warn!("wire mode: unhandled SystemQuery variant; query skipped");
                    return Ok(());
                }
            };
            bridge.send(&OutgoingMessage::SystemQuery {
                session: String::new(),
                op: op_name.to_string(),
                payload: json!({"tag": tag}),
            })
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
            // Schedule a tokio-driven timeout so the deadline fires
            // even when the bridge reader is blocked waiting for
            // renderer input. Cancelled in the resolve path when a
            // response arrives in time.
            async_mgr.schedule_effect_timeout(wire_id.clone(), effective_timeout);
            let (_, payload) = plushie_core::ops::effect_request_to_wire(request);
            bridge.send_effect(&wire_id, kind, &payload)
        }
        RendererOp::Image(image_op) => {
            let (op, payload) = match image_op {
                ImageOp::Create { handle, data } => (
                    "create_from_bytes",
                    json!({"handle": handle, "data": base64_encode(data)}),
                ),
                ImageOp::CreateRaw {
                    handle,
                    width,
                    height,
                    pixels,
                } => (
                    "create_from_rgba",
                    json!({"handle": handle, "pixels": base64_encode(pixels),
                           "width": width, "height": height}),
                ),
                ImageOp::Update { handle, data } => (
                    "update",
                    json!({"handle": handle, "data": base64_encode(data)}),
                ),
                ImageOp::UpdateRaw {
                    handle,
                    width,
                    height,
                    pixels,
                } => (
                    "update_raw",
                    json!({"handle": handle, "pixels": base64_encode(pixels),
                           "width": width, "height": height}),
                ),
                ImageOp::Delete(handle) => ("delete", json!({"handle": handle})),
                ImageOp::List { tag } => ("list", json!({"tag": tag})),
                ImageOp::Clear => ("clear", json!({})),
                _ => {
                    log::warn!("wire mode: unhandled ImageOp variant; op skipped");
                    return Ok(());
                }
            };
            bridge.send(&OutgoingMessage::ImageOp {
                session: String::new(),
                op: op.to_string(),
                payload,
            })
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
        // RendererOp is #[non_exhaustive]; any variant added after this
        // match was written is an unknown op in wire mode and is
        // skipped with a warning rather than silently dropped.
        _ => {
            log::warn!("wire mode: unhandled RendererOp variant; op skipped");
            Ok(())
        }
    }
}

/// Execute a window operation via the bridge.
#[cfg(feature = "wire")]
fn execute_wire_window_op(bridge: &mut Bridge, op: &plushie_core::ops::WindowOp) -> crate::Result {
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
        // WindowOp is #[non_exhaustive]; skip unknown variants with a
        // warning so a newly added op doesn't break wire compilation.
        _ => {
            log::warn!("wire mode: unhandled WindowOp variant; op skipped");
            Ok(())
        }
    }
}

/// Base64-encode binary data for JSON wire transport.
#[cfg(feature = "wire")]
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Wire protocol version. Sent in the settings message and
/// verified by the renderer during handshake.
#[cfg(feature = "wire")]
pub const PROTOCOL_VERSION: u32 = 1;

/// Build settings JSON from the App trait for the wire protocol.
fn build_settings<A: App>() -> Value {
    let settings = A::settings();
    let mut json = serde_json::json!({
        "protocol_version": PROTOCOL_VERSION,
    });

    if let Some(ref font) = settings.default_font {
        json["default_font"] = serde_json::json!(font);
    }
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
    if !settings.fonts.is_empty() {
        json["fonts"] = serde_json::json!(settings.fonts);
    }
    if !settings.widget_config.is_empty() {
        json["widget_config"] =
            serde_json::to_value(&settings.widget_config).unwrap_or(Value::Null);
    }
    if let Some(theme) = settings.theme {
        use plushie_core::types::PlushieType;
        json["theme"] = Value::from(theme.wire_encode());
    }

    json
}

/// Apply subscription operations by sending wire messages.
///
/// Subscribe / Unsubscribe go over the wire to the renderer.
/// StartTimer / StopTimer are SDK-side only and flow into the
/// AsyncTaskManager's runtime for recurring delivery; each tick
/// pushes a `SinkEvent::DelayedEvent` that the main loop drains
/// alongside async results.
#[cfg(feature = "wire")]
fn apply_wire_sub_ops(
    bridge: &mut Bridge,
    async_mgr: &mut AsyncTaskManager,
    ops: Vec<crate::runtime::subscriptions::SubOp>,
) -> crate::Result {
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
            SubOp::StartTimer { tag, interval } => {
                async_mgr.start_timer(tag, interval);
            }
            SubOp::StopTimer { tag } => {
                async_mgr.stop_timer(&tag);
            }
        }
    }
    Ok(())
}
