//! In-process event sink for direct mode.
//!
//! Collects renderer events into a queue that the DirectApp drains
//! after each update cycle.

#[cfg(feature = "direct")]
use std::io;

#[cfg(feature = "direct")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "direct")]
use plushie_widget_sdk::protocol::{EffectResponse, OutgoingEvent};

/// An EventSink that collects events in-process.
///
/// Events are stored in a shared queue. The DirectApp drains the
/// queue after each iced update cycle to convert events to SDK
/// Events and deliver them to the user's App::update().
#[cfg(feature = "direct")]
pub(crate) struct QueueSink {
    queue: Arc<Mutex<Vec<SinkEvent>>>,
}

/// An event collected by the QueueSink or produced by SDK-local
/// commands (async tasks, timers, delayed events).
#[cfg(feature = "direct")]
#[derive(Debug)]
pub(crate) enum SinkEvent {
    /// An OutgoingEvent from the renderer.
    Event(OutgoingEvent),
    /// An effect response from the renderer.
    EffectResponse(EffectResponse),
    /// A query response from the renderer.
    QueryResponse {
        kind: String,
        tag: String,
        data: serde_json::Value,
    },
    /// Result of an async task (Command::Async).
    AsyncResult {
        tag: String,
        result: Result<serde_json::Value, serde_json::Value>,
    },
    /// A delayed event (Command::SendAfter).
    DelayedEvent(crate::event::Event),
}

#[cfg(feature = "direct")]
impl QueueSink {
    pub fn new() -> (Self, Arc<Mutex<Vec<SinkEvent>>>) {
        let queue = Arc::new(Mutex::new(Vec::new()));
        (Self { queue: queue.clone() }, queue)
    }
}

#[cfg(feature = "direct")]
impl plushie_renderer_lib::EventSink for QueueSink {
    fn emit_event(&mut self, event: OutgoingEvent) -> io::Result<()> {
        self.queue.lock().unwrap().push(SinkEvent::Event(event));
        Ok(())
    }

    fn emit_effect_response(&mut self, response: EffectResponse) -> io::Result<()> {
        self.queue.lock().unwrap().push(SinkEvent::EffectResponse(response));
        Ok(())
    }

    fn emit_query_response(
        &mut self, kind: &str, tag: &str, data: &serde_json::Value,
    ) -> io::Result<()> {
        self.queue.lock().unwrap().push(SinkEvent::QueryResponse {
            kind: kind.to_string(),
            tag: tag.to_string(),
            data: data.clone(),
        });
        Ok(())
    }

    fn emit_screenshot_response(
        &mut self, _id: &str, _name: &str, _hash: &str,
        _width: u32, _height: u32, _rgba_bytes: &[u8],
    ) -> io::Result<()> {
        // Screenshots are not used in direct mode SDK.
        Ok(())
    }

    fn emit_hello(
        &mut self, _mode: &str, _backend: &str, _native_widgets: &[&str],
        _widget_set_names: &[&str], _transport: &str,
    ) -> io::Result<()> {
        // Hello is not used in direct mode SDK.
        Ok(())
    }

    fn write_raw(&mut self, _bytes: &[u8]) -> io::Result<()> {
        // Raw writes (stub acks, scripting) are not used in direct mode SDK.
        Ok(())
    }
}
