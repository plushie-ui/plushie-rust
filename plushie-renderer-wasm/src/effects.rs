//! Web effect handler.
//!
//! Stubs all effects as unsupported. Web implementations for clipboard
//! (Clipboard API) and file access (File System Access API) can be
//! added in a future iteration.

use std::future::Future;
use std::pin::Pin;

use plushie_core::ops::EffectRequest;
use plushie_renderer_lib::EffectHandler;
use plushie_widget_sdk::protocol::EffectResponse;

pub struct WebEffectHandler;

impl EffectHandler for WebEffectHandler {
    fn handle_sync(&self, id: &str, _request: &EffectRequest) -> Option<EffectResponse> {
        Some(EffectResponse::unsupported(id.to_string()))
    }

    fn handle_async(
        &self, id: String, _request: EffectRequest,
    ) -> Pin<Box<dyn Future<Output = EffectResponse> + Send>> {
        Box::pin(async move { EffectResponse::unsupported(id) })
    }

    fn is_async(&self, _request: &EffectRequest) -> bool {
        false
    }
}
