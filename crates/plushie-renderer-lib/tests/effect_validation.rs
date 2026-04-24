use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex as PlMutex;
use serde_json::json;

use plushie_core::ops::EffectRequest;
use plushie_renderer_lib::App;
use plushie_renderer_lib::effects::EffectHandler;
use plushie_renderer_lib::emitters::{EventSink, SinkMutex};
use plushie_widget_sdk::protocol::{
    DiagnosticMessage, EffectResponse, IncomingMessage, OutgoingEvent,
};
use plushie_widget_sdk::registry::WidgetRegistry;

struct RecordingSink {
    responses: Arc<Mutex<Vec<EffectResponse>>>,
}

impl EventSink for RecordingSink {
    fn emit_event(&mut self, _: OutgoingEvent) -> io::Result<()> {
        Ok(())
    }

    fn emit_effect_response(&mut self, response: EffectResponse) -> io::Result<()> {
        self.responses.lock().unwrap().push(response);
        Ok(())
    }

    fn emit_query_response(&mut self, _: &str, _: &str, _: &serde_json::Value) -> io::Result<()> {
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

    fn emit_hello(&mut self, _: &str, _: &str, _: &[&str], _: &[&str], _: &str) -> io::Result<()> {
        Ok(())
    }

    fn emit_diagnostic(&mut self, _: DiagnosticMessage) -> io::Result<()> {
        Ok(())
    }

    fn write_raw(&mut self, _: &[u8]) -> io::Result<()> {
        Ok(())
    }
}

struct CountingEffectHandler {
    sync_calls: Arc<AtomicUsize>,
}

impl EffectHandler for CountingEffectHandler {
    fn handle_sync(&self, id: &str, _request: &EffectRequest) -> Option<EffectResponse> {
        self.sync_calls.fetch_add(1, Ordering::SeqCst);
        Some(EffectResponse::ok(id.to_string(), json!(null)))
    }

    fn handle_async(
        &self,
        id: String,
        _request: EffectRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = EffectResponse> + Send>> {
        Box::pin(async move { EffectResponse::ok(id, json!(null)) })
    }

    fn is_async(&self, _request: &EffectRequest) -> bool {
        false
    }
}

fn build_app() -> (App, Arc<Mutex<Vec<EffectResponse>>>, Arc<AtomicUsize>) {
    let responses = Arc::new(Mutex::new(Vec::new()));
    let sync_calls = Arc::new(AtomicUsize::new(0));
    let sink = RecordingSink {
        responses: responses.clone(),
    };
    let sink_arc: Arc<SinkMutex> = Arc::new(PlMutex::new(Box::new(sink) as Box<dyn EventSink>));
    let handler = CountingEffectHandler {
        sync_calls: sync_calls.clone(),
    };
    let app = App::new(WidgetRegistry::new(), Box::new(handler), sink_arc);
    (app, responses, sync_calls)
}

#[test]
fn invalid_effect_payload_emits_error_without_dispatching() {
    let (mut app, responses, sync_calls) = build_app();

    app.apply(IncomingMessage::Effect {
        id: "req-1".to_string(),
        kind: "clipboard_write".to_string(),
        payload: json!({}),
    })
    .unwrap();

    assert_eq!(sync_calls.load(Ordering::SeqCst), 0);
    let responses = responses.lock().unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].id, "req-1");
    assert_eq!(responses[0].status, "error");
    assert_eq!(
        responses[0].error.as_deref(),
        Some("missing required field for clipboard_write: text")
    );
}

#[test]
fn valid_effect_payload_still_dispatches() {
    let (mut app, responses, sync_calls) = build_app();

    app.apply(IncomingMessage::Effect {
        id: "req-1".to_string(),
        kind: "clipboard_write".to_string(),
        payload: json!({"text": "hello"}),
    })
    .unwrap();

    assert_eq!(sync_calls.load(Ordering::SeqCst), 1);
    let responses = responses.lock().unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].status, "ok");
}
