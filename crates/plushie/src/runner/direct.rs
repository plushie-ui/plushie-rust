//! Direct mode runner: in-process rendering via iced.
//!
//! Embeds the plushie renderer directly in the application binary.
//! The user's [`App::view()`] produces a [`View`] which is normalized,
//! rendered through the renderer, and displayed by iced.
//!
//! All iced Messages are delegated to the renderer-lib's
//! [`App::update`](plushie_renderer_lib::App::update), which processes
//! them and emits events through the EventSink. The DirectApp drains
//! those events, converts them to SDK Events via the event bridge,
//! and delivers them to the user's `App::update()`.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use plushie_widget_sdk::iced::{Element, Task, Theme};

use plushie_widget_sdk::message::Message;
use plushie_widget_sdk::protocol::TreeNode;
use plushie_widget_sdk::render_ctx::RenderCtx;
use plushie_widget_sdk::widget::widget_set::iced_widget_set;

use crate::App;
use crate::command::Command;
use crate::event::{EffectEvent, EffectResult, Event, WidgetEvent};
use crate::runtime::subscriptions::{SubOp, SubscriptionManager};
use crate::runtime::view_errors::{ViewErrors, ViewOutcome};
use crate::widget::{EventResult as WidgetEventResult, Interception, WidgetStateStore};

use super::effect_tracker::{self, EffectTracker};
use super::queue_sink::{QueueSink, SinkEvent};

// ---------------------------------------------------------------------------
// DirectApp: wraps the user's App for plushie_widget_sdk::iced::daemon
// ---------------------------------------------------------------------------

/// Internal state for the direct mode iced daemon.
struct DirectApp<A: App> {
    model: A::Model,
    /// Renderer-lib App that handles commands, effects, and state.
    renderer: plushie_renderer_lib::App,
    /// Queue for events emitted by the renderer and SDK-local commands.
    event_queue: Arc<Mutex<Vec<SinkEvent>>>,
    current_tree: Option<TreeNode>,
    /// Window-lifecycle diff tracker. Drives window ops through the
    /// renderer-lib so multi-window apps work in direct mode too.
    ///
    /// The renderer-lib owns the authoritative SDK-id -> iced::window::Id
    /// mapping in `self.renderer.windows`; this tracker only remembers
    /// what was emitted last so we can diff it against the next tree.
    window_sync: crate::runtime::windows::WindowSync,
    widget_store: WidgetStateStore,
    memo_cache: crate::runtime::MemoCache,
    /// Handles for running async tasks, keyed by tag for cancellation.
    running_tasks: HashMap<String, plushie_widget_sdk::iced::task::Handle>,
    /// Subscription lifecycle manager.
    sub_manager: SubscriptionManager,
    /// Active timer subscriptions (tag -> interval). Used by the
    /// iced daemon subscription callback to produce repeating ticks.
    active_timers: HashMap<String, std::time::Duration>,
    /// Tracks in-flight effects for wire ID resolution and timeouts.
    effect_tracker: EffectTracker,
    /// Consecutive-view-panic tracking for the frozen-UI overlay.
    view_errors: ViewErrors,
}

impl<A: App> DirectApp<A> {
    fn init() -> (Self, Task<Message>) {
        let (model, init_cmd) = A::init();

        let builder =
            plushie_widget_sdk::app::PlushieAppBuilder::<plushie_widget_sdk::iced::Renderer>::new()
                .widget_set(&iced_widget_set());
        let registry = builder.build();

        // Create the QueueSink for in-process event collection.
        // Initialized as the global sink (for async effect callbacks)
        // and shared with the App-owned EventEmitter via Arc.
        let (sink, event_queue) = QueueSink::new();
        plushie_renderer_lib::emitters::init_sink(Box::new(sink));
        let sink_arc = plushie_renderer_lib::emitters::sink_arc();

        // Create the renderer-lib App with the SDK's effect handler.
        let effect_handler = Box::new(super::effects::DirectEffectHandler);
        let renderer = plushie_renderer_lib::App::new(registry, effect_handler, sink_arc);

        let mut app = Self {
            model,
            renderer,
            event_queue,
            current_tree: None,
            window_sync: crate::runtime::windows::WindowSync::new(),
            widget_store: WidgetStateStore::new(),
            memo_cache: crate::runtime::MemoCache::new(),
            running_tasks: HashMap::new(),
            sub_manager: SubscriptionManager::new(),
            active_timers: HashMap::new(),
            effect_tracker: EffectTracker::new(),
            view_errors: ViewErrors::default(),
        };

        // Apply user settings to the renderer before the first view.
        apply_settings::<A>(&mut app.renderer);

        app.refresh_view();

        // Initial window sync: if the view declares windows on frame
        // zero, those open requests need to land before the iced
        // daemon resolves its first title/theme callback. Iced's
        // daemon model gives us a default window it opens on startup
        // though, so the Elixir-style pure lifecycle port handles the
        // second-and-subsequent windows cleanly while the first one
        // is pre-created by iced. On the very first sync the tracker
        // is empty, so the ops list includes Open for every window
        // declared in the tree.
        let base_settings = build_direct_base_settings::<A>();
        let win_task = app.sync_windows(&base_settings);

        // Establish initial subscriptions from A::subscribe().
        let mut init_tasks = vec![win_task];
        let initial_subs = A::subscribe(&app.model);
        for op in app.sub_manager.sync(initial_subs) {
            init_tasks.push(app.apply_sub_op(op));
        }

        // Execute the initial command (e.g. focus a field, start
        // async data loading) so apps work from the first frame.
        init_tasks.push(app.execute_command(init_cmd));

        (app, Task::batch(init_tasks))
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        // Handle timer ticks locally (push to event queue, drain below).
        if let Message::TimerTick(tag) = &msg {
            self.handle_timer_tick(tag.clone());
        }

        // Delegate all messages to the renderer. It processes them
        // (transitions, widget ops, event coalescing, rate limiting)
        // and emits events through the QueueSink.
        let renderer_task = self.renderer.update(msg);

        // Drain events emitted by the renderer and deliver to the
        // user's App::update().
        let app_task = self.drain_event_queue().unwrap_or_else(Task::none);

        Task::batch([renderer_task, app_task])
    }

    fn view_window(
        &self,
        _window_id: plushie_widget_sdk::iced::window::Id,
    ) -> Element<'_, Message, Theme, plushie_widget_sdk::iced::Renderer> {
        if let Some(tree) = &self.current_tree {
            let ctx = RenderCtx {
                caches: &self.renderer.core.caches,
                images: &self.renderer.image_registry,
                theme: &self.renderer.theme,
                registry: &self.renderer.registry,
                default_text_size: self.renderer.core.default_text_size,
                default_font: None,
                window_id: "main",
                scale_factor: self.renderer.scale_factor,
            };
            plushie_widget_sdk::widget::render::render(tree, ctx)
        } else {
            plushie_widget_sdk::iced::widget::text("No view").into()
        }
    }

    fn title_for_window(&self, window_id: plushie_widget_sdk::iced::window::Id) -> String {
        self.window_node_for(window_id)
            .and_then(|node| node.props.get_str("title").map(str::to_string))
            .unwrap_or_else(|| "Plushie".to_string())
    }

    fn theme_for_window(&self, window_id: plushie_widget_sdk::iced::window::Id) -> Theme {
        // Per-window theme override lands here when the `theme` prop
        // on the window node resolves to a renderer-recognised theme
        // string. Unrecognised or absent: fall back to the global
        // renderer theme so app-level `Settings::theme` still wins.
        if let Some(node) = self.window_node_for(window_id)
            && let Some(theme_val) = node.props.get_value("theme")
        {
            let resolved = plushie_widget_sdk::theming::resolve_theme(&theme_val);
            return resolved;
        }
        self.renderer.theme.clone()
    }

    fn scale_factor_for_window(&self, window_id: plushie_widget_sdk::iced::window::Id) -> f32 {
        if let Some(node) = self.window_node_for(window_id)
            && let Some(sf) = node
                .props
                .get_value("scale_factor")
                .and_then(|v| v.as_f64())
        {
            return plushie_renderer_lib::app::validate_scale_factor(sf as f32);
        }
        self.renderer.scale_factor
    }

    /// Locate the tree node representing a specific iced window.
    ///
    /// Resolves the iced handle back to the SDK-level window name via
    /// the renderer-lib's authoritative window map, then walks the
    /// current tree for a matching `window` node. Returns `None` when
    /// the mapping or the node is absent (first frame, or window not
    /// yet registered).
    fn window_node_for(
        &self,
        window_id: plushie_widget_sdk::iced::window::Id,
    ) -> Option<&TreeNode> {
        let tree = self.current_tree.as_ref()?;
        let sdk_id = self.renderer.windows.get_window_id(&window_id)?;
        find_window_node(tree, sdk_id)
    }

    /// Drain the event queue, run widget interception, deliver events
    /// to the user's App::update(), then refresh the view once.
    fn drain_event_queue(&mut self) -> Option<Task<Message>> {
        let events: Vec<SinkEvent> = {
            let mut queue = self.event_queue.lock();
            if queue.is_empty() {
                return None;
            }
            std::mem::take(&mut *queue)
        };

        let mut tasks = Vec::new();
        let mut delivered = false;
        for sink_event in events {
            let sdk_event = match sink_event {
                // Effect responses are resolved via the tracker to
                // recover the user's tag and the effect kind for
                // typed result parsing.
                SinkEvent::EffectResponse(response) => self.resolve_effect_response(response),
                other => super::event_bridge::sink_event_to_sdk(other),
            };
            if let Some(event) = sdk_event {
                if let Some(task) = self.deliver_event(event) {
                    tasks.push(task);
                }
                delivered = true;
            }
        }

        // Check for timed-out effects and deliver timeout events.
        let timed_out = self.effect_tracker.check_timeouts();
        for (tag, _kind) in timed_out {
            let event = Event::Effect(EffectEvent {
                tag,
                result: EffectResult::Timeout,
            });
            if let Some(task) = self.deliver_event(event) {
                tasks.push(task);
            }
            delivered = true;
        }

        // Rebuild the view once after all events are processed,
        // not after each individual event.
        if delivered {
            self.refresh_view();

            // Window lifecycle sync runs after refresh_view so
            // added/removed windows land on the iced daemon.
            let base_settings = build_direct_base_settings::<A>();
            let win_task = self.sync_windows(&base_settings);
            tasks.push(win_task);

            // Sync subscriptions after the model has changed.
            let new_subs = A::subscribe(&self.model);
            let ops = self.sub_manager.sync(new_subs);
            for op in ops {
                tasks.push(self.apply_sub_op(op));
            }
        }

        if tasks.is_empty() {
            None
        } else {
            Some(Task::batch(tasks))
        }
    }

    /// Run an SDK event through widget interception and deliver to
    /// the user's App::update(). Returns a Task if a command was
    /// produced. Does NOT refresh the view (the caller batches that).
    fn deliver_event(&mut self, event: Event) -> Option<Task<Message>> {
        // Dev-mode overlay interception: consume `__plushie_dev__/*`
        // events without forwarding them to the app. Compiled out
        // when the `dev` feature is off.
        #[cfg(feature = "dev")]
        {
            if crate::dev::intercept_event(&event) {
                return None;
            }
        }
        match self.widget_store.intercept_event(&event) {
            Some(Interception {
                result: WidgetEventResult::Consumed,
                ..
            }) => None,
            Some(Interception {
                result: WidgetEventResult::Emit { family, value },
                widget_id,
                outer_scope,
                window_id,
            }) => {
                let new_event = Event::Widget(WidgetEvent {
                    event_type: crate::event::family_to_event_type(&family),
                    scoped_id: plushie_core::ScopedId::new(widget_id, outer_scope, Some(window_id)),
                    value,
                });
                // Overlay events can arrive after widget-expand
                // rewrites the scope; re-check the synthesized event
                // to keep the short-circuit reliable.
                #[cfg(feature = "dev")]
                {
                    if crate::dev::intercept_event(&new_event) {
                        return None;
                    }
                }
                let cmd = A::update(&mut self.model, new_event);
                Some(self.execute_command(cmd))
            }
            Some(Interception {
                result: WidgetEventResult::Ignored,
                ..
            })
            | None => {
                let cmd = A::update(&mut self.model, event);
                Some(self.execute_command(cmd))
            }
        }
    }

    /// Resolve an effect response via the tracker. Converts the raw
    /// wire response into a typed SDK event with the user's original
    /// tag and a parsed result.
    fn resolve_effect_response(
        &mut self,
        response: plushie_widget_sdk::protocol::EffectResponse,
    ) -> Option<Event> {
        let wire_id = &response.id;
        match self.effect_tracker.resolve(wire_id) {
            Some((tag, kind)) => {
                let error_as_value = response
                    .error
                    .as_ref()
                    .map(|e| serde_json::Value::String(e.clone()));
                let value = response.result.as_ref().or(error_as_value.as_ref());
                let result = EffectResult::parse(&kind, response.status, value);
                Some(Event::Effect(EffectEvent { tag, result }))
            }
            None => {
                log::warn!(
                    "effect response for unknown wire_id {wire_id}, \
                     falling back to bridge conversion"
                );
                Some(super::event_bridge::effect_response_to_sdk(response))
            }
        }
    }

    /// Drain pending effects with [`EffectResult::Shutdown`].
    ///
    /// Called when `Command::Exit` fires so the app observes a
    /// terminal event per effect rather than a silent drop as iced
    /// tears the daemon down.
    fn drain_effects_as_shutdown(&mut self) {
        let pending = self.effect_tracker.pending_count();
        if pending == 0 {
            return;
        }
        log::info!("direct shutdown: flushing {pending} in-flight effect(s) as Shutdown");
        for (tag, _kind) in self.effect_tracker.flush_all() {
            let event = Event::Effect(crate::event::EffectEvent {
                tag,
                result: crate::event::EffectResult::Shutdown,
            });
            // Fire-and-forget: commands from Shutdown are discarded
            // since iced::exit is about to tear the loop down.
            let _ = A::update(&mut self.model, event);
        }
    }

    /// Diff the current tree against the last-known set of windows
    /// and dispatch any open / close / update ops through the
    /// renderer-lib. Returns a batched iced Task carrying whatever
    /// the renderer handlers produced (open creates a task that
    /// transitions to Message::WindowOpened on completion).
    fn sync_windows(&mut self, base_settings: &serde_json::Value) -> Task<Message> {
        let Some(tree) = self.current_tree.as_ref() else {
            return Task::none();
        };
        let ops = self.window_sync.sync(tree, base_settings);
        let mut tasks = Vec::new();
        for op in ops {
            use crate::runtime::windows::WindowSyncOp;
            let task = match op {
                WindowSyncOp::Open {
                    window_id,
                    settings,
                } => self
                    .renderer
                    .handle_window_op("open", &window_id, &settings),
                WindowSyncOp::Close { window_id } => {
                    self.renderer
                        .handle_window_op("close", &window_id, &serde_json::Value::Null)
                }
                WindowSyncOp::Update {
                    window_id,
                    settings,
                } => self
                    .renderer
                    .handle_window_op("update", &window_id, &settings),
            };
            tasks.push(task);
        }
        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    fn refresh_view(&mut self) {
        // Fall back to the last-good tree when A::view() panics so
        // the UI doesn't disappear while we log the error. The
        // view_errors state owns the consecutive counter and
        // frozen-UI overlay injection.
        let fallback = self.current_tree.clone().unwrap_or_else(placeholder_tree);
        let outcome = crate::runtime::view_errors::run_guarded_view::<A>(
            &mut self.view_errors,
            &self.model,
            &mut self.widget_store,
            &mut self.memo_cache,
            &fallback,
        );
        let mut tree = match outcome {
            ViewOutcome::Ok(tree, warnings) => {
                for warning in &warnings {
                    log::warn!("view normalization: {warning}");
                }
                tree
            }
            ViewOutcome::Panicked { last_good, .. } => last_good,
        };

        self.renderer.registry.prepare_walk(
            &mut tree,
            &mut self.renderer.core.caches,
            &self.renderer.theme,
        );
        self.current_tree = Some(tree);
    }

    fn execute_command(&mut self, cmd: Command) -> Task<Message> {
        match cmd {
            Command::None => Task::none(),
            Command::Exit => {
                // Drain any in-flight effects with EffectResult::Shutdown
                // so the app observes a terminal event per effect
                // instead of a silent drop as iced exits.
                self.drain_effects_as_shutdown();
                plushie_widget_sdk::iced::exit()
            }
            Command::Batch(cmds) => {
                let tasks: Vec<Task<Message>> =
                    cmds.into_iter().map(|c| self.execute_command(c)).collect();
                Task::batch(tasks)
            }
            Command::Renderer(plushie_core::ops::RendererOp::Effect {
                tag,
                request,
                timeout,
            }) => {
                let kind = request.kind();
                let effective_timeout =
                    timeout.unwrap_or_else(|| effect_tracker::default_timeout(kind));
                let (wire_id, replaced) =
                    self.effect_tracker
                        .track_with_replacement(&tag, kind, effective_timeout);
                // One-per-tag replacement: surface a synthetic
                // Cancelled event for the displaced effect so the app
                // can observe the transition instead of silently
                // losing a response.
                if let Some((prior_tag, _prior_kind)) = replaced {
                    self.event_queue
                        .lock()
                        .push(SinkEvent::DelayedEvent(Event::Effect(EffectEvent {
                            tag: prior_tag,
                            result: EffectResult::Cancelled,
                        })));
                }
                // Pass the wire_id as the tag to the renderer. The
                // renderer echoes it back in the EffectResponse.id
                // field, which we resolve via the tracker.
                self.renderer
                    .execute(plushie_core::ops::RendererOp::Effect {
                        tag: wire_id,
                        request,
                        timeout: None,
                    })
            }
            Command::Renderer(op) => self.renderer.execute(op),
            Command::Async { tag, task } => {
                let queue = self.event_queue.clone();
                let tag_clone = tag.clone();
                let tag_for_guard = tag.clone();
                let future = (task)();
                // Guard against user-future panics. Without this, a
                // panic would unwind iced's executor worker, drop the
                // result channel, and leave the app waiting forever.
                let guarded =
                    async move { super::run_task_with_panic_guard(&tag_for_guard, future).await };
                let (task, handle) = Task::perform(guarded, move |result| {
                    queue.lock().push(SinkEvent::AsyncResult {
                        tag: tag_clone,
                        result,
                    });
                    Message::NoOp
                })
                .abortable();
                self.running_tasks.insert(tag, handle);
                task
            }
            Command::Stream { tag, task } => {
                // Stream tasks attach a sink that pushes StreamValue
                // SinkEvents to the queue as the task emits them. The
                // final future result pushes an AsyncResult.
                let queue = self.event_queue.clone();
                let emitter = crate::command::StreamEmitter::buffered(&tag);
                let sink_queue = queue.clone();
                let sink_tag = tag.clone();
                emitter.attach_sink(Box::new(move |t, value| {
                    sink_queue
                        .lock()
                        .push(SinkEvent::StreamValue { tag: t, value });
                    // Nudge iced so the queue drains on next update.
                    // (The renderer batches this naturally via its
                    // existing event loop.)
                    let _ = sink_tag;
                }));
                let final_tag = tag.clone();
                let tag_for_guard = tag.clone();
                let future = (task)(emitter);
                let guarded =
                    async move { super::run_task_with_panic_guard(&tag_for_guard, future).await };
                let (task, handle) = Task::perform(guarded, move |result| {
                    queue.lock().push(SinkEvent::AsyncResult {
                        tag: final_tag,
                        result,
                    });
                    Message::NoOp
                })
                .abortable();
                self.running_tasks.insert(tag, handle);
                task
            }
            Command::Cancel { tag } => {
                if let Some(handle) = self.running_tasks.remove(&tag) {
                    handle.abort();
                }
                Task::none()
            }
            Command::SendAfter { delay, event } => {
                let queue = self.event_queue.clone();
                Task::perform(
                    async move {
                        // Cooperative sleep so iced's executor can drive
                        // other tasks in parallel. `std::thread::sleep`
                        // would block the executor thread for `delay`,
                        // serialising concurrent SendAfter commands and
                        // starving the thread pool.
                        super::platform_sleep(delay).await;
                    },
                    move |_| {
                        queue.lock().push(SinkEvent::DelayedEvent(*event));
                        Message::NoOp
                    },
                )
            }
        }
    }

    /// Iced daemon subscription callback. Combines SDK-side timer
    /// subscriptions (`Subscription::every`) with the renderer's own
    /// subscription set (keyboard, pointer, animation, theme, ...)
    /// so both app-declared subs and widget-scoped subs fire.
    ///
    /// Each timer tick pushes a `TimerEvent` onto the event queue
    /// and is drained on the next `update()` cycle. Renderer-source
    /// messages flow into `renderer.update`, which dispatches events
    /// to both host-level subscribers and widget-scoped subscribers.
    fn subscriptions(&self) -> plushie_widget_sdk::iced::Subscription<Message> {
        let mut subs: Vec<plushie_widget_sdk::iced::Subscription<Message>> = self
            .active_timers
            .iter()
            .map(|(tag, duration)| {
                plushie_widget_sdk::iced::time::every(*duration)
                    .with(tag.clone()) // Unique identity per tag
                    .map(|(tag, _instant)| Message::TimerTick(tag))
            })
            .collect();
        subs.push(self.renderer.renderer_subscriptions());
        plushie_widget_sdk::iced::Subscription::batch(subs)
    }

    /// Handle a timer tick by pushing a TimerEvent to the event queue.
    fn handle_timer_tick(&self, tag: String) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.event_queue
            .lock()
            .push(SinkEvent::DelayedEvent(Event::Timer(
                crate::event::TimerEvent { tag, timestamp },
            )));
    }

    /// Apply a subscription operation (subscribe, unsubscribe, or timer).
    fn apply_sub_op(&mut self, op: SubOp) -> Task<Message> {
        match op {
            SubOp::Subscribe {
                kind,
                tag,
                max_rate,
                window_id,
            } => self
                .renderer
                .execute(plushie_core::ops::RendererOp::Subscribe {
                    kind,
                    tag,
                    max_rate,
                    window_id,
                }),
            SubOp::Unsubscribe { kind, tag } => self
                .renderer
                .execute(plushie_core::ops::RendererOp::Unsubscribe { kind, tag }),
            SubOp::StartTimer { tag, interval } => {
                // Store the interval. The iced daemon's subscription
                // callback reads active_timers and returns iced::time::every
                // subscriptions. Iced manages the timer lifecycle.
                self.active_timers.insert(tag, interval);
                Task::none()
            }
            SubOp::StopTimer { tag } => {
                self.active_timers.remove(&tag);
                Task::none()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Apply the user's `A::settings()` to the renderer.
///
/// Converts the SDK's `Settings` struct to the wire-format JSON that
/// `Core::apply(IncomingMessage::Settings)` expects. Also applies
/// settings that Core doesn't handle (theme, scale factor).
fn apply_settings<A: App>(renderer: &mut plushie_renderer_lib::App) {
    let settings = A::settings();

    // Apply settings directly to renderer fields (no JSON round-trip).
    renderer.core.default_font = settings.default_font.map(|family| {
        if family == "monospace" {
            plushie_widget_sdk::iced::Font::MONOSPACE
        } else {
            plushie_widget_sdk::iced::Font::DEFAULT
        }
    });
    renderer.core.default_text_size = settings.default_text_size;
    renderer.core.default_event_rate = settings.default_event_rate;
    renderer
        .emitter
        .set_default_rate(settings.default_event_rate);

    if let Some(sf) = settings.scale_factor {
        renderer.scale_factor = plushie_renderer_lib::app::validate_scale_factor(sf);
    }

    // Widget config: initialize native widgets if config is provided.
    if !settings.widget_config.is_empty() {
        let config =
            serde_json::to_value(&settings.widget_config).unwrap_or(serde_json::Value::Null);
        let ctx = plushie_widget_sdk::registry::InitCtx {
            config: &config,
            theme: &renderer.theme,
            default_text_size: renderer.core.default_text_size,
            default_font: renderer.core.default_font,
        };
        renderer.registry.init_all(&ctx);
    }

    // Theme (not handled by Core for initial settings).
    if let Some(theme) = settings.theme {
        use plushie_core::types::{PlushieType, Theme};
        match &theme {
            Theme::System => {
                renderer.theme_follows_system = true;
            }
            _ => {
                let wire_val = serde_json::Value::from(theme.wire_encode());
                renderer.theme = plushie_widget_sdk::theming::resolve_theme(&wire_val);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the app in direct mode.
///
/// # Errors
///
/// Returns [`crate::Error::Iced`] when the iced daemon fails to
/// start (event-loop init, window-system failure, etc.), or
/// [`crate::Error::Startup`] when settings validation rejects
/// the configuration.
pub fn run<A: App>() -> crate::Result {
    // Build iced daemon settings from the user's A::settings().
    // These are startup-only values that can't change after launch.
    let settings = A::settings();
    let mut iced_settings = plushie_widget_sdk::iced::Settings::default();
    if let Some(aa) = settings.antialiasing {
        iced_settings.antialiasing = aa;
    }
    if let Some(vsync) = settings.vsync {
        iced_settings.vsync = vsync;
    }
    if let Some(size) = settings.default_text_size {
        iced_settings.default_text_size = plushie_widget_sdk::iced::Pixels(size);
    }

    plushie_widget_sdk::iced::daemon(
        DirectApp::<A>::init,
        DirectApp::<A>::update,
        DirectApp::<A>::view_window,
    )
    .settings(iced_settings)
    .subscription(DirectApp::<A>::subscriptions)
    .title(DirectApp::<A>::title_for_window)
    .theme(DirectApp::<A>::theme_for_window)
    .scale_factor(DirectApp::<A>::scale_factor_for_window)
    .run()
    .map_err(|e| crate::Error::Iced(e.to_string()))
}

/// Locate a `window` node in the tree by its SDK window ID.
fn find_window_node<'a>(tree: &'a TreeNode, window_id: &str) -> Option<&'a TreeNode> {
    if tree.type_name == "window" && tree.id == window_id {
        return Some(tree);
    }
    for child in &tree.children {
        if let Some(n) = find_window_node(child, window_id) {
            return Some(n);
        }
    }
    None
}

/// Build the base window-settings object handed to `WindowSync` in
/// direct mode. Mirrors the per-window props that `apply_settings`
/// applied globally so a tree node without its own overrides
/// inherits the host's defaults.
fn build_direct_base_settings<A: App>() -> serde_json::Value {
    let settings = A::settings();
    let mut obj = serde_json::Map::new();
    if let Some(sf) = settings.scale_factor {
        obj.insert("scale_factor".into(), serde_json::json!(sf));
    }
    if let Some(theme) = settings.theme {
        use plushie_core::types::PlushieType;
        obj.insert("theme".into(), serde_json::Value::from(theme.wire_encode()));
    }
    serde_json::Value::Object(obj)
}

/// Minimal empty tree used as a first-frame fallback if A::view()
/// panics before any successful render. Gives the renderer
/// something valid to draw rather than leaving `current_tree` as
/// `None`.
fn placeholder_tree() -> TreeNode {
    TreeNode {
        id: String::new(),
        type_name: "container".to_string(),
        props: plushie_widget_sdk::protocol::Props::from(
            plushie_widget_sdk::protocol::PropMap::new(),
        ),
        children: vec![],
    }
}
