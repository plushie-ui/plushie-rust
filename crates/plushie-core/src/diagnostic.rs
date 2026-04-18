//! Typed diagnostic variants emitted from tree normalization, widget
//! validation, and runtime bookkeeping.
//!
//! The renderer and SDK historically carried diagnostics as raw
//! `String`s formatted with an ad-hoc `[code=...]` prefix. That worked
//! for human-readable log output but made downstream filtering
//! (TestSession strict-mode, future wire-diagnostic channels, custom
//! sinks on Settings) a string-matching exercise.
//!
//! The [`Diagnostic`] enum is the structured counterpart. Variants
//! carry exactly the context the emitter knew; `Display` mirrors the
//! legacy string formatting so existing `log::warn!("{diag}")` sites
//! keep producing the same human-readable output. `Serialize` /
//! `Deserialize` let the same value flow over the wire when the
//! structured-diagnostic channel eventually lands.
//!
//! Variants are added opportunistically as emit sites migrate. The
//! enum is `#[non_exhaustive]` so adding a new variant is not a
//! semver break.

use serde::{Deserialize, Serialize};

/// Stable kind discriminator for a [`Diagnostic`]. Used by consumers
/// that want to filter on the variant without pattern-matching on the
/// full payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// A widget ID collides with an already-declared ID in the same
    /// scope. Carried by [`Diagnostic::DuplicateId`].
    DuplicateId,
    /// A view declared an empty ID where a real one was expected.
    /// Carried by [`Diagnostic::EmptyId`].
    EmptyId,
    /// The tree has more than one top-level window child. Carried by
    /// [`Diagnostic::MultipleTopLevelWindows`].
    MultipleTopLevelWindows,
    /// A subscription is scoped to a window that does not appear in
    /// the current tree. Carried by [`Diagnostic::UnknownWindow`].
    UnknownWindow,
    /// An `__widget__` placeholder in the view tree has no matching
    /// registered expander. Carried by
    /// [`Diagnostic::UnrecognizedWidgetPlaceholder`].
    UnrecognizedWidgetPlaceholder,
    /// A kind that doesn't (yet) have a dedicated variant. Keeps
    /// parser round-trips lossless without forcing every emit site to
    /// migrate in a single pass.
    Other,
}

/// A structured diagnostic emitted by the SDK or renderer.
///
/// Variants are additive; consumers that pattern-match exhaustively
/// need to include a `_ => { ... }` arm because the enum is marked
/// `#[non_exhaustive]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Diagnostic {
    /// A widget ID collided with one already declared within the same
    /// window scope.
    DuplicateId {
        /// Fully-qualified (scoped) ID that collided.
        id: String,
        /// Window scope, if the tree is inside one.
        window_id: Option<String>,
    },
    /// A view declared a widget with an empty ID where a non-empty
    /// one was expected. Auto-generated IDs (prefixed `auto:`) and
    /// legitimate structural wrappers don't fire this; only an
    /// explicit `.id("")` at a user call site does.
    EmptyId {
        /// Widget type name (e.g. `"container"`).
        type_name: String,
    },
    /// The top level of the view tree holds more than one `window`
    /// child. Elixir / other SDKs tolerate this as peer windows, but
    /// the Rust SDK's idiomatic shape is one root window plus
    /// auxiliary windows opened via `Command::open_window`. This
    /// diagnostic flags the shape at render time.
    MultipleTopLevelWindows {
        /// IDs of the peer windows observed at the top level.
        window_ids: Vec<String>,
    },
    /// A subscription was declared for a window that is not currently
    /// in the tree. The renderer will accept the subscription but
    /// never deliver events.
    UnknownWindow {
        /// Window ID the subscription tried to bind to.
        window_id: String,
        /// Subscription tag (wire kind).
        subscription_tag: String,
    },
    /// An `__widget__` placeholder in the tree had no registered
    /// expander. Indicates an app-level bug: the widget's
    /// `.register(widgets)` was skipped while the placeholder was
    /// still included in the view tree.
    UnrecognizedWidgetPlaceholder {
        /// ID of the placeholder node.
        id: String,
    },
    /// Catch-all for diagnostics that originated as a pre-migration
    /// formatted string. Keeps typed consumers from losing the
    /// payload before the emitter has a dedicated variant.
    Other {
        /// Stable diagnostic kind tag (e.g. `"prop_range_exceeded"`).
        code: String,
        /// Fully-formatted human-readable message.
        message: String,
    },
}

impl Diagnostic {
    /// Stable kind discriminator.
    pub fn kind(&self) -> DiagnosticKind {
        match self {
            Self::DuplicateId { .. } => DiagnosticKind::DuplicateId,
            Self::EmptyId { .. } => DiagnosticKind::EmptyId,
            Self::MultipleTopLevelWindows { .. } => DiagnosticKind::MultipleTopLevelWindows,
            Self::UnknownWindow { .. } => DiagnosticKind::UnknownWindow,
            Self::UnrecognizedWidgetPlaceholder { .. } => {
                DiagnosticKind::UnrecognizedWidgetPlaceholder
            }
            Self::Other { .. } => DiagnosticKind::Other,
        }
    }

    /// Construct a fallback `Other` variant around a pre-migration
    /// string. Used by the bridge layer that still traffics in
    /// `Vec<String>` so typed consumers see a usable shape.
    pub fn other(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Other {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Best-effort parse of a legacy warning string back into a typed
    /// variant.
    ///
    /// Emit sites that still produce `Vec<String>` format via
    /// [`Display`], which makes the inverse direction a prefix match
    /// away. Unknown formats become `Diagnostic::Other` so no
    /// information is lost.
    ///
    /// This is an intentionally limited parser: consumers that need
    /// structured payloads should emit `Diagnostic` directly. It
    /// exists so [`crate::diagnostic::Diagnostic`] is a complete
    /// round-trip boundary during the migration to typed diagnostics.
    pub fn from_legacy_string(s: &str) -> Self {
        if let Some(rest) = s.strip_prefix("duplicate ID: ") {
            // `duplicate ID: "scoped" (window: wid)` or without.
            let (id_part, window_part) = match rest.split_once(" (window: ") {
                Some((id, tail)) => {
                    let wid = tail.trim_end_matches(')').to_string();
                    (
                        id.trim_matches('"').to_string(),
                        Some(wid.trim_matches('"').to_string()),
                    )
                }
                None => (rest.trim_matches('"').to_string(), None),
            };
            return Self::DuplicateId {
                id: id_part,
                window_id: window_part,
            };
        }
        if let Some(rest) = s.strip_prefix("empty_id: ") {
            let type_name = rest
                .split_once(' ')
                .map(|(t, _)| t.to_string())
                .unwrap_or_else(|| rest.to_string());
            return Self::EmptyId { type_name };
        }
        if let Some(rest) = s.strip_prefix("multiple_top_level_windows: ") {
            let ids: Vec<String> = rest
                .split_once('(')
                .and_then(|(_, tail)| tail.strip_suffix(')'))
                .map(|inner| {
                    inner
                        .split(", ")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            return Self::MultipleTopLevelWindows { window_ids: ids };
        }
        // Fallback: stable `code` prefix extracted from the legacy
        // `[code=xxx]` or `code: ...` format, else "unknown".
        let code = if let Some(rest) = s.strip_prefix("[code=") {
            rest.split_once(']')
                .map(|(c, _)| c.to_string())
                .unwrap_or_else(|| "unknown".into())
        } else if let Some((head, _)) = s.split_once(": ") {
            head.to_string()
        } else {
            "unknown".into()
        };
        Self::Other {
            code,
            message: s.to_string(),
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId {
                id,
                window_id: Some(wid),
            } => write!(f, "duplicate ID: \"{id}\" (window: {wid})"),
            Self::DuplicateId {
                id,
                window_id: None,
            } => write!(f, "duplicate ID: \"{id}\""),
            Self::EmptyId { type_name } => write!(
                f,
                "empty_id: {type_name} was declared with an empty ID; IDs must be non-empty"
            ),
            Self::MultipleTopLevelWindows { window_ids } => write!(
                f,
                "multiple_top_level_windows: tree root has more than one window child ({})",
                window_ids.join(", ")
            ),
            Self::UnknownWindow {
                window_id,
                subscription_tag,
            } => write!(
                f,
                "unknown_window: subscription \"{subscription_tag}\" targets window \
                 \"{window_id}\" which is not in the current tree"
            ),
            Self::UnrecognizedWidgetPlaceholder { id } => write!(
                f,
                "unrecognized_widget_placeholder: node id={id:?} carries `__widget__` \
                 type but no expander was registered; placeholder rendered as a no-op"
            ),
            Self::Other { message, .. } => f.write_str(message),
        }
    }
}

impl From<Diagnostic> for String {
    fn from(d: Diagnostic) -> Self {
        d.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_mirrors_legacy_duplicate_id_format() {
        let d = Diagnostic::DuplicateId {
            id: "main#form/email".into(),
            window_id: None,
        };
        assert_eq!(d.to_string(), "duplicate ID: \"main#form/email\"");
    }

    #[test]
    fn display_includes_window_when_present() {
        let d = Diagnostic::DuplicateId {
            id: "form/email".into(),
            window_id: Some("main".into()),
        };
        assert_eq!(d.to_string(), "duplicate ID: \"form/email\" (window: main)");
    }

    #[test]
    fn kind_matches_variant() {
        let d = Diagnostic::EmptyId {
            type_name: "container".into(),
        };
        assert_eq!(d.kind(), DiagnosticKind::EmptyId);
    }

    #[test]
    fn other_variant_round_trips_code_and_message() {
        let d = Diagnostic::other("prop_range_exceeded", "clamped to 1.0");
        match &d {
            Diagnostic::Other { code, message } => {
                assert_eq!(code, "prop_range_exceeded");
                assert_eq!(message, "clamped to 1.0");
            }
            other => panic!("unexpected variant {other:?}"),
        }
        assert_eq!(d.to_string(), "clamped to 1.0");
    }

    #[test]
    fn serde_round_trip() {
        let d = Diagnostic::UnknownWindow {
            window_id: "settings".into(),
            subscription_tag: "on_key_press".into(),
        };
        let json = serde_json::to_value(&d).unwrap();
        let back: Diagnostic = serde_json::from_value(json).unwrap();
        assert_eq!(d, back);
    }
}
