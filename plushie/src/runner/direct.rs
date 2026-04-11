//! Direct mode runner: in-process rendering via iced.
//!
//! Embeds the plushie renderer directly in the application binary.
//! The user's [`App::view()`] produces a [`View`] which is normalized,
//! rendered through the renderer, and displayed by iced.
//!
//! Commands are executed through the renderer-lib's
//! [`App::execute`](plushie_renderer_lib::App::execute) to ensure
//! identical behavior between direct and wire modes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use plushie_widget_sdk::iced::{Element, Task, Theme};

use plushie_widget_sdk::message::Message;
use plushie_widget_sdk::protocol::TreeNode;
use plushie_widget_sdk::render_ctx::RenderCtx;
use plushie_widget_sdk::widget::widget_set::iced_widget_set;

use crate::App;
use crate::command::Command;
use crate::event::{Event, EventType, WidgetEvent};
use crate::runtime::normalize;
use crate::widget::{EventResult, WidgetStateStore};

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
        let (model, _cmd) = A::init();

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

        (app, Task::none())
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        // Convert iced Message to SDK Event.
        if let Some(event) = message_to_event(&msg) {
            // Let composite widgets intercept first.
            let intercepted = self.widget_store.intercept_event(&event);
            match intercepted {
                Some(EventResult::Consumed) | Some(EventResult::UpdateState) => {
                    self.refresh_view();
                    return Task::none();
                }
                Some(EventResult::Emit { family, value }) => {
                    // Widget transformed the event.
                    let widget_id = event.as_widget()
                        .and_then(|w| w.scope.first().cloned())
                        .unwrap_or_default();
                    let new_event = Event::Widget(WidgetEvent {
                        event_type: crate::event::family_to_event_type(&family),
                        id: widget_id,
                        window_id: "main".to_string(),
                        scope: vec![],
                        value,
                    });
                    let cmd = A::update(&mut self.model, new_event);
                    self.refresh_view();
                    return self.execute_command(cmd);
                }
                Some(EventResult::Ignored) | None => {
                    // Widget didn't intercept. Deliver to app as-is.
                    let cmd = A::update(&mut self.model, event);
                    self.refresh_view();
                    return self.execute_command(cmd);
                }
            }
        }

        // Drain any events emitted by the renderer (effect responses,
        // query responses) during command execution or async completion.
        if let Some(task) = self.drain_event_queue() {
            return task;
        }

        // Messages that don't produce SDK events (subscriptions,
        // internal renderer events) are handled here.
        match msg {
            Message::StatusChanged(..) => {}
            Message::MarkdownUrl(..) => {}
            Message::NoOp => {}
            _ => {
                log::debug!("unhandled message in direct runner: {msg:?}");
            }
        }

        Task::none()
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

    /// Drain the event queue and deliver any pending events to the
    /// user's App::update(). Returns Some(Task) if events were processed.
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
            let sdk_event = match sink_event {
                SinkEvent::EffectResponse(resp) => {
                    let result = match resp.status {
                        "ok" => crate::event::EffectResult::Ok(
                            resp.result.unwrap_or(serde_json::Value::Null),
                        ),
                        "cancelled" => crate::event::EffectResult::Cancelled,
                        _ => crate::event::EffectResult::Error(
                            resp.result.unwrap_or(serde_json::Value::Null),
                        ),
                    };
                    Some(Event::Effect(crate::event::EffectEvent {
                        tag: resp.id.clone(),
                        result,
                    }))
                }
                SinkEvent::Event(_) | SinkEvent::QueryResponse { .. } => {
                    // Widget/subscription events and query responses are
                    // handled through the iced Message path, not the queue.
                    None
                }
            };

            if let Some(event) = sdk_event {
                let cmd = A::update(&mut self.model, event);
                self.refresh_view();
                tasks.push(self.execute_command(cmd));
            }
        }

        if tasks.is_empty() {
            None
        } else {
            Some(Task::batch(tasks))
        }
    }

    fn refresh_view(&mut self) {
        let view = A::view(&self.model);
        let expanded = self.widget_store.expand_widgets(&serde_json::to_value(&view).unwrap());
        let (normalized, warnings) = normalize::normalize(&expanded);
        for warning in &warnings {
            log::warn!("view normalization: {warning}");
        }

        match serde_json::from_value::<TreeNode>(normalized) {
            Ok(tree) => {
                self.renderer.registry
                    .prepare_walk(&tree, &mut self.renderer.core.caches, &self.renderer.theme);
                self.current_tree = Some(tree);
            }
            Err(e) => {
                log::error!("failed to convert View to TreeNode: {e}");
            }
        }
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
// Message -> Event conversion
// ---------------------------------------------------------------------------

/// Convert an iced Message to an SDK Event.
///
/// Returns None for messages that don't produce user-facing events
/// (internal renderer state, subscriptions not yet wired, etc.).
fn message_to_event(msg: &Message) -> Option<Event> {
    match msg {
        Message::Click(window_id, id) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Click,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Null,
        })),

        Message::Input(window_id, id, text) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Input,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::String(text.clone()),
        })),

        Message::Submit(window_id, id, text) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Submit,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::String(text.clone()),
        })),

        Message::Toggle(window_id, id, checked) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Toggle,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Bool(*checked),
        })),

        Message::Select(window_id, id, value) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Select,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::String(value.clone()),
        })),

        Message::Slide(window_id, id, value) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Slide,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::json!(*value),
        })),

        Message::SlideRelease(window_id, id) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::SlideRelease,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Null,
        })),

        Message::Paste(window_id, id, text) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Paste,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::String(text.clone()),
        })),

        Message::Event {
            window_id,
            id,
            data,
            family,
        } => Some(Event::Widget(WidgetEvent {
            event_type: family_to_event_type(family),
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: data.clone(),
        })),

        Message::OptionHovered(window_id, id, option) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::OptionHovered,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::String(option.clone()),
        })),

        Message::SensorResize(window_id, id, w, h) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Resize,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::json!({"width": w, "height": h}),
        })),

        Message::ScrollEvent(window_id, id, viewport) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Scrolled,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::json!({
                "absolute_x": viewport.absolute_x,
                "absolute_y": viewport.absolute_y,
                "relative_x": viewport.relative_x,
                "relative_y": viewport.relative_y,
            }),
        })),

        // Mouse area events
        Message::MouseAreaEvent(window_id, id, kind, x, y) => Some(Event::Widget(WidgetEvent {
            event_type: mouse_area_kind_to_event_type(kind),
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::json!({"x": x, "y": y}),
        })),

        Message::MouseAreaMove(window_id, id, x, y) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Move,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::json!({"x": x, "y": y}),
        })),

        Message::MouseAreaScroll(window_id, id, delta_x, delta_y, _x, _y) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Scroll,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::json!({"delta_x": delta_x, "delta_y": delta_y}),
        })),

        // Canvas element events
        Message::CanvasEvent { window_id, id, .. } => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Press,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Null,
        })),

        Message::CanvasElementClick { window_id, canvas_id, element_id, .. } =>
            Some(canvas_element_event(EventType::Click, window_id, canvas_id, element_id)),

        Message::CanvasElementEnter { window_id, canvas_id, element_id, .. } =>
            Some(canvas_element_event(EventType::Enter, window_id, canvas_id, element_id)),

        Message::CanvasElementLeave { window_id, canvas_id, element_id, .. } =>
            Some(canvas_element_event(EventType::Exit, window_id, canvas_id, element_id)),

        Message::CanvasElementDrag { window_id, canvas_id, element_id, .. } =>
            Some(canvas_element_event(EventType::Drag, window_id, canvas_id, element_id)),

        Message::CanvasElementDragEnd { window_id, canvas_id, element_id, .. } =>
            Some(canvas_element_event(EventType::DragEnd, window_id, canvas_id, element_id)),

        Message::CanvasElementFocused { window_id, canvas_id, element_id, .. } =>
            Some(canvas_element_event(EventType::Focused, window_id, canvas_id, element_id)),

        Message::CanvasElementBlurred { window_id, canvas_id, element_id, .. } =>
            Some(canvas_element_event(EventType::Blurred, window_id, canvas_id, element_id)),

        Message::CanvasElementKeyPress { window_id, canvas_id, element_id, .. } =>
            Some(canvas_element_event(EventType::KeyPress, window_id, canvas_id, element_id)),

        Message::CanvasElementKeyRelease { window_id, canvas_id, element_id, .. } =>
            Some(canvas_element_event(EventType::KeyRelease, window_id, canvas_id, element_id)),

        Message::CanvasFocused { window_id, canvas_id, .. } => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Focused,
            id: local_id(canvas_id),
            window_id: window_id.clone(),
            scope: extract_scope(canvas_id),
            value: serde_json::Value::Null,
        })),

        Message::CanvasBlurred { window_id, canvas_id, .. } => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Blurred,
            id: local_id(canvas_id),
            window_id: window_id.clone(),
            scope: extract_scope(canvas_id),
            value: serde_json::Value::Null,
        })),

        Message::CanvasScroll { window_id, id, .. } => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Scroll,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Null,
        })),

        // Pane grid events
        Message::PaneFocusCycle(window_id, id, _) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::PaneFocusCycle,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Null,
        })),

        Message::PaneResized(window_id, id, _) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::PaneResized,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Null,
        })),

        Message::PaneDragged(window_id, id, _) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::PaneDragged,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Null,
        })),

        Message::PaneClicked(window_id, id, _) => Some(Event::Widget(WidgetEvent {
            event_type: EventType::PaneClicked,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Null,
        })),

        // Messages that don't produce SDK events (internal state).
        _ => None,
    }
}

/// Extract the local ID from a scoped ID (e.g. "form/save" -> "save").
fn local_id(scoped: &str) -> String {
    scoped
        .rsplit_once('/')
        .map(|(_, local)| local.to_string())
        .unwrap_or_else(|| scoped.to_string())
}

/// Extract the reversed scope chain from a scoped ID.
/// "form/section/field" -> ["section", "form"]
fn extract_scope(scoped: &str) -> Vec<String> {
    let parts: Vec<&str> = scoped.split('/').collect();
    if parts.len() <= 1 {
        vec![]
    } else {
        parts[..parts.len() - 1]
            .iter()
            .rev()
            .map(|s| s.to_string())
            .collect()
    }
}

/// Extract scope for canvas element events.
fn extract_scope_from_canvas(canvas_id: &str, _element_id: &str) -> Vec<String> {
    let mut scope = extract_scope(canvas_id);
    let canvas_local = local_id(canvas_id);
    if !canvas_local.is_empty() {
        scope.insert(0, canvas_local);
    }
    scope
}

/// Create a canvas element event.
fn canvas_element_event(
    event_type: EventType,
    window_id: &str,
    canvas_id: &str,
    element_id: &str,
) -> Event {
    Event::Widget(WidgetEvent {
        event_type,
        id: element_id.to_string(),
        window_id: window_id.to_string(),
        scope: extract_scope_from_canvas(canvas_id, element_id),
        value: serde_json::Value::Null,
    })
}

/// Convert MouseAreaEvent kind string to EventType.
fn mouse_area_kind_to_event_type(kind: &str) -> EventType {
    match kind {
        "press" => EventType::Press,
        "release" => EventType::Release,
        "middle_press" => EventType::Press,
        "middle_release" => EventType::Release,
        "right_press" => EventType::Press,
        "right_release" => EventType::Release,
        "enter" => EventType::Enter,
        "exit" => EventType::Exit,
        "double_click" => EventType::DoubleClick,
        _ => EventType::Other(0),
    }
}

/// Convert an event family string to an EventType.
fn family_to_event_type(family: &str) -> EventType {
    crate::event::family_to_event_type(family)
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
