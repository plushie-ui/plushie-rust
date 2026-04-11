//! Widget helpers: parsing, style application, and utilities.
//!
//! This module re-exports the public [`prop_helpers`](crate::prop_helpers)
//! and provides functions for parsing complex prop types (padding, style
//! maps) and applying style overrides to iced widget styles.
//! Widget authors can access these via `plushie_widget_sdk::widget::helpers::*`.

use iced::widget::{
    button, checkbox, container, pick_list, progress_bar, rule, slider, text_editor, text_input,
    toggler,
};
use iced::{Border, Color, Font, Pixels, Shadow};
use plushie_core::protocol::Props;
use plushie_core::types::PlushieType;
use serde_json::Value;

use crate::iced_convert;

// Aliases for plushie-core types to avoid conflicts with iced types.
use plushie_core::types::Background as CoreBackground;
use plushie_core::types::Border as CoreBorder;
use plushie_core::types::Color as CoreColor;
use plushie_core::types::Font as CoreFont;
use plushie_core::types::LineHeight as CoreLineHeight;
use plushie_core::types::Shadow as CoreShadow;
use plushie_core::types::Shaping as CoreShaping;

// Re-export all public prop helpers so widget submodules using `use super::*`
// continue to find them without changes.
pub use crate::prop_helpers::*;

// ---------------------------------------------------------------------------
// Font family interning
// ---------------------------------------------------------------------------

/// Maximum length for a custom font family name. Names longer than this
/// are truncated with a warning.
const MAX_FONT_FAMILY_LEN: usize = 256;

/// Maximum number of unique custom font family names cached. Beyond this
/// limit, new names are still leaked (iced requires `'static` family
/// names) but not inserted into the cache, bounding the HashMap's memory.
const MAX_FONT_FAMILY_CACHE: usize = 1024;

/// Intern a custom font family name so identical strings share one leaked
/// allocation. Names exceeding [`MAX_FONT_FAMILY_LEN`] are truncated.
/// The cache is bounded to [`MAX_FONT_FAMILY_CACHE`] entries.
pub(crate) fn intern_font_family(name: &str) -> &'static str {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{LazyLock, Mutex};

    static CACHE: LazyLock<Mutex<HashMap<String, &'static str>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    static WARNED: AtomicBool = AtomicBool::new(false);

    let name = if name.len() > MAX_FONT_FAMILY_LEN {
        log::warn!(
            "font family name truncated from {} to {MAX_FONT_FAMILY_LEN} chars",
            name.len()
        );
        let mut end = MAX_FONT_FAMILY_LEN.min(name.len());
        while end > 0 && !name.is_char_boundary(end) {
            end -= 1;
        }
        &name[..end]
    } else {
        name
    };

    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(existing) = cache.get(name) {
        return existing;
    }

    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());

    if cache.len() >= MAX_FONT_FAMILY_CACHE {
        if !WARNED.swap(true, Ordering::Relaxed) {
            log::warn!(
                "font family cache full ({MAX_FONT_FAMILY_CACHE} entries); \
                 new names will leak without caching"
            );
        }
        return leaked;
    }

    cache.insert(name.to_owned(), leaked);
    leaked
}

// ---------------------------------------------------------------------------
// Style map parsing
// ---------------------------------------------------------------------------

/// Parsed fields from a style map JSON object. All fields are optional;
/// only those present in the JSON get populated.
#[derive(Clone, Default)]
pub struct StyleMapFields {
    pub background: Option<iced::Background>,
    pub text_color: Option<Color>,
    pub border: Option<Border>,
    pub shadow: Option<Shadow>,
}

pub fn parse_style_map_fields(obj: &serde_json::Map<String, Value>) -> StyleMapFields {
    StyleMapFields {
        background: obj
            .get("background")
            .and_then(CoreBackground::wire_decode)
            .map(|b| iced_convert::background(&b)),
        text_color: obj
            .get("text_color")
            .and_then(CoreColor::wire_decode)
            .map(|c| iced_convert::color(&c)),
        border: obj
            .get("border")
            .and_then(CoreBorder::wire_decode)
            .map(|b| iced_convert::border(&b)),
        shadow: obj
            .get("shadow")
            .and_then(CoreShadow::wire_decode)
            .map(|s| iced_convert::shadow(&s)),
    }
}

/// Parsed style overrides for all status variants. The base fields are always
/// present; status-specific overrides are optional.
#[derive(Clone)]
pub struct StyleOverrides {
    pub base: StyleMapFields,
    pub preset_base: Option<String>,
    pub hovered: Option<StyleMapFields>,
    pub pressed: Option<StyleMapFields>,
    pub disabled: Option<StyleMapFields>,
    pub focused: Option<StyleMapFields>,
}

/// Look up cached StyleOverrides for a node, falling back to parsing
/// if the cache doesn't have an entry (shouldn't happen in practice
/// since prepare_walk populates it, but safe to fall back).
pub fn get_style_overrides(
    node_id: &str,
    obj: &serde_json::Map<String, Value>,
    caches: &super::SharedState,
) -> StyleOverrides {
    if let Some(cached) = crate::shared_state::cached_style_overrides(caches, node_id) {
        return cached.clone();
    }
    parse_style_overrides(obj)
}

pub fn parse_style_overrides(obj: &serde_json::Map<String, Value>) -> StyleOverrides {
    StyleOverrides {
        base: parse_style_map_fields(obj),
        preset_base: obj.get("base").and_then(|v| v.as_str()).map(str::to_owned),
        hovered: obj
            .get("hovered")
            .and_then(|v| v.as_object())
            .map(parse_style_map_fields),
        pressed: obj
            .get("pressed")
            .and_then(|v| v.as_object())
            .map(parse_style_map_fields),
        disabled: obj
            .get("disabled")
            .and_then(|v| v.as_object())
            .map(parse_style_map_fields),
        focused: obj
            .get("focused")
            .and_then(|v| v.as_object())
            .map(parse_style_map_fields),
    }
}

/// Convert a [`plushie_core::types::Style`] `Custom` variant into
/// `StyleOverrides` by round-tripping through the wire format.
///
/// `Preset` variants should be handled with direct string matching
/// before reaching this function.
pub fn style_overrides_from_style_map(
    node_id: &str,
    style_map: &plushie_core::types::StyleMap,
    caches: &super::SharedState,
) -> StyleOverrides {
    let prop_value = style_map.wire_encode();
    let json_value: Value = prop_value.into();
    match json_value.as_object() {
        Some(obj) => get_style_overrides(node_id, obj, caches),
        None => parse_style_overrides(&serde_json::Map::new()),
    }
}

/// Auto-derive hover background. Lightens dark colors, darkens light colors.
pub fn auto_derive_hover_bg(bg: Option<iced::Background>) -> Option<iced::Background> {
    bg.map(|b| deviate_background(b, 0.1))
}

/// Auto-derive disabled background by reducing alpha to 50%.
pub fn auto_derive_disabled_bg(bg: Option<iced::Background>) -> Option<iced::Background> {
    bg.map(|b| match b {
        iced::Background::Color(c) => iced::Background::Color(alpha_color(c, 0.5)),
        iced::Background::Gradient(g) => iced::Background::Gradient(alpha_gradient(g, 0.5)),
    })
}

/// Auto-derive disabled text color by reducing alpha to 50%.
pub fn auto_derive_disabled_text(color: Color) -> Color {
    alpha_color(color, 0.5)
}

/// Auto-derive disabled border by reducing border color alpha to 50%.
pub fn auto_derive_disabled_border(border: Border) -> Border {
    Border {
        color: alpha_color(border.color, 0.5),
        ..border
    }
}

/// Auto-derive disabled shadow by reducing shadow color alpha to 50%.
pub fn auto_derive_disabled_shadow(shadow: Shadow) -> Shadow {
    Shadow {
        color: alpha_color(shadow.color, 0.5),
        ..shadow
    }
}

/// Apply style map fields to a button style. Background wraps in `Some`,
/// text_color, border, and shadow map directly.
pub fn apply_button_fields(style: &mut button::Style, fields: &StyleMapFields) {
    if let Some(bg) = fields.background {
        style.background = Some(bg);
    }
    if let Some(tc) = fields.text_color {
        style.text_color = tc;
    }
    if let Some(brd) = fields.border {
        style.border = brd;
    }
    if let Some(shd) = fields.shadow {
        style.shadow = shd;
    }
}

/// Apply style map fields to a progress_bar style. Background maps as
/// `Background::Color`, text_color maps to the bar fill, border directly.
pub fn apply_progress_bar_fields(style: &mut progress_bar::Style, fields: &StyleMapFields) {
    if let Some(iced::Background::Color(c)) = fields.background {
        style.background = iced::Background::Color(c);
    }
    if let Some(tc) = fields.text_color {
        style.bar = iced::Background::Color(tc);
    }
    if let Some(brd) = fields.border {
        style.border = brd;
    }
}

/// Apply style map fields to a text_input or text_editor style. Both widgets
/// map background as `Background::Color`, border directly, and text_color to
/// the `value` field (the typed text color).
pub fn apply_text_input_fields(style: &mut text_input::Style, fields: &StyleMapFields) {
    if let Some(iced::Background::Color(c)) = fields.background {
        style.background = iced::Background::Color(c);
    }
    if let Some(brd) = fields.border {
        style.border = brd;
    }
    if let Some(tc) = fields.text_color {
        style.value = tc;
    }
}

/// Apply style map fields to a text_editor style. Mirrors
/// [`apply_text_input_fields`] -- both style types have the same
/// background/border/value fields but are distinct iced types.
pub fn apply_text_editor_fields(style: &mut text_editor::Style, fields: &StyleMapFields) {
    if let Some(iced::Background::Color(c)) = fields.background {
        style.background = iced::Background::Color(c);
    }
    if let Some(brd) = fields.border {
        style.border = brd;
    }
    if let Some(tc) = fields.text_color {
        style.value = tc;
    }
}

/// Apply style map fields to a pick_list style. Background is
/// `Background::Color`, text_color and border map directly.
pub fn apply_pick_list_fields(style: &mut pick_list::Style, fields: &StyleMapFields) {
    if let Some(tc) = fields.text_color {
        style.text_color = tc;
    }
    if let Some(iced::Background::Color(c)) = fields.background {
        style.background = iced::Background::Color(c);
    }
    if let Some(brd) = fields.border {
        style.border = brd;
    }
}

/// Apply style map fields to a slider handle. Background maps to
/// handle.background as `Background::Color`, border maps to
/// handle.border_width/border_color. Shared by slider and vertical_slider.
pub fn apply_slider_handle_fields(handle: &mut slider::Handle, fields: &StyleMapFields) {
    if let Some(iced::Background::Color(c)) = fields.background {
        handle.background = iced::Background::Color(c);
    }
    if let Some(brd) = fields.border {
        handle.border_width = brd.width;
        handle.border_color = brd.color;
    }
}

/// Apply style map fields to a radio style. Background is `Background::Color`,
/// text_color wraps in `Some`, border maps to border_width/border_color.
pub fn apply_radio_fields(style: &mut iced::widget::radio::Style, fields: &StyleMapFields) {
    if let Some(iced::Background::Color(c)) = fields.background {
        style.background = iced::Background::Color(c);
    }
    if let Some(tc) = fields.text_color {
        style.text_color = Some(tc);
    }
    if let Some(brd) = fields.border {
        style.border_width = brd.width;
        style.border_color = brd.color;
    }
}

/// Apply style map fields to a toggler style. Background maps directly,
/// text_color wraps in `Some`, border maps to border_width/border_color.
pub fn apply_toggler_fields(style: &mut toggler::Style, fields: &StyleMapFields) {
    if let Some(bg) = fields.background {
        style.background = bg;
    }
    if let Some(tc) = fields.text_color {
        style.text_color = Some(tc);
    }
    if let Some(brd) = fields.border {
        style.background_border_width = brd.width;
        style.background_border_color = brd.color;
    }
}

/// Apply style map fields to a rule style. Maps background -> color,
/// border -> radius.
pub fn apply_rule_style(mut style: rule::Style, fields: &StyleMapFields) -> rule::Style {
    if let Some(iced::Background::Color(c)) = fields.background {
        style.color = c;
    }
    if let Some(brd) = fields.border {
        style.radius = brd.radius;
    }
    style
}

/// Apply style map fields to a checkbox style. Background is `Background::Color`,
/// border directly, text_color wrapped in `Some`.
pub fn apply_checkbox_fields(style: &mut checkbox::Style, fields: &StyleMapFields) {
    if let Some(iced::Background::Color(c)) = fields.background {
        style.background = iced::Background::Color(c);
    }
    if let Some(brd) = fields.border {
        style.border = brd;
    }
    if let Some(tc) = fields.text_color {
        style.text_color = Some(tc);
    }
}

/// Build a `container::Style` from base style map fields. Used by both
/// container and tooltip widgets which share the same style type.
pub fn container_style_from_base(base: &StyleMapFields) -> container::Style {
    let mut style = container::Style {
        background: base.background,
        text_color: base.text_color,
        ..Default::default()
    };
    if let Some(brd) = base.border {
        style.border = brd;
    }
    if let Some(shd) = base.shadow {
        style.shadow = shd;
    }
    style
}

pub fn alpha_color(color: Color, alpha: f32) -> Color {
    Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a * alpha,
    }
}

/// Lighten dark colors, darken light colors by the given amount.
pub fn deviate_color(color: Color, amount: f32) -> Color {
    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance > 0.5 {
        // Light color: darken
        Color {
            r: (color.r - amount).max(0.0),
            g: (color.g - amount).max(0.0),
            b: (color.b - amount).max(0.0),
            a: color.a,
        }
    } else {
        // Dark color: lighten
        Color {
            r: (color.r + amount).min(1.0),
            g: (color.g + amount).min(1.0),
            b: (color.b + amount).min(1.0),
            a: color.a,
        }
    }
}

pub fn deviate_background(bg: iced::Background, amount: f32) -> iced::Background {
    match bg {
        iced::Background::Color(c) => iced::Background::Color(deviate_color(c, amount)),
        iced::Background::Gradient(g) => iced::Background::Gradient(deviate_gradient(g, amount)),
    }
}

pub fn deviate_gradient(gradient: iced::Gradient, amount: f32) -> iced::Gradient {
    match gradient {
        iced::Gradient::Linear(mut linear) => {
            for stop in linear.stops.iter_mut().flatten() {
                stop.color = deviate_color(stop.color, amount);
            }
            iced::Gradient::Linear(linear)
        }
    }
}

pub fn alpha_gradient(gradient: iced::Gradient, alpha: f32) -> iced::Gradient {
    match gradient {
        iced::Gradient::Linear(mut linear) => {
            for stop in linear.stops.iter_mut().flatten() {
                stop.color = alpha_color(stop.color, alpha);
            }
            iced::Gradient::Linear(linear)
        }
    }
}

/// Parsed menu style overrides for pick_list/combo_box dropdown menus.
#[derive(Clone)]
pub struct MenuStyleOverrides {
    pub background: Option<iced::Background>,
    pub text_color: Option<Color>,
    pub selected_text_color: Option<Color>,
    pub selected_background: Option<iced::Background>,
    pub border: Option<Border>,
    pub shadow: Option<Shadow>,
}

/// Parse a `menu_style` prop into overrides for dropdown menu styling.
pub fn parse_menu_style(props: &Props) -> Option<MenuStyleOverrides> {
    let menu_val = props.get_value("menu_style")?;
    let obj = menu_val.as_object()?;

    Some(MenuStyleOverrides {
        background: obj
            .get("background")
            .and_then(CoreBackground::wire_decode)
            .map(|b| iced_convert::background(&b)),
        text_color: obj
            .get("text_color")
            .and_then(CoreColor::wire_decode)
            .map(|c| iced_convert::color(&c)),
        selected_text_color: obj
            .get("selected_text_color")
            .and_then(CoreColor::wire_decode)
            .map(|c| iced_convert::color(&c)),
        selected_background: obj
            .get("selected_background")
            .and_then(CoreBackground::wire_decode)
            .map(|b| iced_convert::background(&b)),
        border: obj
            .get("border")
            .and_then(CoreBorder::wire_decode)
            .map(|b| iced_convert::border(&b)),
        shadow: obj
            .get("shadow")
            .and_then(CoreShadow::wire_decode)
            .map(|s| iced_convert::shadow(&s)),
    })
}

/// Apply `MenuStyleOverrides` on top of a base `menu::Style`.
pub fn apply_menu_style_overrides(style: &mut iced::overlay::menu::Style, ov: &MenuStyleOverrides) {
    if let Some(bg) = ov.background {
        style.background = bg;
    }
    if let Some(tc) = ov.text_color {
        style.text_color = tc;
    }
    if let Some(stc) = ov.selected_text_color {
        style.selected_text_color = stc;
    }
    if let Some(sbg) = ov.selected_background {
        style.selected_background = sbg;
    }
    if let Some(brd) = ov.border {
        style.border = brd;
    }
    if let Some(shd) = ov.shadow {
        style.shadow = shd;
    }
}

/// Parse a text_input::Icon from a JSON value.
pub fn parse_text_input_icon(value: &Value) -> Option<text_input::Icon<Font>> {
    let obj = value.as_object()?;

    let code_point = obj
        .get("code_point")
        .and_then(|v| v.as_str())
        .and_then(|s| s.chars().next())?;

    let font = obj
        .get("font")
        .and_then(CoreFont::wire_decode)
        .map(|f| iced_convert::font(&f))
        .unwrap_or(Font::DEFAULT);

    let size = obj
        .get("size")
        .and_then(|v| v.as_f64())
        .map(|v| Pixels(v as f32));

    let spacing = obj
        .get("spacing")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(4.0);

    let side = match obj.get("side").and_then(|v| v.as_str()).unwrap_or("left") {
        "right" => text_input::Side::Right,
        _ => text_input::Side::Left,
    };

    Some(text_input::Icon {
        font,
        code_point,
        size,
        spacing,
        side,
    })
}

/// Parse a pick_list::Icon from a JSON value.
pub fn parse_pick_list_icon(value: &Value) -> Option<pick_list::Icon<Font>> {
    let obj = value.as_object()?;

    let code_point = obj
        .get("code_point")
        .and_then(|v| v.as_str())
        .and_then(|s| s.chars().next())?;

    let font = obj
        .get("font")
        .and_then(CoreFont::wire_decode)
        .map(|f| iced_convert::font(&f))
        .unwrap_or(Font::DEFAULT);

    let size = obj
        .get("size")
        .and_then(|v| v.as_f64())
        .map(|v| Pixels(v as f32));

    let line_height = obj
        .get("line_height")
        .and_then(CoreLineHeight::wire_decode)
        .map(|lh| iced_convert::line_height(lh))
        .unwrap_or(iced::widget::text::LineHeight::Relative(1.2));

    let shaping = obj
        .get("shaping")
        .and_then(CoreShaping::wire_decode)
        .map(|s| iced_convert::shaping(s))
        .unwrap_or(iced::widget::text::Shaping::Basic);

    Some(pick_list::Icon {
        font,
        code_point,
        size,
        line_height,
        shaping,
    })
}

/// Parse a PickList Handle from props.
pub fn parse_pick_list_handle(props: &Props) -> Option<pick_list::Handle<Font>> {
    let handle_val = props.get_value("handle")?;
    let handle_obj = handle_val.as_object()?;
    let handle_type = handle_obj.get("type")?.as_str()?;

    match handle_type {
        "arrow" => {
            let size = handle_obj
                .get("size")
                .and_then(|v| v.as_f64())
                .map(|v| Pixels(v as f32));
            Some(pick_list::Handle::Arrow { size })
        }
        "static" => {
            let icon = parse_pick_list_icon(handle_obj.get("icon")?)?;
            Some(pick_list::Handle::Static(icon))
        }
        "dynamic" => {
            let closed = parse_pick_list_icon(handle_obj.get("closed")?)?;
            let open = parse_pick_list_icon(handle_obj.get("open")?)?;
            Some(pick_list::Handle::Dynamic { closed, open })
        }
        "none" => Some(pick_list::Handle::None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: build a Props from a json! value. The value must be an object.
    fn make_props(v: Value) -> Props {
        Props::Wire(v)
    }

    // -- prop_f32 --

    #[test]
    fn prop_f32_returns_number() {
        let p = make_props(json!({"size": 16.0}));
        assert_eq!(prop_f32(&p, "size"), Some(16.0));
    }

    #[test]
    fn prop_f32_parses_string() {
        let p = make_props(json!({"size": "24.5"}));
        assert_eq!(prop_f32(&p, "size"), Some(24.5));
    }

    #[test]
    fn prop_f32_returns_none_for_missing_key() {
        let p = make_props(json!({"other": 10}));
        assert_eq!(prop_f32(&p, "size"), None);
    }

    #[test]
    fn prop_f32_returns_none_for_bool() {
        let p = make_props(json!({"size": true}));
        assert_eq!(prop_f32(&p, "size"), None);
    }

    // -- prop_bool --

    #[test]
    fn prop_bool_returns_true() {
        let p = make_props(json!({"visible": true}));
        assert_eq!(prop_bool(&p, "visible"), Some(true));
    }

    #[test]
    fn prop_bool_returns_false() {
        let p = make_props(json!({"visible": false}));
        assert_eq!(prop_bool(&p, "visible"), Some(false));
    }

    #[test]
    fn prop_bool_returns_none_for_missing() {
        let p = make_props(json!({"other": 1}));
        assert_eq!(prop_bool(&p, "visible"), None);
    }

    #[test]
    fn prop_bool_default_uses_fallback() {
        let p = make_props(json!({}));
        assert!(prop_bool_default(&p, "clip", true));
        assert!(!prop_bool_default(&p, "clip", false));
    }

    // -- prop_str --

    #[test]
    fn prop_str_returns_string() {
        let p = make_props(json!({"label": "hello"}));
        assert_eq!(prop_str(&p, "label"), Some("hello".to_string()));
    }

    // -- Style map tests --

    #[test]
    fn style_map_parse_overrides_basic() {
        let obj = json!({
            "background": "#ff0000",
            "text_color": "#00ff00",
            "border": {"color": "#0000ff", "width": 2.0, "radius": 4.0},
            "hovered": {
                "background": "#880000",
                "text_color": "#008800"
            },
            "pressed": {
                "background": "#440000"
            },
            "disabled": {
                "text_color": "#999999"
            },
            "focused": {
                "border": {"color": "#ffffff", "width": 3.0, "radius": 0.0}
            }
        });
        let map = obj.as_object().unwrap();
        let overrides = parse_style_overrides(map);

        // Base fields
        assert!(overrides.base.background.is_some());
        assert!(overrides.base.text_color.is_some());
        assert!(overrides.base.border.is_some());
        assert_eq!(
            overrides.base.text_color.unwrap(),
            Color::from_rgb8(0, 255, 0)
        );

        // Hovered override present with both fields
        let hovered = overrides.hovered.unwrap();
        assert!(hovered.background.is_some());
        assert!(hovered.text_color.is_some());

        // Pressed override present with background only
        let pressed = overrides.pressed.unwrap();
        assert!(pressed.background.is_some());
        assert!(pressed.text_color.is_none());

        // Disabled override present with text_color only
        let disabled = overrides.disabled.unwrap();
        assert!(disabled.background.is_none());
        assert!(disabled.text_color.is_some());

        // Focused override present with border only
        let focused = overrides.focused.unwrap();
        assert!(focused.border.is_some());
        assert!(focused.background.is_none());
    }

    #[test]
    fn style_map_parse_overrides_missing() {
        // Only base fields, no status overrides at all.
        let obj = json!({"background": "#aabbcc"});
        let map = obj.as_object().unwrap();
        let overrides = parse_style_overrides(map);

        assert!(overrides.base.background.is_some());
        assert!(overrides.hovered.is_none());
        assert!(overrides.pressed.is_none());
        assert!(overrides.disabled.is_none());
        assert!(overrides.focused.is_none());
    }

    #[test]
    fn style_map_auto_derive_hover_light() {
        // Light color (luminance > 0.5) should darken by 0.1.
        let bg = Some(iced::Background::Color(Color::from_rgba(
            1.0, 0.8, 0.6, 1.0,
        )));
        let result = auto_derive_hover_bg(bg);
        match result {
            Some(iced::Background::Color(c)) => {
                assert!((c.r - 0.9).abs() < 0.001);
                assert!((c.g - 0.7).abs() < 0.001);
                assert!((c.b - 0.5).abs() < 0.001);
                assert!((c.a - 1.0).abs() < 0.001);
            }
            other => panic!("expected Background::Color, got {other:?}"),
        }
    }

    #[test]
    fn style_map_auto_derive_hover_dark() {
        // Dark color (luminance <= 0.5) should lighten by 0.1.
        let bg = Some(iced::Background::Color(Color::from_rgba(
            0.1, 0.1, 0.1, 1.0,
        )));
        let result = auto_derive_hover_bg(bg);
        match result {
            Some(iced::Background::Color(c)) => {
                assert!((c.r - 0.2).abs() < 0.001);
                assert!((c.g - 0.2).abs() < 0.001);
                assert!((c.b - 0.2).abs() < 0.001);
                assert!((c.a - 1.0).abs() < 0.001);
            }
            other => panic!("expected Background::Color, got {other:?}"),
        }
    }

    #[test]
    fn style_map_auto_derive_disabled_bg() {
        // Reduces alpha by 0.5, RGB unchanged.
        let bg = Some(iced::Background::Color(Color::from_rgba(
            0.8, 0.6, 0.4, 1.0,
        )));
        let result = auto_derive_disabled_bg(bg);
        match result {
            Some(iced::Background::Color(c)) => {
                assert!((c.r - 0.8).abs() < 0.001);
                assert!((c.g - 0.6).abs() < 0.001);
                assert!((c.b - 0.4).abs() < 0.001);
                assert!((c.a - 0.5).abs() < 0.001);
            }
            other => panic!("expected Background::Color, got {other:?}"),
        }
    }

    #[test]
    fn style_map_auto_derive_disabled_text() {
        let color = Color::from_rgba(1.0, 1.0, 1.0, 0.8);
        let result = auto_derive_disabled_text(color);
        // RGB unchanged, alpha halved: 0.8 * 0.5 = 0.4
        assert!((result.r - 1.0).abs() < 0.001);
        assert!((result.g - 1.0).abs() < 0.001);
        assert!((result.b - 1.0).abs() < 0.001);
        assert!((result.a - 0.4).abs() < 0.001);
    }
}
