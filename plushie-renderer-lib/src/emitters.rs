//! Output emitters for the wire protocol.
//!
//! All renderer output (events, handshake, effect responses, query
//! responses, screenshots) flows through this module. Each emitter
//! encodes via the global [`Codec`] and writes to the output writer.
//!
//! The output writer is initialized at startup via [`init_output`].
//! Native mode uses a ChannelWriter backed by a background thread
//! (provided by the binary crate). WASM mode uses a JS callback wrapper.

use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};

use iced::Task;

use plushie_widget_sdk::codec::Codec;
use plushie_widget_sdk::message::Message;
use plushie_widget_sdk::protocol::OutgoingEvent;

// ---------------------------------------------------------------------------
// configurable output writer
// ---------------------------------------------------------------------------

static OUTPUT_WRITER: OnceLock<Mutex<Box<dyn Write + Send>>> = OnceLock::new();

/// Initialize the global output writer.
///
/// Must be called exactly once before any `emit_*` functions. Panics
/// if called twice. On native, pass a `ChannelWriter` for non-blocking
/// I/O. On WASM, pass a `WebOutputWriter` wrapping a JS callback.
pub fn init_output(writer: Box<dyn Write + Send>) {
    if OUTPUT_WRITER.set(Mutex::new(writer)).is_err() {
        panic!("output writer already initialized");
    }
}

/// Write bytes to the protocol output channel.
///
/// Each call acquires the writer lock and flushes. Falls back to
/// direct stdout if the global writer has not been initialized yet
/// (only possible during very early startup errors on native).
pub fn write_output(bytes: &[u8]) -> io::Result<()> {
    if let Some(writer) = OUTPUT_WRITER.get() {
        let mut guard = writer.lock().unwrap_or_else(|e| e.into_inner());
        guard.write_all(bytes)?;
        guard.flush()
    } else {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(bytes)?;
            handle.flush()
        }
        #[cfg(target_arch = "wasm32")]
        {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "output writer not initialized",
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// event emitters
// ---------------------------------------------------------------------------

/// Emit an event and return `Task::none()`, or log the error and return
/// `iced::exit()` if the write fails.
pub fn emit_or_exit(event: OutgoingEvent) -> Task<Message> {
    if let Err(e) = emit_event(event) {
        log::error!("write error: {e}");
        return iced::exit();
    }
    Task::none()
}

/// Encode and write an [`OutgoingEvent`] to the output writer.
pub fn emit_event(event: OutgoingEvent) -> io::Result<()> {
    let codec = Codec::get_global();
    let bytes = codec.encode(&event).map_err(io::Error::other)?;
    write_output(&bytes)
}

// ---------------------------------------------------------------------------
// hello message emitter
// ---------------------------------------------------------------------------

/// Emit a `hello` handshake message immediately after codec negotiation.
///
/// The `widget_sets` field reports which named widget sets are registered.
/// The "iced" set is always present (provides the 36 built-in widget types).
/// Custom widgets are reported separately in `native_widgets`.
pub fn emit_hello(
    mode: &str,
    backend: &str,
    native_widgets: &[&str],
    widget_set_names: &[&str],
    transport: &str,
) -> io::Result<()> {
    let builtin = plushie_widget_sdk::widget::widget_set::IcedWidgetSet::type_names();
    let all_widgets: Vec<&str> = builtin
        .iter()
        .copied()
        .chain(native_widgets.iter().copied())
        .collect();

    let msg = serde_json::json!({
        "type": "hello",
        "session": "",
        "protocol": plushie_widget_sdk::protocol::PROTOCOL_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "name": "plushie-renderer",
        "mode": mode,
        "backend": backend,
        "transport": transport,
        "native_widgets": native_widgets,
        "widget_sets": widget_set_names,
        "widgets": all_widgets,
    });
    let codec = Codec::get_global();
    let bytes = codec.encode(&msg).map_err(io::Error::other)?;
    write_output(&bytes)
}

// ---------------------------------------------------------------------------
// effect response emitter
// ---------------------------------------------------------------------------

/// Encode and write an [`EffectResponse`](plushie_widget_sdk::protocol::EffectResponse).
pub fn emit_effect_response(
    response: plushie_widget_sdk::protocol::EffectResponse,
) -> io::Result<()> {
    let codec = Codec::get_global();
    let bytes = codec.encode(&response).map_err(io::Error::other)?;
    write_output(&bytes)
}

/// Emit a query_response message.
pub fn emit_query_response(kind: &str, tag: &str, data: serde_json::Value) -> io::Result<()> {
    let msg = serde_json::json!({
        "type": "op_query_response",
        "session": "",
        "kind": kind,
        "tag": tag,
        "data": data,
    });
    let codec = Codec::get_global();
    let bytes = codec.encode(&msg).map_err(io::Error::other)?;
    write_output(&bytes)
}

// ---------------------------------------------------------------------------
// screenshot response emitter
// ---------------------------------------------------------------------------

/// Emit a screenshot_response. Uses `Codec::encode_binary_message`
/// so that RGBA pixel data is encoded as native msgpack binary.
pub fn emit_screenshot_response(
    id: &str,
    name: &str,
    hash: &str,
    width: u32,
    height: u32,
    rgba_bytes: &[u8],
) -> io::Result<()> {
    use serde_json::json;

    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), json!("screenshot_response"));
    map.insert("session".to_string(), json!(""));
    map.insert("id".to_string(), json!(id));
    map.insert("name".to_string(), json!(name));
    map.insert("hash".to_string(), json!(hash));
    map.insert("width".to_string(), json!(width));
    map.insert("height".to_string(), json!(height));

    let binary = if rgba_bytes.is_empty() {
        None
    } else {
        Some(("rgba", rgba_bytes))
    };
    let codec = Codec::get_global();
    let bytes = codec
        .encode_binary_message(map, binary)
        .map_err(io::Error::other)?;
    write_output(&bytes)
}

// ---------------------------------------------------------------------------
// Message -> OutgoingEvent mapping
// ---------------------------------------------------------------------------

/// Convert a widget [`Message`] to an [`OutgoingEvent`], if applicable.
///
/// Delegates to [`Message::to_outgoing_event`].
pub fn message_to_event(msg: &Message) -> Option<OutgoingEvent> {
    msg.to_outgoing_event()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_to_event_click() {
        let msg = Message::Click("main".into(), "btn1".into());
        let event = message_to_event(&msg).unwrap();
        assert_eq!(event.family, "click");
        assert_eq!(event.id, "btn1");
        assert_eq!(event.window_id.as_deref(), Some("main"));
    }

    #[test]
    fn message_to_event_input() {
        let msg = Message::Input("main".into(), "field1".into(), "hello".into());
        let event = message_to_event(&msg).unwrap();
        assert_eq!(event.family, "input");
        assert_eq!(event.id, "field1");
        assert_eq!(event.window_id.as_deref(), Some("main"));
    }

    #[test]
    fn message_to_event_submit() {
        let msg = Message::Submit("main".into(), "form1".into(), "data".into());
        let event = message_to_event(&msg).unwrap();
        assert_eq!(event.family, "submit");
    }

    #[test]
    fn message_to_event_toggle() {
        let msg = Message::Toggle("main".into(), "cb1".into(), true);
        let event = message_to_event(&msg).unwrap();
        assert_eq!(event.family, "toggle");
    }

    #[test]
    fn message_to_event_select() {
        let msg = Message::Select("main".into(), "pick1".into(), "option_a".into());
        let event = message_to_event(&msg).unwrap();
        assert_eq!(event.family, "select");
    }

    #[test]
    fn message_to_event_slide_returns_none() {
        let msg = Message::Slide("main".into(), "sl1".into(), 0.5);
        assert!(message_to_event(&msg).is_none());
    }

    #[test]
    fn message_to_event_slide_release_returns_none() {
        let msg = Message::SlideRelease("main".into(), "sl1".into());
        assert!(message_to_event(&msg).is_none());
    }

    #[test]
    fn message_to_event_noop_returns_none() {
        let msg = Message::NoOp;
        assert!(message_to_event(&msg).is_none());
    }

    #[test]
    fn message_to_event_mouse_area_events() {
        for kind in &[
            "right_press",
            "right_release",
            "middle_press",
            "middle_release",
            "double_click",
            "enter",
            "exit",
        ] {
            let msg =
                Message::MouseAreaEvent("main".into(), "ma1".into(), kind.to_string(), 10.0, 20.0);
            assert!(
                message_to_event(&msg).is_some(),
                "mouse area event `{kind}` should map"
            );
        }
        let msg = Message::MouseAreaEvent("main".into(), "ma1".into(), "unknown".into(), 0.0, 0.0);
        assert!(message_to_event(&msg).is_none());
    }

    #[test]
    fn message_to_event_sensor_resize() {
        let msg = Message::SensorResize("main".into(), "s1".into(), 100.0, 200.0);
        let event = message_to_event(&msg).unwrap();
        assert_eq!(event.family, "resize");
    }

    #[test]
    fn message_to_event_paste() {
        let msg = Message::Paste("main".into(), "f1".into(), "pasted text".into());
        let event = message_to_event(&msg).unwrap();
        assert_eq!(event.family, "paste");
    }

    #[test]
    fn message_to_event_option_hovered() {
        let msg = Message::OptionHovered("main".into(), "pick1".into(), "opt_a".into());
        let event = message_to_event(&msg).unwrap();
        assert_eq!(event.family, "option_hovered");
    }

    #[test]
    fn message_to_event_canvas_events() {
        let mods = plushie_widget_sdk::protocol::KeyModifiers::default();
        for kind in &["press", "release", "move"] {
            let extra = match *kind {
                "press" | "release" => "left:mouse".to_string(),
                _ => "mouse".to_string(),
            };
            let msg = Message::CanvasEvent {
                window_id: "main".into(),
                id: "c1".into(),
                kind: kind.to_string(),
                x: 10.0,
                y: 20.0,
                extra,
                modifiers: mods.clone(),
            };
            assert!(
                message_to_event(&msg).is_some(),
                "canvas event `{kind}` should map"
            );
        }
        let msg = Message::CanvasEvent {
            window_id: "main".into(),
            id: "c1".into(),
            kind: "unknown".into(),
            x: 0.0,
            y: 0.0,
            extra: String::new(),
            modifiers: mods.clone(),
        };
        assert!(message_to_event(&msg).is_none());
    }

    #[test]
    fn message_to_event_canvas_scroll() {
        let msg = Message::CanvasScroll {
            window_id: "main".into(),
            id: "c1".into(),
            x: 10.0,
            y: 20.0,
            delta_x: 1.0,
            delta_y: -1.0,
            pointer_type: "mouse".into(),
            modifiers: plushie_widget_sdk::protocol::KeyModifiers::default(),
        };
        let event = message_to_event(&msg).unwrap();
        assert_eq!(event.family, "scroll");
    }

    #[test]
    fn message_to_event_custom_event_returns_none() {
        let msg = Message::Event {
            window_id: "main".into(),
            id: "node1".into(),
            data: serde_json::json!({"key": "value"}),
            family: "custom_family".into(),
        };
        assert!(message_to_event(&msg).is_none());
    }
}
