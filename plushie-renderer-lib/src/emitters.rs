//! Output emitters for the renderer.
//!
//! All renderer output (events, effect responses, query responses,
//! screenshots) flows through the [`EventSink`] trait. The global
//! sink is initialized at startup via [`init_sink`].
//!
//! Wire mode uses a `StdoutSink` that encodes via [`Codec`] and
//! writes framed bytes. Direct mode uses a `QueueSink` that collects
//! events in-process for the SDK to drain.

use std::io;
use std::sync::{Arc, Mutex, OnceLock};

use plushie_widget_sdk::protocol::OutgoingEvent;

// ---------------------------------------------------------------------------
// EventSink trait
// ---------------------------------------------------------------------------

/// Pluggable output for renderer events.
///
/// Wire mode: encodes to bytes and writes to stdout.
/// Direct mode: queues events for the SDK to read in-process.
pub trait EventSink: Send {
    /// Emit a widget/subscription event.
    fn emit_event(&mut self, event: OutgoingEvent) -> io::Result<()>;

    /// Emit an effect response.
    fn emit_effect_response(
        &mut self, response: plushie_widget_sdk::protocol::EffectResponse,
    ) -> io::Result<()>;

    /// Emit a query response (tree hash, find focused, system info).
    fn emit_query_response(
        &mut self, kind: &str, tag: &str, data: &serde_json::Value,
    ) -> io::Result<()>;

    /// Emit a screenshot response with binary RGBA data.
    fn emit_screenshot_response(
        &mut self,
        id: &str,
        name: &str,
        hash: &str,
        width: u32,
        height: u32,
        rgba_bytes: &[u8],
    ) -> io::Result<()>;

    /// Emit the hello handshake message.
    fn emit_hello(
        &mut self,
        mode: &str,
        backend: &str,
        native_widgets: &[&str],
        widget_set_names: &[&str],
        transport: &str,
    ) -> io::Result<()>;

    /// Write pre-encoded bytes (for stub acks and scripting).
    fn write_raw(&mut self, bytes: &[u8]) -> io::Result<()>;
}

// ---------------------------------------------------------------------------
// Global sink
// ---------------------------------------------------------------------------

static EVENT_SINK: OnceLock<Arc<Mutex<Box<dyn EventSink>>>> = OnceLock::new();

/// Initialize the global event sink.
///
/// Must be called exactly once before any `emit_*` functions.
/// Wire mode: pass a `WriterSink`. Direct mode: use `init_sink_arc`.
pub fn init_sink(sink: Box<dyn EventSink>) {
    init_sink_arc(Arc::new(Mutex::new(sink)));
}

/// Initialize the global event sink from a shared Arc.
///
/// Used by the direct runner to share the same sink between the
/// global (for async callbacks) and the App-owned EventEmitter.
pub fn init_sink_arc(sink: Arc<Mutex<Box<dyn EventSink>>>) {
    if EVENT_SINK.set(sink).is_err() {
        panic!("event sink already initialized");
    }
}

/// Get a clone of the global sink Arc.
///
/// Returns the shared sink for passing to the App constructor.
/// Panics if the sink has not been initialized.
pub fn sink_arc() -> Arc<Mutex<Box<dyn EventSink>>> {
    EVENT_SINK.get().expect("event sink not initialized").clone()
}

fn with_sink<R>(f: impl FnOnce(&mut dyn EventSink) -> io::Result<R>) -> io::Result<R> {
    let sink = EVENT_SINK.get().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotConnected, "event sink not initialized")
    })?;
    let mut guard = sink.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut **guard)
}

// ---------------------------------------------------------------------------
// Legacy compatibility: init_output / write_output
// ---------------------------------------------------------------------------

/// Initialize the global output writer (legacy API).
///
/// Wraps the writer in a `WriterSink` that encodes via the global
/// Codec. Prefer [`init_sink`] with a typed sink for new code.
pub fn init_output(writer: Box<dyn std::io::Write + Send>) {
    init_sink(Box::new(WriterSink { writer }));
}

/// Write pre-encoded bytes through the global sink.
pub fn write_output(bytes: &[u8]) -> io::Result<()> {
    with_sink(|sink| sink.write_raw(bytes))
}

/// A sink that wraps a raw writer and encodes via the global Codec.
/// Used by [`init_output`] for backwards compatibility.
struct WriterSink {
    writer: Box<dyn std::io::Write + Send>,
}

impl EventSink for WriterSink {
    fn emit_event(&mut self, event: OutgoingEvent) -> io::Result<()> {
        let codec = plushie_widget_sdk::codec::Codec::get_global();
        let bytes = codec.encode(&event).map_err(io::Error::other)?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()
    }

    fn emit_effect_response(
        &mut self, response: plushie_widget_sdk::protocol::EffectResponse,
    ) -> io::Result<()> {
        let codec = plushie_widget_sdk::codec::Codec::get_global();
        let bytes = codec.encode(&response).map_err(io::Error::other)?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()
    }

    fn emit_query_response(
        &mut self, kind: &str, tag: &str, data: &serde_json::Value,
    ) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "op_query_response",
            "session": "",
            "kind": kind,
            "tag": tag,
            "data": data,
        });
        let codec = plushie_widget_sdk::codec::Codec::get_global();
        let bytes = codec.encode(&msg).map_err(io::Error::other)?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()
    }

    fn emit_screenshot_response(
        &mut self,
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
        let codec = plushie_widget_sdk::codec::Codec::get_global();
        let bytes = codec
            .encode_binary_message(map, binary)
            .map_err(io::Error::other)?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()
    }

    fn emit_hello(
        &mut self,
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
        let codec = plushie_widget_sdk::codec::Codec::get_global();
        let bytes = codec.encode(&msg).map_err(io::Error::other)?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()
    }

    fn write_raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }
}

// ---------------------------------------------------------------------------
// hello message emitter
// ---------------------------------------------------------------------------

/// Emit a `hello` handshake message through the global sink.
pub fn emit_hello(
    mode: &str,
    backend: &str,
    native_widgets: &[&str],
    widget_set_names: &[&str],
    transport: &str,
) -> io::Result<()> {
    with_sink(|sink| sink.emit_hello(mode, backend, native_widgets, widget_set_names, transport))
}

// ---------------------------------------------------------------------------
// effect response emitter
// ---------------------------------------------------------------------------

/// Emit an [`EffectResponse`](plushie_widget_sdk::protocol::EffectResponse) through the global sink.
pub fn emit_effect_response(
    response: plushie_widget_sdk::protocol::EffectResponse,
) -> io::Result<()> {
    with_sink(|sink| sink.emit_effect_response(response))
}

/// Emit a query_response message through the global sink.
pub fn emit_query_response(kind: &str, tag: &str, data: serde_json::Value) -> io::Result<()> {
    with_sink(|sink| sink.emit_query_response(kind, tag, &data))
}

// ---------------------------------------------------------------------------
// screenshot response emitter
// ---------------------------------------------------------------------------

/// Emit a screenshot_response through the global sink.
pub fn emit_screenshot_response(
    id: &str,
    name: &str,
    hash: &str,
    width: u32,
    height: u32,
    rgba_bytes: &[u8],
) -> io::Result<()> {
    with_sink(|sink| sink.emit_screenshot_response(id, name, hash, width, height, rgba_bytes))
}

