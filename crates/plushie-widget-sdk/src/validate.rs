//! Prop validation for tree nodes.
//!
//! When enabled, [`validate_props`] checks each node's props against a
//! schema of expected property names and types. Unexpected property
//! names or type mismatches are emitted as prop_validation events over
//! the wire so the SDK can detect and report them.
//!
//! Validation is opt-in via `validate_props: true` in the Settings
//! message. SDKs can opt in via `validate_props: true` in the Settings message.

use std::sync::OnceLock;

use serde_json::Value;

use crate::protocol::TreeNode;

/// Props accepted by all widget types (skipped during per-widget validation).
const UNIVERSAL_PROPS: &[&str] = &["a11y", "event_rate", "id"];

/// Global flag to enable prop validation in release builds.
/// Set via `set_validate_props(true)` during settings init.
static VALIDATE_PROPS: OnceLock<bool> = OnceLock::new();

/// Enable or disable prop validation at runtime. Called once during
/// settings initialization. Returns false if already set.
pub fn set_validate_props(enabled: bool) -> bool {
    VALIDATE_PROPS.set(enabled).is_ok()
}

/// Returns true if prop validation is enabled.
///
/// Debug builds auto-enable validation by default so prop warnings
/// surface during development without the host SDK having to opt
/// in. Release builds require explicit `validate_props: true` in
/// Settings.
pub fn is_validate_props_enabled() -> bool {
    if let Some(v) = VALIDATE_PROPS.get() {
        return *v;
    }
    cfg!(debug_assertions)
}

#[derive(Debug, Clone, Copy)]
enum PropType {
    Str,
    Number,
    Bool,
    Array,
    Color,
    Length,
    OneOf(&'static [&'static str]),
    Any,
}

fn prop_type_matches(val: &Value, expected: PropType) -> bool {
    match expected {
        PropType::Str => val.is_string(),
        PropType::Number => val.is_number(),
        PropType::Bool => val.is_boolean(),
        PropType::Array => val.is_array(),
        PropType::Color => val.is_string() || val.is_object(),
        PropType::Length => val.is_string() || val.is_number(),
        PropType::OneOf(values) => val.as_str().is_some_and(|s| values.contains(&s)),
        PropType::Any => true,
    }
}

fn prop_type_expected_debug(expected: PropType) -> String {
    match expected {
        PropType::Str => "string".to_string(),
        PropType::Number => "number".to_string(),
        PropType::Bool => "boolean".to_string(),
        PropType::Array => "array".to_string(),
        PropType::Color => "string or object color".to_string(),
        PropType::Length => "string or number length".to_string(),
        PropType::OneOf(values) => format!("one of {values:?}"),
        PropType::Any => "any".to_string(),
    }
}

const WINDOW_LEVEL_VALUES: &[&str] = &["normal", "always_on_top", "always_on_bottom"];
const TEXT_ALIGNMENT_VALUES: &[&str] = &[
    "default",
    "left",
    "center",
    "right",
    "start",
    "end",
    "justified",
];
const HORIZONTAL_ALIGNMENT_VALUES: &[&str] = &["left", "center", "right"];
const VERTICAL_ALIGNMENT_VALUES: &[&str] = &["top", "center", "bottom"];
const TEXT_DIRECTION_VALUES: &[&str] = &["auto", "ltr", "rtl"];
const WRAPPING_VALUES: &[&str] = &["none", "word", "glyph", "word_or_glyph"];
const SHAPING_VALUES: &[&str] = &["basic", "advanced", "auto"];
const ELLIPSIS_VALUES: &[&str] = &["none", "start", "middle", "end"];
const INPUT_PURPOSE_VALUES: &[&str] = &[
    "normal", "secure", "terminal", "number", "decimal", "phone", "email", "url", "search",
];
const CONTENT_FIT_VALUES: &[&str] = &["contain", "cover", "fill", "scale_down", "none"];
const FILTER_METHOD_VALUES: &[&str] = &["nearest", "linear"];
const DIRECTION_VALUES: &[&str] = &["horizontal", "vertical", "both"];
const ANCHOR_VALUES: &[&str] = &["start", "end"];
const TOOLTIP_POSITION_VALUES: &[&str] =
    &["top", "bottom", "left", "right", "follow", "follow_cursor"];
const CURSOR_STYLE_VALUES: &[&str] = &[
    "pointer",
    "grab",
    "grabbing",
    "crosshair",
    "text",
    "move",
    "not_allowed",
    "progress",
    "wait",
    "help",
    "cell",
    "copy",
    "alias",
    "no_drop",
    "all_scroll",
    "zoom_in",
    "zoom_out",
    "context_menu",
    "resizing_horizontally",
    "resizing_vertically",
    "resizing_diagonally_up",
    "resizing_diagonally_down",
    "resizing_column",
    "resizing_row",
];
const RULE_DIRECTION_VALUES: &[&str] = &["horizontal", "vertical"];
const POSITION_VALUES: &[&str] = &["below", "above", "left", "right"];
const OVERLAY_ALIGN_VALUES: &[&str] = &["start", "center", "end"];
const SORT_ORDER_VALUES: &[&str] = &["asc", "desc"];
const SPLIT_AXIS_VALUES: &[&str] = &["horizontal", "vertical"];
const ERROR_CORRECTION_VALUES: &[&str] = &["low", "medium", "quartile", "high"];
const ARROW_MODE_VALUES: &[&str] = &["wrap", "clamp", "linear", "none"];
const HIGHLIGHT_THEME_VALUES: &[&str] = &[
    "base16_mocha",
    "base16_ocean",
    "base16_eighties",
    "solarized_dark",
    "inspired_github",
];
const ROLE_VALUES: &[&str] = &[
    "alert",
    "alert_dialog",
    "button",
    "canvas",
    "check_box",
    "cell",
    "table_cell",
    "column_header",
    "combo_box",
    "dialog",
    "document",
    "container",
    "generic",
    "generic_container",
    "group",
    "heading",
    "image",
    "label",
    "link",
    "list",
    "list_item",
    "menu",
    "menu_bar",
    "menu_item",
    "meter",
    "text_editor",
    "multiline_text_input",
    "navigation",
    "progress_bar",
    "progress_indicator",
    "radio",
    "radio_button",
    "radio_group",
    "region",
    "row",
    "table_row",
    "scroll_bar",
    "scroll_view",
    "search",
    "separator",
    "slider",
    "static_text",
    "status",
    "switch",
    "tab",
    "tab_list",
    "tab_panel",
    "table",
    "text_input",
    "toolbar",
    "tooltip",
    "tree",
    "tree_item",
    "window",
];

/// Numeric range constraint applied to a prop value. `None` on either
/// end means unbounded in that direction.
#[derive(Debug, Clone, Copy, Default)]
struct NumericRange {
    min: Option<f64>,
    max: Option<f64>,
}

impl NumericRange {
    const fn min(min: f64) -> Self {
        Self {
            min: Some(min),
            max: None,
        }
    }

    const fn min_max(min: f64, max: f64) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
        }
    }

    /// Legitimate negative values (shadow offsets, translate offsets)
    /// bypass the >= 0 check for Length props that generally should
    /// be non-negative.
    const fn any() -> Self {
        Self {
            min: None,
            max: None,
        }
    }
}

/// Per-widget range constraints. Keyed by (type_name, prop_name).
///
/// Only props whose out-of-range values cause real problems downstream
/// are listed. Props with clear semantics around negatives (shadow
/// offsets, translate offsets) are deliberately excluded.
fn range_for(type_name: &str, prop_name: &str) -> Option<NumericRange> {
    // Window dimensions: clamp to a reasonable pixel range. Values
    // wildly outside this range tend to crash native window managers
    // or produce garbage output.
    const WINDOW_DIMS: &[&str] = &[
        "width",
        "height",
        "max_width",
        "max_height",
        "min_width",
        "min_height",
    ];
    if type_name == "window" && WINDOW_DIMS.contains(&prop_name) {
        return Some(NumericRange::min_max(0.0, 32767.0));
    }

    // Fonts: >= 0 and a sanity cap well above any real use.
    if prop_name == "font_size" || prop_name == "text_size" {
        return Some(NumericRange::min_max(0.0, 1024.0));
    }

    // Common layout props that should never be negative.
    match prop_name {
        "spacing" | "size" | "width_fraction" | "line_height" | "scale" | "opacity"
        | "scale_factor" => Some(NumericRange::min(0.0)),
        _ => None,
    }
}

/// Check a numeric prop value against its declared range. Returns a
/// warning string when out of range and the clamped value that should
/// replace the raw one. Caller decides whether to apply the clamp.
fn check_numeric_range(
    node_id: &str,
    type_name: &str,
    prop_name: &str,
    val: &Value,
    range: NumericRange,
) -> Option<(String, f64)> {
    let raw = val.as_f64()?;
    if !raw.is_finite() {
        // Non-finite values should have been filtered upstream. Treat
        // them as out-of-range and clamp to zero.
        let clamped = 0.0;
        let diag = plushie_core::Diagnostic::PropRangeExceeded {
            id: node_id.to_string(),
            type_name: type_name.to_string(),
            prop: prop_name.to_string(),
            raw,
            clamped,
            non_finite: true,
        };
        return Some((diag.to_string(), clamped));
    }
    let mut clamped = raw;
    if let Some(min) = range.min
        && raw < min
    {
        clamped = min;
    }
    if let Some(max) = range.max
        && raw > max
    {
        clamped = max;
    }
    if clamped != raw {
        let diag = plushie_core::Diagnostic::PropRangeExceeded {
            id: node_id.to_string(),
            type_name: type_name.to_string(),
            prop: prop_name.to_string(),
            raw,
            clamped,
            non_finite: false,
        };
        Some((diag.to_string(), clamped))
    } else {
        None
    }
}

#[allow(dead_code)] // consumed by tests and future enforcement layers
pub(crate) fn numeric_range_for_prop(
    type_name: &str,
    prop_name: &str,
) -> Option<(Option<f64>, Option<f64>)> {
    range_for(type_name, prop_name).map(|r| (r.min, r.max))
}

/// Widget type names covered by the built-in validation schema.
///
/// Exposed for drift detection: CI tests can cross-check this list
/// against the registry's active type names so a new widget cannot
/// ship without a corresponding schema entry.
#[allow(dead_code)] // consumed by the drift test in `tests`
pub const VALIDATED_WIDGET_TYPES: &[&str] = &[
    "window",
    "button",
    "text",
    "column",
    "row",
    "container",
    "text_input",
    "slider",
    "checkbox",
    "toggler",
    "progress_bar",
    "image",
    "svg",
    "scrollable",
    "grid",
    "radio",
    "tooltip",
    "pointer_area",
    "sensor",
    "space",
    "rule",
    "pick_list",
    "combo_box",
    "text_editor",
    "overlay",
    "themer",
    "stack",
    "pin",
    "floating",
    "float",
    "keyed_column",
    "responsive",
    "rich_text",
    "rich",
    "vertical_slider",
    "table",
    "pane_grid",
    "markdown",
    "canvas",
    "qr_code",
];

/// Collect prop validation warnings for a node without logging them.
///
/// Returns a list of human-readable warning strings. Useful for testing
/// and for callers that want to inspect warnings programmatically.
pub fn collect_prop_warnings(node: &TreeNode) -> Vec<String> {
    use PropType::*;

    let expected: &[(&str, PropType)] = match node.type_name.as_str() {
        "window" => &[
            ("title", Str),
            ("width", Number),
            ("height", Number),
            ("padding", Any),
            ("scale_factor", Number),
            ("position", Any),
            ("min_size", Any),
            ("max_size", Any),
            ("maximized", Bool),
            ("fullscreen", Bool),
            ("visible", Bool),
            ("resizable", Bool),
            ("closeable", Bool),
            ("minimizable", Bool),
            ("decorations", Bool),
            ("transparent", Bool),
            ("blur", Bool),
            ("level", OneOf(WINDOW_LEVEL_VALUES)),
            ("exit_on_close_request", Bool),
            ("size", Any),
        ],
        "button" => &[
            ("label", Str),
            ("content", Str),
            ("style", Any),
            ("width", Length),
            ("height", Length),
            ("padding", Any),
            ("clip", Bool),
            ("disabled", Bool),
            ("enabled", Bool),
            ("mnemonic", Str),
            ("access_key", Str),
        ],
        "text" => &[
            ("content", Str),
            ("size", Number),
            ("color", Color),
            ("font", Any),
            ("width", Length),
            ("height", Length),
            ("align_x", OneOf(TEXT_ALIGNMENT_VALUES)),
            ("align_y", OneOf(VERTICAL_ALIGNMENT_VALUES)),
            ("text_direction", OneOf(TEXT_DIRECTION_VALUES)),
            ("line_height", Number),
            ("shaping", OneOf(SHAPING_VALUES)),
            ("wrapping", OneOf(WRAPPING_VALUES)),
            ("ellipsis", OneOf(ELLIPSIS_VALUES)),
            ("style", Str),
        ],
        "column" => &[
            ("spacing", Number),
            ("padding", Any),
            ("width", Length),
            ("height", Length),
            ("max_width", Number),
            ("align_x", OneOf(HORIZONTAL_ALIGNMENT_VALUES)),
            ("clip", Bool),
            ("wrap", Bool),
        ],
        "row" => &[
            ("spacing", Number),
            ("padding", Any),
            ("width", Length),
            ("height", Length),
            ("max_width", Number),
            ("max_height", Number),
            ("align_y", OneOf(VERTICAL_ALIGNMENT_VALUES)),
            ("clip", Bool),
            ("wrap", Bool),
        ],
        "container" => &[
            ("padding", Any),
            ("width", Length),
            ("height", Length),
            ("max_width", Number),
            ("max_height", Number),
            ("center", Bool),
            ("align_x", OneOf(HORIZONTAL_ALIGNMENT_VALUES)),
            ("align_y", OneOf(VERTICAL_ALIGNMENT_VALUES)),
            ("clip", Bool),
            ("style", Any),
            ("background", Any),
            ("color", Color),
            ("border", Any),
            ("shadow", Any),
        ],
        "text_input" => &[
            ("value", Str),
            ("placeholder", Str),
            ("font", Any),
            ("width", Length),
            ("padding", Any),
            ("size", Number),
            ("line_height", Number),
            ("secure", Bool),
            ("style", Any),
            ("icon", Any),
            ("disabled", Bool),
            ("on_submit", Bool),
            ("on_paste", Bool),
            ("align_x", OneOf(HORIZONTAL_ALIGNMENT_VALUES)),
            ("placeholder_color", Color),
            ("selection_color", Color),
            ("input_purpose", OneOf(INPUT_PURPOSE_VALUES)),
            ("ime_purpose", OneOf(INPUT_PURPOSE_VALUES)),
        ],
        "slider" => &[
            ("value", Number),
            ("range", Array),
            ("step", Number),
            ("keyboard_step", Number),
            ("width", Length),
            ("height", Number),
            ("style", Any),
            ("shift_step", Number),
            ("default", Number),
            ("rail_color", Color),
            ("rail_width", Number),
            ("circular_handle", Bool),
            ("handle_radius", Number),
            ("label", Str),
        ],
        "checkbox" => &[
            ("label", Str),
            ("checked", Bool),
            ("size", Number),
            ("font", Any),
            ("text_size", Number),
            ("spacing", Number),
            ("width", Length),
            ("style", Any),
            ("icon", Any),
            ("disabled", Bool),
            ("mnemonic", Str),
            ("access_key", Str),
            ("line_height", Number),
            ("wrapping", OneOf(WRAPPING_VALUES)),
            ("shaping", OneOf(SHAPING_VALUES)),
        ],
        "toggler" => &[
            ("label", Str),
            ("is_toggled", Bool),
            ("size", Number),
            ("font", Any),
            ("text_size", Number),
            ("spacing", Number),
            ("width", Length),
            ("style", Any),
            ("disabled", Bool),
            ("line_height", Number),
            ("wrapping", OneOf(WRAPPING_VALUES)),
            ("shaping", OneOf(SHAPING_VALUES)),
            ("text_alignment", OneOf(HORIZONTAL_ALIGNMENT_VALUES)),
        ],
        "progress_bar" => &[
            ("value", Number),
            ("range", Array),
            ("width", Length),
            ("height", Length),
            ("style", Any),
            ("vertical", Bool),
            ("label", Str),
        ],
        "image" => &[
            ("source", Any),
            ("width", Length),
            ("height", Length),
            ("content_fit", OneOf(CONTENT_FIT_VALUES)),
            ("filter_method", OneOf(FILTER_METHOD_VALUES)),
            ("rotation", Any),
            ("opacity", Number),
            ("border_radius", Number),
            ("expand", Bool),
            ("scale", Number),
            ("alt", Str),
            ("description", Str),
            ("decorative", Bool),
            ("crop", Any),
        ],
        "svg" => &[
            ("source", Str),
            ("width", Length),
            ("height", Length),
            ("content_fit", OneOf(CONTENT_FIT_VALUES)),
            ("rotation", Any),
            ("opacity", Number),
            ("color", Color),
            ("alt", Str),
            ("description", Str),
            ("decorative", Bool),
        ],
        "scrollable" => &[
            ("width", Length),
            ("height", Length),
            ("direction", OneOf(DIRECTION_VALUES)),
            ("style", Any),
            ("anchor", OneOf(ANCHOR_VALUES)),
            ("spacing", Number),
            ("scrollbar_width", Number),
            ("scrollbar_margin", Number),
            ("scroller_width", Number),
            ("scrollbar_color", Color),
            ("scroller_color", Color),
            ("auto_scroll", Bool),
            ("on_scroll", Bool),
        ],
        "grid" => &[
            ("num_columns", Number),
            ("spacing", Number),
            ("width", Length),
            ("height", Length),
            ("column_width", Length),
            ("row_height", Length),
            ("fluid", Number),
        ],
        "radio" => &[
            ("label", Str),
            ("value", Str),
            ("selected", Any),
            ("group", Str),
            ("size", Number),
            ("font", Any),
            ("text_size", Number),
            ("spacing", Number),
            ("width", Length),
            ("style", Any),
            ("mnemonic", Str),
            ("access_key", Str),
            ("line_height", Number),
            ("wrapping", OneOf(WRAPPING_VALUES)),
            ("shaping", OneOf(SHAPING_VALUES)),
        ],
        "tooltip" => &[
            ("tip", Str),
            ("position", OneOf(TOOLTIP_POSITION_VALUES)),
            ("gap", Number),
            ("padding", Number),
            ("snap_within_viewport", Bool),
            ("delay", Number),
            ("style", Any),
        ],
        "pointer_area" => &[
            ("cursor", OneOf(CURSOR_STYLE_VALUES)),
            ("on_press", Bool),
            ("on_release", Bool),
            ("on_right_press", Bool),
            ("on_right_release", Bool),
            ("on_middle_press", Bool),
            ("on_middle_release", Bool),
            ("on_double_click", Bool),
            ("on_enter", Bool),
            ("on_exit", Bool),
            ("on_move", Bool),
            ("on_scroll", Bool),
        ],
        "sensor" => &[
            ("delay", Number),
            ("anticipate", Number),
            ("on_resize", Bool),
        ],
        "space" => &[("width", Length), ("height", Length)],
        "rule" => &[
            ("direction", OneOf(RULE_DIRECTION_VALUES)),
            ("width", Number),
            ("height", Number),
            ("thickness", Number),
            ("style", Any),
        ],
        "pick_list" => &[
            ("options", Array),
            ("selected", Str),
            ("placeholder", Str),
            ("width", Length),
            ("padding", Any),
            ("text_size", Number),
            ("font", Any),
            ("menu_height", Number),
            ("line_height", Number),
            ("shaping", OneOf(SHAPING_VALUES)),
            ("handle", Any),
            ("ellipsis", OneOf(ELLIPSIS_VALUES)),
            ("menu_style", Any),
            ("style", Any),
            ("on_open", Bool),
            ("on_close", Bool),
        ],
        "combo_box" => &[
            ("selected", Str),
            ("placeholder", Str),
            ("options", Array),
            ("width", Length),
            ("padding", Any),
            ("size", Number),
            ("font", Any),
            ("line_height", Number),
            ("shaping", OneOf(SHAPING_VALUES)),
            ("menu_height", Number),
            ("icon", Any),
            ("on_option_hovered", Bool),
            ("on_open", Bool),
            ("on_close", Bool),
            ("on_submit", Bool),
            ("ellipsis", OneOf(ELLIPSIS_VALUES)),
            ("menu_style", Any),
            ("style", Any),
        ],
        "text_editor" => &[
            ("content", Str),
            ("placeholder", Str),
            ("height", Length),
            ("width", Length),
            ("min_height", Number),
            ("max_height", Number),
            ("size", Number),
            ("font", Any),
            ("line_height", Number),
            ("padding", Any),
            ("wrapping", OneOf(WRAPPING_VALUES)),
            ("text_direction", OneOf(TEXT_DIRECTION_VALUES)),
            ("key_bindings", Array),
            ("style", Any),
            ("highlight_syntax", Str),
            ("highlight_theme", OneOf(HIGHLIGHT_THEME_VALUES)),
            ("placeholder_color", Color),
            ("selection_color", Color),
            ("input_purpose", OneOf(INPUT_PURPOSE_VALUES)),
            ("ime_purpose", OneOf(INPUT_PURPOSE_VALUES)),
        ],
        "overlay" => &[
            ("position", OneOf(POSITION_VALUES)),
            ("gap", Number),
            ("offset_x", Number),
            ("offset_y", Number),
            ("flip", Bool),
            ("align", OneOf(OVERLAY_ALIGN_VALUES)),
            ("width", Length),
        ],
        "themer" => &[("theme", Any)],
        "stack" => &[
            ("width", Length),
            ("height", Length),
            ("padding", Any),
            ("clip", Bool),
        ],
        "pin" => &[
            ("x", Number),
            ("y", Number),
            ("width", Length),
            ("height", Length),
        ],
        "floating" | "float" => &[
            ("translate_x", Number),
            ("translate_y", Number),
            ("scale", Number),
            ("width", Length),
            ("height", Length),
        ],
        "keyed_column" => &[
            ("spacing", Number),
            ("padding", Any),
            ("width", Length),
            ("height", Length),
            ("max_width", Number),
            ("align_x", OneOf(HORIZONTAL_ALIGNMENT_VALUES)),
        ],
        "responsive" => &[("width", Length), ("height", Length)],
        "rich_text" => &[
            ("spans", Array),
            ("size", Number),
            ("font", Any),
            ("color", Color),
            ("width", Length),
            ("height", Length),
            ("line_height", Number),
            ("wrapping", OneOf(WRAPPING_VALUES)),
            ("ellipsis", OneOf(ELLIPSIS_VALUES)),
        ],
        "vertical_slider" => &[
            ("value", Number),
            ("range", Array),
            ("step", Number),
            ("keyboard_step", Number),
            ("width", Number),
            ("height", Length),
            ("style", Any),
            ("shift_step", Number),
            ("default", Number),
            ("rail_color", Color),
            ("rail_width", Number),
            ("label", Str),
        ],
        "table" => &[
            ("columns", Array),
            ("rows", Array),
            ("width", Length),
            ("height", Length),
            ("header", Bool),
            ("separator", Bool),
            ("padding", Any),
            ("sort_by", Str),
            ("sort_order", OneOf(SORT_ORDER_VALUES)),
            ("header_text_size", Number),
            ("row_text_size", Number),
            ("cell_spacing", Number),
            ("row_spacing", Number),
            ("separator_thickness", Number),
        ],
        "pane_grid" => &[
            ("panes", Any),
            ("spacing", Number),
            ("width", Length),
            ("height", Length),
            ("min_size", Number),
            ("leeway", Number),
            ("divider_color", Color),
            ("divider_width", Number),
            ("split_axis", OneOf(SPLIT_AXIS_VALUES)),
        ],
        "markdown" => &[
            ("content", Str),
            ("text_size", Number),
            ("h1_size", Number),
            ("h2_size", Number),
            ("h3_size", Number),
            ("code_size", Number),
            ("spacing", Number),
            ("width", Length),
            ("link_color", Color),
            ("code_theme", OneOf(HIGHLIGHT_THEME_VALUES)),
            ("style", Any),
        ],
        "canvas" => &[
            ("layers", Any),
            ("shapes", Any),
            ("background", Color),
            ("width", Length),
            ("height", Length),
            ("interactive", Any),
            ("on_press", Bool),
            ("on_release", Bool),
            ("on_move", Bool),
            ("on_scroll", Bool),
            ("alt", Str),
            ("description", Str),
            ("role", OneOf(ROLE_VALUES)),
            ("arrow_mode", OneOf(ARROW_MODE_VALUES)),
            ("event_rate", Number),
            ("a11y", Any),
        ],
        "qr_code" => &[
            ("data", Str),
            ("cell_size", Number),
            ("total_size", Number),
            ("error_correction", OneOf(ERROR_CORRECTION_VALUES)),
            ("cell_color", Color),
            ("background", Color),
            ("alt", Str),
            ("description", Str),
            ("style", Any),
        ],
        _ => return Vec::new(), // Unknown widget type, skip validation
    };

    let props_cow = node.props.as_value_cow();
    let props = match props_cow.as_object() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let expected_names: Vec<&str> = expected.iter().map(|(name, _)| *name).collect();
    let mut warnings = Vec::new();

    for (key, val) in props {
        // Skip props accepted by all widget types.
        if UNIVERSAL_PROPS.contains(&key.as_str()) {
            continue;
        }
        match expected.iter().find(|(name, _)| name == key) {
            Some((_, expected_type)) => {
                if !prop_type_matches(val, *expected_type) {
                    let diag = plushie_core::Diagnostic::PropTypeMismatch {
                        id: node.id.clone(),
                        type_name: node.type_name.clone(),
                        prop: key.clone(),
                        value_debug: format!("{val:?}"),
                        expected_debug: prop_type_expected_debug(*expected_type),
                    };
                    warnings.push(diag.to_string());
                }
                // Range check only applies to numeric values; string
                // lengths are validated elsewhere.
                if let Some(range) = range_for(&node.type_name, key)
                    && let Some((msg, _clamped)) =
                        check_numeric_range(&node.id, &node.type_name, key, val, range)
                {
                    warnings.push(msg);
                }
            }
            None => {
                let diag = plushie_core::Diagnostic::PropUnknown {
                    id: node.id.clone(),
                    type_name: node.type_name.clone(),
                    prop: key.clone(),
                    known_debug: format!("{expected_names:?}"),
                };
                warnings.push(diag.to_string());
            }
        }
    }

    // NumericRange is intentionally opaque outside this module; nudge
    // the linter to keep the unused-but-reachable variant allowed.
    let _ = NumericRange::any();

    warnings
}

/// Validate a node's props and log any warnings.
pub(crate) fn validate_props(node: &TreeNode) {
    for warning in collect_prop_warnings(node) {
        log::warn!("{warning}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_node(type_name: &str, props: serde_json::Value) -> TreeNode {
        crate::testing::node_with_props(&format!("test-{type_name}"), type_name, props)
    }

    /// Verify validate_props doesn't panic for every supported widget type,
    /// including with an empty props object and with representative props.
    #[test]
    fn validate_all_supported_types_no_panic() {
        let types_with_sample_props: Vec<(&str, serde_json::Value)> = vec![
            ("button", json!({"label": "Click me"})),
            ("text", json!({"content": "Hello"})),
            ("column", json!({"spacing": 8})),
            ("row", json!({"spacing": 8})),
            ("container", json!({"padding": 16})),
            ("text_input", json!({"value": "hello"})),
            ("slider", json!({"value": 50, "range": [0, 100]})),
            ("checkbox", json!({"label": "ok", "checked": true})),
            ("toggler", json!({"label": "on", "is_toggled": false})),
            ("progress_bar", json!({"value": 50, "range": [0, 100]})),
            ("image", json!({"source": "test.png"})),
            ("svg", json!({"source": "icon.svg"})),
            ("scrollable", json!({})),
            ("grid", json!({"num_columns": 3})),
            ("radio", json!({"value": "a", "label": "A"})),
            ("tooltip", json!({"tip": "Help text"})),
            ("pointer_area", json!({})),
            ("sensor", json!({})),
            ("space", json!({})),
            ("rule", json!({})),
            ("pick_list", json!({"options": ["a", "b"]})),
            ("combo_box", json!({"options": ["a", "b"]})),
            ("text_editor", json!({"content": "edit me"})),
            ("overlay", json!({})),
            ("themer", json!({"theme": "dark"})),
            ("stack", json!({})),
            ("pin", json!({"x": 10, "y": 20})),
            ("floating", json!({})),
            ("keyed_column", json!({})),
            ("responsive", json!({})),
            ("rich_text", json!({"spans": []})),
            ("vertical_slider", json!({"value": 50, "range": [0, 100]})),
            ("table", json!({"columns": [], "rows": []})),
            ("pane_grid", json!({})),
            ("markdown", json!({"content": "# Hello"})),
            ("canvas", json!({"width": "fill", "height": "fill"})),
            ("qr_code", json!({"data": "https://example.com"})),
            ("window", json!({"title": "Test"})),
        ];

        for (type_name, props) in &types_with_sample_props {
            let node = make_node(type_name, props.clone());
            validate_props(&node); // must not panic

            // Also test with empty props
            let empty_node = make_node(type_name, json!({}));
            validate_props(&empty_node);
        }
    }

    #[test]
    fn unknown_prop_produces_warning() {
        let node = make_node("button", json!({"label": "ok", "bogus": 42}));
        let warnings = collect_prop_warnings(&node);
        assert!(!warnings.is_empty());
        // Warning renders via Diagnostic::PropUnknown Display; `bogus`
        // and the `prop_unknown` tag both appear.
        assert!(warnings[0].contains("prop_unknown"));
        assert!(warnings[0].contains("bogus"));
    }

    #[test]
    fn negative_spacing_emits_range_warning() {
        let node = make_node("column", json!({"spacing": -5}));
        let warnings = collect_prop_warnings(&node);
        assert!(
            warnings.iter().any(|w| w.contains("prop_range_exceeded")),
            "expected range warning, got {warnings:?}"
        );
    }

    #[test]
    fn oversize_font_emits_range_warning() {
        let node = make_node("text", json!({"size": 5000}));
        // `size` is Number for text with a lower-only bound; 5000 passes.
        let warnings = collect_prop_warnings(&node);
        assert!(
            !warnings.iter().any(|w| w.contains("prop_range_exceeded")),
            "size has no upper bound; got {warnings:?}"
        );
        let node = make_node("checkbox", json!({"text_size": 5000}));
        let warnings = collect_prop_warnings(&node);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("prop_range_exceeded") && w.contains("text_size")),
            "expected text_size range warning, got {warnings:?}"
        );
    }

    #[test]
    fn window_huge_dimensions_emit_range_warning() {
        let node = make_node("window", json!({"width": 50_000, "title": "x"}));
        let warnings = collect_prop_warnings(&node);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("prop_range_exceeded") && w.contains("width")),
            "expected window width range warning, got {warnings:?}"
        );
    }

    #[test]
    fn valid_props_produce_no_warnings() {
        let node = make_node(
            "button",
            json!({"label": "ok", "mnemonic": "O", "access_key": "K"}),
        );
        let warnings = collect_prop_warnings(&node);
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);

        let node = make_node(
            "slider",
            json!({"value": 5, "range": [0, 10], "step": 1, "keyboard_step": 2}),
        );
        let warnings = collect_prop_warnings(&node);
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);

        let node = make_node(
            "checkbox",
            json!({"label": "ok", "mnemonic": "O", "access_key": "K"}),
        );
        let warnings = collect_prop_warnings(&node);
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);

        let node = make_node(
            "radio",
            json!({"value": "ok", "mnemonic": "O", "access_key": "K"}),
        );
        let warnings = collect_prop_warnings(&node);
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);

        let node = make_node(
            "vertical_slider",
            json!({"value": 5, "range": [0, 10], "step": 1, "keyboard_step": 2}),
        );
        let warnings = collect_prop_warnings(&node);
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    }

    #[test]
    fn invalid_enum_literal_emits_allowed_values() {
        let node = make_node("text", json!({"content": "Hello", "align_x": "sideways"}));
        let warnings = collect_prop_warnings(&node);

        assert_eq!(warnings.len(), 1, "expected one warning, got {warnings:?}");
        assert!(warnings[0].contains("prop_type_mismatch"));
        assert!(warnings[0].contains("align_x"));
        assert!(warnings[0].contains("\"sideways\""));
        assert!(warnings[0].contains("\"justified\""));
    }

    #[test]
    fn valid_enum_literal_produces_no_warnings() {
        let node = make_node(
            "text",
            json!({"content": "Hello", "align_x": "start", "text_direction": "rtl"}),
        );
        let warnings = collect_prop_warnings(&node);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn enum_like_any_prop_is_validated() {
        let node = make_node("canvas", json!({"arrow_mode": "diagonal"}));
        let warnings = collect_prop_warnings(&node);

        assert_eq!(warnings.len(), 1, "expected one warning, got {warnings:?}");
        assert!(warnings[0].contains("prop_type_mismatch"));
        assert!(warnings[0].contains("arrow_mode"));
        assert!(warnings[0].contains("\"wrap\""));
    }

    #[test]
    fn canvas_role_alias_is_accepted() {
        let node = make_node("canvas", json!({"role": "radio"}));
        let warnings = collect_prop_warnings(&node);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn invalid_highlight_theme_emits_allowed_values() {
        let node = make_node(
            "text_editor",
            json!({"content": "fn main() {}", "highlight_theme": "neon"}),
        );
        let warnings = collect_prop_warnings(&node);

        assert_eq!(warnings.len(), 1, "expected one warning, got {warnings:?}");
        assert!(warnings[0].contains("prop_type_mismatch"));
        assert!(warnings[0].contains("highlight_theme"));
        assert!(warnings[0].contains("\"solarized_dark\""));
    }

    #[test]
    fn invalid_markdown_code_theme_emits_allowed_values() {
        let node = make_node(
            "markdown",
            json!({"content": "# Title", "code_theme": "neon"}),
        );
        let warnings = collect_prop_warnings(&node);

        assert_eq!(warnings.len(), 1, "expected one warning, got {warnings:?}");
        assert!(warnings[0].contains("prop_type_mismatch"));
        assert!(warnings[0].contains("code_theme"));
        assert!(warnings[0].contains("\"inspired_github\""));
    }

    #[test]
    fn window_title_is_valid() {
        let node = make_node("window", json!({"title": "My App"}));
        let warnings = collect_prop_warnings(&node);
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    }

    /// Guard against schema drift: every built-in widget type that
    /// the iced widget set registers must appear in
    /// `VALIDATED_WIDGET_TYPES`. A new widget added to the set but
    /// not to the validation schema would silently ship without
    /// prop validation.
    #[test]
    fn validation_schema_covers_registered_widget_types() {
        use crate::widget::widget_set::IcedWidgetSet;

        let registered = IcedWidgetSet::type_names();
        let covered: std::collections::HashSet<&str> =
            VALIDATED_WIDGET_TYPES.iter().copied().collect();

        let mut missing: Vec<String> = registered
            .iter()
            .filter(|name| !covered.contains(name.as_str()))
            .cloned()
            .collect();
        missing.sort();

        assert!(
            missing.is_empty(),
            "the following widget types are registered but have no validation schema entry: {:?}",
            missing
        );
    }
}
