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

#[cfg(test)]
mod tests {
    //! Display strings on Error variants are part of the SDK's
    //! observable surface: they appear in logs, error dialogs, and
    //! exception chains in host bindings. A regression that drops
    //! the source path from `Spawn` or the byte counts from
    //! `BufferOverflow` makes a real-user error report harder to
    //! interpret. Pin every variant.

    use super::*;

    #[test]
    fn spawn_helper_carries_binary_and_source() {
        let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let err = Error::spawn("/path/to/renderer", inner);
        match &err {
            Error::Spawn { binary, source } => {
                assert_eq!(binary, "/path/to/renderer");
                assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
            }
            other => panic!("expected Spawn, got {other:?}"),
        }
        let display = err.to_string();
        assert!(display.contains("/path/to/renderer"));
        assert!(display.contains("no such file"));
    }

    #[test]
    fn io_variant_display_includes_inner_message() {
        let inner = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "writer hung up");
        let err = Error::Io(inner);
        let display = err.to_string();
        assert!(display.contains("io error"));
        assert!(display.contains("writer hung up"));
    }

    #[test]
    fn protocol_version_mismatch_display_lists_both_versions() {
        let err = Error::ProtocolVersionMismatch {
            expected: 1,
            got: Some(2),
        };
        let display = err.to_string();
        assert!(display.contains("expected 1"));
        assert!(display.contains("Some(2)"));

        // Unknown remote version case still produces a readable
        // string; no panic, no Some/None confusion in the wire log.
        let none = Error::ProtocolVersionMismatch {
            expected: 1,
            got: None,
        };
        assert!(none.to_string().contains("None"));
    }

    #[test]
    fn wire_decode_and_encode_display_carry_inner_message() {
        let decode = Error::WireDecode("unexpected EOF".into());
        assert!(decode.to_string().contains("unexpected EOF"));

        let encode = Error::WireEncode("frame too big".into());
        assert!(encode.to_string().contains("frame too big"));
    }

    #[test]
    fn renderer_exit_carries_exit_reason() {
        let err = Error::RendererExit(ExitReason::Shutdown);
        let display = err.to_string();
        assert!(display.contains("renderer exited"));
        assert!(display.contains("Shutdown"));
    }

    #[test]
    fn startup_iced_invalid_settings_display() {
        for (err, expected_substring) in [
            (Error::Startup("settings parse failed".into()), "startup"),
            (Error::Iced("daemon init".into()), "iced"),
            (
                Error::InvalidSettings("unknown field foo".into()),
                "invalid settings",
            ),
        ] {
            let display = err.to_string();
            assert!(
                display.contains(expected_substring),
                "{err:?}: expected substring `{expected_substring}` in `{display}`",
            );
        }
    }

    #[test]
    fn no_runner_feature_display_is_actionable() {
        let err = Error::NoRunnerFeature;
        let display = err.to_string();
        assert!(display.contains("direct"));
        assert!(display.contains("wire"));
        assert!(display.contains("plushie::run"));
    }

    #[test]
    fn binary_not_found_display_carries_hint() {
        let err = Error::BinaryNotFound {
            hint: "set PLUSHIE_RENDERER or build the binary with cargo build -p plushie-renderer"
                .into(),
        };
        let display = err.to_string();
        assert!(display.contains("PLUSHIE_RENDERER"));
        assert!(display.contains("renderer binary not found"));
    }

    #[test]
    fn buffer_overflow_display_lists_size_and_limit() {
        let err = Error::BufferOverflow {
            size: 100_000_000,
            limit: 67_108_864,
        };
        let display = err.to_string();
        assert!(display.contains("100000000"));
        assert!(display.contains("67108864"));
    }

    #[test]
    fn io_from_conversion_works_via_question_mark() {
        // Error::Io has #[from], so an io::Error converts via ? in
        // user code. Pin the conversion shape.
        fn perform() -> std::result::Result<(), Error> {
            let _: std::fs::File =
                std::fs::File::open("/this/path/should/never/exist/plushie-error-test")?;
            Ok(())
        }
        match perform().unwrap_err() {
            Error::Io(_) => {}
            other => panic!("expected Io, got {other:?}"),
        }
    }
}
