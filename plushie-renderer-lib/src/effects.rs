//! Platform abstraction for side effects.
//!
//! The renderer needs to perform platform-specific operations (file
//! dialogs, clipboard, notifications) that differ between native and
//! WASM targets. The [`EffectHandler`] trait abstracts these so
//! plushie-renderer can compile to both targets.

use std::future::Future;
use std::pin::Pin;

use plushie_core::ops::EffectRequest;
use plushie_widget_sdk::protocol::EffectResponse;

/// Handler for platform-specific side effects.
///
/// Native implementations use rfd (file dialogs), arboard (clipboard),
/// and notify-rust (notifications). WASM implementations stub or use
/// web platform APIs.
///
/// Handlers produce data (EffectResponse). The caller (App::execute)
/// is responsible for emitting the response through the EventSink.
/// This keeps handlers decoupled from the emission mechanism.
///
/// The `Send + 'static` bound is required because iced's daemon holds
/// the App across async boundaries and may move it between executor
/// contexts on native (tokio). On wasm32, `Send` is trivially satisfied.
pub trait EffectHandler: Send + 'static {
    /// Handle a synchronous effect. Returns `Some(response)` for effects
    /// that complete immediately (clipboard, notifications).
    ///
    /// Returns `None` only if the request is completely unrecognized.
    fn handle_sync(&self, id: &str, request: &EffectRequest) -> Option<EffectResponse>;

    /// Handle an async effect, returning a future that resolves to the
    /// response. Used for operations that must not block the event loop
    /// (file dialogs on native).
    fn handle_async(
        &self,
        id: String,
        request: EffectRequest,
    ) -> Pin<Box<dyn Future<Output = EffectResponse> + Send>>;

    /// Returns true if the given request should be handled async.
    fn is_async(&self, request: &EffectRequest) -> bool;
}
