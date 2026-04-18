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
///
/// # Errors
///
/// Returns [`crate::Error::Spawn`] when the renderer binary cannot
/// be spawned, [`crate::Error::ProtocolVersionMismatch`] on a
/// handshake version mismatch, [`crate::Error::WireEncode`] or
/// [`crate::Error::WireDecode`] on framing failures, and
/// [`crate::Error::RendererExit`] after the restart policy is
/// exhausted.
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
///
/// # Errors
///
/// Same error conditions as [`run_wire`].
#[cfg(feature = "wire")]
pub fn run_wire_with_runtime<A: App>(
    binary_path: &str,
    runtime: tokio::runtime::Handle,
) -> crate::Result {
    run_wire_inner::<A>(binary_path, Some(runtime))
}

/// Run the app over an already-connected renderer socket.
///
/// Resolves the socket via `opts`, opens a [`SocketAdapter`], hands
/// it to [`Bridge::connect`], and drives a single session (no
/// restart loop; socket mode can't respawn a remote renderer).
///
/// Merges `opts.token` into the Settings message so the renderer's
/// listen-mode token check accepts the connection.
///
/// # Errors
///
/// Same error surface as [`run_wire`] plus:
///
/// - [`crate::Error::InvalidSettings`] when no socket is supplied
///   (neither `opts.socket` nor `PLUSHIE_SOCKET`).
/// - [`crate::Error::Io`] when the connect fails.
#[cfg(feature = "wire")]
pub fn run_connect<A: App>(opts: crate::ConnectOpts) -> crate::Result {
    run_connect_inner::<A>(opts, None)
}

/// [`run_connect`] on a caller-provided tokio runtime.
///
/// # Errors
///
/// Same as [`run_connect`].
#[cfg(feature = "wire")]
pub fn run_connect_with_runtime<A: App>(
    opts: crate::ConnectOpts,
    runtime: tokio::runtime::Handle,
) -> crate::Result {
    run_connect_inner::<A>(opts, Some(runtime))
}

#[cfg(feature = "wire")]
fn run_connect_inner<A: App>(
    opts: crate::ConnectOpts,
    runtime: Option<tokio::runtime::Handle>,
) -> crate::Result {
    let socket_str = opts
        .socket
        .clone()
        .or_else(|| std::env::var("PLUSHIE_SOCKET").ok())
        .ok_or_else(|| {
            crate::Error::InvalidSettings(
                "no socket address supplied: pass `ConnectOpts.socket`, set \
                 PLUSHIE_SOCKET, or use `--plushie-socket <path>`"
                    .to_string(),
            )
        })?;

    let adapter = super::socket::SocketAdapter::connect(&socket_str)?;
    log::info!(
        "plushie::run_connect: connected to renderer at {:?} (token: {})",
        adapter.addr,
        if opts.token.is_some() {
            "present"
        } else {
            "none"
        }
    );
    let bridge = Bridge::connect(adapter.stream)?;

    let mut settings = build_settings::<A>();
    if let Some(tok) = opts.token.as_deref() {
        settings["token"] = Value::String(tok.to_string());
    }

    run_session_single::<A>(bridge, settings, runtime)
}

/// Run one full session against a pre-built bridge.
///
/// Shared by `run_connect_inner`: handshake, snapshot send,
/// subscription sync, main loop. No restart logic; on exit, the
/// normal shutdown / effect-flush / error-propagation path runs and
/// the function returns.
#[cfg(feature = "wire")]
fn run_session_single<A: App>(
    mut bridge: Bridge,
    settings: Value,
    runtime: Option<tokio::runtime::Handle>,
) -> crate::Result {
    let (mut model, init_cmd) = A::init();

    let mut sub_manager = crate::runtime::subscriptions::SubscriptionManager::new();
    let mut effect_tracker = EffectTracker::new();
    let mut async_mgr = AsyncTaskManager::new(runtime);
    let mut view_errors = crate::runtime::view_errors::ViewErrors::default();
    let mut window_sync = crate::runtime::windows::WindowSync::new();

    let seed = plushie_core::protocol::TreeNode {
        id: String::new(),
        type_name: "container".to_string(),
        props: plushie_core::protocol::Props::from(plushie_core::protocol::PropMap::new()),
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

    bridge.send_settings(&settings)?;

    let hello = bridge.receive()?;
    log::info!(
        "renderer hello: {}",
        hello
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    );

    let expected = plushie_core::protocol::PROTOCOL_VERSION;
    let remote_protocol = hello
        .get("protocol")
        .or_else(|| hello.get("protocol_version"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    if remote_protocol != Some(expected) {
        log::error!(
            "protocol version mismatch: SDK expects {expected}, renderer advertised {remote_protocol:?}"
        );
        drop(bridge);
        return Err(crate::Error::ProtocolVersionMismatch {
            expected,
            got: remote_protocol,
        });
    }

    if let Some(remote) = hello.get("version").and_then(|v| v.as_str())
        && remote != crate::RENDERER_VERSION
    {
        log::warn!(
            "renderer version skew: SDK built against {expected}, \
             renderer reports {got}",
            expected = crate::RENDERER_VERSION,
            got = remote,
        );
    }

    if let Some(codec_str) = hello.get("codec").and_then(|v| v.as_str()) {
        let codec = match codec_str {
            "msgpack" => super::bridge::Codec::MsgPack,
            "json" => super::bridge::Codec::Json,
            other => {
                log::warn!("renderer advertised unknown codec `{other}`; keeping JSON");
                super::bridge::Codec::Json
            }
        };
        bridge.set_codec(codec);
    }

    bridge.start_reader()?;

    let snapshot_value = serde_json::to_value(&current_tree)
        .map_err(|e| crate::Error::WireEncode(format!("snapshot: {e}")))?;
    bridge.send_snapshot(&snapshot_value)?;

    for op in window_sync.sync(&current_tree, &settings) {
        dispatch_window_sync_op(&mut bridge, &op)?;
    }

    if let Err(e) = execute_wire_command(&mut bridge, init_cmd, &mut effect_tracker, &mut async_mgr)
    {
        log::error!("initial command execution failed: {e}");
    }

    let new_subs = A::subscribe(&model);
    validate_subscription_windows(&new_subs, &current_tree);
    apply_wire_sub_ops(&mut bridge, &mut async_mgr, sub_manager.sync(new_subs))?;

    let policy = A::restart_policy();
    let reason = run_session::<A>(
        &mut bridge,
        &mut model,
        &mut current_tree,
        &mut effect_tracker,
        &mut async_mgr,
        &mut sub_manager,
        &mut view_errors,
        &mut window_sync,
        &settings,
        policy.heartbeat_interval,
        // Connect mode: the renderer lives in a separate process we
        // didn't spawn. A hot-reload swap signal has no meaningful
        // action here, so the session loop drains and ignores it.
        false,
    );

    log::warn!("plushie wire: renderer exited ({})", reason.label());
    A::handle_renderer_exit(&mut model, reason.clone());

    if matches!(reason, ExitReason::Shutdown) {
        flush_effects_on_shutdown::<A>(&mut model, &mut effect_tracker);
        return Ok(());
    }

    flush_effects_on_shutdown::<A>(&mut model, &mut effect_tracker);
    Err(crate::Error::RendererExit(reason))
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
    let mut window_sync = crate::runtime::windows::WindowSync::new();

    // Initial view; shared across restarts as the "current tree". If
    // the first view call panics there is no last-good tree to fall
    // back to, so we use an empty container as a seed.
    let seed = plushie_core::protocol::TreeNode {
        id: String::new(),
        type_name: "container".to_string(),
        props: plushie_core::protocol::Props::from(plushie_core::protocol::PropMap::new()),
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

    // Binary path can be swapped between iterations when a dev-mode
    // hot-reload finishes and rediscovery picks a fresh custom build.
    // Seed it with the caller's value; RendererSwap branches rewrite
    // `binary_owned` and the next iteration uses it.
    #[cfg_attr(not(feature = "dev"), allow(unused_mut, unused_assignments))]
    let mut binary_owned: Option<String> = None;

    loop {
        let active_binary: &str = binary_owned.as_deref().unwrap_or(binary_path);

        // Bring up (or respawn) the renderer and establish the
        // reader thread. On respawn we resend settings + snapshot +
        // subscription state so the renderer catches up.
        let mut bridge = Bridge::spawn(active_binary)
            .map_err(|e| crate::Error::spawn(active_binary.to_string(), e))?;

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

        // Protocol-version gate. A mismatch between the SDK's
        // PROTOCOL_VERSION and whatever the renderer advertises is
        // fatal: wire shapes are tied to the version, and proceeding
        // would lead to misparsed messages further down. Reap the
        // child before returning so we don't leave a zombie.
        let expected = plushie_core::protocol::PROTOCOL_VERSION;
        let remote_protocol = hello
            .get("protocol")
            .or_else(|| hello.get("protocol_version"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        if remote_protocol != Some(expected) {
            log::error!(
                "protocol version mismatch: SDK expects {expected}, renderer advertised {remote_protocol:?}"
            );
            drop(bridge);
            return Err(crate::Error::ProtocolVersionMismatch {
                expected,
                got: remote_protocol,
            });
        }

        // CARGO_PKG_VERSION skew is advisory: wire-protocol compatibility
        // is PROTOCOL_VERSION, not CARGO_PKG_VERSION. Divergence often
        // signals a stale installed renderer binary though, so the hint
        // names the exact install command.
        if let Some(remote) = hello.get("version").and_then(|v| v.as_str())
            && remote != crate::RENDERER_VERSION
        {
            log::warn!(
                "renderer version skew: SDK built against {expected}, \
                 renderer reports {got}; run `cargo install plushie-renderer --version {expected}` \
                 if this is unexpected",
                expected = crate::RENDERER_VERSION,
                got = remote,
            );
        }

        // Honour the renderer's declared codec if the hello message
        // carries one. The SDK sends the settings in JSON; the renderer
        // detects codec from the first byte and mirrors it today, so
        // this path is effectively a no-op in the happy case. It
        // remains here as the documented negotiation hook so a future
        // renderer that selects a codec upstream is respected without
        // an SDK change.
        if let Some(codec_str) = hello.get("codec").and_then(|v| v.as_str()) {
            let codec = match codec_str {
                "msgpack" => super::bridge::Codec::MsgPack,
                "json" => super::bridge::Codec::Json,
                other => {
                    log::warn!("renderer advertised unknown codec `{other}`; keeping JSON");
                    super::bridge::Codec::Json
                }
            };
            bridge.set_codec(codec);
        }

        bridge.start_reader()?;

        // Send snapshot so the renderer has the current tree.
        let snapshot_value = serde_json::to_value(&current_tree)
            .map_err(|e| crate::Error::WireEncode(format!("snapshot: {e}")))?;
        bridge.send_snapshot(&snapshot_value)?;

        // Synchronize window lifecycle with the renderer. On restart,
        // reset the tracker so every current window is resent as an
        // `open` op; otherwise replay from whatever the tracker held
        // before. The base settings object is the same JSON the SDK
        // sent via send_settings, so per-window prop merges reuse it
        // directly.
        if restart_count > 0 {
            window_sync = crate::runtime::windows::WindowSync::new();
        }
        for op in window_sync.sync(&current_tree, &settings) {
            dispatch_window_sync_op(&mut bridge, &op)?;
        }

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
        validate_subscription_windows(&new_subs, &current_tree);
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
            &mut window_sync,
            &settings,
            policy.heartbeat_interval,
            // Spawn mode: we own the renderer subprocess and can
            // respawn it, so dev-mode swap signals are actionable.
            true,
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

        // Dev-mode hot-reload: skip backoff, reset restart count
        // (a clean swap shouldn't eat into the crash budget),
        // rediscover the binary so the fresh build is picked up,
        // then loop around. The Bridge drop at the bottom of this
        // iteration tears down the old subprocess; the next
        // iteration spawns its replacement.
        if matches!(reason, ExitReason::RendererSwap) {
            restart_count = 0;
            #[cfg(feature = "dev")]
            {
                match crate::runner::wire_discovery::discover_renderer() {
                    Ok(fresh) => {
                        log::info!("plushie wire: swap-discovered renderer at {fresh}");
                        binary_owned = Some(fresh);
                    }
                    Err(e) => {
                        log::warn!(
                            "plushie wire: swap-rediscovery failed ({e}); \
                             reusing current binary path"
                        );
                    }
                }
            }
            drop(bridge);
            continue;
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
///
/// `manages_renderer_lifecycle` controls whether the dev-mode swap
/// signal is honored. Spawn-mode callers (`run_wire_inner`) own the
/// renderer subprocess and can respawn it, so they pass `true` and
/// get back `ExitReason::RendererSwap`. Connect-mode callers
/// (`run_session_single`) are attached to an external renderer they
/// did not spawn; they pass `false` and the signal is drained and
/// ignored so a hot-reload rebuild in a parallel dev session does
/// not tear down the connected session.
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
    window_sync: &mut crate::runtime::windows::WindowSync,
    base_settings: &Value,
    heartbeat_interval: Option<std::time::Duration>,
    manages_renderer_lifecycle: bool,
) -> ExitReason {
    // Dev-mode needs to wake up periodically to check for swap
    // signals even when the heartbeat is disabled or set to a long
    // interval. The poll window picks the shorter of the two.
    #[cfg(feature = "dev")]
    let poll_interval = heartbeat_interval
        .unwrap_or(std::time::Duration::from_millis(250))
        .min(std::time::Duration::from_millis(250));
    #[cfg(not(feature = "dev"))]
    let poll_interval_opt = heartbeat_interval;
    #[cfg(feature = "dev")]
    let mut since_last_msg = std::time::Instant::now();

    loop {
        // Dev-mode swap signal takes priority when this session owns
        // the renderer subprocess: a successful rebuild means the
        // current binary is stale, so we return RendererSwap and let
        // the outer loop respawn without counting against the restart
        // policy. In connect mode we drain the queue but discard the
        // signal so a parallel dev session's rebuild doesn't tear
        // down the connected session. Compiled out when the `dev`
        // feature is absent.
        #[cfg(feature = "dev")]
        {
            if handle_dev_control_signals() && manages_renderer_lifecycle {
                return ExitReason::RendererSwap;
            }
        }
        #[cfg(not(feature = "dev"))]
        let _ = manages_renderer_lifecycle;

        #[cfg(feature = "dev")]
        let incoming = bridge.recv_timeout(Some(poll_interval));
        #[cfg(not(feature = "dev"))]
        let incoming = bridge.recv_timeout(poll_interval_opt);

        // Map a short dev-mode poll Timeout into either "keep going"
        // (heartbeat hasn't elapsed yet, or is disabled) or a real
        // HeartbeatTimeout.
        #[cfg(feature = "dev")]
        let incoming = match (&incoming, heartbeat_interval) {
            (super::bridge::Incoming::Timeout, Some(hb)) => {
                if since_last_msg.elapsed() >= hb {
                    incoming
                } else {
                    continue;
                }
            }
            (super::bridge::Incoming::Timeout, None) => continue,
            _ => {
                since_last_msg = std::time::Instant::now();
                incoming
            }
        };

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
                        window_sync,
                        base_settings,
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
                    window_sync,
                    base_settings,
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
    window_sync: &mut crate::runtime::windows::WindowSync,
    base_settings: &Value,
) -> crate::Result {
    // Dev-mode overlay interception: if the event belongs to the
    // rebuilding-overlay ID namespace, handle it internally and skip
    // the app's update. Compiled out when the `dev` feature is off.
    #[cfg(feature = "dev")]
    {
        if crate::dev::intercept_event(&event) {
            return Ok(());
        }
    }
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

    // Window lifecycle sync runs *before* tree diff so an
    // open_window op precedes any patch that references the new
    // window's subtree. Close ops trail for the same reason: the
    // renderer still needs the window alive while applying the
    // remove patches inside it. Update ops are order-insensitive.
    for op in window_sync.sync(&new_tree, base_settings) {
        dispatch_window_sync_op(bridge, &op)?;
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
    validate_subscription_windows(&new_subs, current_tree);
    apply_wire_sub_ops(bridge, async_mgr, sub_manager.sync(new_subs))?;

    Ok(())
}

/// Emit an `unknown_window` diagnostic for any subscription whose
/// `window_id` does not appear in the current tree.
///
/// The renderer accepts the subscription either way and just never
/// delivers events for a dangling window, which is a silent failure
/// mode. This diagnostic surfaces the typo / stale wiring loudly.
#[cfg(feature = "wire")]
fn validate_subscription_windows(
    subs: &[crate::subscription::Subscription],
    tree: &plushie_core::protocol::TreeNode,
) {
    let windows = crate::runtime::windows::detect_windows(tree);
    for sub in subs {
        if let Some(wid) = sub.window_id()
            && !windows.contains(wid)
        {
            let diag = plushie_core::Diagnostic::UnknownWindow {
                window_id: wid.to_string(),
                subscription_tag: sub.kind().to_string(),
            };
            log::warn!("{diag}");
        }
    }
}

/// Translate a [`WindowSyncOp`] into the bridge's window-op wire
/// message.
#[cfg(feature = "wire")]
fn dispatch_window_sync_op(
    bridge: &mut Bridge,
    op: &crate::runtime::windows::WindowSyncOp,
) -> crate::Result {
    use crate::runtime::windows::WindowSyncOp;
    match op {
        WindowSyncOp::Open {
            window_id,
            settings,
        } => bridge.send_window_op("open", window_id, settings),
        WindowSyncOp::Close { window_id } => {
            bridge.send_window_op("close", window_id, &Value::Null)
        }
        WindowSyncOp::Update {
            window_id,
            settings,
        } => bridge.send_window_op("update", window_id, settings),
    }
}

/// Classify a bridge receive error into a typed [`ExitReason`].
///
/// `UnexpectedEof` indicates the renderer closed stdout cleanly but
/// without sending a proper shutdown marker; everything else is
/// treated as a crash. Reaps the child (non-blocking) to capture the
/// exit code for `Crash`.
/// Drain the dev-mode control-signal queue. Returns true when a
/// [`crate::dev::ControlSignal::SwapRenderer`] is pending so the
/// session loop can return a [`ExitReason::RendererSwap`]. Any other
/// signal variants are logged and discarded (reserved for future
/// use).
#[cfg(all(feature = "wire", feature = "dev"))]
fn handle_dev_control_signals() -> bool {
    let signals = crate::dev::drain_control_signals();
    let mut swap = false;
    for signal in signals {
        match signal {
            crate::dev::ControlSignal::SwapRenderer => {
                log::info!("plushie wire: dev-mode swap requested; restarting renderer");
                swap = true;
            }
        }
    }
    swap
}

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
        let diag = plushie_core::Diagnostic::UnknownMessageType {
            msg_type: String::new(),
        };
        log::warn!("{diag}: {msg}");
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
            // SDK is receiving renderer events here, so there is no
            // outgoing sink to route through. Emit via the typed
            // Diagnostic's Display impl to the shared log channel.
            let diag = plushie_core::Diagnostic::UnknownMessageType { msg_type };
            log::error!("{diag}");
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

    /// Synchronously push a SinkEvent onto the delivery channel. Used
    /// for synthetic events (e.g. one-per-tag cancellation) that need
    /// to interleave with async results drained by the main loop.
    ///
    /// A full channel drops the event silently: the runtime is
    /// already wedged, there is no recovery path worth taking.
    fn deliver_sink_event(&self, event: SinkEvent) {
        let _ = self.tx.send(event);
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
            // Best-effort: tell the renderer we're shutting down so
            // it can close cleanly instead of seeing stdin drop as
            // the bridge is torn down. The main loop observes the
            // subsequent pipe closure and classifies it as
            // `ExitReason::Shutdown` via the shutdown flag below,
            // which flushes pending effects and delivers an exit
            // hook before the runner returns. See `run_wire_inner`
            // and `classify_exit` for the full lifecycle.
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
    use plushie_core::ops::{ImageOp, RendererOp};
    use serde_json::json;

    match op {
        RendererOp::Command { id, family, value } => bridge.send_command(id, family, value),
        RendererOp::Commands(commands) => bridge.send_commands(commands.clone()),
        RendererOp::FocusNext => bridge.send_widget_op("focus_next", &json!({})),
        RendererOp::FocusPrevious => bridge.send_widget_op("focus_previous", &json!({})),
        RendererOp::FocusNextWithin { scope } => {
            bridge.send_widget_op("focus_next_within", &json!({"scope": scope}))
        }
        RendererOp::FocusPreviousWithin { scope } => {
            bridge.send_widget_op("focus_previous_within", &json!({"scope": scope}))
        }
        RendererOp::Window(op) => execute_wire_window_op(bridge, op),
        RendererOp::WindowQuery(query) => {
            let (op_name, window_id, payload) = query.to_wire();
            bridge.send_window_op(op_name, &window_id, &payload)
        }
        RendererOp::SystemOp(op) => {
            let (op_name, payload) = op.to_wire();
            bridge.send(&OutgoingMessage::SystemOp {
                session: String::new(),
                op: op_name.to_string(),
                payload,
            })
        }
        RendererOp::SystemQuery(query) => {
            let (op_name, payload) = query.to_wire();
            bridge.send(&OutgoingMessage::SystemQuery {
                session: String::new(),
                op: op_name.to_string(),
                payload,
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
            let (wire_id, replaced) =
                effect_tracker.track_with_replacement(tag, kind, effective_timeout);
            if let Some((prior_tag, _prior_kind)) = replaced {
                // Surface the one-per-tag replacement as a synthetic
                // Cancelled event, routed through the same channel the
                // async manager drains so it interleaves correctly
                // with other delayed events.
                async_mgr.deliver_sink_event(SinkEvent::DelayedEvent(Event::Effect(EffectEvent {
                    tag: prior_tag,
                    result: EffectResult::Cancelled,
                })));
            }
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
        RendererOp::Announce { text, politeness } => bridge.send_widget_op(
            "announce",
            &json!({
                "text": text,
                "politeness": match politeness {
                    plushie_core::types::a11y::Live::Polite => "polite",
                    plushie_core::types::a11y::Live::Assertive => "assertive",
                },
            }),
        ),
        RendererOp::LoadFont { family, bytes } => bridge.send_widget_op(
            "load_font",
            &json!({"family": family, "data": base64_encode(bytes)}),
        ),
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
///
/// Uses [`plushie_core::ops::WindowOp::to_wire`] so the enum itself owns
/// the wire serialisation; wire mode just forwards the string triple.
#[cfg(feature = "wire")]
fn execute_wire_window_op(bridge: &mut Bridge, op: &plushie_core::ops::WindowOp) -> crate::Result {
    let (op_str, window_id, payload) = op.to_wire();
    bridge.send_window_op(op_str, &window_id, &payload)
}

/// Base64-encode binary data for JSON wire transport.
#[cfg(feature = "wire")]
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Build settings JSON from the App trait for the wire protocol.
fn build_settings<A: App>() -> Value {
    let settings = A::settings();
    let mut json = serde_json::json!({
        "protocol_version": plushie_core::protocol::PROTOCOL_VERSION,
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
