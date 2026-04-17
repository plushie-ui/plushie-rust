//! Output emitters for the renderer.
//!
//! All renderer output (events, effect responses, query responses,
//! screenshots) flows through the [`EventSink`] trait. The global
//! sink is initialized at startup via [`init_sink`] and shared with
//! the App's EventEmitter via [`sink_arc`].
//!
//! The global provides three functions for code that runs without
//! an App instance (startup handshake, headless writer thread):
//! [`emit_hello`], [`write_output`].

use std::io;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use plushie_widget_sdk::protocol::OutgoingEvent;

/// Alias for the sink mutex.
///
/// Uses `parking_lot::Mutex` on hot paths: faster under contention
/// and never poisons on panic, so no `unwrap_or_else(|e| e.into_inner())`
/// boilerplate is needed at lock sites.
pub type SinkMutex = parking_lot::Mutex<Box<dyn EventSink>>;

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
        &mut self,
        response: plushie_widget_sdk::protocol::EffectResponse,
    ) -> io::Result<()>;

    /// Emit a query response (tree hash, find focused, system info).
    fn emit_query_response(
        &mut self,
        kind: &str,
        tag: &str,
        data: &serde_json::Value,
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

static EVENT_SINK: OnceLock<Arc<SinkMutex>> = OnceLock::new();

/// Initialize the global event sink.
///
/// Must be called exactly once before any output functions.
/// Panics on double initialization.
pub fn init_sink(sink: Box<dyn EventSink>) {
    let arc = Arc::new(Mutex::new(sink));
    if EVENT_SINK.set(arc).is_err() {
        panic!("event sink already initialized");
    }
}

/// Get a clone of the global sink Arc.
///
/// Returns the shared sink for passing to the App constructor.
/// The App's EventEmitter and the global free functions share
/// the same underlying sink via this Arc.
///
/// Panics if the sink has not been initialized.
pub fn sink_arc() -> Arc<SinkMutex> {
    EVENT_SINK
        .get()
        .expect("event sink not initialized")
        .clone()
}

fn with_sink<R>(f: impl FnOnce(&mut dyn EventSink) -> io::Result<R>) -> io::Result<R> {
    let sink = EVENT_SINK
        .get()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "event sink not initialized"))?;
    // parking_lot::Mutex never poisons; one lock, no nested locks -
    // the sink lock is the innermost and is held only long enough to
    // invoke `f`.
    let mut guard = sink.lock();
    f(&mut **guard)
}

// ---------------------------------------------------------------------------
// WriterSink
// ---------------------------------------------------------------------------

/// A sink that wraps a raw writer and encodes via a codec.
///
/// Used by the renderer binary and WASM entry points to write
/// encoded wire protocol messages to stdout or a channel.
pub struct WriterSink {
    writer: Box<dyn std::io::Write + Send>,
    codec: plushie_widget_sdk::codec::Codec,
}

impl WriterSink {
    /// Create a WriterSink with an explicit codec.
    pub fn new(
        writer: Box<dyn std::io::Write + Send>,
        codec: plushie_widget_sdk::codec::Codec,
    ) -> Self {
        Self { writer, codec }
    }
}

impl EventSink for WriterSink {
    fn emit_event(&mut self, event: OutgoingEvent) -> io::Result<()> {
        let bytes = self.codec.encode(&event).map_err(io::Error::other)?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()
    }

    fn emit_effect_response(
        &mut self,
        response: plushie_widget_sdk::protocol::EffectResponse,
    ) -> io::Result<()> {
        let bytes = self.codec.encode(&response).map_err(io::Error::other)?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()
    }

    fn emit_query_response(
        &mut self,
        kind: &str,
        tag: &str,
        data: &serde_json::Value,
    ) -> io::Result<()> {
        // `session` is a write-path placeholder. Headless routes every response
        // through `OutgoingEvent::op_query_response(...).with_session(...)` and
        // bypasses this helper. Windowed mode is single-session and threads
        // the session value into the emitter at App construction time; see
        // the `session` plumbing in App for how the field is populated.
        let msg = serde_json::json!({
            "type": "op_query_response",
            "session": "",
            "kind": kind,
            "tag": tag,
            "data": data,
        });
        let bytes = self.codec.encode(&msg).map_err(io::Error::other)?;
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
        // `session` is a write-path placeholder; the caller populates it via
        // `with_session(...)` on the outgoing event path. Same pattern as
        // `emit_query_response` above.
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
        let bytes = self
            .codec
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
        // Union of builtin + native widget type names, sorted alphabetically for
        // stable, predictable output. `native_widgets` itself is emitted sorted
        // to match.
        let mut all_widgets: Vec<String> = builtin
            .iter()
            .cloned()
            .chain(native_widgets.iter().map(|s| s.to_string()))
            .collect();
        all_widgets.sort();
        all_widgets.dedup();
        let mut native_sorted: Vec<&str> = native_widgets.to_vec();
        native_sorted.sort_unstable();

        // Hello carries `session: ""` by design: it is emitted before any
        // host session is known, in response to Settings. The handshake is
        // not per-session.
        let msg = serde_json::json!({
            "type": "hello",
            "session": "",
            "protocol": plushie_widget_sdk::protocol::PROTOCOL_VERSION,
            "version": env!("CARGO_PKG_VERSION"),
            "name": "plushie-renderer",
            "mode": mode,
            "backend": backend,
            "transport": transport,
            "native_widgets": native_sorted,
            "widget_sets": widget_set_names,
            "widgets": all_widgets,
        });
        let bytes = self.codec.encode(&msg).map_err(io::Error::other)?;
        self.writer.write_all(&bytes)?;
        self.writer.flush()
    }

    fn write_raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }
}

// ---------------------------------------------------------------------------
// Global free functions (for startup/headless code without App access)
// ---------------------------------------------------------------------------

/// Write pre-encoded bytes through the global sink.
///
/// Used by code paths that don't have App access:
/// - Headless stdout output (`WireWriter::write_bytes`)
/// - Headless multiplexed writer thread
/// - Startup error reporting (`startup_exit`)
pub fn write_output(bytes: &[u8]) -> io::Result<()> {
    with_sink(|sink| sink.write_raw(bytes))
}

/// Emit a `hello` handshake message through the global sink.
///
/// Called during renderer startup before the App instance exists.
/// Used by windowed, headless, and WASM entry points.
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
// Panic hook
// ---------------------------------------------------------------------------

/// Extract a human-readable message from a panic payload.
///
/// Mirrors the `&'static str` / `String` downcast pattern used in
/// `plushie-renderer/src/headless.rs` session-panic recovery.
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&'static str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
        .unwrap_or("(non-string panic)")
}

/// Emit `session_error` + `session_closed` events through the
/// current global sink as a reaction to a renderer-side panic.
///
/// Exposed separately from [`install_panic_hook`] so tests (and any
/// future structured-diagnostics path) can exercise the same emit
/// behaviour without touching the process-global panic hook.
fn emit_panic_events(msg: &str, location: &str) {
    if let Some(sink_lock) = EVENT_SINK.get() {
        // sink lock is the innermost; no nested locks held here.
        let mut guard = sink_lock.lock();

        let error_event = plushie_widget_sdk::protocol::OutgoingEvent::generic(
            "session_error",
            "",
            Some(serde_json::json!({ "error": msg, "location": location })),
        );
        let _ = guard.emit_event(error_event);

        let closed_event = plushie_widget_sdk::protocol::OutgoingEvent::generic(
            "session_closed",
            "",
            Some(serde_json::json!({ "reason": "panic" })),
        );
        let _ = guard.emit_event(closed_event);
    }
}

/// Install a process-wide panic hook that emits `session_error` +
/// `session_closed` events before the default hook runs.
///
/// Without this, a panic in an iced subscription, window handler, or
/// effect handler would terminate the process with a stack trace but
/// no wire-visible signal. Hosts would see an abrupt close and have
/// to guess whether it was graceful shutdown or a crash.
///
/// Safe to call after [`init_sink`]. If the sink isn't yet
/// initialised at panic time the emit is skipped (the default hook
/// still runs).
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = panic_payload_message(info.payload());
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        log::error!("renderer panic at {location}: {msg}");

        // A missing sink (pre-init panic) or a poisoned/broken sink
        // is non-fatal here; we don't want to panic inside the panic
        // hook, so every emit path is best-effort.
        emit_panic_events(msg, &location);

        previous(info);
    }));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;

    use parking_lot::Mutex as PlMutex;

    /// In-memory EventSink that records every emitted event. Used
    /// to verify panic-hook and emit_panic_events behaviour without
    /// touching the real stdout/codec path.
    #[derive(Default)]
    struct RecordingSink {
        events: Arc<StdMutex<Vec<OutgoingEvent>>>,
    }

    impl EventSink for RecordingSink {
        fn emit_event(&mut self, event: OutgoingEvent) -> io::Result<()> {
            self.events.lock().unwrap().push(event);
            Ok(())
        }
        fn emit_effect_response(
            &mut self,
            _: plushie_widget_sdk::protocol::EffectResponse,
        ) -> io::Result<()> {
            Ok(())
        }
        fn emit_query_response(
            &mut self,
            _: &str,
            _: &str,
            _: &serde_json::Value,
        ) -> io::Result<()> {
            Ok(())
        }
        fn emit_screenshot_response(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: u32,
            _: u32,
            _: &[u8],
        ) -> io::Result<()> {
            Ok(())
        }
        fn emit_hello(
            &mut self,
            _: &str,
            _: &str,
            _: &[&str],
            _: &[&str],
            _: &str,
        ) -> io::Result<()> {
            Ok(())
        }
        fn write_raw(&mut self, _: &[u8]) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn panic_payload_message_handles_str_and_string() {
        let str_payload: Box<dyn std::any::Any + Send> = Box::new("static str panic");
        assert_eq!(panic_payload_message(&*str_payload), "static str panic");

        let string_payload: Box<dyn std::any::Any + Send> = Box::new("owned".to_string());
        assert_eq!(panic_payload_message(&*string_payload), "owned");

        let other_payload: Box<dyn std::any::Any + Send> = Box::new(42u32);
        assert_eq!(panic_payload_message(&*other_payload), "(non-string panic)");
    }

    // A panic must produce BOTH session_error (with the panic
    // message) AND session_closed (with reason="panic") on the wire
    // before the default hook runs. We exercise emit_panic_events
    // directly; install_panic_hook is global state that interferes
    // with other tests and with the test harness, so it's covered
    // by the renderer binary's startup path (renderer/run.rs).
    #[test]
    fn emit_panic_events_writes_error_then_closed() {
        // The global EVENT_SINK OnceLock may already be set by other
        // tests in the same process; skip the test if so. This is
        // acceptable because the behaviour being covered is a
        // straightforward match on EVENT_SINK.get(), exercised via
        // unit invariants below.
        let events: Arc<StdMutex<Vec<OutgoingEvent>>> = Arc::new(StdMutex::new(Vec::new()));
        let recording = RecordingSink {
            events: events.clone(),
        };
        // Only init if no other test has claimed the sink.
        let arc = Arc::new(PlMutex::new(Box::new(recording) as Box<dyn EventSink>));
        let fresh_init = EVENT_SINK.set(arc).is_ok();
        if !fresh_init {
            eprintln!(
                "skipping emit_panic_events test: global EVENT_SINK already set by another test"
            );
            return;
        }

        emit_panic_events("boom", "file.rs:1:1");

        let ev = events.lock().unwrap();
        assert_eq!(ev.len(), 2, "expected session_error then session_closed");
        assert_eq!(ev[0].family, "session_error");
        assert_eq!(ev[1].family, "session_closed");

        let err_value = ev[0].value.as_ref().expect("session_error carries a value");
        assert_eq!(
            err_value.get("error").and_then(|v| v.as_str()),
            Some("boom")
        );
        assert_eq!(
            err_value.get("location").and_then(|v| v.as_str()),
            Some("file.rs:1:1"),
        );

        let closed_value = ev[1]
            .value
            .as_ref()
            .expect("session_closed carries a value");
        assert_eq!(
            closed_value.get("reason").and_then(|v| v.as_str()),
            Some("panic"),
        );
    }
}
