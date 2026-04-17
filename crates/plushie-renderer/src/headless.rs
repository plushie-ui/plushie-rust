//! Headless and mock modes for the plushie renderer.
//!
//! `--headless`: real rendering via tiny-skia with persistent widget
//! state. Accurate screenshots after interactions.
//!
//! `--mock`: protocol-only via the null renderer `()`. Full iced widget
//! pipeline (event injection, focus tracking) but stub screenshots. Fast
//! protocol-level testing from any language.
//!
//! Both modes read framed messages from stdin, process them through
//! [`Core`](plushie_widget_sdk::engine::Core), and write responses to stdout.
//! No iced daemon, no windows, no GPU. Both modes maintain a persistent
//! renderer and UI cache. Headless uses `iced::Renderer` (tiny-skia)
//! for real screenshots, mock uses the null renderer `()` for speed.
//!
//! # Session multiplexing
//!
//! When `max_sessions > 1`, multiple sessions run concurrently in
//! separate threads. A reader thread dispatches incoming messages by
//! the `session` field to per-session threads. A writer thread
//! collects responses from all sessions and writes them to stdout.
//! Each session is fully isolated (own Core, caches, widgets, UI).

use std::io::{self, BufRead, BufReader, Read};
use std::sync::mpsc;
use std::thread;

use iced::mouse;
use iced::{Event, Size, Theme};
use serde::Serialize;

use plushie_widget_sdk::PlushieRenderer;
use plushie_widget_sdk::codec::Codec;
use plushie_widget_sdk::engine::Core;
use plushie_widget_sdk::image_registry::ImageRegistry;
use plushie_widget_sdk::message::Message;
use plushie_widget_sdk::protocol::{IncomingMessage, OutgoingEvent, SessionMessage};
use plushie_widget_sdk::render_ctx::RenderCtx;

use plushie_renderer_lib::scripting::{interaction_to_iced_events, resolve_widget_id};

fn log_hello_error(err: &io::Error) {
    if err.kind() != io::ErrorKind::BrokenPipe {
        log::error!("failed to emit hello: {err}");
    }
}

/// Default screenshot width when not specified by the caller.
const DEFAULT_SCREENSHOT_WIDTH: u32 = 1024;
/// Default screenshot height when not specified by the caller.
const DEFAULT_SCREENSHOT_HEIGHT: u32 = 768;
/// Maximum screenshot dimension (width or height). Matches
/// `ImageRegistry::MAX_DIMENSION`. Prevents untrusted input from
/// triggering a multi-GiB RGBA allocation.
const MAX_SCREENSHOT_DIMENSION: u32 = 16384;

/// Execution mode for the headless/mock event loop.
#[derive(Clone, Copy)]
pub(crate) enum Mode {
    /// Real rendering via tiny-skia with persistent widget state.
    Headless,
    /// Protocol-only, no rendering. Stub screenshots.
    Mock,
}

// ---------------------------------------------------------------------------
// WireWriter: abstracts output destination
// ---------------------------------------------------------------------------

/// Encodes and writes wire messages. Each session owns one.
///
/// Encoded bytes flow through a bounded channel to a dedicated
/// writer thread that owns stdout. Single-session and multiplexed
/// modes share this shape so stdout backpressure pauses the
/// session thread consistently.
struct WireWriter {
    inner: WriterInner,
    codec: Codec,
}

enum WriterInner {
    /// Send encoded bytes to the writer thread.
    Channel(mpsc::SyncSender<Vec<u8>>),
}

impl WireWriter {
    fn channel(tx: mpsc::SyncSender<Vec<u8>>, codec: Codec) -> Self {
        Self {
            inner: WriterInner::Channel(tx),
            codec,
        }
    }

    /// Encode a serializable value and write it.
    fn emit<T: Serialize>(&self, value: &T) -> io::Result<()> {
        let bytes = self.codec.encode(value).map_err(io::Error::other)?;
        self.write_bytes(&bytes)
    }

    /// Encode a message with a binary field (e.g. screenshot RGBA data)
    /// and write it.
    fn emit_binary(
        &self,
        map: serde_json::Map<String, serde_json::Value>,
        binary: Option<(&str, &[u8])>,
    ) -> io::Result<()> {
        let bytes = self
            .codec
            .encode_binary_message(map, binary)
            .map_err(io::Error::other)?;
        self.write_bytes(&bytes)
    }

    fn write_bytes(&self, bytes: &[u8]) -> io::Result<()> {
        match &self.inner {
            WriterInner::Channel(tx) => tx
                .send(bytes.to_vec())
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "writer channel closed")),
        }
    }
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

type UiCache = iced_test::runtime::user_interface::Cache;

/// Persistent iced renderer and UI cache. Both modes maintain one:
/// headless uses `iced::Renderer` (tiny-skia), mock uses the null
/// renderer `()`.
struct UiState<R: PlushieRenderer> {
    renderer: R,
    ui_cache: UiCache,
    viewport_size: Size,
    cursor: mouse::Cursor,
}

/// All mutable state for a headless/mock session.
///
/// Both modes maintain a persistent renderer and UI cache for full
/// iced widget pipeline support (event injection, focus tracking).
/// The `R` parameter selects the renderer: `iced::Renderer` for
/// headless (real screenshots), `()` for mock (stub screenshots).
struct Session<R: PlushieRenderer> {
    core: Core,
    theme: Theme,
    registry: plushie_widget_sdk::registry::WidgetRegistry<R>,
    images: ImageRegistry,
    writer: WireWriter,
    ui: UiState<R>,
    mode: Mode,
    /// Renderer-side animation manager.
    transition_manager: plushie_widget_sdk::animation::TransitionManager,
    /// Current keyboard modifier state, updated on every ModifiersChanged
    /// event. Included on all outgoing pointer events.
    current_modifiers: iced::keyboard::Modifiers,
    /// Number of fonts this session has loaded via load_font.
    ///
    /// Attribution is per-session; the cap itself is process-global
    /// because iced's font system is process-global. When a specific
    /// session hits the cap, only that session gets a font_cap_exceeded
    /// error, not every other session in the process.
    fonts_loaded: u32,
}

impl<R: PlushieRenderer> Session<R> {
    /// Construct a new session with the default built-in iced widget set.
    fn new(mode: Mode, writer: WireWriter) -> Self {
        let mut registry = plushie_widget_sdk::registry::WidgetRegistry::new();
        registry.register_set(&plushie_widget_sdk::widget::widget_set::iced_widget_set());
        Self::with_registry(mode, writer, registry)
    }

    /// Construct a new session with an explicit, pre-built registry.
    ///
    /// Used by the multiplexed dispatcher so each session thread can
    /// invoke a caller-supplied factory closure (produced by
    /// [`PlushieAppBuilder::with_session_factory`](plushie_widget_sdk::app::PlushieAppBuilder::with_session_factory))
    /// to obtain a registry populated with both built-in and custom
    /// widgets.
    fn with_registry(
        mode: Mode,
        writer: WireWriter,
        registry: plushie_widget_sdk::registry::WidgetRegistry<R>,
    ) -> Self {
        let renderer_settings = iced::advanced::renderer::Settings {
            default_font: iced::Font::DEFAULT,
            default_text_size: iced::Pixels(16.0),
        };
        let renderer = iced::futures::executor::block_on(R::new(renderer_settings, None))
            .expect("renderer must be available");

        let ui = UiState {
            renderer,
            ui_cache: UiCache::default(),
            viewport_size: Size::new(
                DEFAULT_SCREENSHOT_WIDTH as f32,
                DEFAULT_SCREENSHOT_HEIGHT as f32,
            ),
            cursor: mouse::Cursor::Unavailable,
        };

        Self {
            core: Core::new(),
            theme: Theme::Dark,
            registry,
            images: ImageRegistry::new(),
            writer,
            ui,
            mode,
            fonts_loaded: 0,
            transition_manager: plushie_widget_sdk::animation::TransitionManager::new(),
            current_modifiers: iced::keyboard::Modifiers::default(),
        }
    }

    /// Rebuild the renderer when default font/text size changes.
    fn rebuild_renderer(&mut self) {
        let renderer_settings = iced::advanced::renderer::Settings {
            default_font: self.core.default_font.unwrap_or(iced::Font::DEFAULT),
            default_text_size: iced::Pixels(self.core.default_text_size.unwrap_or(16.0)),
        };
        if let Some(r) = iced::futures::executor::block_on(R::new(renderer_settings, None)) {
            self.ui.renderer = r;
            self.ui.ui_cache = UiCache::default();
        }
    }

    /// Build a temporary UserInterface from the current tree, run a
    /// closure against it, then store the resulting cache back.
    ///
    /// Returns `None` if no tree root exists yet (no snapshot received).
    fn with_ui<Ret>(
        &mut self,
        f: impl FnOnce(
            &mut iced_test::runtime::UserInterface<
                '_,
                plushie_widget_sdk::message::Message,
                Theme,
                R,
            >,
            &mut R,
            mouse::Cursor,
        ) -> Ret,
    ) -> Option<Ret> {
        let root = self.core.tree.root()?;

        let ctx = RenderCtx {
            caches: &self.core.caches,
            images: &self.images,
            theme: &self.theme,
            registry: &self.registry,
            default_text_size: self.core.default_text_size,
            default_font: self.core.default_font,
            window_id: "",
            scale_factor: 1.0,
        };
        let element = plushie_widget_sdk::widget::render(root, ctx);

        let cache = std::mem::take(&mut self.ui.ui_cache);
        let mut ui = iced_test::runtime::UserInterface::build(
            element,
            self.ui.viewport_size,
            cache,
            &mut self.ui.renderer,
        );

        let result = f(&mut ui, &mut self.ui.renderer, self.ui.cursor);

        self.ui.ui_cache = ui.into_cache();
        Some(result)
    }

    /// Process a RedrawRequested event through the UI after a tree change.
    /// Also processes any messages produced (e.g. pending focus notifications
    /// from Tab navigation that were queued by operate()).
    fn settle_ui(&mut self, session_id: &str) -> Vec<OutgoingEvent> {
        let messages = self
            .with_ui(|ui, renderer, cursor| {
                let mut messages = Vec::new();
                let redraw = Event::Window(iced::window::Event::RedrawRequested(
                    iced_test::core::time::Instant::now(),
                ));
                let _status = ui.update(&[redraw], cursor, renderer, &mut messages);
                messages
            })
            .unwrap_or_default();

        self.process_captured_messages(messages)
            .into_iter()
            .map(|e| e.with_session(session_id))
            .collect()
    }

    /// Inject iced events one at a time, capturing the Messages that
    /// widgets produce. Settles the UI between each event so widget
    /// state updates are visible to subsequent events.
    ///
    /// For each iced event that produces widget Messages:
    /// 1. The event is injected and Messages are captured
    /// 2. Messages are converted to OutgoingEvents (with session set)
    /// 3. An `interact_step` is emitted with those events
    /// 4. The `read_next` callback is called, which blocks until the
    ///    host sends back a tree update (Snapshot or Patch)
    /// 5. The tree update is applied, caches prepared, UI settled
    ///
    /// This matches the production flow where each iced event triggers
    /// a full host round-trip before the next event is processed.
    ///
    /// For events that produce no widget Messages (e.g. CursorMoved),
    /// no step is emitted and the loop continues immediately.
    ///
    /// Returns `true` if any events were captured and emitted via
    /// interact_step, `false` if no widget Messages were produced.
    fn inject_and_capture(
        &mut self,
        session_id: &str,
        interact_id: &str,
        events: &[Event],
        read_next: &mut dyn FnMut() -> Option<IncomingMessage>,
    ) -> bool {
        if events.is_empty() {
            return false;
        }

        let mut emitted_steps = false;

        for event in events {
            // Update cursor from CursorMoved events before injection.
            if let Event::Mouse(mouse::Event::CursorMoved { position }) = event {
                self.ui.cursor = mouse::Cursor::Available(*position);
            }
            // Track modifier state for pointer events.
            if let Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) = event {
                self.current_modifiers = *mods;
            }

            // Inject ONE event and capture the Messages iced produces.
            let messages = self
                .with_ui(|ui, renderer, cursor| {
                    let mut messages = Vec::new();
                    let statuses =
                        ui.update(std::slice::from_ref(event), cursor, renderer, &mut messages);

                    // Post-process Tab for focus navigation (same as the normal
                    // iced application loop). Without this, Tab doesn't move
                    // focus between widgets in headless mode.
                    let (_ui_state, event_statuses) = statuses;
                    if let Some(&status) = event_statuses.first() {
                        iced_test::runtime::keyboard::handle_tab(event, status, ui, renderer);
                    }

                    messages
                })
                .unwrap_or_default();

            // Convert captured Messages to OutgoingEvents.
            let step_events: Vec<OutgoingEvent> = self
                .process_captured_messages(messages)
                .into_iter()
                .map(|e| e.with_session(session_id))
                .collect();

            if !step_events.is_empty() {
                emitted_steps = true;

                // Emit an interact_step so the host can process
                // these events and send back an updated tree.
                let step = plushie_widget_sdk::protocol::InteractResponse {
                    message_type: "interact_step",
                    session: session_id.to_string(),
                    id: interact_id.to_string(),
                    events: step_events,
                };
                if self.writer.emit(&step).is_err() {
                    break;
                }

                // Read the next message from the host. In the normal
                // flow this is a Snapshot or Patch with the updated
                // tree. We apply whatever arrives through the normal
                // path so tree changes, settings updates, etc. all work.
                let next = read_next();
                if let Some(msg) = next {
                    let is_tree_change = matches!(
                        msg,
                        IncomingMessage::Snapshot { .. } | IncomingMessage::Patch { .. }
                    );
                    if !is_tree_change {
                        let msg_type = match &msg {
                            IncomingMessage::Snapshot { .. } => "snapshot",
                            IncomingMessage::Patch { .. } => "patch",
                            IncomingMessage::Query { .. } => "query",
                            IncomingMessage::Interact { .. } => "interact",
                            IncomingMessage::Reset { .. } => "reset",
                            IncomingMessage::Settings { .. } => "settings",
                            IncomingMessage::Effect { .. } => "effect",
                            IncomingMessage::WidgetOp { .. } => "widget_op",
                            IncomingMessage::WindowOp { .. } => "window_op",
                            IncomingMessage::SystemOp { .. } => "system_op",
                            IncomingMessage::SystemQuery { .. } => "system_query",
                            IncomingMessage::ImageOp { .. } => "image_op",
                            IncomingMessage::Subscribe { .. } => "subscribe",
                            IncomingMessage::Unsubscribe { .. } => "unsubscribe",
                            IncomingMessage::TreeHash { .. } => "tree_hash",
                            IncomingMessage::Screenshot { .. } => "screenshot",
                            IncomingMessage::Command { .. } => "command",
                            IncomingMessage::Commands { .. } => "commands",
                            IncomingMessage::AdvanceFrame { .. } => "advance_frame",
                            IncomingMessage::RegisterEffectStub { .. } => "register_effect_stub",
                            IncomingMessage::UnregisterEffectStub { .. } => {
                                "unregister_effect_stub"
                            }
                        };
                        log::warn!(
                            "interact_step: expected snapshot or patch from host, \
                             got {msg_type}; tree state may be stale"
                        );
                    }
                    let effects = self.core.apply(msg);
                    for effect in effects {
                        use plushie_widget_sdk::engine::CoreEffect;
                        match effect {
                            CoreEffect::ThemeChanged(t) => self.theme = t,
                            CoreEffect::WidgetConfig(config) => {
                                let ctx = plushie_widget_sdk::registry::InitCtx {
                                    config: &config,
                                    theme: &self.theme,
                                    default_text_size: self.core.default_text_size,
                                    default_font: self.core.default_font,
                                };
                                self.registry.init_all(&ctx);
                            }
                            _ => {}
                        }
                    }
                    if is_tree_change && let Some(root) = self.core.tree.root_mut() {
                        self.registry
                            .prepare_walk(root, &mut self.core.caches, &self.theme);
                    }
                } else {
                    // stdin closed or channel dropped mid-interact.
                    log::warn!("stdin closed mid-interact, stopping event injection");
                    break;
                }
            }

            // Settle the UI so widget state updates before the
            // next event is processed. This also drains pending focus
            // notifications from Tab navigation.
            let settle_events = self.settle_ui(session_id);
            if !settle_events.is_empty() {
                emitted_steps = true;
                let step = plushie_widget_sdk::protocol::InteractResponse {
                    message_type: "interact_step",
                    session: session_id.to_string(),
                    id: interact_id.to_string(),
                    events: settle_events,
                };
                self.writer.emit(&step).ok();

                if let Some(msg) = read_next() {
                    let _ = self.core.apply(msg);
                }
            }
        }

        emitted_steps
    }

    /// Convert iced Messages to OutgoingEvents using the shared
    /// message processing logic.
    fn process_captured_messages(&mut self, messages: Vec<Message>) -> Vec<OutgoingEvent> {
        let mut events = Vec::new();
        for msg in messages {
            // Suppress status events in headless interact mode. Status
            // changes are high-frequency, internal to iced's widget state
            // machine, and not meaningful during scripted interactions.
            // In daemon mode they flow as standalone wire events.
            if matches!(&msg, Message::Event { family, .. } if family == "status") {
                continue;
            }
            events.extend(self.registry.process_message(&msg));
        }
        events
    }
}

// ---------------------------------------------------------------------------
// Message handling
// ---------------------------------------------------------------------------

/// Process one incoming message through a session.
///
/// All output goes through `session.writer`. The `session_id` is
/// echoed on every outgoing message to identify which session
/// produced it.
///
/// The `read_next` callback is used during iterative interact
/// processing to read snapshot messages from the host between
/// event injections. In single-session mode, it reads from stdin.
/// In multiplexed mode, it reads from the session's mpsc channel.
/// Returns `None` if the source is closed.
fn handle_message<R: PlushieRenderer>(
    s: &mut Session<R>,
    session_id: &str,
    msg: IncomingMessage,
    read_next: &mut dyn FnMut() -> Option<IncomingMessage>,
) -> io::Result<()> {
    let is_snapshot = matches!(msg, IncomingMessage::Snapshot { .. });
    let is_tree_change = is_snapshot || matches!(msg, IncomingMessage::Patch { .. });
    let is_settings = matches!(msg, IncomingMessage::Settings { .. });

    // Extract font file paths from Settings before Core consumes the message.
    // Fonts are loaded directly into the global font system (no iced Task
    // runtime available in headless mode).
    if let IncomingMessage::Settings { ref settings } = msg {
        load_fonts_from_settings(settings);
    }

    match msg {
        IncomingMessage::Snapshot { .. }
        | IncomingMessage::Patch { .. }
        | IncomingMessage::Effect { .. }
        | IncomingMessage::WidgetOp { .. }
        | IncomingMessage::Subscribe { .. }
        | IncomingMessage::Unsubscribe { .. }
        | IncomingMessage::WindowOp { .. }
        | IncomingMessage::SystemOp { .. }
        | IncomingMessage::SystemQuery { .. }
        | IncomingMessage::Settings { .. }
        | IncomingMessage::ImageOp { .. }
        | IncomingMessage::RegisterEffectStub { .. }
        | IncomingMessage::UnregisterEffectStub { .. } => {
            let effects = s.core.apply(msg);

            for effect in effects {
                use plushie_widget_sdk::engine::CoreEffect;
                match effect {
                    CoreEffect::EmitEvent(event) => {
                        s.writer.emit(&event.with_session(session_id))?;
                    }
                    CoreEffect::EmitEffectResponse(response) => {
                        s.writer.emit(&response.with_session(session_id))?;
                    }
                    CoreEffect::EmitStubAck(ack) => {
                        s.writer.emit(&ack.with_session(session_id))?;
                    }
                    CoreEffect::HandleEffect {
                        request_id,
                        kind,
                        payload,
                    } => {
                        if crate::effects::is_async_effect(&kind) {
                            let mode = match s.mode {
                                Mode::Headless => "headless",
                                Mode::Mock => "mock",
                            };
                            log::debug!("{mode}: async effect {kind} unsupported (no display)");
                            s.writer.emit(
                                &plushie_widget_sdk::protocol::EffectResponse::unsupported(
                                    request_id,
                                )
                                .with_session(session_id),
                            )?;
                        } else {
                            let response =
                                crate::effects::handle_effect(request_id, &kind, &payload);
                            s.writer.emit(&response.with_session(session_id))?;
                        }
                    }
                    CoreEffect::ThemeChanged(t) => {
                        let mode_str = if t == iced::Theme::Light {
                            "light"
                        } else {
                            "dark"
                        };
                        // Emit theme_changed subscription event if active,
                        // matching windowed mode behavior.
                        for entry in s.core.matching_entries(
                            plushie_renderer_lib::constants::SUB_THEME_CHANGE,
                            None,
                        ) {
                            let _ = s.writer.emit(
                                &plushie_widget_sdk::protocol::OutgoingEvent::theme_changed(
                                    entry.tag.clone(),
                                    mode_str.to_string(),
                                )
                                .with_session(session_id),
                            );
                        }
                        s.theme = t;
                    }
                    CoreEffect::ImageOp {
                        op,
                        handle,
                        data,
                        pixels,
                        width,
                        height,
                    } => {
                        let mode = match s.mode {
                            Mode::Headless => "headless",
                            Mode::Mock => "mock",
                        };
                        if let Err(e) = s.images.apply_op(&op, &handle, data, pixels, width, height)
                        {
                            log::warn!("{mode}: image_op {op} failed: {e}");
                        }
                    }
                    CoreEffect::WidgetConfig(config) => {
                        let ctx = plushie_widget_sdk::registry::InitCtx {
                            config: &config,
                            theme: &s.theme,
                            default_text_size: s.core.default_text_size,
                            default_font: s.core.default_font,
                        };
                        s.registry.init_all(&ctx);
                    }
                    CoreEffect::SyncWindows => {}
                    CoreEffect::WidgetOp {
                        ref op,
                        ref payload,
                    } if op == "load_font" => {
                        load_font_from_payload(s, session_id, payload);
                    }
                    CoreEffect::WidgetOp {
                        ref op,
                        ref payload,
                    } if op == "announce" => {
                        let announce_text = payload
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let politeness = payload
                            .get("politeness")
                            .and_then(|v| v.as_str())
                            .unwrap_or("assertive")
                            .to_string();
                        let event = plushie_widget_sdk::protocol::OutgoingEvent::generic(
                            "announce",
                            "",
                            Some(serde_json::json!({
                                "text": announce_text,
                                "politeness": politeness,
                            })),
                        );
                        let _ = s.writer.emit(&event.with_session(session_id));
                    }
                    CoreEffect::WidgetOp {
                        ref op,
                        ref payload,
                    } if op == "find_focused" => {
                        // In headless mode we cannot query iced's focus state
                        // (no persistent widget operation runtime). Emit a
                        // response with null to indicate focus tracking is
                        // unavailable.
                        let tag = payload
                            .get("tag")
                            .and_then(|v| v.as_str())
                            .unwrap_or("find_focused")
                            .to_string();
                        let resp = serde_json::json!({
                            "type": "op_query_response",
                            "session": session_id,
                            "kind": "find_focused",
                            "tag": tag,
                            "data": {"focused": null}
                        });
                        let _ = s.writer.emit(&resp);
                    }
                    CoreEffect::WidgetOp { .. } => {}
                    CoreEffect::WindowOp { .. }
                    | CoreEffect::SystemOp { .. }
                    | CoreEffect::SystemQuery { .. } => {}
                    CoreEffect::ThemeFollowsSystem => {}
                    CoreEffect::ExitNodes(nodes) => {
                        for (parent_id, index, node) in nodes {
                            s.transition_manager
                                .ghosts
                                .add_ghost(&parent_id, node, index);
                        }
                    }
                }
            }

            if is_settings {
                s.rebuild_renderer();
            }
            if is_tree_change {
                if is_snapshot {
                    s.transition_manager.clear();
                }
                if let Some(root) = s.core.tree.root_mut() {
                    s.registry.prepare_and_scan(
                        root,
                        &mut s.core.caches,
                        &s.theme,
                        &mut s.transition_manager,
                    );
                }

                let settle_events = s.settle_ui(session_id);
                for event in settle_events {
                    s.writer.emit(&event).ok();
                }
            }
        }

        IncomingMessage::Query {
            id,
            target,
            selector,
        } => {
            let resp = plushie_renderer_lib::scripting::build_query_response(
                &s.core, id, target, selector,
            )
            .with_session(session_id);
            s.writer.emit(&resp)?;
        }
        IncomingMessage::Interact {
            id,
            action,
            selector,
            payload,
        } => {
            let widget_id = resolve_widget_id(&s.core, &selector);

            // Mock mode: use focus+Space for click/toggle actions to avoid
            // depending on cursor position (which requires accurate layout).
            // The null renderer gives approximate layout that's not reliable
            // enough for cursor-based hit testing. Focus operations work by
            // widget ID (position-independent), and Space activates the
            // focused widget through the existing keyboard handling in
            // buttons, checkboxes, and togglers.
            // In mock mode, actions that would normally inject mouse events
            // at cursor positions use alternative paths because the null
            // renderer's approximate layout isn't reliable for hit testing.
            //
            // - click/toggle: focus by ID + Space key (position-independent)
            // - select: synthetic event (pick_list selection can't be done
            //   via focus+Space since it requires opening a dropdown and
            //   choosing a specific option)
            // Semantic actions (click, toggle, select) use the synthetic
            // event path in ALL modes. The synthetic path emits the correct
            // semantic event directly from the tree, which is more reliable
            // than injecting mouse events at widget coordinates. This works
            // identically in mock and headless modes.
            //
            // Canvas actions also use synthetic construction because
            // canvas_press/release/move coordinates are canvas-relative.
            //
            // Low-level actions (press, release, type_key, type_text) still
            // use iced event injection in headless mode since they depend on
            // cursor position and keyboard state.
            let use_synthetic = matches!(
                action.as_str(),
                "click" | "toggle" | "select" | "canvas_press" | "canvas_release" | "canvas_move"
            ) && widget_id.is_some();

            let iced_events = if use_synthetic {
                // Handled via synthetic path below.
                vec![]
            } else {
                let cursor = s.ui.cursor;
                interaction_to_iced_events(&action, widget_id.as_deref(), &payload, cursor)
            };

            let events = if use_synthetic {
                // Synthetic event path: emit events directly from the
                // tree without going through iced's widget system.
                plushie_renderer_lib::scripting::build_interact_response(
                    &s.core,
                    id.clone(),
                    action,
                    selector,
                    payload,
                )
                .events
            } else if !iced_events.is_empty() {
                // Headless mode or keyboard actions: inject real iced
                // events with host round-trips between events that
                // produce widget Messages.
                let had_steps = s.inject_and_capture(session_id, &id, &iced_events, read_next);

                if had_steps {
                    vec![]
                } else {
                    plushie_renderer_lib::scripting::build_interact_response(
                        &s.core,
                        id.clone(),
                        action,
                        selector,
                        payload,
                    )
                    .events
                }
            } else {
                // Action with no iced events: use synthetic event
                // construction.
                plushie_renderer_lib::scripting::build_interact_response(
                    &s.core,
                    id.clone(),
                    action,
                    selector,
                    payload,
                )
                .events
            };

            let resp = plushie_widget_sdk::protocol::InteractResponse::new(id, events)
                .with_session(session_id);
            s.writer.emit(&resp)?;
        }
        IncomingMessage::TreeHash { id, name, .. } => {
            let resp = plushie_renderer_lib::scripting::build_tree_hash_response(&s.core, id, name)
                .with_session(session_id);
            s.writer.emit(&resp)?;
        }
        IncomingMessage::Screenshot {
            id,
            name,
            width,
            height,
        } => {
            let w = width
                .unwrap_or(DEFAULT_SCREENSHOT_WIDTH)
                .clamp(1, MAX_SCREENSHOT_DIMENSION);
            let h = height
                .unwrap_or(DEFAULT_SCREENSHOT_HEIGHT)
                .clamp(1, MAX_SCREENSHOT_DIMENSION);
            handle_screenshot(s, session_id, id, name, w, h)?;
        }
        IncomingMessage::Reset { id } => {
            s.images = ImageRegistry::new();
            s.theme = Theme::Dark;
            s.transition_manager.clear();
            s.ui.ui_cache = UiCache::default();
            s.ui.cursor = mouse::Cursor::Unavailable;
            s.rebuild_renderer();
            let resp = plushie_renderer_lib::scripting::build_reset_response(&mut s.core, id)
                .with_session(session_id);
            s.writer.emit(&resp)?;
        }
        IncomingMessage::Command { id, family, value } => {
            if let Some(events) = s.registry.handle_widget_op(&id, &family, &value) {
                for event in events {
                    s.writer.emit(&event.with_session(session_id))?;
                }
            }
        }
        IncomingMessage::Commands { commands } => {
            for cmd in commands {
                if let Some(events) = s
                    .registry
                    .handle_widget_op(&cmd.id, &cmd.family, &cmd.value)
                {
                    for event in events {
                        s.writer.emit(&event.with_session(session_id))?;
                    }
                }
            }
        }
        IncomingMessage::AdvanceFrame { timestamp } => {
            // Advance renderer-side transitions
            let completions = s
                .transition_manager
                .advance_with_timestamp(timestamp, &mut s.core.caches.interpolated_props);

            // Emit transition_complete events
            for c in completions {
                let event = plushie_widget_sdk::protocol::OutgoingEvent::generic(
                    "transition_complete",
                    c.widget_id.clone(),
                    Some(serde_json::json!({
                        "tag": c.tag,
                        "prop": c.prop_name,
                    })),
                );
                s.writer.emit(&event.with_session(session_id))?;
            }

            // Emit animation_frame events to SDK
            for entry in s
                .core
                .matching_entries(plushie_renderer_lib::constants::SUB_ANIMATION_FRAME, None)
            {
                s.writer.emit(
                    &plushie_widget_sdk::protocol::OutgoingEvent::animation_frame(
                        entry.tag.clone(),
                        timestamp as u128,
                    )
                    .with_session(session_id),
                )?;
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Screenshot capture
// ---------------------------------------------------------------------------

fn handle_screenshot<R: PlushieRenderer>(
    s: &mut Session<R>,
    session_id: &str,
    id: String,
    name: String,
    width: u32,
    height: u32,
) -> io::Result<()> {
    let emit_stub = |s: &Session<R>| {
        let map = screenshot_map(session_id, &id, &name, "", 0, 0);
        s.writer.emit_binary(map, None)
    };

    // Mock mode: return stub screenshots for compatibility with
    // existing tests that check for empty RGBA data.
    if matches!(s.mode, Mode::Mock) {
        return emit_stub(s);
    }

    use iced_test::core::theme::Base;
    use sha2::{Digest, Sha256};

    s.ui.viewport_size = Size::new(width as f32, height as f32);

    let root = match s.core.tree.root() {
        Some(r) => r,
        None => return emit_stub(s),
    };

    let ctx = RenderCtx {
        caches: &s.core.caches,
        images: &s.images,
        theme: &s.theme,
        registry: &s.registry,
        default_text_size: s.core.default_text_size,
        default_font: s.core.default_font,
        window_id: "",
        scale_factor: 1.0,
    };
    let element: iced::Element<'_, plushie_widget_sdk::message::Message, Theme, R> =
        plushie_widget_sdk::widget::render(root, ctx);

    let cache = std::mem::take(&mut s.ui.ui_cache);
    let mut ui = iced_test::runtime::UserInterface::build(
        element,
        s.ui.viewport_size,
        cache,
        &mut s.ui.renderer,
    );

    {
        let cursor = s.ui.cursor;
        let mut messages = Vec::new();
        let redraw = Event::Window(iced::window::Event::RedrawRequested(
            iced_test::core::time::Instant::now(),
        ));
        let _status = ui.update(&[redraw], cursor, &mut s.ui.renderer, &mut messages);
    }

    let base = s.theme.base();
    ui.draw(
        &mut s.ui.renderer,
        &s.theme,
        &iced_test::core::renderer::Style {
            text_color: base.text_color,
        },
        s.ui.cursor,
    );

    s.ui.ui_cache = ui.into_cache();

    let phys_size = iced::Size::new(width, height);
    let rgba =
        s.ui.renderer
            .screenshot(phys_size, 1.0, base.background_color);

    let hash = {
        let mut hasher = Sha256::new();
        hasher.update(&rgba);
        format!("{:x}", hasher.finalize())
    };

    let map = screenshot_map(session_id, &id, &name, &hash, width, height);
    let binary = if rgba.is_empty() {
        None
    } else {
        Some(("rgba", rgba.as_slice()))
    };
    s.writer.emit_binary(map, binary)
}

/// Build the JSON map for a screenshot_response message.
fn screenshot_map(
    session: &str,
    id: &str,
    name: &str,
    hash: &str,
    width: u32,
    height: u32,
) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::json;
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), json!("screenshot_response"));
    map.insert("session".to_string(), json!(session));
    map.insert("id".to_string(), json!(id));
    map.insert("name".to_string(), json!(name));
    map.insert("hash".to_string(), json!(hash));
    map.insert("width".to_string(), json!(width));
    map.insert("height".to_string(), json!(height));
    map
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Run the headless/mock event loop.
///
/// When `max_sessions` is 1, runs a single session on the current
/// thread (same as the original design). When > 1, spawns reader,
/// writer, and per-session threads for concurrent multiplexing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    forced_codec: Option<Codec>,
    mode: Mode,
    max_sessions: usize,
    ext_keys: &[String],
    transport_name: &str,
    mut reader: BufReader<Box<dyn Read + Send>>,
    writer: Box<dyn std::io::Write + Send>,
    expected_token: Option<&str>,
    session_factory: Option<plushie_widget_sdk::app::SessionRegistryFactory<iced::Renderer>>,
) {
    // Detect codec BEFORE initializing the sink so the WriterSink
    // encodes with the correct codec from the first message.
    // Codec detection errors can't be sent on the wire (no codec yet),
    // so they log to stderr and return.
    let codec = match crate::startup::detect_codec(forced_codec, &mut reader) {
        Ok(c) => c,
        Err(e) => {
            log::error!("{e}");
            return;
        }
    };

    let sink = plushie_renderer_lib::WriterSink::new(writer, codec);
    plushie_renderer_lib::emitters::init_sink(Box::new(sink));

    let (mode_str, backend) = match mode {
        Mode::Headless => ("headless", "tiny-skia"),
        Mode::Mock => ("mock", "mock"),
    };
    let ext_key_refs: Vec<&str> = ext_keys.iter().map(|s| s.as_str()).collect();
    if let Err(e) = plushie_renderer_lib::emitters::emit_hello(
        mode_str,
        backend,
        &ext_key_refs,
        &["iced"],
        transport_name,
    ) {
        log_hello_error(&e);
        return;
    }

    // Settings gate: require Settings as the first message from the host.
    // Fonts are loaded later when handle_message processes the forwarded
    // Settings through the session (via load_fonts_from_settings).
    let initial = match crate::startup::read_required_settings(&codec, &mut reader) {
        Ok(v) => v,
        Err(e) => {
            crate::startup::emit_startup_error(&codec, &e);
            return;
        }
    };
    if let Err(e) = crate::startup::validate_settings(&initial.settings, expected_token) {
        crate::startup::emit_startup_error(&codec, &e);
        return;
    }

    // Branch on mode once at the top. Headless uses iced::Renderer
    // (tiny-skia) for real screenshots. Mock uses the null renderer ()
    // for speed (synthetic events handle all interactions).
    //
    // The session_factory parameter is only honoured for iced::Renderer
    // sessions; mock mode is protocol-only and always uses the built-in
    // iced widget set.
    match mode {
        Mode::Headless => {
            if max_sessions <= 1 {
                run_single::<iced::Renderer>(codec, mode, &mut reader, initial, None);
            } else {
                run_multiplexed::<iced::Renderer>(
                    codec,
                    mode,
                    max_sessions,
                    &mut reader,
                    initial,
                    session_factory,
                );
            }
        }
        Mode::Mock => {
            if max_sessions <= 1 {
                run_single::<()>(codec, mode, &mut reader, initial, None);
            } else {
                run_multiplexed::<()>(codec, mode, max_sessions, &mut reader, initial, None);
            }
        }
    }

    log::info!("stdin closed, exiting");
}

// ---------------------------------------------------------------------------
// Font loading (headless-specific)
// ---------------------------------------------------------------------------

/// Load fonts from the Settings message's `fonts` array.
///
/// In windowed mode, fonts are loaded via iced Tasks during daemon init.
/// Headless mode has no Task runtime, so we load fonts directly into
/// the global font system that the tiny-skia renderer uses.
fn load_fonts_from_settings(settings: &serde_json::Value) {
    // Load inline font data (base64 or binary).
    for bytes in plushie_renderer_lib::settings::parse_inline_fonts(settings) {
        load_font_bytes(bytes);
    }

    // Load fonts from file paths (native only).
    let Some(fonts) = settings.get("fonts").and_then(|v| v.as_array()) else {
        return;
    };
    for font_val in fonts {
        if let Some(path) = font_val.as_str() {
            match std::fs::read(path) {
                Ok(bytes) => {
                    load_font_bytes(bytes);
                    log::info!("loaded font: {path}");
                }
                Err(e) => {
                    log::error!("failed to load font {path}: {e}");
                }
            }
        }
    }
}

use plushie_renderer_lib::constants::MAX_FONT_BYTES;

/// Maximum number of runtime font loads per process lifetime. Each
/// load permanently leaks font bytes into the global font system.
const MAX_LOADED_FONTS: u32 = 256;

/// Process-wide counter of runtime font loads (headless mode).
static LOADED_FONT_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Load a font from a `load_font` WidgetOp payload (base64 or binary data).
///
/// Every outcome is logged with a `[code=...]` tag so host SDKs can
/// filter font lifecycle events. When the process-wide cap is hit, an
/// outgoing `session_error` with code `font_cap_exceeded` is emitted
/// to the specific session that tripped it.
///
/// The counter is bumped both per-session (`session.fonts_loaded`) and
/// process-wide (`LOADED_FONT_COUNT`) so the cap is enforced globally
/// but attribution is local; hosts can tell which session exhausted
/// the budget.
fn load_font_from_payload<R: PlushieRenderer>(
    session: &mut Session<R>,
    session_id: &str,
    payload: &serde_json::Value,
) {
    let Some(data_val) = payload.get("data") else {
        log::error!("[code=font_load_failed] load_font: missing 'data' field");
        return;
    };
    let Some(bytes) = plushie_renderer_lib::settings::decode_font_data(data_val) else {
        log::error!("[code=font_load_failed] load_font: failed to decode font data");
        return;
    };
    if bytes.is_empty() {
        log::error!("[code=font_load_failed] load_font: empty font data");
        return;
    }
    if bytes.len() > MAX_FONT_BYTES {
        log::error!(
            "[code=font_load_failed] load_font: font data ({} bytes) exceeds {} byte limit, rejecting",
            bytes.len(),
            MAX_FONT_BYTES
        );
        return;
    }
    if LOADED_FONT_COUNT.load(std::sync::atomic::Ordering::Relaxed) >= MAX_LOADED_FONTS {
        let msg = format!(
            "load_font rejected: process-wide cap of {MAX_LOADED_FONTS} fonts reached \
             (this session has loaded {})",
            session.fonts_loaded
        );
        log::error!("[code=font_cap_exceeded] session '{session_id}': {msg}");
        // Emit a session_error so the specific session knows it hit
        // the cap rather than silently failing. The cap is shared
        // across all sessions; other sessions are unaffected.
        let event = plushie_widget_sdk::protocol::OutgoingEvent::generic(
            "session_error",
            "",
            Some(serde_json::json!({ "code": "font_cap_exceeded", "error": msg })),
        );
        let _ = session.writer.emit(&event.with_session(session_id));
        return;
    }
    LOADED_FONT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    session.fonts_loaded = session.fonts_loaded.saturating_add(1);
    let len = bytes.len();
    load_font_bytes(bytes);
    log::info!(
        "[code=font_loaded] session '{session_id}': loaded font ({len} bytes, session total {})",
        session.fonts_loaded
    );
}

/// Register font bytes with the global font system.
///
/// The font system is shared between all iced renderers (windowed and
/// headless). Once loaded, fonts are available to all subsequent renders.
fn load_font_bytes(bytes: Vec<u8>) {
    let fs = iced::advanced::graphics::text::font_system();
    let mut guard = fs.write().expect("font system lock");
    guard.load_font(std::borrow::Cow::Owned(bytes));
}

/// Read and decode the next message from a BufRead source.
fn read_message(codec: Codec, reader: &mut impl BufRead) -> Option<SessionMessage> {
    loop {
        match codec.read_message(reader) {
            Ok(None) => return None,
            Ok(Some(bytes)) => {
                let value: serde_json::Value = match codec.decode(&bytes) {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("decode error: {e}");
                        continue;
                    }
                };
                match SessionMessage::from_value(value) {
                    Ok(sm) => return Some(sm),
                    Err(e) => {
                        log::error!("decode error: {e}");
                        continue;
                    }
                }
            }
            Err(e) => {
                log::error!("read error: {e}");
                return None;
            }
        }
    }
}

/// Single-session event loop (max_sessions=1).
///
/// Uses the same bounded channel + dedicated writer thread as the
/// multiplexed path so backpressure from a slow host pauses the
/// session thread instead of silently growing buffers inside
/// stdout.
fn run_single<R: PlushieRenderer>(
    codec: Codec,
    mode: Mode,
    reader: &mut impl BufRead,
    initial: crate::startup::InitialSettings,
    session_factory: Option<plushie_widget_sdk::app::SessionRegistryFactory<R>>,
) {
    // Writer thread: drains the channel and writes to stdout. Same
    // capacity (256) as the multiplexed path.
    let (writer_tx, writer_rx) = mpsc::sync_channel::<Vec<u8>>(256);
    let writer_handle = thread::spawn(move || {
        for bytes in writer_rx {
            if plushie_renderer_lib::emitters::write_output(&bytes).is_err() {
                break;
            }
        }
    });

    let writer = WireWriter::channel(writer_tx.clone(), codec);
    let mut session = match session_factory {
        Some(factory) => Session::<R>::with_registry(mode, writer, factory()),
        None => Session::<R>::new(mode, writer),
    };

    // Process the initial Settings through the session so Core.apply()
    // picks up default_event_rate, default_text_size, widget config, etc.
    {
        let (session_id, msg) = initial.into_parts();
        let mut read_next = || read_message(codec, reader).map(|sm| sm.message);
        if let Err(e) = handle_message(&mut session, &session_id, msg, &mut read_next) {
            log::error!("write error processing initial settings: {e}");
            // Drop session FIRST so its WireWriter (which holds a
            // clone of writer_tx) releases the sender before we
            // join the writer thread - otherwise join() deadlocks.
            drop(session);
            drop(writer_tx);
            let _ = writer_handle.join();
            return;
        }
    }

    while let Some(sm) = read_message(codec, reader) {
        // Provide a callback that reads the next message from stdin.
        // Used by inject_and_capture during iterative interact to
        // wait for the host's snapshot between events.
        let mut read_next = || read_message(codec, reader).map(|sm| sm.message);

        if let Err(e) = handle_message(&mut session, &sm.session, sm.message, &mut read_next) {
            log::error!("write error: {e}");
            break;
        }
    }

    // Drop session (and its cloned writer_tx) BEFORE joining the
    // writer thread, otherwise the thread's for-loop over writer_rx
    // never exits because a sender is still alive.
    drop(session);
    drop(writer_tx);
    let _ = writer_handle.join();
}

// ---------------------------------------------------------------------------
// Multiplex machinery: dispatch, lifecycle, and back-channel signals
// ---------------------------------------------------------------------------

/// Per-session capacity for the reader -> session channel.
///
/// Reader never blocks on a single session: messages are pushed via
/// `try_send`; if full, the pending buffer at [`PENDING_BUFFER_CAP`]
/// absorbs the burst. If both the channel and the pending buffer are
/// full, the session is ejected with a backpressure-overflow error.
const SESSION_CHANNEL_CAP: usize = 512;

/// Per-session overflow buffer. The reader drops into this buffer when
/// the primary channel is full and flushes from it on the next read
/// iteration. Each session can absorb up to
/// `SESSION_CHANNEL_CAP + PENDING_BUFFER_CAP` queued messages before
/// the dispatcher gives up and ejects it.
const PENDING_BUFFER_CAP: usize = 512;

/// Signals emitted by session threads for the reader to act on.
enum SessionSignal {
    /// Session finished normally (may or may not have emitted panic
    /// event before). Reader removes it from `closing_sessions` so the
    /// host can reuse the ID.
    Closed(String),
    /// Session thread panicked; the panic event has already been sent
    /// to the writer. Reader proactively removes dispatch entry so a
    /// follow-up message on the same ID doesn't silently create a new
    /// session; it gets a structured rejection instead.
    Panicked(String),
}

/// Multiplexed event loop (max_sessions > 1). Reader thread dispatches
/// to per-session threads. Writer thread serializes output to stdout.
#[allow(clippy::too_many_lines)]
fn run_multiplexed<R: PlushieRenderer>(
    codec: Codec,
    mode: Mode,
    max_sessions: usize,
    reader: &mut impl BufRead,
    initial: crate::startup::InitialSettings,
    session_factory: Option<plushie_widget_sdk::app::SessionRegistryFactory<R>>,
) {
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    // Writer thread: drains the channel and writes to stdout.
    // Bounded channel (256) since this is the aggregation point for all
    // sessions; back-pressure blocks session threads when stdout is slow.
    //
    // `writer_alive` is toggled to false when the writer exits (EOF or
    // write error). Session threads and the reader check it before
    // enqueueing to avoid blocking on a dead channel.
    let writer_alive = Arc::new(AtomicBool::new(true));
    let (writer_tx, writer_rx) = mpsc::sync_channel::<Vec<u8>>(256);
    let writer_alive_for_thread = writer_alive.clone();
    let writer_handle = thread::spawn(move || {
        for bytes in writer_rx {
            if plushie_renderer_lib::emitters::write_output(&bytes).is_err() {
                break;
            }
        }
        // Flag down whether we broke out early or the channel closed.
        writer_alive_for_thread.store(false, Ordering::SeqCst);
    });

    // Writer -> reader signal channel. Used by session threads to
    // inform the reader when they close so the reader can proactively
    // clean up its dispatch map (avoiding F-04's "host sees errors on
    // unrelated follow-ups"), and to unblock the Reset / closing_sessions
    // lifecycle (F-02).
    let (signal_tx, signal_rx) = mpsc::channel::<SessionSignal>();

    // Session dispatch table: session_id -> primary sender + pending
    // overflow buffer. The pending buffer absorbs bursts when the
    // primary channel is full so the reader never blocks on a single
    // slow session.
    struct Dispatch {
        tx: mpsc::SyncSender<IncomingMessage>,
        pending: VecDeque<IncomingMessage>,
    }
    let mut sessions: HashMap<String, Dispatch> = HashMap::new();
    let mut session_handles: Vec<thread::JoinHandle<()>> = Vec::new();

    // Sessions that received Reset and are waiting on session_closed.
    // New messages for an ID in this set are rejected with
    // `session_reset_in_progress` until the session thread reports
    // Closed on the signal channel.
    let mut closing_sessions: HashSet<String> = HashSet::new();

    // The initial Settings was already validated by the startup gate.
    // Feed it as the first message so the first session gets Core.apply().
    let mut pending_initial_settings = Some(initial.into_incoming_message());

    // Helper: emit a structured session_error on the writer channel.
    let emit_session_error =
        |writer_tx: &mpsc::SyncSender<Vec<u8>>, session_id: &str, code: &str, message: &str| {
            let event = serde_json::json!({
                "type": "event",
                "session": session_id,
                "family": "session_error",
                "id": "",
                "data": { "code": code, "error": message }
            });
            if let Ok(bytes) = codec.encode(&event) {
                let _ = writer_tx.send(bytes);
            }
        };

    loop {
        // Drain writer -> reader signals before each read so lifecycle
        // cleanup runs promptly. A second drain runs after read_message
        // returns (see below) to catch signals that arrived while we
        // were blocked waiting for host input.
        while let Ok(signal) = signal_rx.try_recv() {
            match signal {
                SessionSignal::Closed(sid) => {
                    closing_sessions.remove(&sid);
                    sessions.remove(&sid);
                }
                SessionSignal::Panicked(sid) => {
                    sessions.remove(&sid);
                    closing_sessions.remove(&sid);
                }
            }
        }

        // Bail if writer died: emit session_error to every still-active
        // session so the host sees structured failure per session, then
        // exit the dispatch loop.
        if !writer_alive.load(Ordering::SeqCst) {
            for sid in sessions.keys().cloned().collect::<Vec<_>>() {
                emit_session_error(
                    &writer_tx,
                    &sid,
                    "writer_dead",
                    "renderer stdout writer thread exited unexpectedly",
                );
            }
            log::error!("writer thread exited; dispatcher stopping");
            break;
        }

        match codec.read_message(reader) {
            Ok(None) => break,
            Ok(Some(bytes)) => {
                let value: serde_json::Value = match codec.decode(&bytes) {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("decode error: {e}");
                        continue;
                    }
                };
                let sm = match SessionMessage::from_value(value) {
                    Ok(sm) => sm,
                    Err(e) => {
                        log::error!("decode error: {e}");
                        continue;
                    }
                };

                let session_id = sm.session.clone();

                // Drain any signals that arrived while we were blocked
                // on read_message. A host can see session_closed on the
                // wire and reuse the session ID faster than the reader
                // can return to the top of the loop; without this
                // second drain the new message would be rejected as
                // "session_reset_in_progress" for a session that's
                // actually finished closing.
                while let Ok(signal) = signal_rx.try_recv() {
                    match signal {
                        SessionSignal::Closed(sid) => {
                            closing_sessions.remove(&sid);
                            sessions.remove(&sid);
                        }
                        SessionSignal::Panicked(sid) => {
                            sessions.remove(&sid);
                            closing_sessions.remove(&sid);
                        }
                    }
                }

                // Messages for a session that's in the middle of a Reset
                // teardown: reject cleanly rather than spawn a fresh
                // session under the same ID. Host should wait for
                // session_closed before reusing the ID.
                if closing_sessions.contains(&session_id) {
                    log::debug!("session '{session_id}': message rejected during reset teardown");
                    emit_session_error(
                        &writer_tx,
                        &session_id,
                        "session_reset_in_progress",
                        "session is closing; wait for session_closed before reusing this ID",
                    );
                    continue;
                }

                // Check if this is a Reset; if so, mark for teardown after
                // forwarding the Reset itself.
                let is_reset = matches!(sm.message, IncomingMessage::Reset { .. });

                // Get or create the session thread.
                let session_existed = sessions.contains_key(&session_id);
                if !session_existed {
                    if sessions.len() >= max_sessions {
                        log::error!(
                            "max sessions ({max_sessions}) reached; \
                             rejecting session '{session_id}'"
                        );
                        emit_session_error(
                            &writer_tx,
                            &session_id,
                            "max_sessions_reached",
                            &format!(
                                "max sessions ({max_sessions}) reached; session \
                                 '{session_id}' rejected"
                            ),
                        );
                        continue;
                    }

                    // Per-session bounded channel; reader uses try_send so
                    // a slow session never blocks the dispatcher.
                    let (tx, rx) = mpsc::sync_channel::<IncomingMessage>(SESSION_CHANNEL_CAP);
                    let writer = WireWriter::channel(writer_tx.clone(), codec);
                    let sid = session_id.clone();

                    let closed_writer_tx = writer_tx.clone();
                    let thread_factory = session_factory.clone();
                    let signal_tx_for_thread = signal_tx.clone();
                    let handle = thread::spawn(move || {
                        let panicked = {
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    let mut session = match thread_factory {
                                        Some(factory) => {
                                            Session::<R>::with_registry(mode, writer, factory())
                                        }
                                        None => Session::<R>::new(mode, writer),
                                    };
                                    for msg in &rx {
                                        let mut read_next = || rx.recv().ok();
                                        if let Err(e) =
                                            handle_message(&mut session, &sid, msg, &mut read_next)
                                        {
                                            log::error!("session '{sid}': write error: {e}");
                                            break;
                                        }
                                    }
                                    log::debug!("session '{sid}' thread exiting");
                                }));
                            if let Err(payload) = result {
                                let msg = payload
                                    .downcast_ref::<&str>()
                                    .copied()
                                    .or_else(|| {
                                        payload.downcast_ref::<String>().map(|s| s.as_str())
                                    })
                                    .unwrap_or("(non-string panic)");
                                log::error!("session '{sid}' thread panicked: {msg}");
                                let error = serde_json::json!({
                                    "type": "event",
                                    "session": sid,
                                    "family": "session_error",
                                    "id": "",
                                    "data": { "code": "session_panic", "error": msg }
                                });
                                if let Ok(bytes) = codec.encode(&error) {
                                    let _ = closed_writer_tx.send(bytes);
                                }
                                true
                            } else {
                                false
                            }
                        };

                        // Signal the reader BEFORE the wire-visible
                        // session_closed. The reader drains signals at
                        // the top of each dispatch iteration, so by
                        // the time the host sees session_closed and
                        // issues a follow-up message reusing the ID,
                        // the reader has already removed the ID from
                        // its closing set and will spawn a fresh
                        // session. Reversing the order introduces a
                        // race where a fast host gets rejected as
                        // "reset in progress" for a session that has
                        // in fact closed.
                        let signal = if panicked {
                            SessionSignal::Panicked(sid.clone())
                        } else {
                            SessionSignal::Closed(sid.clone())
                        };
                        let _ = signal_tx_for_thread.send(signal);

                        // Now emit session_closed on the wire so the
                        // host sees its terminal event. Delivery is
                        // best-effort: a dead writer just means the
                        // send fails quietly.
                        let closed = serde_json::json!({
                            "type": "event",
                            "session": sid,
                            "family": "session_closed",
                            "id": "",
                            "data": {}
                        });
                        match codec.encode(&closed) {
                            Ok(bytes) => {
                                if closed_writer_tx.send(bytes).is_err() {
                                    log::info!(
                                        "session '{sid}': session_closed send failed \
                                         (writer likely gone)"
                                    );
                                }
                            }
                            Err(e) => {
                                log::info!("session '{sid}': session_closed encode failed: {e}");
                            }
                        }
                    });

                    sessions.insert(
                        session_id.clone(),
                        Dispatch {
                            tx,
                            pending: VecDeque::new(),
                        },
                    );
                    session_handles.push(handle);
                    log::info!(
                        "session '{}' created (active: {})",
                        session_id,
                        sessions.len()
                    );

                    // Send the initial Settings to the first session so
                    // Core.apply() processes it.
                    if let Some(settings_msg) = pending_initial_settings.take()
                        && let Some(d) = sessions.get_mut(&session_id)
                        && try_enqueue(&mut d.tx, &mut d.pending, settings_msg).is_err()
                    {
                        log::error!("session '{session_id}': failed to queue initial settings");
                    }
                }

                // Deliver the message. Non-blocking dispatch via
                // try_send + pending buffer; if both are full the
                // session is ejected with a backpressure overflow error.
                let (ejected, dispatch_error) = if let Some(d) = sessions.get_mut(&session_id) {
                    match try_enqueue(&mut d.tx, &mut d.pending, sm.message) {
                        Ok(()) => (false, None),
                        Err(EnqueueError::Overflow) => (
                            true,
                            Some((
                                "session_backpressure_overflow",
                                format!(
                                    "session '{session_id}' send queue saturated \
                                     (channel + pending = {}); ejecting",
                                    SESSION_CHANNEL_CAP + PENDING_BUFFER_CAP
                                ),
                            )),
                        ),
                        Err(EnqueueError::Disconnected) => (
                            true,
                            Some((
                                "session_channel_closed",
                                format!("session '{session_id}' channel closed unexpectedly"),
                            )),
                        ),
                    }
                } else {
                    (false, None)
                };

                if let Some((code, msg)) = dispatch_error {
                    emit_session_error(&writer_tx, &session_id, code, &msg);
                }
                if ejected {
                    sessions.remove(&session_id);
                    continue;
                }

                // If this was a Reset, mark the session as closing so
                // follow-up same-ID messages hit the reject path until
                // the thread reports session_closed on the signal channel.
                if is_reset {
                    closing_sessions.insert(session_id.clone());
                    // Drop the primary sender so the session thread
                    // exits once it has drained the Reset message.
                    sessions.remove(&session_id);
                    log::info!(
                        "session '{session_id}' reset in progress (active: {})",
                        sessions.len()
                    );
                }
            }
            Err(e) => {
                log::error!("read error: {e}");
                break;
            }
        }
    }

    // Drop all session senders so threads exit.
    sessions.clear();
    // Drop the writer sender so the writer thread exits.
    drop(writer_tx);
    drop(signal_tx);

    for handle in session_handles {
        if let Err(payload) = handle.join() {
            // Extract a useful message from the panic payload.
            let msg = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
                .unwrap_or("(non-string panic)");
            log::error!("session thread panicked: {msg}");
        }
    }
    if let Err(payload) = writer_handle.join() {
        let msg = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("(non-string panic)");
        log::error!("writer thread panicked: {msg}");
    }
}

/// Result of attempting to enqueue a message to a session thread.
enum EnqueueError {
    /// Channel full and pending buffer also full.
    Overflow,
    /// Receiver dropped (session thread exited).
    Disconnected,
}

/// Non-blocking enqueue with overflow buffer.
///
/// Flushes any pending messages first, then tries the new message.
/// When `try_send` reports full, the message goes into the pending
/// buffer up to [`PENDING_BUFFER_CAP`]. Past that, the session is
/// considered unrecoverable and the caller ejects it.
fn try_enqueue(
    tx: &mut mpsc::SyncSender<IncomingMessage>,
    pending: &mut std::collections::VecDeque<IncomingMessage>,
    msg: IncomingMessage,
) -> Result<(), EnqueueError> {
    // Drain the pending buffer first so ordering is preserved.
    while let Some(buffered) = pending.pop_front() {
        match tx.try_send(buffered) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(back)) => {
                pending.push_front(back);
                break;
            }
            Err(mpsc::TrySendError::Disconnected(_)) => return Err(EnqueueError::Disconnected),
        }
    }

    if pending.is_empty() {
        // Pending drained successfully; try the new message directly.
        match tx.try_send(msg) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(back)) => {
                if pending.len() >= PENDING_BUFFER_CAP {
                    Err(EnqueueError::Overflow)
                } else {
                    pending.push_back(back);
                    Ok(())
                }
            }
            Err(mpsc::TrySendError::Disconnected(_)) => Err(EnqueueError::Disconnected),
        }
    } else if pending.len() >= PENDING_BUFFER_CAP {
        Err(EnqueueError::Overflow)
    } else {
        pending.push_back(msg);
        Ok(())
    }
}
