//! Diagnostic emission hook.
//!
//! Inline sites across widget-sdk, renderer-lib, and the SDK crate
//! (font family cache, SVG guards, canvas shape validation, content
//! caps, wire decode) emit typed diagnostics through [`emit`]. Each
//! call logs at the chosen level (the always-on fallback) and, when
//! the renderer has installed a sink hook via [`set_hook`], also
//! routes the diagnostic to the wire as a
//! [`DiagnosticMessage`](crate::protocol::DiagnosticMessage) so hosts
//! can observe it programmatically.
//!
//! The hook is a function pointer-like trait object installed exactly
//! once at renderer startup. If no sink exists (tests, widget-sdk
//! unit tests), emit just logs and returns.

use std::sync::OnceLock;

use crate::Diagnostic;
use crate::protocol::DiagnosticLevel;

/// Sink hook installed by the renderer; receives every emitted
/// diagnostic. Set once via [`set_hook`].
pub type DiagnosticHook = dyn Fn(DiagnosticLevel, &Diagnostic) + Send + Sync + 'static;

static HOOK: OnceLock<Box<DiagnosticHook>> = OnceLock::new();

/// Install the renderer-side sink hook.
///
/// Must be called at most once per process. Subsequent calls are
/// ignored and a warning is logged so tests that set up their own
/// harness don't fight the global.
pub fn set_hook(hook: Box<DiagnosticHook>) {
    if HOOK.set(hook).is_err() {
        log::warn!("diagnostics::set_hook called twice; keeping the first hook");
    }
}

/// Emit a typed diagnostic.
///
/// Always logs via `log::warn!` so any captured log stream still shows
/// the diagnostic. When the renderer's sink hook is installed, also
/// forwards the diagnostic to the wire (as a `DiagnosticMessage`).
pub fn emit(level: DiagnosticLevel, diagnostic: Diagnostic) {
    match level {
        DiagnosticLevel::Info => log::info!("{diagnostic}"),
        DiagnosticLevel::Warn => log::warn!("{diagnostic}"),
        DiagnosticLevel::Error => log::error!("{diagnostic}"),
    }
    if let Some(hook) = HOOK.get() {
        hook(level, &diagnostic);
    }
}

/// Emit a warning-level diagnostic. Shorthand for
/// [`emit`] with [`DiagnosticLevel::Warn`].
pub fn warn(diagnostic: Diagnostic) {
    emit(DiagnosticLevel::Warn, diagnostic);
}

/// Emit an info-level diagnostic.
pub fn info(diagnostic: Diagnostic) {
    emit(DiagnosticLevel::Info, diagnostic);
}

/// Emit an error-level diagnostic.
pub fn error(diagnostic: Diagnostic) {
    emit(DiagnosticLevel::Error, diagnostic);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_without_hook_logs_without_panicking() {
        emit(
            DiagnosticLevel::Warn,
            Diagnostic::FontFamilyNotFound {
                family: "NeverLoaded".into(),
            },
        );
    }

    #[test]
    fn level_display_is_snake_case() {
        assert_eq!(DiagnosticLevel::Info.to_string(), "info");
        assert_eq!(DiagnosticLevel::Warn.to_string(), "warn");
        assert_eq!(DiagnosticLevel::Error.to_string(), "error");
    }
}
