//! Error type for the plushie SDK.
//!
//! [`Error`] is a `#[non_exhaustive]` enum covering every failure
//! mode an app can hit at the SDK surface: renderer spawn failures,
//! protocol version mismatch, wire encode/decode errors, renderer
//! exits, startup failures, and iced initialisation problems.
//!
//! Use `thiserror`-derived [`std::fmt::Display`] for log messages,
//! and match on variants when the caller needs to decide retry vs
//! abort vs user-facing dialog.

use crate::settings::ExitReason;

/// Error type for plushie entry points.
///
/// Marked `#[non_exhaustive]` so adding variants is not a semver
/// break.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Generic I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to spawn the renderer subprocess (wire mode).
    #[error("failed to spawn renderer binary `{binary}`: {source}")]
    Spawn {
        binary: String,
        #[source]
        source: std::io::Error,
    },

    /// Renderer reported a different protocol version than this SDK
    /// was built against.
    #[error("protocol version mismatch: expected {expected}, got {got:?}")]
    ProtocolVersionMismatch { expected: u32, got: Option<u32> },

    /// Failed to decode a wire message.
    #[error("wire decode error: {0}")]
    WireDecode(String),

    /// Failed to encode a wire message.
    #[error("wire encode error: {0}")]
    WireEncode(String),

    /// The renderer process exited. Inspect [`ExitReason`] to decide
    /// whether this is expected (shutdown) or unexpected (crash,
    /// heartbeat timeout, max-restarts exhaustion).
    #[error("renderer exited: {0:?}")]
    RendererExit(ExitReason),

    /// Startup failed before the main event loop began (settings
    /// validation, handshake failure, socket setup).
    #[error("startup failure: {0}")]
    Startup(String),

    /// An iced-side error surfaced from the in-process daemon (direct
    /// mode) or WASM bootstrap. Stringified at the boundary because
    /// `iced::Error` is not always `Send`.
    #[error("iced error: {0}")]
    Iced(String),

    /// Invalid settings (field type coercion, unknown keys, etc.).
    #[error("invalid settings: {0}")]
    InvalidSettings(String),
}

impl Error {
    /// Construct a `Spawn` error from an I/O error and the binary
    /// path we were trying to run.
    pub fn spawn(binary: impl Into<String>, source: std::io::Error) -> Self {
        Self::Spawn {
            binary: binary.into(),
            source,
        }
    }
}
