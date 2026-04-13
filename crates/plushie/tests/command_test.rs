//! Tests for Command construction and matching.

use std::time::Duration;

use plushie::command::*;
use plushie::event::{Event, TimerEvent};

#[test]
fn command_none_is_none() {
    assert!(matches!(Command::none(), Command::None));
}

#[test]
fn command_batch_collects_commands() {
    let cmd = Command::batch([Command::focus("email"), Command::focus_next()]);
    match cmd {
        Command::Batch(cmds) => assert_eq!(cmds.len(), 2),
        _ => panic!("expected Batch"),
    }
}

#[test]
fn command_focus_carries_id() {
    match Command::focus("email") {
        Command::Renderer(RendererOp::Command { id, family, .. }) => {
            assert_eq!(id, "email");
            assert_eq!(family, "focus");
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

#[test]
fn command_close_window_produces_window_op() {
    match Command::close_window("main") {
        Command::Renderer(RendererOp::Window(WindowOp::Close(id))) => assert_eq!(id, "main"),
        _ => panic!("expected Renderer(Window(Close))"),
    }
}

#[test]
fn command_send_after_carries_delay_and_event() {
    let event = Event::Timer(TimerEvent {
        tag: "tick".into(),
        timestamp: 0,
    });
    let cmd = Command::send_after(Duration::from_millis(500), event);
    match cmd {
        Command::SendAfter { delay, .. } => {
            assert_eq!(delay, Duration::from_millis(500));
        }
        _ => panic!("expected SendAfter"),
    }
}

#[test]
fn command_scroll_to_carries_coordinates() {
    match Command::scroll_to("list", 0.0, 100.0) {
        Command::Renderer(RendererOp::Command { id, family, value }) => {
            assert_eq!(id, "list");
            assert_eq!(family, "scroll_to");
            assert_eq!(value["x"], 0.0);
            assert_eq!(value["y"], 100.0);
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

#[test]
fn command_clipboard_read() {
    match Command::clipboard_read("paste") {
        Command::Renderer(RendererOp::Effect {
            tag,
            request: EffectRequest::ClipboardRead,
            ..
        }) => {
            assert_eq!(tag, "paste");
        }
        _ => panic!("expected Renderer(Effect(ClipboardRead))"),
    }
}

#[test]
fn command_send_carries_payload() {
    let cmd = Command::send("gauge-1", "set_value", serde_json::json!({"value": 42}));
    match cmd {
        Command::Renderer(RendererOp::Command { id, family, value }) => {
            assert_eq!(id, "gauge-1");
            assert_eq!(family, "set_value");
            assert_eq!(value["value"], 42);
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

#[test]
fn command_builder_creates_command() {
    let cmd = Command::send("gauge-1", "set_value", serde_json::json!(42));
    match cmd {
        Command::Renderer(RendererOp::Command { id, family, value }) => {
            assert_eq!(id, "gauge-1");
            assert_eq!(family, "set_value");
            assert_eq!(value, 42);
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

#[test]
fn command_widget_typed_builder() {
    use plushie_core::WidgetCommand;

    #[derive(WidgetCommand)]
    enum TestCmd {
        SetValue(f32),
        Reset,
    }

    let cmd = Command::widget("gauge-1", TestCmd::SetValue(72.0));
    match cmd {
        Command::Renderer(RendererOp::Command { id, family, value }) => {
            assert_eq!(id, "gauge-1");
            assert_eq!(family, "set_value");
            assert_eq!(value.as_f64(), Some(72.0));
        }
        _ => panic!("expected Renderer(Command)"),
    }

    let cmd = Command::widget("gauge-1", TestCmd::Reset);
    match cmd {
        Command::Renderer(RendererOp::Command { id, family, value }) => {
            assert_eq!(id, "gauge-1");
            assert_eq!(family, "reset");
            assert!(value.is_null());
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

#[test]
fn commands_are_inspectable_for_testing() {
    let cmd = Command::focus("email");
    assert!(
        matches!(cmd, Command::Renderer(RendererOp::Command { ref id, ref family, .. }) if id == "email" && family == "focus")
    );
}
