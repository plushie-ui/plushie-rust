//! In-process event sink for direct mode.
//!
//! Collects renderer events into a queue that the DirectApp drains
//! after each update cycle.

#[cfg(feature = "direct")]
use std::io;

#[cfg(feature = "direct")]
use std::sync::Arc;

#[cfg(feature = "direct")]
use parking_lot::Mutex;

#[cfg(feature = "direct")]
use plushie_core::protocol::{EffectResponse, OutgoingEvent};

// Re-export SinkEvent from event_bridge (where it's defined).
pub(crate) use super::event_bridge::SinkEvent;

/// An EventSink that collects events in-process.
///
/// Events are stored in a shared queue. The DirectApp drains the
/// queue after each iced update cycle to convert events to SDK
/// Events and deliver them to the user's App::update().
///
/// Uses `parking_lot::Mutex` rather than `std::sync::Mutex` to match
/// the renderer-lib emitter sink. parking_lot locks are faster under
/// contention and have no poisoning state to reason about.
#[cfg(feature = "direct")]
pub(crate) struct QueueSink {
    queue: Arc<Mutex<Vec<SinkEvent>>>,
}

#[cfg(feature = "direct")]
impl QueueSink {
    pub fn new() -> (Self, Arc<Mutex<Vec<SinkEvent>>>) {
        let queue = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                queue: queue.clone(),
            },
            queue,
        )
    }
}

#[cfg(feature = "direct")]
impl plushie_renderer_lib::EventSink for QueueSink {
    fn emit_event(&mut self, event: OutgoingEvent) -> io::Result<()> {
        self.queue.lock().push(SinkEvent::Event(event));
        Ok(())
    }

    fn emit_effect_response(&mut self, response: EffectResponse) -> io::Result<()> {
        self.queue.lock().push(SinkEvent::EffectResponse(response));
        Ok(())
    }

    fn emit_query_response(
        &mut self,
        kind: &str,
        tag: &str,
        data: &serde_json::Value,
    ) -> io::Result<()> {
        self.queue.lock().push(SinkEvent::QueryResponse {
            kind: kind.to_string(),
            tag: tag.to_string(),
            data: data.clone(),
        });
        Ok(())
    }

    fn emit_screenshot_response(
        &mut self,
        _id: &str,
        _name: &str,
        _hash: &str,
        _width: u32,
        _height: u32,
        _rgba_bytes: &[u8],
    ) -> io::Result<()> {
        Ok(())
    }

    fn emit_hello(
        &mut self,
        _mode: &str,
        _backend: &str,
        _native_widgets: &[&str],
        _widget_set_names: &[&str],
        _transport: &str,
    ) -> io::Result<()> {
        Ok(())
    }

    fn emit_diagnostic(
        &mut self,
        _message: plushie_core::protocol::DiagnosticMessage,
    ) -> io::Result<()> {
        // Direct mode: the diagnostics::emit call already logged the
        // message at its chosen level. There is no wire consumer here,
        // so nothing to queue. When direct-mode apps need programmatic
        // observation, add an SDK Event variant and deliver through
        // sink_event_to_sdk.
        Ok(())
    }

    fn write_raw(&mut self, _bytes: &[u8]) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(all(test, feature = "direct"))]
mod tests {
    use super::*;
    use plushie_renderer_lib::EventSink;

    #[test]
    fn queues_renderer_events() {
        let (mut sink, queue) = QueueSink::new();
        let event = OutgoingEvent::generic("click", "button", None);

        sink.emit_event(event).expect("emit event");

        let queued = queue.lock();
        assert_eq!(queued.len(), 1);
        assert!(matches!(queued.first(), Some(SinkEvent::Event(_))));
    }

    #[test]
    fn queues_effect_and_query_responses() {
        let (mut sink, queue) = QueueSink::new();

        sink.emit_effect_response(EffectResponse {
            message_type: "effect_response",
            session: String::new(),
            id: "ef_0".to_string(),
            status: "ok",
            result: Some(serde_json::json!({"value": true})),
            error: None,
        })
        .expect("emit effect response");
        sink.emit_query_response("tree_hash", "hash", &serde_json::json!("abc"))
            .expect("emit query response");

        let queued = queue.lock();
        assert_eq!(queued.len(), 2);
        assert!(matches!(queued.first(), Some(SinkEvent::EffectResponse(_))));
        assert!(matches!(
            queued.get(1),
            Some(SinkEvent::QueryResponse { kind, tag, data })
                if kind == "tree_hash" && tag == "hash" && data == &serde_json::json!("abc")
        ));
    }
}
