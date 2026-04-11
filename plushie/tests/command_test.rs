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
    let cmd = Command::batch([
        Command::focus("email"),
        Command::focus_next(),
    ]);
    match cmd {
        Command::Batch(cmds) => assert_eq!(cmds.len(), 2),
        _ => panic!("expected Batch"),
    }
}

#[test]
fn command_focus_carries_id() {
    match Command::focus("email") {
        Command::Renderer(RendererOp::Focus(id)) => assert_eq!(id, "email"),
        _ => panic!("expected Renderer(Focus)"),
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
    let event = Event::Timer(TimerEvent { tag: "tick".into(), timestamp: 0 });
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
        Command::Renderer(RendererOp::ScrollTo { target, x, y }) => {
            assert_eq!(target, "list");
            assert_eq!(x, 0.0);
            assert_eq!(y, 100.0);
        }
        _ => panic!("expected Renderer(ScrollTo)"),
    }
}

#[test]
fn command_clipboard_read() {
    match Command::clipboard_read("paste") {
        Command::Renderer(RendererOp::Effect { tag, request: EffectRequest::ClipboardRead }) => {
            assert_eq!(tag, "paste");
        }
        _ => panic!("expected Renderer(Effect(ClipboardRead))"),
    }
}

#[test]
fn command_widget_command_carries_payload() {
    let cmd = Command::widget_command("gauge-1", "set_value", serde_json::json!({"value": 42}));
    match cmd {
        Command::Renderer(RendererOp::WidgetCommand { node_id, op, payload }) => {
            assert_eq!(node_id, "gauge-1");
            assert_eq!(op, "set_value");
            assert_eq!(payload["value"], 42);
        }
        _ => panic!("expected Renderer(WidgetCommand)"),
    }
}

#[test]
fn commands_are_inspectable_for_testing() {
    let cmd = Command::focus("email");
    assert!(matches!(cmd, Command::Renderer(RendererOp::Focus(ref id)) if id == "email"));
}
