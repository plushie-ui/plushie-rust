//! Typed diagnostic variants emitted from tree normalization, widget
//! validation, and runtime bookkeeping.
//!
//! [`Diagnostic`] is the canonical payload shape for every diagnostic
//! emit site in the SDK, widget SDK, and renderer-lib. Variants carry
//! the structured context the emitter knew (widget ID, prop name,
//! clamped value, etc.); `Display` renders a terse single-line form
//! suitable for logs and test assertions. The type also derives
//! `Serialize` / `Deserialize` so a structured-diagnostic wire
//! channel can carry the same value unchanged if one is added later.
//!
//! The enum is `#[non_exhaustive]` so adding a new variant is not a
//! semver break. New emit sites should add a dedicated variant rather
//! than shoehorn through an existing one.

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
    /// Tree traversal hit the depth cap. Carried by
    /// [`Diagnostic::TreeDepthExceeded`].
    TreeDepthExceeded,
    /// Duplicate-ID validation short-circuited after collecting the
    /// configured maximum. Carried by [`Diagnostic::TooManyDuplicates`].
    TooManyDuplicates,
    /// A user-authored widget ID violated the canonical ID ruleset.
    /// Carried by [`Diagnostic::WidgetIdInvalid`].
    WidgetIdInvalid,
    /// A widget required an accessible name but declared none.
    /// Carried by [`Diagnostic::MissingAccessibleName`].
    MissingAccessibleName,
    /// A cross-widget a11y reference did not resolve to a declared
    /// widget. Carried by [`Diagnostic::A11yRefUnresolved`].
    A11yRefUnresolved,
    /// A numeric prop value fell outside its declared range and was
    /// clamped. Carried by [`Diagnostic::PropRangeExceeded`].
    PropRangeExceeded,
    /// A prop value had an unexpected type. Carried by
    /// [`Diagnostic::PropTypeMismatch`].
    PropTypeMismatch,
    /// A widget carried a prop name not recognised by its schema.
    /// Carried by [`Diagnostic::PropUnknown`].
    PropUnknown,
    /// A text-like content prop exceeded its per-widget byte cap.
    /// Carried by [`Diagnostic::ContentLengthExceeded`].
    ContentLengthExceeded,
    /// The leaked font-family-name cache reached its entry cap.
    /// Carried by [`Diagnostic::FontCacheCapExceeded`].
    FontCacheCapExceeded,
    /// Inline fonts declared in Settings exceeded the process-wide
    /// font load cap. Carried by [`Diagnostic::FontCapExceeded`].
    FontCapExceeded,
    /// A font family from `default_font` or its fallback chain could
    /// not be resolved. Carried by [`Diagnostic::FontFamilyNotFound`].
    FontFamilyNotFound,
    /// The Settings payload failed typed `deny_unknown_fields`
    /// validation. Carried by [`Diagnostic::InvalidSettings`].
    InvalidSettings,
    /// A non-trusted widget panicked inside the panic firewall.
    /// Carried by [`Diagnostic::WidgetPanic`].
    WidgetPanic,
    /// SVG decode returned a parse error. Carried by
    /// [`Diagnostic::SvgParseError`].
    SvgParseError,
    /// SVG decode exceeded its wall-clock budget. Carried by
    /// [`Diagnostic::SvgDecodeTimeout`].
    SvgDecodeTimeout,
    /// The leaked dash-segment cache reached its entry cap. Carried
    /// by [`Diagnostic::DashCacheCapExceeded`].
    DashCacheCapExceeded,
    /// The renderer-lib event coalesce map hit its cap and was
    /// force-flushed. Carried by
    /// [`Diagnostic::EmitterCoalesceCapExceeded`].
    EmitterCoalesceCapExceeded,
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
    /// Tree traversal reached the global depth cap. The subtree is
    /// skipped and (in normalize / render) pruned.
    TreeDepthExceeded {
        /// ID of the node whose subtree was skipped.
        id: String,
        /// The depth cap that was hit.
        max_depth: usize,
    },
    /// Duplicate-ID validation stopped collecting after reaching the
    /// configured cap. The tree may contain more duplicates than are
    /// listed.
    TooManyDuplicates {
        /// Cap at which collection was stopped.
        limit: usize,
    },
    /// A user-authored widget ID violated the canonical ID ruleset
    /// (length, ASCII-printable range, reserved characters).
    WidgetIdInvalid {
        /// Stable machine-readable reason tag. One of `too_long`,
        /// `non_ascii`, or `reserved_char`.
        reason: String,
        /// Widget type name the ID was declared on.
        type_name: String,
        /// The offending ID verbatim (may be empty or truncated in
        /// `Display` for very long cases).
        id: String,
        /// Human-readable detail sentence describing the violation.
        /// `Display` appends this to `widget_id_invalid: `.
        detail: String,
    },
    /// A widget that requires a screen-reader-announcable name was
    /// declared without one.
    MissingAccessibleName {
        /// Widget type name (e.g. `"button"`).
        type_name: String,
        /// Scoped ID of the offending widget.
        id: String,
    },
    /// A cross-widget a11y reference (`labelled_by`, `described_by`,
    /// `error_message`, `active_descendant`, or a `radio_group`
    /// entry) did not resolve to any declared widget.
    A11yRefUnresolved {
        /// Scoped ID of the widget carrying the reference.
        id: String,
        /// a11y field name. For `radio_group` entries, this is
        /// `"radio_group"`.
        key: String,
        /// Raw (pre-rewrite) reference value.
        value: String,
        /// True when the reference appeared as an element of an
        /// array-valued a11y field like `radio_group`. Used by
        /// `Display` to produce the "member" phrasing.
        is_member: bool,
    },
    /// A numeric prop was outside its declared range and clamped.
    PropRangeExceeded {
        /// Scoped ID of the widget.
        id: String,
        /// Widget type name.
        type_name: String,
        /// Prop name.
        prop: String,
        /// Raw (pre-clamp) value as it appeared on the wire.
        raw: f64,
        /// Clamped value stored back into the prop.
        clamped: f64,
        /// True when the raw value was non-finite (NaN or Inf);
        /// changes the `Display` phrasing.
        non_finite: bool,
    },
    /// A prop value had an unexpected JSON type.
    PropTypeMismatch {
        /// Scoped ID of the widget.
        id: String,
        /// Widget type name.
        type_name: String,
        /// Prop name.
        prop: String,
        /// Debug-rendered value that failed the type check.
        value_debug: String,
        /// Debug-rendered expected type enum value.
        expected_debug: String,
    },
    /// A widget carried a prop name not in its declared schema.
    PropUnknown {
        /// Scoped ID of the widget.
        id: String,
        /// Widget type name.
        type_name: String,
        /// The unexpected prop name.
        prop: String,
        /// Debug-rendered list of known prop names.
        known_debug: String,
    },
    /// A text-like content prop exceeded its per-widget byte cap and
    /// was truncated on a UTF-8 char boundary.
    ContentLengthExceeded {
        /// Widget ID whose content was truncated.
        id: String,
        /// Field name (`value`, `content`, etc.).
        field: String,
        /// Actual byte length on input.
        actual: usize,
        /// Configured cap.
        cap: usize,
        /// Byte length after truncation (may be < `cap` when the cap
        /// fell mid-codepoint).
        truncated: usize,
    },
    /// The leaked font-family-name cache reached its entry cap.
    /// Subsequent names still resolve but leak without caching.
    FontCacheCapExceeded {
        /// Cap value (max cache entries).
        max: usize,
    },
    /// Inline fonts declared in Settings exceeded the process-wide
    /// font load cap. Excess entries are dropped.
    FontCapExceeded {
        /// Process-wide font cap.
        max: u32,
        /// Entries requested in this Settings call.
        requested: u32,
        /// Entries granted (loaded).
        granted: u32,
        /// Entries dropped (`requested - granted`).
        dropped: u32,
    },
    /// A font family from `default_font` or its fallback chain did
    /// not resolve to a loaded or built-in family.
    FontFamilyNotFound {
        /// Family name that could not be resolved.
        family: String,
    },
    /// The Settings payload failed typed `deny_unknown_fields`
    /// validation. The per-field `get`-and-coerce path still runs so
    /// partial settings take effect.
    InvalidSettings {
        /// Detail from the serde decode error.
        detail: String,
    },
    /// A non-trusted widget panicked inside the registry's
    /// catch_unwind firewall. The renderer ignores the widget's
    /// contribution for this call and continues.
    WidgetPanic {
        /// Scoped ID of the panicking node.
        id: String,
        /// Widget type name.
        type_name: String,
        /// Human-readable method label (e.g. `"prepare"`, `"render"`).
        label: String,
    },
    /// SVG decode returned a parse error.
    SvgParseError {
        /// Scoped ID of the SVG widget.
        id: String,
        /// Source path or identifier that failed.
        source: String,
        /// Detail from the parser.
        detail: String,
    },
    /// SVG decode exceeded its wall-clock budget.
    SvgDecodeTimeout {
        /// Scoped ID of the SVG widget.
        id: String,
        /// Source path or identifier that timed out.
        source: String,
        /// Deadline that was exceeded, rendered exactly as `{:?}` on
        /// the original `std::time::Duration` for Display parity.
        deadline_debug: String,
    },
    /// The leaked dash-segment cache reached its entry cap.
    DashCacheCapExceeded {
        /// Cap value (max cache entries).
        max: usize,
    },
    /// The renderer-lib event coalesce map hit its cap and was
    /// force-flushed to keep memory bounded.
    EmitterCoalesceCapExceeded {
        /// Cap value (max pending entries).
        cap: usize,
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
            Self::TreeDepthExceeded { .. } => DiagnosticKind::TreeDepthExceeded,
            Self::TooManyDuplicates { .. } => DiagnosticKind::TooManyDuplicates,
            Self::WidgetIdInvalid { .. } => DiagnosticKind::WidgetIdInvalid,
            Self::MissingAccessibleName { .. } => DiagnosticKind::MissingAccessibleName,
            Self::A11yRefUnresolved { .. } => DiagnosticKind::A11yRefUnresolved,
            Self::PropRangeExceeded { .. } => DiagnosticKind::PropRangeExceeded,
            Self::PropTypeMismatch { .. } => DiagnosticKind::PropTypeMismatch,
            Self::PropUnknown { .. } => DiagnosticKind::PropUnknown,
            Self::ContentLengthExceeded { .. } => DiagnosticKind::ContentLengthExceeded,
            Self::FontCacheCapExceeded { .. } => DiagnosticKind::FontCacheCapExceeded,
            Self::FontCapExceeded { .. } => DiagnosticKind::FontCapExceeded,
            Self::FontFamilyNotFound { .. } => DiagnosticKind::FontFamilyNotFound,
            Self::InvalidSettings { .. } => DiagnosticKind::InvalidSettings,
            Self::WidgetPanic { .. } => DiagnosticKind::WidgetPanic,
            Self::SvgParseError { .. } => DiagnosticKind::SvgParseError,
            Self::SvgDecodeTimeout { .. } => DiagnosticKind::SvgDecodeTimeout,
            Self::DashCacheCapExceeded { .. } => DiagnosticKind::DashCacheCapExceeded,
            Self::EmitterCoalesceCapExceeded { .. } => DiagnosticKind::EmitterCoalesceCapExceeded,
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId {
                id,
                window_id: Some(wid),
            } => write!(f, "duplicate_id: {id} (window {wid})"),
            Self::DuplicateId {
                id,
                window_id: None,
            } => write!(f, "duplicate_id: {id}"),
            Self::EmptyId { type_name } => {
                write!(f, "empty_id: {type_name} declared with empty id")
            }
            Self::MultipleTopLevelWindows { window_ids } => {
                write!(f, "multiple_top_level_windows: [{}]", window_ids.join(", "))
            }
            Self::UnknownWindow {
                window_id,
                subscription_tag,
            } => write!(
                f,
                "unknown_window: subscription {subscription_tag} targets {window_id} \
                 which is not in the tree"
            ),
            Self::UnrecognizedWidgetPlaceholder { id } => write!(
                f,
                "unrecognized_widget_placeholder: {id} has no registered expander"
            ),
            Self::TreeDepthExceeded { id, max_depth } => write!(
                f,
                "tree_depth_exceeded: subtree at {id} exceeds MAX_TREE_DEPTH={max_depth}"
            ),
            Self::TooManyDuplicates { limit } => {
                write!(f, "too_many_duplicates: stopped at {limit}")
            }
            Self::WidgetIdInvalid {
                reason,
                type_name,
                id,
                detail,
            } => write!(
                f,
                "widget_id_invalid: {type_name} id={id:?} ({reason}): {detail}"
            ),
            Self::MissingAccessibleName { type_name, id } => write!(
                f,
                "missing_accessible_name: {type_name} {id} has no label, text child, \
                 a11y.label, or a11y.labelled_by"
            ),
            Self::A11yRefUnresolved {
                id,
                key,
                value,
                is_member,
            } => {
                if *is_member {
                    write!(
                        f,
                        "a11y_ref_unresolved: {id} {key} member {value:?} is not a \
                         declared widget id"
                    )
                } else {
                    write!(
                        f,
                        "a11y_ref_unresolved: {id} {key}={value:?} is not a declared \
                         widget id"
                    )
                }
            }
            Self::PropRangeExceeded {
                id,
                type_name,
                prop,
                raw,
                clamped,
                non_finite,
            } => {
                let cause = if *non_finite {
                    "non-finite"
                } else {
                    "out of range"
                };
                write!(
                    f,
                    "prop_range_exceeded: {type_name} {id} prop {prop}={raw} \
                     ({cause}), clamped to {clamped}"
                )
            }
            Self::PropTypeMismatch {
                id,
                type_name,
                prop,
                value_debug,
                expected_debug,
            } => write!(
                f,
                "prop_type_mismatch: {type_name} {id} prop {prop} got {value_debug}, \
                 expected {expected_debug}"
            ),
            Self::PropUnknown {
                id,
                type_name,
                prop,
                known_debug,
            } => write!(
                f,
                "prop_unknown: {type_name} {id} has no prop {prop:?} (known: {known_debug})"
            ),
            Self::ContentLengthExceeded {
                id,
                field,
                actual,
                cap,
                truncated,
            } => write!(
                f,
                "content_length_exceeded: {id}.{field} = {actual} bytes, cap {cap}, \
                 truncated to {truncated}"
            ),
            Self::FontCacheCapExceeded { max } => write!(
                f,
                "font_cache_cap_exceeded: cache full ({max} entries); new names leak uncached"
            ),
            Self::FontCapExceeded {
                max,
                requested,
                granted,
                dropped,
            } => write!(
                f,
                "font_cap_exceeded: {requested} requested, {granted} granted, {dropped} \
                 dropped (max {max})"
            ),
            Self::FontFamilyNotFound { family } => {
                write!(f, "font_family_not_found: {family}")
            }
            Self::InvalidSettings { detail } => {
                write!(f, "invalid_settings: {detail}")
            }
            Self::WidgetPanic {
                id,
                type_name,
                label,
            } => write!(f, "widget_panic: {type_name} {id} panicked in {label}"),
            Self::SvgParseError { id, source, detail } => {
                write!(f, "svg_parse_error: {id} {source:?}: {detail}")
            }
            Self::SvgDecodeTimeout {
                id,
                source,
                deadline_debug,
            } => write!(
                f,
                "svg_decode_timeout: {id} {source:?} exceeded {deadline_debug}"
            ),
            Self::DashCacheCapExceeded { max } => write!(
                f,
                "dash_cache_cap_exceeded: cache full ({max} entries); new patterns leak uncached"
            ),
            Self::EmitterCoalesceCapExceeded { cap } => write!(
                f,
                "emitter_coalesce_cap_exceeded: pending map hit cap ({cap}); flushing"
            ),
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
    fn duplicate_id_display() {
        let plain = Diagnostic::DuplicateId {
            id: "main#form/email".into(),
            window_id: None,
        };
        assert_eq!(plain.to_string(), "duplicate_id: main#form/email");

        let scoped = Diagnostic::DuplicateId {
            id: "form/email".into(),
            window_id: Some("main".into()),
        };
        assert_eq!(scoped.to_string(), "duplicate_id: form/email (window main)");
    }

    #[test]
    fn kind_matches_variant() {
        let d = Diagnostic::EmptyId {
            type_name: "container".into(),
        };
        assert_eq!(d.kind(), DiagnosticKind::EmptyId);
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

    #[test]
    fn tree_depth_exceeded_display() {
        let d = Diagnostic::TreeDepthExceeded {
            id: "root".into(),
            max_depth: 256,
        };
        assert_eq!(
            d.to_string(),
            "tree_depth_exceeded: subtree at root exceeds MAX_TREE_DEPTH=256"
        );
    }

    #[test]
    fn widget_id_invalid_display_carries_reason_and_detail() {
        let d = Diagnostic::WidgetIdInvalid {
            reason: "reserved_char".into(),
            type_name: "text_input".into(),
            id: "form/field".into(),
            detail: "'/' is reserved for scoping".into(),
        };
        assert_eq!(
            d.to_string(),
            "widget_id_invalid: text_input id=\"form/field\" (reserved_char): \
             '/' is reserved for scoping"
        );
    }

    #[test]
    fn prop_range_exceeded_display() {
        let oob = Diagnostic::PropRangeExceeded {
            id: "slider-1".into(),
            type_name: "slider".into(),
            prop: "value".into(),
            raw: 200.0,
            clamped: 100.0,
            non_finite: false,
        };
        assert_eq!(
            oob.to_string(),
            "prop_range_exceeded: slider slider-1 prop value=200 (out of range), clamped to 100"
        );

        let non_finite = Diagnostic::PropRangeExceeded {
            id: "slider-1".into(),
            type_name: "slider".into(),
            prop: "value".into(),
            raw: f64::INFINITY,
            clamped: 0.0,
            non_finite: true,
        };
        assert!(non_finite.to_string().contains("(non-finite)"));
    }

    #[test]
    fn content_length_exceeded_display() {
        let d = Diagnostic::ContentLengthExceeded {
            id: "input".into(),
            field: "value".into(),
            actual: 100_000,
            cap: 65_536,
            truncated: 65_535,
        };
        assert_eq!(
            d.to_string(),
            "content_length_exceeded: input.value = 100000 bytes, cap 65536, truncated to 65535"
        );
    }

    #[test]
    fn widget_panic_display() {
        let d = Diagnostic::WidgetPanic {
            id: "btn".into(),
            type_name: "custom_button".into(),
            label: "render".into(),
        };
        assert_eq!(
            d.to_string(),
            "widget_panic: custom_button btn panicked in render"
        );
    }

    #[test]
    fn font_cap_exceeded_display() {
        let d = Diagnostic::FontCapExceeded {
            max: 256,
            requested: 10,
            granted: 3,
            dropped: 7,
        };
        assert_eq!(
            d.to_string(),
            "font_cap_exceeded: 10 requested, 3 granted, 7 dropped (max 256)"
        );
    }

    #[test]
    fn a11y_ref_unresolved_switches_phrasing_on_is_member() {
        let single = Diagnostic::A11yRefUnresolved {
            id: "r1".into(),
            key: "labelled_by".into(),
            value: "missing".into(),
            is_member: false,
        };
        assert_eq!(
            single.to_string(),
            "a11y_ref_unresolved: r1 labelled_by=\"missing\" is not a declared widget id"
        );

        let member = Diagnostic::A11yRefUnresolved {
            id: "r1".into(),
            key: "radio_group".into(),
            value: "missing".into(),
            is_member: true,
        };
        assert_eq!(
            member.to_string(),
            "a11y_ref_unresolved: r1 radio_group member \"missing\" is not a declared widget id"
        );
    }

    #[test]
    fn kind_for_new_variants_is_unique() {
        let tde = Diagnostic::TreeDepthExceeded {
            id: "x".into(),
            max_depth: 256,
        };
        assert_eq!(tde.kind(), DiagnosticKind::TreeDepthExceeded);
        let wid = Diagnostic::WidgetIdInvalid {
            reason: "r".into(),
            type_name: "t".into(),
            id: "i".into(),
            detail: "d".into(),
        };
        assert_eq!(wid.kind(), DiagnosticKind::WidgetIdInvalid);
    }
}
