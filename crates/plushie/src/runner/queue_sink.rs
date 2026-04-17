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

// Re-export SinkEvent from event_bridge (where it's defined).
pub(crate) use super::event_bridge::SinkEvent;

/// An EventSink that collects events in-process.
///
/// Events are stored in a shared queue. The DirectApp drains the
/// queue after each iced update cycle to convert events to SDK
/// Events and deliver them to the user's App::update().
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
impl QueueSink {
    /// Lock the queue, recovering from poisoning rather than aborting
    /// the process. Poisoning means some upstream path panicked while
    /// holding the lock; the queue contents are still valid to observe.
    fn lock_queue(&self) -> std::sync::MutexGuard<'_, Vec<SinkEvent>> {
        self.queue.lock().unwrap_or_else(|e| {
            log::error!("QueueSink event queue lock poisoned; recovering");
            e.into_inner()
        })
    }
}

#[cfg(feature = "direct")]
impl plushie_renderer_lib::EventSink for QueueSink {
    fn emit_event(&mut self, event: OutgoingEvent) -> io::Result<()> {
        self.lock_queue().push(SinkEvent::Event(event));
        Ok(())
    }

    fn emit_effect_response(&mut self, response: EffectResponse) -> io::Result<()> {
        self.lock_queue().push(SinkEvent::EffectResponse(response));
        Ok(())
    }

    fn emit_query_response(
        &mut self,
        kind: &str,
        tag: &str,
        data: &serde_json::Value,
    ) -> io::Result<()> {
        self.lock_queue().push(SinkEvent::QueryResponse {
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

    fn write_raw(&mut self, _bytes: &[u8]) -> io::Result<()> {
        Ok(())
    }
}
