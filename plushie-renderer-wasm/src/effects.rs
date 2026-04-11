//! Web effect handler.
//!
//! Stubs all effects as unsupported. Web implementations for clipboard
//! (Clipboard API) and file access (File System Access API) can be
//! added in a future iteration.

use iced::Task;

use plushie_core::ops::EffectRequest;
use plushie_renderer_lib::EffectHandler;
use plushie_widget_sdk::message::Message;
use plushie_widget_sdk::protocol::EffectResponse;

pub struct WebEffectHandler;

impl EffectHandler for WebEffectHandler {
    fn handle_sync(&self, id: &str, _request: &EffectRequest) -> Option<EffectResponse> {
        Some(EffectResponse::unsupported(id.to_string()))
    }

    fn handle_async(&self, _id: String, _request: EffectRequest) -> Task<Message> {
        Task::none()
    }

    fn is_async(&self, _request: &EffectRequest) -> bool {
        false
    }
}
