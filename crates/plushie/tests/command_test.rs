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

// ---------------------------------------------------------------------------
// Text cursor
// ---------------------------------------------------------------------------

#[test]
fn command_select_all() {
    match Command::select_all("editor") {
        Command::Renderer(RendererOp::Command { id, family, .. }) => {
            assert_eq!(id, "editor");
            assert_eq!(family, "select_all");
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

#[test]
fn command_move_cursor_to() {
    match Command::move_cursor_to("input", 5) {
        Command::Renderer(RendererOp::Command { id, family, value }) => {
            assert_eq!(id, "input");
            assert_eq!(family, "move_cursor_to");
            assert_eq!(value["position"], 5);
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

#[test]
fn command_select_range() {
    match Command::select_range("input", 2, 8) {
        Command::Renderer(RendererOp::Command { id, family, value }) => {
            assert_eq!(id, "input");
            assert_eq!(family, "select_range");
            assert_eq!(value["start"], 2);
            assert_eq!(value["end"], 8);
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

// ---------------------------------------------------------------------------
// Scroll
// ---------------------------------------------------------------------------

#[test]
fn command_scroll_by() {
    match Command::scroll_by("list", 0.0, 50.0) {
        Command::Renderer(RendererOp::Command { id, family, value }) => {
            assert_eq!(id, "list");
            assert_eq!(family, "scroll_by");
            assert_eq!(value["y"], 50.0);
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

#[test]
fn command_snap_to_end() {
    match Command::snap_to_end("list") {
        Command::Renderer(RendererOp::Command { id, family, .. }) => {
            assert_eq!(id, "list");
            assert_eq!(family, "snap_to_end");
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

// ---------------------------------------------------------------------------
// Window ops
// ---------------------------------------------------------------------------

#[test]
fn command_maximize_window() {
    match Command::maximize_window("main", true) {
        Command::Renderer(RendererOp::Window(WindowOp::Maximize {
            window_id,
            maximized,
        })) => {
            assert_eq!(window_id, "main");
            assert!(maximized);
        }
        _ => panic!("expected Window(Maximize)"),
    }
}

#[test]
fn command_toggle_maximize() {
    match Command::toggle_maximize("main") {
        Command::Renderer(RendererOp::Window(WindowOp::ToggleMaximize(id))) => {
            assert_eq!(id, "main");
        }
        _ => panic!("expected Window(ToggleMaximize)"),
    }
}

#[test]
fn command_set_window_mode() {
    match Command::set_window_mode("main", WindowMode::Fullscreen) {
        Command::Renderer(RendererOp::Window(WindowOp::SetMode { window_id, mode })) => {
            assert_eq!(window_id, "main");
            assert_eq!(mode, WindowMode::Fullscreen);
        }
        _ => panic!("expected Window(SetMode)"),
    }
}

#[test]
fn command_screenshot() {
    match Command::screenshot("main", "snap") {
        Command::Renderer(RendererOp::Window(WindowOp::Screenshot { window_id, tag })) => {
            assert_eq!(window_id, "main");
            assert_eq!(tag, "snap");
        }
        _ => panic!("expected Window(Screenshot)"),
    }
}

#[test]
fn command_set_min_size() {
    match Command::set_min_size("main", 400.0, 300.0) {
        Command::Renderer(RendererOp::Window(WindowOp::SetMinSize {
            window_id,
            width,
            height,
        })) => {
            assert_eq!(window_id, "main");
            assert_eq!(width, 400.0);
            assert_eq!(height, 300.0);
        }
        _ => panic!("expected Window(SetMinSize)"),
    }
}

// ---------------------------------------------------------------------------
// Window queries
// ---------------------------------------------------------------------------

#[test]
fn command_get_window_size() {
    match Command::get_window_size("main", "size_check") {
        Command::Renderer(RendererOp::WindowQuery(WindowQuery::GetSize { window_id, tag })) => {
            assert_eq!(window_id, "main");
            assert_eq!(tag, "size_check");
        }
        _ => panic!("expected WindowQuery(GetSize)"),
    }
}

#[test]
fn command_get_scale_factor() {
    match Command::get_scale_factor("main", "dpi") {
        Command::Renderer(RendererOp::WindowQuery(WindowQuery::GetScaleFactor {
            window_id,
            tag,
        })) => {
            assert_eq!(window_id, "main");
            assert_eq!(tag, "dpi");
        }
        _ => panic!("expected WindowQuery(GetScaleFactor)"),
    }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

#[test]
fn command_get_system_theme() {
    match Command::get_system_theme("theme_check") {
        Command::Renderer(RendererOp::SystemQuery(SystemQuery::GetTheme { tag })) => {
            assert_eq!(tag, "theme_check");
        }
        _ => panic!("expected SystemQuery(GetTheme)"),
    }
}

#[test]
fn command_get_system_info() {
    match Command::get_system_info("info") {
        Command::Renderer(RendererOp::SystemQuery(SystemQuery::GetInfo { tag })) => {
            assert_eq!(tag, "info");
        }
        _ => panic!("expected SystemQuery(GetInfo)"),
    }
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

#[test]
fn command_create_image() {
    match Command::create_image("logo", vec![0x89, 0x50, 0x4e, 0x47]) {
        Command::Renderer(RendererOp::Image(ImageOp::Create { handle, data })) => {
            assert_eq!(handle, "logo");
            assert_eq!(data.len(), 4);
        }
        _ => panic!("expected Image(Create)"),
    }
}

#[test]
fn command_delete_image() {
    match Command::delete_image("logo") {
        Command::Renderer(RendererOp::Image(ImageOp::Delete(handle))) => {
            assert_eq!(handle, "logo");
        }
        _ => panic!("expected Image(Delete)"),
    }
}

#[test]
fn command_clear_images() {
    assert!(matches!(
        Command::clear_images(),
        Command::Renderer(RendererOp::Image(ImageOp::Clear))
    ));
}

// ---------------------------------------------------------------------------
// Pane grid
// ---------------------------------------------------------------------------

#[test]
fn command_pane_split() {
    match Command::pane_split("grid", "p1", "horizontal", "p2") {
        Command::Renderer(RendererOp::Command { id, family, value }) => {
            assert_eq!(id, "grid");
            assert_eq!(family, "pane_split");
            assert_eq!(value["pane"], "p1");
            assert_eq!(value["axis"], "horizontal");
            assert_eq!(value["new_pane"], "p2");
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

#[test]
fn command_pane_restore() {
    match Command::pane_restore("grid") {
        Command::Renderer(RendererOp::Command { id, family, .. }) => {
            assert_eq!(id, "grid");
            assert_eq!(family, "pane_restore");
        }
        _ => panic!("expected Renderer(Command)"),
    }
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

#[test]
fn command_announce() {
    match Command::announce("Item saved") {
        Command::Renderer(RendererOp::Announce(text)) => assert_eq!(text, "Item saved"),
        _ => panic!("expected Announce"),
    }
}

#[test]
fn command_tree_hash() {
    match Command::tree_hash("check") {
        Command::Renderer(RendererOp::TreeHash { tag }) => assert_eq!(tag, "check"),
        _ => panic!("expected TreeHash"),
    }
}

#[test]
fn command_find_focused() {
    match Command::find_focused("focus_check") {
        Command::Renderer(RendererOp::FindFocused { tag }) => assert_eq!(tag, "focus_check"),
        _ => panic!("expected FindFocused"),
    }
}

#[test]
fn command_advance_frame() {
    match Command::advance_frame(16000) {
        Command::Renderer(RendererOp::AdvanceFrame { timestamp }) => {
            assert_eq!(timestamp, 16000);
        }
        _ => panic!("expected AdvanceFrame"),
    }
}

// ---------------------------------------------------------------------------
// Inspection
// ---------------------------------------------------------------------------

#[test]
fn commands_are_inspectable_for_testing() {
    let cmd = Command::focus("email");
    assert!(
        matches!(cmd, Command::Renderer(RendererOp::Command { ref id, ref family, .. }) if id == "email" && family == "focus")
    );
}
