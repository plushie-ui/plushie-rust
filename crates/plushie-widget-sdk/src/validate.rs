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
        PropType::Any => true,
    }
}

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
        return Some((
            format!(
                "prop_range_exceeded: widget \"{node_id}\" ({type_name}) prop \
                 \"{prop_name}\" value {raw} is not finite, clamped to \
                 {clamped}"
            ),
            clamped,
        ));
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
        Some((
            format!(
                "prop_range_exceeded: widget \"{node_id}\" ({type_name}) prop \
                 \"{prop_name}\" value {raw} out of range, clamped to \
                 {clamped}"
            ),
            clamped,
        ))
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
            ("level", Str),
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
        ],
        "text" => &[
            ("content", Str),
            ("size", Number),
            ("color", Color),
            ("font", Any),
            ("width", Length),
            ("height", Length),
            ("align_x", Str),
            ("align_y", Str),
            ("line_height", Number),
            ("shaping", Str),
            ("wrapping", Str),
            ("ellipsis", Str),
            ("style", Str),
        ],
        "column" => &[
            ("spacing", Number),
            ("padding", Any),
            ("width", Length),
            ("height", Length),
            ("max_width", Number),
            ("align_x", Str),
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
            ("align_y", Str),
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
            ("align_x", Str),
            ("align_y", Str),
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
            ("align_x", Str),
            ("placeholder_color", Color),
            ("selection_color", Color),
            ("input_purpose", Str),
            ("ime_purpose", Str),
        ],
        "slider" => &[
            ("value", Number),
            ("range", Array),
            ("step", Number),
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
            ("line_height", Number),
            ("wrapping", Str),
            ("shaping", Str),
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
            ("wrapping", Str),
            ("shaping", Str),
            ("text_alignment", Str),
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
            ("content_fit", Str),
            ("filter_method", Str),
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
            ("content_fit", Str),
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
            ("direction", Str),
            ("style", Any),
            ("anchor", Str),
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
            ("columns", Number),
            ("column_count", Number),
            ("spacing", Number),
            ("padding", Any),
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
            ("line_height", Number),
            ("wrapping", Str),
            ("shaping", Str),
        ],
        "tooltip" => &[
            ("tip", Str),
            ("position", Str),
            ("gap", Number),
            ("padding", Number),
            ("snap_within_viewport", Bool),
            ("delay", Number),
            ("style", Any),
        ],
        "pointer_area" => &[
            ("cursor", Str),
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
            ("direction", Str),
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
            ("shaping", Str),
            ("handle", Any),
            ("ellipsis", Str),
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
            ("shaping", Str),
            ("menu_height", Number),
            ("icon", Any),
            ("on_option_hovered", Bool),
            ("on_open", Bool),
            ("on_close", Bool),
            ("on_submit", Bool),
            ("ellipsis", Str),
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
            ("wrapping", Str),
            ("key_bindings", Array),
            ("style", Any),
            ("highlight_syntax", Str),
            ("highlight_theme", Str),
            ("placeholder_color", Color),
            ("selection_color", Color),
            ("input_purpose", Str),
            ("ime_purpose", Str),
        ],
        "overlay" => &[
            ("position", Str),
            ("gap", Number),
            ("offset_x", Number),
            ("offset_y", Number),
            ("flip", Bool),
            ("align", Str),
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
            ("align_x", Str),
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
            ("wrapping", Str),
            ("ellipsis", Str),
        ],
        "vertical_slider" => &[
            ("value", Number),
            ("range", Array),
            ("step", Number),
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
            ("sort_order", Str),
            ("header_text_size", Number),
            ("row_text_size", Number),
            ("cell_spacing", Number),
            ("row_spacing", Number),
            ("separator_thickness", Number),
            ("separator_color", Color),
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
            ("split_axis", Str),
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
            ("code_theme", Str),
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
            ("role", Str),
            ("arrow_mode", Any),
            ("event_rate", Number),
            ("a11y", Any),
        ],
        "qr_code" => &[
            ("data", Str),
            ("cell_size", Number),
            ("total_size", Number),
            ("error_correction", Str),
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
                    warnings.push(format!(
                        "widget '{}' ({}): prop '{}' has unexpected type {:?} (expected {:?})",
                        node.id, node.type_name, key, val, expected_type
                    ));
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
                warnings.push(format!(
                    "widget '{}' ({}): unexpected prop '{}' (known: {:?})",
                    node.id, node.type_name, key, expected_names
                ));
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
            ("grid", json!({"columns": 3})),
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
        assert!(warnings[0].contains("unexpected prop 'bogus'"));
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
        // "size" is Number for text; range check fires.
        let warnings = collect_prop_warnings(&node);
        // size has a lower bound only; 5000 is fine. Use text_size
        // via a widget that accepts it.
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
        let node = make_node("button", json!({"label": "ok"}));
        let warnings = collect_prop_warnings(&node);
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
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
