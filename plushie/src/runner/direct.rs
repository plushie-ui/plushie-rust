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
use crate::event::{Event, WidgetEvent};
use crate::runtime;
use crate::widget::{EventResult, Interception, WidgetStateStore};

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
    /// Queue for events emitted by the renderer during command execution.
    event_queue: Arc<Mutex<Vec<SinkEvent>>>,
    current_tree: Option<TreeNode>,
    window_iced_ids: HashMap<String, plushie_widget_sdk::iced::window::Id>,
    widget_store: WidgetStateStore,
}

impl<A: App> DirectApp<A> {
    fn init() -> (Self, Task<Message>) {
        let (model, init_cmd) = A::init();

        let builder = plushie_widget_sdk::app::PlushieAppBuilder::<plushie_widget_sdk::iced::Renderer>::new()
            .widget_set(&iced_widget_set());
        let registry = builder.build();

        // Create the QueueSink for in-process event collection and
        // initialize the global event sink so the renderer-lib emitter
        // routes through it.
        let (sink, event_queue) = QueueSink::new();
        plushie_renderer_lib::emitters::init_sink(Box::new(sink));

        // Create the renderer-lib App with the SDK's effect handler.
        let effect_handler = Box::new(super::effects::DirectEffectHandler);
        let renderer = plushie_renderer_lib::App::new(registry, effect_handler);

        let mut app = Self {
            model,
            renderer,
            event_queue,
            current_tree: None,
            window_iced_ids: HashMap::new(),
            widget_store: WidgetStateStore::new(),
        };

        app.refresh_view();

        // Execute the initial command (e.g. focus a field, start
        // async data loading) so apps work from the first frame.
        let init_task = app.execute_command(init_cmd);

        (app, init_task)
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        // Delegate all messages to the renderer. It processes them
        // (transitions, widget ops, event coalescing, rate limiting)
        // and emits events through the QueueSink.
        let renderer_task = self.renderer.update(msg);

        // Drain events emitted by the renderer and deliver to the
        // user's App::update().
        let app_task = self.drain_event_queue().unwrap_or_else(Task::none);

        Task::batch([renderer_task, app_task])
    }

    fn view_window(&self, _window_id: plushie_widget_sdk::iced::window::Id) -> Element<'_, Message, Theme, plushie_widget_sdk::iced::Renderer> {
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
        1.0
    }

    /// Drain the event queue, run widget interception, and deliver
    /// events to the user's App::update().
    fn drain_event_queue(&mut self) -> Option<Task<Message>> {
        let events: Vec<SinkEvent> = {
            let mut queue = self.event_queue.lock().unwrap();
            if queue.is_empty() {
                return None;
            }
            std::mem::take(&mut *queue)
        };

        let mut tasks = Vec::new();
        for sink_event in events {
            if let Some(sdk_event) = super::event_bridge::sink_event_to_sdk(sink_event) {
                if let Some(task) = self.deliver_event(sdk_event) {
                    tasks.push(task);
                }
            }
        }

        if tasks.is_empty() {
            None
        } else {
            Some(Task::batch(tasks))
        }
    }

    /// Run an SDK event through widget interception and deliver to
    /// the user's App::update(). Returns a Task if a command was produced.
    fn deliver_event(&mut self, event: Event) -> Option<Task<Message>> {
        match self.widget_store.intercept_event(&event) {
            Some(Interception { result: EventResult::Consumed, .. })
            | Some(Interception { result: EventResult::UpdateState, .. }) => {
                self.refresh_view();
                None
            }
            Some(Interception {
                result: EventResult::Emit { family, value },
                widget_id,
                outer_scope,
                window_id,
            }) => {
                let new_event = Event::Widget(WidgetEvent {
                    event_type: crate::event::family_to_event_type(&family),
                    id: widget_id,
                    window_id,
                    scope: outer_scope,
                    value,
                });
                let cmd = A::update(&mut self.model, new_event);
                self.refresh_view();
                Some(self.execute_command(cmd))
            }
            Some(Interception { result: EventResult::Ignored, .. }) | None => {
                let cmd = A::update(&mut self.model, event);
                self.refresh_view();
                Some(self.execute_command(cmd))
            }
        }
    }

    fn refresh_view(&mut self) {
        let (tree, warnings) = runtime::prepare_tree::<A>(&self.model, &mut self.widget_store);
        for warning in &warnings {
            log::warn!("view normalization: {warning}");
        }

        self.renderer.registry
            .prepare_walk(&tree, &mut self.renderer.core.caches, &self.renderer.theme);
        self.current_tree = Some(tree);
    }

    fn execute_command(&mut self, cmd: Command) -> Task<Message> {
        match cmd {
            Command::None => Task::none(),
            Command::Exit => plushie_widget_sdk::iced::exit(),
            Command::Batch(cmds) => {
                let tasks: Vec<Task<Message>> = cmds
                    .into_iter()
                    .map(|c| self.execute_command(c))
                    .collect();
                Task::batch(tasks)
            }
            Command::Renderer(op) => self.renderer.execute(op),
            _ => {
                log::debug!("unhandled command in direct runner: {cmd:?}");
                Task::none()
            }
        }
    }

}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the app in direct mode.
pub fn run<A: App>() -> crate::Result {
    plushie_widget_sdk::iced::daemon(
        DirectApp::<A>::init,
        DirectApp::<A>::update,
        DirectApp::<A>::view_window,
    )
    .title(DirectApp::<A>::title_for_window)
    .theme(DirectApp::<A>::theme_for_window)
    .scale_factor(DirectApp::<A>::scale_factor_for_window)
    .run()
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}
