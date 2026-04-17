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
use std::sync::{Arc, Mutex};

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
#[allow(dead_code)] // window_iced_ids reserved for multi-window support
struct DirectApp<A: App> {
    model: A::Model,
    /// Renderer-lib App that handles commands, effects, and state.
    renderer: plushie_renderer_lib::App,
    /// Queue for events emitted by the renderer and SDK-local commands.
    event_queue: Arc<Mutex<Vec<SinkEvent>>>,
    current_tree: Option<TreeNode>,
    window_iced_ids: HashMap<String, plushie_widget_sdk::iced::window::Id>,
    widget_store: WidgetStateStore,
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
            window_iced_ids: HashMap::new(),
            widget_store: WidgetStateStore::new(),
            running_tasks: HashMap::new(),
            sub_manager: SubscriptionManager::new(),
            active_timers: HashMap::new(),
            effect_tracker: EffectTracker::new(),
            view_errors: ViewErrors::default(),
        };

        // Apply user settings to the renderer before the first view.
        apply_settings::<A>(&mut app.renderer);

        app.refresh_view();

        // Establish initial subscriptions from A::subscribe().
        let mut init_tasks = Vec::new();
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

    fn title_for_window(&self, _window_id: plushie_widget_sdk::iced::window::Id) -> String {
        if let Some(tree) = &self.current_tree {
            if tree.type_name == "window"
                && let Some(title) = tree.props.get("title").and_then(|v| v.as_str())
            {
                return title.to_string();
            }
            for child in &tree.children {
                if child.type_name == "window"
                    && let Some(title) = child.props.get("title").and_then(|v| v.as_str())
                {
                    return title.to_string();
                }
            }
        }
        "Plushie".to_string()
    }

    fn theme_for_window(&self, _window_id: plushie_widget_sdk::iced::window::Id) -> Theme {
        self.renderer.theme.clone()
    }

    fn scale_factor_for_window(&self, _window_id: plushie_widget_sdk::iced::window::Id) -> f32 {
        self.renderer.scale_factor
    }

    /// Drain the event queue, run widget interception, deliver events
    /// to the user's App::update(), then refresh the view once.
    fn drain_event_queue(&mut self) -> Option<Task<Message>> {
        let events: Vec<SinkEvent> = {
            let mut queue = self.event_queue.lock().unwrap_or_else(|e| {
                log::error!("event_queue lock poisoned in drain_event_queue");
                e.into_inner()
            });
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
            &fallback,
        );
        let tree = match outcome {
            ViewOutcome::Ok(tree, warnings) => {
                for warning in &warnings {
                    log::warn!("view normalization: {warning}");
                }
                tree
            }
            ViewOutcome::Panicked { last_good, .. } => last_good,
        };

        self.renderer.registry.prepare_walk(
            &tree,
            &mut self.renderer.core.caches,
            &self.renderer.theme,
        );
        self.current_tree = Some(tree);
    }

    fn execute_command(&mut self, cmd: Command) -> Task<Message> {
        match cmd {
            Command::None => Task::none(),
            Command::Exit => plushie_widget_sdk::iced::exit(),
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
                let wire_id = self.effect_tracker.track(&tag, kind, effective_timeout);
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
                    queue
                        .lock()
                        .unwrap_or_else(|e| {
                            log::error!("event_queue lock poisoned in Async completion");
                            e.into_inner()
                        })
                        .push(SinkEvent::AsyncResult {
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
                        .unwrap_or_else(|e| {
                            log::error!("event_queue lock poisoned in Stream sink");
                            e.into_inner()
                        })
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
                    queue
                        .lock()
                        .unwrap_or_else(|e| {
                            log::error!("event_queue lock poisoned in Stream completion");
                            e.into_inner()
                        })
                        .push(SinkEvent::AsyncResult {
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
                        queue
                            .lock()
                            .unwrap_or_else(|e| {
                                log::error!("event_queue lock poisoned in SendAfter");
                                e.into_inner()
                            })
                            .push(SinkEvent::DelayedEvent(*event));
                        Message::NoOp
                    },
                )
            }
        }
    }

    /// Iced daemon subscription callback. Returns active timer
    /// subscriptions as `iced::time::every` subscriptions. Each tick
    /// pushes a `TimerEvent` to the event queue, which is drained
    /// on the next `update()` cycle.
    fn subscriptions(&self) -> plushie_widget_sdk::iced::Subscription<Message> {
        let subs: Vec<plushie_widget_sdk::iced::Subscription<Message>> = self
            .active_timers
            .iter()
            .map(|(tag, duration)| {
                plushie_widget_sdk::iced::time::every(*duration)
                    .with(tag.clone()) // Unique identity per tag
                    .map(|(tag, _instant)| Message::TimerTick(tag))
            })
            .collect();
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
            .unwrap_or_else(|e| {
                log::error!("event_queue lock poisoned in handle_timer_tick");
                e.into_inner()
            })
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

/// Minimal empty tree used as a first-frame fallback if A::view()
/// panics before any successful render. Gives the renderer
/// something valid to draw rather than leaving `current_tree` as
/// `None`.
fn placeholder_tree() -> TreeNode {
    TreeNode {
        id: String::new(),
        type_name: "container".to_string(),
        props: plushie_widget_sdk::protocol::Props::Typed(
            plushie_widget_sdk::protocol::PropMap::new(),
        ),
        children: vec![],
    }
}
