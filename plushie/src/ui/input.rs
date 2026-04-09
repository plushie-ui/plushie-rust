//! Input widget builders (ID required as first argument).
//!
//! These widgets accept user input. The ID is always the first
//! argument because interactive widgets need stable, explicit IDs
//! for event routing.

use serde_json::{Map, Value, json};

use crate::View;
use crate::types::*;

// ---------------------------------------------------------------------------
// text_input
// ---------------------------------------------------------------------------

/// Builder for a single-line text input field.
pub struct TextInputBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a text input with the given ID and current value.
///
/// ```ignore
/// text_input("name", &model.name).placeholder("Enter your name")
/// ```
pub fn text_input(id: &str, value: &str) -> TextInputBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "value", value);
    TextInputBuilder { id: id.to_string(), props }
}

impl TextInputBuilder {
    pub fn placeholder(mut self, p: &str) -> Self { super::set_prop(&mut self.props, "placeholder", p); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "size", s); self }
    pub fn padding(mut self, p: impl Into<Padding>) -> Self { super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into())); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    pub fn line_height(mut self, lh: f32) -> Self { super::set_prop(&mut self.props, "line_height", lh); self }
    pub fn on_submit(mut self, enabled: bool) -> Self { super::set_prop(&mut self.props, "on_submit", enabled); self }
    pub fn secure(mut self, enabled: bool) -> Self { super::set_prop(&mut self.props, "secure", enabled); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
}

impl From<TextInputBuilder> for View {
    fn from(b: TextInputBuilder) -> View {
        View::leaf(b.id, "text_input", b.props)
    }
}

// ---------------------------------------------------------------------------
// text_editor
// ---------------------------------------------------------------------------

/// Builder for a multi-line text editor.
pub struct TextEditorBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a multi-line text editor with the given ID and initial content.
///
/// ```ignore
/// text_editor("notes", &model.notes).placeholder("Write here...")
/// ```
pub fn text_editor(id: &str, content: &str) -> TextEditorBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "content", content);
    TextEditorBuilder { id: id.to_string(), props }
}

impl TextEditorBuilder {
    pub fn placeholder(mut self, p: &str) -> Self { super::set_prop(&mut self.props, "placeholder", p); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    pub fn padding(mut self, p: impl Into<Padding>) -> Self { super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into())); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    pub fn size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "size", s); self }
    pub fn line_height(mut self, lh: f32) -> Self { super::set_prop(&mut self.props, "line_height", lh); self }
    pub fn wrapping(mut self, w: &str) -> Self { super::set_prop(&mut self.props, "wrapping", w); self }
    pub fn highlight_syntax(mut self, lang: &str) -> Self { super::set_prop(&mut self.props, "highlight_syntax", lang); self }
    pub fn highlight_theme(mut self, theme: &str) -> Self { super::set_prop(&mut self.props, "highlight_theme", theme); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
}

impl From<TextEditorBuilder> for View {
    fn from(b: TextEditorBuilder) -> View {
        View::leaf(b.id, "text_editor", b.props)
    }
}

// ---------------------------------------------------------------------------
// checkbox
// ---------------------------------------------------------------------------

/// Builder for a toggleable checkbox.
pub struct CheckboxBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a checkbox with the given ID and checked state.
///
/// ```ignore
/// checkbox("agree", model.agreed).label("I agree to the terms")
/// ```
pub fn checkbox(id: &str, checked: bool) -> CheckboxBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "checked", checked);
    CheckboxBuilder { id: id.to_string(), props }
}

impl CheckboxBuilder {
    pub fn label(mut self, l: &str) -> Self { super::set_prop(&mut self.props, "label", l); self }
    pub fn spacing(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "spacing", s); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "size", s); self }
    pub fn text_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "text_size", s); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    pub fn icon(mut self, icon: Value) -> Self { super::set_prop(&mut self.props, "icon", icon); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
}

impl From<CheckboxBuilder> for View {
    fn from(b: CheckboxBuilder) -> View {
        View::leaf(b.id, "checkbox", b.props)
    }
}

// ---------------------------------------------------------------------------
// toggler
// ---------------------------------------------------------------------------

/// Builder for an on/off toggle switch.
pub struct TogglerBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a toggler with the given ID and toggle state.
///
/// ```ignore
/// toggler("dark_mode", model.dark_mode).label("Dark mode")
/// ```
pub fn toggler(id: &str, is_toggled: bool) -> TogglerBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "is_toggled", is_toggled);
    TogglerBuilder { id: id.to_string(), props }
}

impl TogglerBuilder {
    pub fn label(mut self, l: &str) -> Self { super::set_prop(&mut self.props, "label", l); self }
    pub fn spacing(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "spacing", s); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "size", s); self }
    pub fn text_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "text_size", s); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
}

impl From<TogglerBuilder> for View {
    fn from(b: TogglerBuilder) -> View {
        View::leaf(b.id, "toggler", b.props)
    }
}

// ---------------------------------------------------------------------------
// radio
// ---------------------------------------------------------------------------

/// Builder for a radio button (one-of-many selection).
pub struct RadioBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a radio button with the given ID, value, and current selection.
///
/// All radios in a group should share the same `selected` value.
/// Pass `None` when nothing is selected yet.
///
/// ```ignore
/// radio("size_small", "small", Some("medium")).label("Small").group("size")
/// ```
pub fn radio(id: &str, value: &str, selected: Option<&str>) -> RadioBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "value", value);
    super::set_opt(&mut props, "selected", selected.map(|s| json!(s)));
    RadioBuilder { id: id.to_string(), props }
}

impl RadioBuilder {
    pub fn label(mut self, l: &str) -> Self { super::set_prop(&mut self.props, "label", l); self }
    pub fn group(mut self, g: &str) -> Self { super::set_prop(&mut self.props, "group", g); self }
    pub fn spacing(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "spacing", s); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "size", s); self }
    pub fn text_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "text_size", s); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
}

impl From<RadioBuilder> for View {
    fn from(b: RadioBuilder) -> View {
        View::leaf(b.id, "radio", b.props)
    }
}

// ---------------------------------------------------------------------------
// slider
// ---------------------------------------------------------------------------

/// Builder for a horizontal slider.
pub struct SliderBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a horizontal slider with the given ID, `(min, max)` range, and value.
///
/// ```ignore
/// slider("volume", (0.0, 100.0), model.volume).step(1.0)
/// ```
pub fn slider(id: &str, range: (f32, f32), value: f32) -> SliderBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "range", json!([range.0, range.1]));
    super::set_prop(&mut props, "value", value);
    SliderBuilder { id: id.to_string(), props }
}

impl SliderBuilder {
    pub fn step(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "step", s); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    pub fn default(mut self, d: f32) -> Self { super::set_prop(&mut self.props, "default", d); self }
    pub fn shift_step(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "shift_step", s); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
}

impl From<SliderBuilder> for View {
    fn from(b: SliderBuilder) -> View {
        View::leaf(b.id, "slider", b.props)
    }
}

// ---------------------------------------------------------------------------
// vertical_slider
// ---------------------------------------------------------------------------

/// Builder for a vertical slider.
pub struct VerticalSliderBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a vertical slider with the given ID, `(min, max)` range, and value.
///
/// ```ignore
/// vertical_slider("volume", (0.0, 100.0), model.volume).step(1.0)
/// ```
pub fn vertical_slider(id: &str, range: (f32, f32), value: f32) -> VerticalSliderBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "range", json!([range.0, range.1]));
    super::set_prop(&mut props, "value", value);
    VerticalSliderBuilder { id: id.to_string(), props }
}

impl VerticalSliderBuilder {
    pub fn step(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "step", s); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    pub fn default(mut self, d: f32) -> Self { super::set_prop(&mut self.props, "default", d); self }
    pub fn shift_step(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "shift_step", s); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
}

impl From<VerticalSliderBuilder> for View {
    fn from(b: VerticalSliderBuilder) -> View {
        View::leaf(b.id, "vertical_slider", b.props)
    }
}

// ---------------------------------------------------------------------------
// pick_list
// ---------------------------------------------------------------------------

/// Builder for a dropdown selection list.
pub struct PickListBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a pick list with the given ID, options, and current selection.
///
/// ```ignore
/// pick_list("color", &["Red", "Green", "Blue"], Some("Red"))
///     .placeholder("Choose a color")
/// ```
pub fn pick_list(id: &str, options: &[&str], selected: Option<&str>) -> PickListBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "options", json!(options));
    super::set_opt(&mut props, "selected", selected.map(|s| json!(s)));
    PickListBuilder { id: id.to_string(), props }
}

impl PickListBuilder {
    pub fn placeholder(mut self, p: &str) -> Self { super::set_prop(&mut self.props, "placeholder", p); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn padding(mut self, p: impl Into<Padding>) -> Self { super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into())); self }
    pub fn text_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "text_size", s); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
}

impl From<PickListBuilder> for View {
    fn from(b: PickListBuilder) -> View {
        View::leaf(b.id, "pick_list", b.props)
    }
}

// ---------------------------------------------------------------------------
// combo_box
// ---------------------------------------------------------------------------

/// Builder for a searchable combo box (dropdown with text input).
pub struct ComboBoxBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a combo box with the given ID, options, and current text value.
///
/// ```ignore
/// combo_box("lang", &["Rust", "Elixir", "Python"], "Rust")
///     .placeholder("Search languages...")
/// ```
pub fn combo_box(id: &str, options: &[&str], value: &str) -> ComboBoxBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "options", json!(options));
    super::set_prop(&mut props, "selected", value);
    ComboBoxBuilder { id: id.to_string(), props }
}

impl ComboBoxBuilder {
    pub fn placeholder(mut self, p: &str) -> Self { super::set_prop(&mut self.props, "placeholder", p); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn padding(mut self, p: impl Into<Padding>) -> Self { super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into())); self }
    pub fn size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "size", s); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
}

impl From<ComboBoxBuilder> for View {
    fn from(b: ComboBoxBuilder) -> View {
        View::leaf(b.id, "combo_box", b.props)
    }
}
