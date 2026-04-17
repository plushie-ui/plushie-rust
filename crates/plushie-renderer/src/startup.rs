//! Shared startup sequence for windowed and headless modes.
//!
//! Both modes follow the same wire protocol handshake:
//!
//! 1. Detect codec (peek first byte or use CLI flag)
//! 2. Set global codec
//! 3. Emit Hello
//! 4. Read first message, require it to be Settings
//! 5. Validate protocol version, token, validate_props
//! 6. Process backend-specific concerns (iced settings, fonts)
//! 7. Enter message loop
//!
//! This module provides the shared steps (1, 4, 5, 6) so each mode
//! only handles its own backend-specific setup.
//!
//! Fatal handshake failures return [`StartupError`] so the caller can
//! unwind normally. RAII guards (transport sockets, spawned children)
//! run their Drop implementations during the return path. The caller
//! is responsible for emitting a wire error (via [`emit_startup_error`])
//! and setting the process exit status.

use std::io::BufRead;

use plushie_widget_sdk::codec::Codec;
use plushie_widget_sdk::protocol::{IncomingMessage, SessionMessage};
use serde_json::Value;

/// Fatal startup failure.
///
/// Carries a human-readable message; no structured variants yet because
/// every current failure mode logs the same way. Wrap in a thiserror
/// enum later if callers need to distinguish causes.
#[derive(Debug)]
pub(crate) struct StartupError {
    pub(crate) message: String,
}

impl StartupError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for StartupError {}

pub(crate) type StartupResult<T> = Result<T, StartupError>;

/// Emit a startup failure to the wire (best-effort) and log it.
///
/// The caller handles process exit. Drop order unwinds normally so
/// transport sockets and spawned children are cleaned up via RAII
/// before the process exits.
pub(crate) fn emit_startup_error(codec: &Codec, err: &StartupError) {
    log::error!("{}", err.message);
    let error = serde_json::json!({"type": "error", "message": err.message});
    if let Ok(bytes) = codec.encode(&error) {
        let _ = plushie_renderer_lib::emitters::write_output(&bytes);
    }
}

// ---------------------------------------------------------------------------
// Codec detection
// ---------------------------------------------------------------------------

/// Detect the wire codec from a CLI flag or the first byte of input.
///
/// Peeks at the first byte via `fill_buf()` without consuming it,
/// so the caller can still read the full first message normally.
///
/// Returns the detected codec. The caller is responsible for
/// threading it to all consumers (WriterSink, App, stdin reader).
///
/// Returns an error on I/O failure. Codec-detection errors cannot
/// send a wire error (no codec is known), so callers log and exit;
/// any wire-safe errors after this point flow through [`StartupError`].
pub(crate) fn detect_codec(
    forced: Option<Codec>,
    reader: &mut impl BufRead,
) -> StartupResult<Codec> {
    match forced {
        Some(c) => {
            log::info!("wire codec (forced): {c}");
            Ok(c)
        }
        None => {
            let buf = match reader.fill_buf() {
                Ok(buf) if !buf.is_empty() => buf,
                Ok(_) => {
                    return Err(StartupError::new("stdin closed before first message"));
                }
                Err(e) => {
                    return Err(StartupError::new(format!(
                        "stdin read error during codec detection: {e}"
                    )));
                }
            };
            let codec = Codec::detect_from_first_byte(buf[0]);
            log::info!("wire codec (detected): {codec}");
            Ok(codec)
        }
    }
}

// ---------------------------------------------------------------------------
// Settings gate
// ---------------------------------------------------------------------------

/// The initial Settings message read from the wire, with its session
/// routing metadata preserved.
///
/// Returned by [`read_required_settings`]. The `session` field is the
/// wire-level session routing key (typically `""` in single-session
/// mode). The `settings` field is the raw JSON object from the
/// Settings message body.
pub(crate) struct InitialSettings {
    /// Session routing key from the wire message.
    pub session: String,
    /// Raw Settings JSON object.
    pub settings: Value,
}

impl InitialSettings {
    /// Decompose into the session routing key and the reconstructed
    /// [`IncomingMessage::Settings`] for forwarding to a session's
    /// `Core.apply()`.
    pub fn into_parts(self) -> (String, IncomingMessage) {
        (
            self.session,
            IncomingMessage::Settings {
                settings: self.settings,
            },
        )
    }

    /// Reconstruct the [`IncomingMessage`] for forwarding to a session's
    /// `Core.apply()`, discarding the session routing key.
    pub fn into_incoming_message(self) -> IncomingMessage {
        IncomingMessage::Settings {
            settings: self.settings,
        }
    }
}

/// Read the first message from the transport, requiring it to be Settings.
///
/// Decodes one framed message via [`SessionMessage::from_value`] to
/// preserve the session routing field, then validates the message is
/// the Settings variant. Returns an [`InitialSettings`] with both the
/// session key and settings body.
///
/// Returns an error if the message cannot be read, decoded, or is not
/// a Settings message. The caller typically hands the error to
/// [`emit_startup_error`] before returning.
pub(crate) fn read_required_settings(
    codec: &Codec,
    reader: &mut impl BufRead,
) -> StartupResult<InitialSettings> {
    let payload = match codec.read_message(reader) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return Err(StartupError::new("stdin closed before settings received"));
        }
        Err(e) => {
            return Err(StartupError::new(format!(
                "failed to read initial settings: {e}"
            )));
        }
    };

    let value: Value = match codec.decode(&payload) {
        Ok(v) => v,
        Err(e) => {
            return Err(StartupError::new(format!(
                "failed to decode initial settings: {e}"
            )));
        }
    };

    let sm = match SessionMessage::from_value(value) {
        Ok(sm) => sm,
        Err(e) => {
            return Err(StartupError::new(format!(
                "failed to parse initial settings: {e}"
            )));
        }
    };

    match sm.message {
        IncomingMessage::Settings { settings } => {
            log::info!("initial settings received (session {:?})", sm.session);
            Ok(InitialSettings {
                session: sm.session,
                settings,
            })
        }
        ref other => {
            let variant = message_variant_name(other);
            Err(StartupError::new(format!(
                "expected settings as first message, got {variant}"
            )))
        }
    }
}

/// Validate protocol version and token in the initial Settings.
///
/// Returns [`StartupError`] on:
///
/// - **Protocol version**: `protocol_version` must be present and
///   match [`PROTOCOL_VERSION`]. A mismatch or missing value is
///   fatal: running with mismatched protocols leads to subtle,
///   hard-to-debug failures.
/// - **Token** (listen mode): if `expected_token` is `Some`, the
///   settings must contain a matching `token` field. Comparison uses
///   constant-time equality to prevent timing attacks.
///
/// On success, applies the prop validation flag via `OnceLock`.
///
/// [`PROTOCOL_VERSION`]: plushie_widget_sdk::protocol::PROTOCOL_VERSION
pub(crate) fn validate_settings(
    settings: &Value,
    expected_token: Option<&str>,
) -> StartupResult<()> {
    // Protocol version check (mandatory).
    let expected = u64::from(plushie_widget_sdk::protocol::PROTOCOL_VERSION);
    match settings.get("protocol_version").and_then(|v| v.as_u64()) {
        Some(version) if version == expected => {}
        Some(version) => {
            return Err(StartupError::new(format!(
                "protocol version mismatch: host sent {version}, renderer expects {expected}"
            )));
        }
        None => {
            return Err(StartupError::new(format!(
                "missing protocol_version in Settings (expected {expected})"
            )));
        }
    }

    // Token verification (listen mode).
    if let Some(expected_tok) = expected_token {
        match settings.get("token").and_then(|v| v.as_str()) {
            Some(tok) if constant_time_eq(tok.as_bytes(), expected_tok.as_bytes()) => {
                log::info!("token verified");
            }
            Some(_) => {
                return Err(StartupError::new("token mismatch: connection rejected"));
            }
            None => {
                return Err(StartupError::new(
                    "missing token in Settings: connection rejected",
                ));
            }
        }
    }

    // Prop validation flag.
    plushie_renderer_lib::settings::apply_validate_props(settings);
    Ok(())
}

// ---------------------------------------------------------------------------
// Font collection
// ---------------------------------------------------------------------------

/// Collect all font bytes from a Settings message.
///
/// Returns both inline font data (base64/binary objects via
/// [`parse_inline_fonts`]) and fonts loaded from file paths on disk.
/// This consolidates the font collection logic that was previously
/// duplicated between the windowed and headless startup paths.
///
/// [`parse_inline_fonts`]: plushie_renderer_lib::settings::parse_inline_fonts
pub(crate) fn collect_font_bytes(settings: &Value) -> Vec<Vec<u8>> {
    let mut font_bytes = plushie_renderer_lib::settings::parse_inline_fonts(settings);

    if let Some(fonts) = settings.get("fonts").and_then(|v| v.as_array()) {
        for font_val in fonts {
            if let Some(path) = font_val.as_str() {
                match std::fs::read(path) {
                    Ok(bytes) => {
                        log::info!("loaded font: {path}");
                        font_bytes.push(bytes);
                    }
                    Err(e) => {
                        log::error!("failed to load font {path}: {e}");
                    }
                }
            }
        }
    }

    font_bytes
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Constant-time byte comparison to prevent timing attacks on token
/// verification.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Human-readable name for an [`IncomingMessage`] variant, for error
/// messages.
fn message_variant_name(msg: &IncomingMessage) -> &'static str {
    match msg {
        IncomingMessage::Snapshot { .. } => "snapshot",
        IncomingMessage::Patch { .. } => "patch",
        IncomingMessage::Effect { .. } => "effect",
        IncomingMessage::WidgetOp { .. } => "widget_op",
        IncomingMessage::Subscribe { .. } => "subscribe",
        IncomingMessage::Unsubscribe { .. } => "unsubscribe",
        IncomingMessage::WindowOp { .. } => "window_op",
        IncomingMessage::SystemOp { .. } => "system_op",
        IncomingMessage::SystemQuery { .. } => "system_query",
        IncomingMessage::Settings { .. } => "settings",
        IncomingMessage::Query { .. } => "query",
        IncomingMessage::Interact { .. } => "interact",
        IncomingMessage::TreeHash { .. } => "tree_hash",
        IncomingMessage::Screenshot { .. } => "screenshot",
        IncomingMessage::Reset { .. } => "reset",
        IncomingMessage::ImageOp { .. } => "image_op",
        IncomingMessage::Command { .. } => "command",
        IncomingMessage::Commands { .. } => "commands",
        IncomingMessage::AdvanceFrame { .. } => "advance_frame",
        IncomingMessage::RegisterEffectStub { .. } => "register_effect_stub",
        IncomingMessage::UnregisterEffectStub { .. } => "unregister_effect_stub",
    }
}
