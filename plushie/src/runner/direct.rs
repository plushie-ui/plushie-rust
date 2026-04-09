//! Direct mode runner: in-process rendering via iced.
//!
//! Embeds the plushie renderer directly in the application binary.
//! The user's [`App::view()`] produces a [`View`] which is normalized,
//! rendered through the [`WidgetRegistry`], and displayed by iced.
//! Widget interactions produce iced [`Message`]s which are converted
//! to SDK [`Event`]s and delivered to [`App::update()`].
//!
//! No subprocess, no serialization, no wire protocol.

use std::collections::HashMap;

use plushie_widget_sdk::iced::{Element, Task, Theme};

use plushie_widget_sdk::image_registry::ImageRegistry;
use plushie_widget_sdk::message::Message;
use plushie_widget_sdk::protocol::TreeNode;
use plushie_widget_sdk::registry::WidgetRegistry;
use plushie_widget_sdk::render_ctx::RenderCtx;
use plushie_widget_sdk::shared_state::SharedState;
use plushie_widget_sdk::widget::widget_set::iced_widget_set;

use crate::App;
use crate::command::Command;
use crate::event::{self, Event, EventType, WidgetEvent};
use crate::runtime::normalize;

// ---------------------------------------------------------------------------
// DirectApp: wraps the user's App for plushie_widget_sdk::iced::daemon
// ---------------------------------------------------------------------------

/// Internal state for the direct mode iced daemon.
struct DirectApp<A: App> {
    model: A::Model,
    registry: WidgetRegistry,
    shared_state: SharedState,
    image_registry: ImageRegistry,
    theme: Theme,
    current_tree: Option<TreeNode>,
    window_iced_ids: HashMap<String, plushie_widget_sdk::iced::window::Id>,
}

impl<A: App> DirectApp<A> {
    fn init() -> (Self, Task<Message>) {
        let (model, _cmd) = A::init();

        let builder = plushie_widget_sdk::app::PlushieAppBuilder::<plushie_widget_sdk::iced::Renderer>::new()
            .widget_set(&iced_widget_set());
        let registry = builder.build();

        let mut app = Self {
            model,
            registry,
            shared_state: SharedState::new(),
            image_registry: ImageRegistry::new(),
            theme: Theme::Dark,
            current_tree: None,
            window_iced_ids: HashMap::new(),
        };

        app.refresh_view();

        // TODO: convert initial Command to iced Task
        (app, Task::none())
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        // Convert iced Message to SDK Event.
        if let Some(event) = message_to_event(&msg) {
            let cmd = A::update(&mut self.model, event);
            self.refresh_view();
            return self.execute_command(cmd);
        }

        // Messages that don't produce SDK events (subscriptions,
        // internal renderer events) are handled here.
        match &msg {
            Message::StatusChanged(..) => {}
            Message::MarkdownUrl(..) => {}
            Message::NoOp => {}
            _ => {
                // For messages not yet handled, log at debug level.
                log::debug!("unhandled message in direct runner: {msg:?}");
            }
        }

        Task::none()
    }

    fn view_window(&self, _window_id: plushie_widget_sdk::iced::window::Id) -> Element<Message, Theme, plushie_widget_sdk::iced::Renderer> {
        if let Some(tree) = &self.current_tree {
            let ctx = RenderCtx {
                caches: &self.shared_state,
                images: &self.image_registry,
                theme: &self.theme,
                registry: &self.registry,
                default_text_size: None,
                default_font: None,
                window_id: "main",
                scale_factor: 1.0,
            };
            plushie_widget_sdk::widget::render::render(tree, ctx)
        } else {
            plushie_widget_sdk::iced::widget::text("No view").into()
        }
    }

    fn title_for_window(&self, _window_id: plushie_widget_sdk::iced::window::Id) -> String {
        // Extract title from the window node's props if available.
        if let Some(tree) = &self.current_tree {
            if tree.type_name == "window" {
                if let Some(title) = tree.props.get("title").and_then(|v| v.as_str()) {
                    return title.to_string();
                }
            }
            // Look for window in children.
            for child in &tree.children {
                if child.type_name == "window" {
                    if let Some(title) = child.props.get("title").and_then(|v| v.as_str()) {
                        return title.to_string();
                    }
                }
            }
        }
        "Plushie".to_string()
    }

    fn theme_for_window(&self, _window_id: plushie_widget_sdk::iced::window::Id) -> Theme {
        self.theme.clone()
    }

    fn scale_factor_for_window(&self, _window_id: plushie_widget_sdk::iced::window::Id) -> f32 {
        1.0
    }

    fn refresh_view(&mut self) {
        let view = A::view(&self.model);
        let (normalized, warnings) = normalize::normalize(&view.0);
        for warning in &warnings {
            log::warn!("view normalization: {warning}");
        }

        match serde_json::from_value::<TreeNode>(normalized) {
            Ok(tree) => {
                self.registry
                    .prepare_walk(&tree, &mut self.shared_state, &self.theme);
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
            Command::Focus(id) => {
                plushie_widget_sdk::iced::widget::operation::focus::<Message>(
                    plushie_widget_sdk::iced::widget::Id::from(id),
                )
            }
            Command::FocusNext => plushie_widget_sdk::iced::widget::operation::focus_next(),
            Command::FocusPrevious => plushie_widget_sdk::iced::widget::operation::focus_previous(),
            Command::Window(op) => self.execute_window_op(op),
            _ => {
                log::debug!("unhandled command in direct runner: {cmd:?}");
                Task::none()
            }
        }
    }

    fn execute_window_op(&mut self, op: crate::command::WindowOp) -> Task<Message> {
        use crate::command::WindowOp;
        match op {
            WindowOp::Close(_id) => {
                plushie_widget_sdk::iced::window::oldest().and_then(plushie_widget_sdk::iced::window::close)
            }
            _ => {
                log::debug!("unhandled window op in direct runner: {op:?}");
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
            id: id.clone(),
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

        // Canvas events
        Message::CanvasEvent { window_id, id, .. } => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Press,
            id: local_id(id),
            window_id: window_id.clone(),
            scope: extract_scope(id),
            value: serde_json::Value::Null,
        })),

        Message::CanvasElementClick {
            window_id,
            canvas_id,
            element_id,
            ..
        } => Some(Event::Widget(WidgetEvent {
            event_type: EventType::Click,
            id: element_id.clone(),
            window_id: window_id.clone(),
            scope: extract_scope_from_canvas(canvas_id, element_id),
            value: serde_json::Value::Null,
        })),

        // Messages that don't produce SDK events.
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
    extract_scope(canvas_id)
}

/// Convert an event family string to an EventType.
fn family_to_event_type(family: &str) -> EventType {
    match family {
        "click" => EventType::Click,
        "double_click" => EventType::DoubleClick,
        "input" => EventType::Input,
        "submit" => EventType::Submit,
        "toggle" => EventType::Toggle,
        "select" => EventType::Select,
        "slide" => EventType::Slide,
        "slide_release" => EventType::SlideRelease,
        "paste" => EventType::Paste,
        "press" => EventType::Press,
        "release" => EventType::Release,
        "move" => EventType::Move,
        "scroll" => EventType::Scroll,
        "scrolled" => EventType::Scrolled,
        "enter" => EventType::Enter,
        "exit" => EventType::Exit,
        "resize" => EventType::Resize,
        "focused" => EventType::Focused,
        "blurred" => EventType::Blurred,
        "drag" => EventType::Drag,
        "drag_end" => EventType::DragEnd,
        "sort" => EventType::Sort,
        "status" => EventType::Status,
        "transition_complete" => EventType::TransitionComplete,
        _ => EventType::Other(0),
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
