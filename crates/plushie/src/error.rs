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
        /// Path the SDK attempted to spawn.
        binary: String,
        /// Underlying spawn failure from the OS.
        #[source]
        source: std::io::Error,
    },

    /// Renderer reported a different protocol version than this SDK
    /// was built against.
    #[error("protocol version mismatch: expected {expected}, got {got:?}")]
    ProtocolVersionMismatch {
        /// Protocol version this SDK was built for.
        expected: u32,
        /// Protocol version the renderer advertised (if any).
        got: Option<u32>,
    },

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

    /// Neither `direct` nor `wire` feature is enabled. Build plushie
    /// with at least one of the runner features to get a working
    /// `run()` entry point.
    #[error(
        "plushie was built with neither the `direct` nor the `wire` feature; \
         enable at least one to use `plushie::run`"
    )]
    NoRunnerFeature,

    /// Wire mode could not locate a renderer binary. The four-step
    /// discovery chain (see [`crate::runner`]) exhausted every
    /// candidate without finding an executable.
    #[error("renderer binary not found: {hint}")]
    BinaryNotFound {
        /// Human-readable hint describing the search order and remediation.
        hint: String,
    },

    /// A wire frame exceeded the 64 MiB per-message cap. Emitted as a
    /// typed error so callers can distinguish a protocol-violation
    /// frame from a generic decode failure without string parsing.
    #[error("wire frame of {size} bytes exceeds {limit} byte limit")]
    BufferOverflow {
        /// Size of the offending frame in bytes.
        size: usize,
        /// Configured cap in bytes.
        limit: usize,
    },
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
