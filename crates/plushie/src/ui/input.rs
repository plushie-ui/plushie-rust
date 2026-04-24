//! Input widget builders (ID required as first argument).
//!
//! These widgets accept user input. The ID is always the first
//! argument because interactive widgets need stable, explicit IDs
//! for event routing.

use super::{PropMap, PropValue};

use crate::View;
use crate::derive_support::PlushieType;
use crate::selection::Selection;
use crate::types::*;

// ---------------------------------------------------------------------------
// text_input
// ---------------------------------------------------------------------------

/// Builder for a single-line text input field.
pub struct TextInputBuilder {
    id: String,
    props: PropMap,
}

/// Create a text input with the given ID and current value.
///
/// ```ignore
/// text_input("name", &model.name).placeholder("Enter your name")
/// ```
pub fn text_input(id: &str, value: &str) -> TextInputBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "value", value);
    TextInputBuilder {
        id: id.to_string(),
        props,
    }
}

impl TextInputBuilder {
    /// Set the placeholder text shown when empty.
    pub fn placeholder(mut self, p: &str) -> Self {
        super::set_prop(&mut self.props, "placeholder", p);
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    /// Set the font size in pixels.
    pub fn size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "size", s.into().wire_encode());
        self
    }
    /// Set the inner padding.
    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(
            &mut self.props,
            "padding",
            super::padding_to_value(p.into()),
        );
        self
    }
    /// Set the font family and weight.
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
        self
    }
    /// Line height (pixels or multiplier of font size).
    pub fn line_height(mut self, lh: impl Into<Animatable<LineHeight>>) -> Self {
        super::set_prop(&mut self.props, "line_height", lh.into().wire_encode());
        self
    }
    /// Handler for submit events.
    pub fn on_submit(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_submit", enabled);
        self
    }
    /// Mask input as a password field.
    pub fn secure(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "secure", enabled);
        self
    }
    /// Set the horizontal alignment.
    pub fn align_x(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_x", super::halign_to_value(a));
        self
    }
    /// Set the window icon from a file path.
    pub fn icon(mut self, icon: PropValue) -> Self {
        super::set_prop(&mut self.props, "icon", icon);
        self
    }
    /// Handler for paste events.
    pub fn on_paste(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_paste", enabled);
        self
    }
    /// Configure `input_purpose`.
    pub fn input_purpose(mut self, purpose: InputPurpose) -> Self {
        super::set_prop(&mut self.props, "input_purpose", purpose.wire_encode());
        self
    }
    /// Configure `placeholder_color`.
    pub fn placeholder_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "placeholder_color", c.into().wire_encode());
        self
    }
    /// Configure `selection_color`.
    pub fn selection_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "selection_color", c.into().wire_encode());
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second (0 = unbounded).
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    /// Mark this field as required. Flows into `a11y.required`.
    pub fn required(mut self, required: bool) -> Self {
        super::set_prop(&mut self.props, "required", required);
        self
    }
    /// Set the form-validation state. Flows into `a11y.invalid` and
    /// `a11y.error_message`.
    pub fn validation(mut self, v: Validation) -> Self {
        super::set_prop(&mut self.props, "validation", v.wire_encode());
        self
    }
    /// Attach accessibility metadata.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<TextInputBuilder> for View {
    fn from(b: TextInputBuilder) -> View {
        super::view_leaf(b.id, "text_input", b.props)
    }
}

// ---------------------------------------------------------------------------
// text_editor
// ---------------------------------------------------------------------------

/// Builder for a multi-line text editor.
pub struct TextEditorBuilder {
    id: String,
    props: PropMap,
}

/// Create a multi-line text editor with the given ID and initial content.
///
/// ```ignore
/// text_editor("notes", &model.notes).placeholder("Write here...")
/// ```
pub fn text_editor(id: &str, content: &str) -> TextEditorBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "content", content);
    TextEditorBuilder {
        id: id.to_string(),
        props,
    }
}

impl TextEditorBuilder {
    /// Set the placeholder text shown when empty.
    pub fn placeholder(mut self, p: &str) -> Self {
        super::set_prop(&mut self.props, "placeholder", p);
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    /// Set the preferred height.
    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }
    /// Minimum editor height in pixels.
    pub fn min_height(mut self, h: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "min_height", h.into().wire_encode());
        self
    }
    /// Maximum editor height in pixels.
    pub fn max_height(mut self, h: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "max_height", h.into().wire_encode());
        self
    }
    /// Set the inner padding.
    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(
            &mut self.props,
            "padding",
            super::padding_to_value(p.into()),
        );
        self
    }
    /// Set the font family and weight.
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
        self
    }
    /// Set the font size in pixels.
    pub fn size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "size", s.into().wire_encode());
        self
    }
    /// Line height (pixels or multiplier of font size).
    pub fn line_height(mut self, lh: impl Into<Animatable<LineHeight>>) -> Self {
        super::set_prop(&mut self.props, "line_height", lh.into().wire_encode());
        self
    }
    /// Line-wrap strategy for long content.
    pub fn wrapping(mut self, w: Wrapping) -> Self {
        super::set_prop(&mut self.props, "wrapping", w.wire_encode());
        self
    }
    /// Direction used for logical editor operations.
    pub fn text_direction(mut self, direction: TextDirection) -> Self {
        super::set_prop(&mut self.props, "text_direction", direction.wire_encode());
        self
    }
    /// Input purpose hint for the text editor.
    pub fn input_purpose(mut self, purpose: InputPurpose) -> Self {
        super::set_prop(&mut self.props, "input_purpose", purpose.wire_encode());
        self
    }
    /// Configure `highlight_syntax`.
    pub fn highlight_syntax(mut self, lang: &str) -> Self {
        super::set_prop(&mut self.props, "highlight_syntax", lang);
        self
    }
    /// Configure `highlight_theme`.
    pub fn highlight_theme(mut self, theme: &str) -> Self {
        super::set_prop(&mut self.props, "highlight_theme", theme);
        self
    }
    /// Declarative key binding rules for the editor.
    pub fn key_bindings(mut self, bindings: PropValue) -> Self {
        super::set_prop(&mut self.props, "key_bindings", bindings);
        self
    }
    /// Placeholder text color.
    pub fn placeholder_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "placeholder_color", c.into().wire_encode());
        self
    }
    /// Text selection highlight color.
    pub fn selection_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "selection_color", c.into().wire_encode());
        self
    }
    /// Handler for paste events.
    pub fn on_paste(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_paste", enabled);
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second (0 = unbounded).
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    /// Mark this field as required. Flows into `a11y.required`.
    pub fn required(mut self, required: bool) -> Self {
        super::set_prop(&mut self.props, "required", required);
        self
    }
    /// Set the form-validation state. Flows into `a11y.invalid` and
    /// `a11y.error_message`.
    pub fn validation(mut self, v: Validation) -> Self {
        super::set_prop(&mut self.props, "validation", v.wire_encode());
        self
    }
    /// Attach accessibility metadata.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<TextEditorBuilder> for View {
    fn from(b: TextEditorBuilder) -> View {
        super::view_leaf(b.id, "text_editor", b.props)
    }
}

// ---------------------------------------------------------------------------
// checkbox
// ---------------------------------------------------------------------------

/// Builder for a toggleable checkbox.
pub struct CheckboxBuilder {
    id: String,
    props: PropMap,
}

/// Create a checkbox with the given ID and checked state.
///
/// ```ignore
/// checkbox("agree", model.agreed).label("I agree to the terms")
/// ```
pub fn checkbox(id: &str, checked: bool) -> CheckboxBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "checked", checked);
    CheckboxBuilder {
        id: id.to_string(),
        props,
    }
}

/// Create a checkbox whose checked state comes from a [`Selection`].
///
/// The checkbox is checked when `item_id` is selected.
pub fn checkbox_for_selection(id: &str, item_id: &str, selection: &Selection) -> CheckboxBuilder {
    checkbox(id, selection.is_selected(item_id))
}

impl CheckboxBuilder {
    /// Set the accessible or displayed label.
    pub fn label(mut self, l: &str) -> Self {
        super::set_prop(&mut self.props, "label", l);
        self
    }
    /// Set the spacing between children, in pixels.
    pub fn spacing(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "spacing", s.into().wire_encode());
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    /// Set the font size in pixels.
    pub fn size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "size", s.into().wire_encode());
        self
    }
    /// Body text size in pixels.
    pub fn text_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "text_size", s.into().wire_encode());
        self
    }
    /// Set the font family and weight.
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
        self
    }
    /// Set the window icon from a file path.
    pub fn icon(mut self, icon: PropValue) -> Self {
        super::set_prop(&mut self.props, "icon", icon);
        self
    }
    /// Line height (pixels or multiplier of font size).
    pub fn line_height(mut self, lh: impl Into<Animatable<LineHeight>>) -> Self {
        super::set_prop(&mut self.props, "line_height", lh.into().wire_encode());
        self
    }
    /// Text shaping strategy (basic or advanced).
    pub fn shaping(mut self, s: Shaping) -> Self {
        super::set_prop(&mut self.props, "shaping", s.wire_encode());
        self
    }
    /// Line-wrap strategy for long content.
    pub fn wrapping(mut self, w: Wrapping) -> Self {
        super::set_prop(&mut self.props, "wrapping", w.wire_encode());
        self
    }
    /// Disable this widget.
    pub fn disabled(mut self, d: bool) -> Self {
        super::set_prop(&mut self.props, "disabled", d);
        self
    }
    /// Set the Alt-key mnemonic used to toggle this checkbox.
    pub fn mnemonic(mut self, mnemonic: char) -> Self {
        super::set_prop(&mut self.props, "mnemonic", mnemonic.to_string());
        self
    }
    /// Alias for [`Self::mnemonic`].
    pub fn access_key(self, access_key: char) -> Self {
        self.mnemonic(access_key)
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second (0 = unbounded).
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    /// Mark this checkbox as required. Flows into `a11y.required`.
    pub fn required(mut self, required: bool) -> Self {
        super::set_prop(&mut self.props, "required", required);
        self
    }
    /// Set the form-validation state. Flows into `a11y.invalid` and
    /// `a11y.error_message`.
    pub fn validation(mut self, v: Validation) -> Self {
        super::set_prop(&mut self.props, "validation", v.wire_encode());
        self
    }
    /// Attach accessibility metadata.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<CheckboxBuilder> for View {
    fn from(b: CheckboxBuilder) -> View {
        super::view_leaf(b.id, "checkbox", b.props)
    }
}

// ---------------------------------------------------------------------------
// toggler
// ---------------------------------------------------------------------------

/// Builder for an on/off toggle switch.
pub struct TogglerBuilder {
    id: String,
    props: PropMap,
}

/// Create a toggler with the given ID and toggle state.
///
/// ```ignore
/// toggler("dark_mode", model.dark_mode).label("Dark mode")
/// ```
pub fn toggler(id: &str, is_toggled: bool) -> TogglerBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "is_toggled", is_toggled);
    TogglerBuilder {
        id: id.to_string(),
        props,
    }
}

impl TogglerBuilder {
    /// Set the accessible or displayed label.
    pub fn label(mut self, l: &str) -> Self {
        super::set_prop(&mut self.props, "label", l);
        self
    }
    /// Set the spacing between children, in pixels.
    pub fn spacing(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "spacing", s.into().wire_encode());
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    /// Set the font size in pixels.
    pub fn size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "size", s.into().wire_encode());
        self
    }
    /// Body text size in pixels.
    pub fn text_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "text_size", s.into().wire_encode());
        self
    }
    /// Set the font family and weight.
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
        self
    }
    /// Line height (pixels or multiplier of font size).
    pub fn line_height(mut self, lh: impl Into<Animatable<LineHeight>>) -> Self {
        super::set_prop(&mut self.props, "line_height", lh.into().wire_encode());
        self
    }
    /// Text shaping strategy (basic or advanced).
    pub fn shaping(mut self, s: Shaping) -> Self {
        super::set_prop(&mut self.props, "shaping", s.wire_encode());
        self
    }
    /// Line-wrap strategy for long content.
    pub fn wrapping(mut self, w: Wrapping) -> Self {
        super::set_prop(&mut self.props, "wrapping", w.wire_encode());
        self
    }
    /// Configure `text_alignment`.
    pub fn text_alignment(mut self, a: HorizontalAlignment) -> Self {
        super::set_prop(&mut self.props, "text_alignment", a.wire_encode());
        self
    }
    /// Disable this widget.
    pub fn disabled(mut self, d: bool) -> Self {
        super::set_prop(&mut self.props, "disabled", d);
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second (0 = unbounded).
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    /// Attach accessibility metadata.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<TogglerBuilder> for View {
    fn from(b: TogglerBuilder) -> View {
        super::view_leaf(b.id, "toggler", b.props)
    }
}

// ---------------------------------------------------------------------------
// radio
// ---------------------------------------------------------------------------

/// Builder for a radio button (one-of-many selection).
pub struct RadioBuilder {
    id: String,
    props: PropMap,
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
    let mut props = PropMap::new();
    super::set_prop(&mut props, "value", value);
    super::set_opt(
        &mut props,
        "selected",
        selected.map(|s| PropValue::Str(s.to_string())),
    );
    RadioBuilder {
        id: id.to_string(),
        props,
    }
}

/// Create a radio button whose selected value comes from a [`Selection`].
///
/// Empty and multi-item selections leave the radio group unselected.
pub fn radio_for_selection(id: &str, value: &str, selection: &Selection) -> RadioBuilder {
    radio(id, value, selection.selected_value())
}

impl RadioBuilder {
    /// Set the accessible or displayed label.
    pub fn label(mut self, l: &str) -> Self {
        super::set_prop(&mut self.props, "label", l);
        self
    }
    /// Configure `group`.
    pub fn group(mut self, g: &str) -> Self {
        super::set_prop(&mut self.props, "group", g);
        self
    }
    /// Set the spacing between children, in pixels.
    pub fn spacing(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "spacing", s.into().wire_encode());
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    /// Set the font size in pixels.
    pub fn size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "size", s.into().wire_encode());
        self
    }
    /// Body text size in pixels.
    pub fn text_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "text_size", s.into().wire_encode());
        self
    }
    /// Set the font family and weight.
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
        self
    }
    /// Line height (pixels or multiplier of font size).
    pub fn line_height(mut self, lh: impl Into<Animatable<LineHeight>>) -> Self {
        super::set_prop(&mut self.props, "line_height", lh.into().wire_encode());
        self
    }
    /// Text shaping strategy (basic or advanced).
    pub fn shaping(mut self, s: Shaping) -> Self {
        super::set_prop(&mut self.props, "shaping", s.wire_encode());
        self
    }
    /// Line-wrap strategy for long content.
    pub fn wrapping(mut self, w: Wrapping) -> Self {
        super::set_prop(&mut self.props, "wrapping", w.wire_encode());
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Set the Alt-key mnemonic used to select this radio button.
    pub fn mnemonic(mut self, mnemonic: char) -> Self {
        super::set_prop(&mut self.props, "mnemonic", mnemonic.to_string());
        self
    }
    /// Alias for [`Self::mnemonic`].
    pub fn access_key(self, access_key: char) -> Self {
        self.mnemonic(access_key)
    }
    /// Maximum events per second (0 = unbounded).
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    /// Attach accessibility metadata.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<RadioBuilder> for View {
    fn from(b: RadioBuilder) -> View {
        super::view_leaf(b.id, "radio", b.props)
    }
}

// ---------------------------------------------------------------------------
// slider
// ---------------------------------------------------------------------------

/// Builder for a horizontal slider.
pub struct SliderBuilder {
    id: String,
    props: PropMap,
}

/// Create a horizontal slider with the given ID, `(min, max)` range, and value.
///
/// ```ignore
/// slider("volume", (0.0, 100.0), model.volume).step(1.0)
/// ```
pub fn slider(id: &str, range: (f32, f32), value: f32) -> SliderBuilder {
    let mut props = PropMap::new();
    super::set_prop(
        &mut props,
        "range",
        PropValue::Array(vec![
            PropValue::F64(range.0 as f64),
            PropValue::F64(range.1 as f64),
        ]),
    );
    super::set_prop(&mut props, "value", value);
    SliderBuilder {
        id: id.to_string(),
        props,
    }
}

impl SliderBuilder {
    /// Set the value adjustment step.
    pub fn step(mut self, s: f32) -> Self {
        super::set_prop(&mut self.props, "step", s);
        self
    }
    /// Override the base step used for keyboard movement and snapping.
    pub fn keyboard_step(mut self, s: f32) -> Self {
        super::set_prop(&mut self.props, "keyboard_step", s);
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    /// Set the preferred height.
    pub fn height(mut self, h: f32) -> Self {
        super::set_prop(&mut self.props, "height", PropValue::F64(h as f64));
        self
    }
    /// Configure `default`.
    pub fn default(mut self, d: f32) -> Self {
        super::set_prop(&mut self.props, "default", d);
        self
    }
    /// Configure `shift_step`.
    pub fn shift_step(mut self, s: f32) -> Self {
        super::set_prop(&mut self.props, "shift_step", s);
        self
    }
    /// Configure `circular_handle`.
    pub fn circular_handle(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "circular_handle", enabled);
        self
    }
    /// Configure `handle_radius`.
    pub fn handle_radius(mut self, r: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "handle_radius", r.into().wire_encode());
        self
    }
    /// Configure `rail_color`.
    pub fn rail_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "rail_color", c.into().wire_encode());
        self
    }
    /// Configure `rail_width`.
    pub fn rail_width(mut self, w: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "rail_width", w.into().wire_encode());
        self
    }
    /// Set the accessible or displayed label.
    pub fn label(mut self, l: &str) -> Self {
        super::set_prop(&mut self.props, "label", l);
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second (0 = unbounded).
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    /// Attach accessibility metadata.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<SliderBuilder> for View {
    fn from(b: SliderBuilder) -> View {
        super::view_leaf(b.id, "slider", b.props)
    }
}

// ---------------------------------------------------------------------------
// vertical_slider
// ---------------------------------------------------------------------------

/// Builder for a vertical slider.
pub struct VerticalSliderBuilder {
    id: String,
    props: PropMap,
}

/// Create a vertical slider with the given ID, `(min, max)` range, and value.
///
/// ```ignore
/// vertical_slider("volume", (0.0, 100.0), model.volume).step(1.0)
/// ```
pub fn vertical_slider(id: &str, range: (f32, f32), value: f32) -> VerticalSliderBuilder {
    let mut props = PropMap::new();
    super::set_prop(
        &mut props,
        "range",
        PropValue::Array(vec![
            PropValue::F64(range.0 as f64),
            PropValue::F64(range.1 as f64),
        ]),
    );
    super::set_prop(&mut props, "value", value);
    VerticalSliderBuilder {
        id: id.to_string(),
        props,
    }
}

impl VerticalSliderBuilder {
    /// Set the value adjustment step.
    pub fn step(mut self, s: f32) -> Self {
        super::set_prop(&mut self.props, "step", s);
        self
    }
    /// Override the base step used for keyboard movement and snapping.
    pub fn keyboard_step(mut self, s: f32) -> Self {
        super::set_prop(&mut self.props, "keyboard_step", s);
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: f32) -> Self {
        super::set_prop(&mut self.props, "width", PropValue::F64(w as f64));
        self
    }
    /// Set the preferred height.
    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }
    /// Configure `default`.
    pub fn default(mut self, d: f32) -> Self {
        super::set_prop(&mut self.props, "default", d);
        self
    }
    /// Configure `shift_step`.
    pub fn shift_step(mut self, s: f32) -> Self {
        super::set_prop(&mut self.props, "shift_step", s);
        self
    }
    /// Configure `rail_color`.
    pub fn rail_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "rail_color", c.into().wire_encode());
        self
    }
    /// Configure `rail_width`.
    pub fn rail_width(mut self, w: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "rail_width", w.into().wire_encode());
        self
    }
    /// Set the accessible or displayed label.
    pub fn label(mut self, l: &str) -> Self {
        super::set_prop(&mut self.props, "label", l);
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second (0 = unbounded).
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    /// Attach accessibility metadata.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<VerticalSliderBuilder> for View {
    fn from(b: VerticalSliderBuilder) -> View {
        super::view_leaf(b.id, "vertical_slider", b.props)
    }
}

// ---------------------------------------------------------------------------
// pick_list
// ---------------------------------------------------------------------------

/// Builder for a dropdown selection list.
pub struct PickListBuilder {
    id: String,
    props: PropMap,
}

/// Create a pick list with the given ID, options, and current selection.
///
/// ```ignore
/// pick_list("color", &["Red", "Green", "Blue"], Some("Red"))
///     .placeholder("Choose a color")
/// ```
pub fn pick_list(id: &str, options: &[&str], selected: Option<&str>) -> PickListBuilder {
    let mut props = PropMap::new();
    let opts: Vec<PropValue> = options
        .iter()
        .map(|s| PropValue::Str(s.to_string()))
        .collect();
    super::set_prop(&mut props, "options", PropValue::Array(opts));
    super::set_opt(
        &mut props,
        "selected",
        selected.map(|s| PropValue::Str(s.to_string())),
    );
    PickListBuilder {
        id: id.to_string(),
        props,
    }
}

/// Create a pick list whose selected value comes from a [`Selection`].
///
/// Empty and multi-item selections leave the pick list unselected.
pub fn pick_list_for_selection(
    id: &str,
    options: &[&str],
    selection: &Selection,
) -> PickListBuilder {
    pick_list(id, options, selection.selected_value())
}

impl PickListBuilder {
    /// Set the placeholder text shown when empty.
    pub fn placeholder(mut self, p: &str) -> Self {
        super::set_prop(&mut self.props, "placeholder", p);
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    /// Set the inner padding.
    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(
            &mut self.props,
            "padding",
            super::padding_to_value(p.into()),
        );
        self
    }
    /// Body text size in pixels.
    pub fn text_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "text_size", s.into().wire_encode());
        self
    }
    /// Set the font family and weight.
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
        self
    }
    /// Line height (pixels or multiplier of font size).
    pub fn line_height(mut self, lh: impl Into<Animatable<LineHeight>>) -> Self {
        super::set_prop(&mut self.props, "line_height", lh.into().wire_encode());
        self
    }
    /// Configure `menu_height`.
    pub fn menu_height(mut self, h: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "menu_height", h.into().wire_encode());
        self
    }
    /// Text shaping strategy (basic or advanced).
    pub fn shaping(mut self, s: Shaping) -> Self {
        super::set_prop(&mut self.props, "shaping", s.wire_encode());
        self
    }
    /// Configure `handle`.
    pub fn handle(mut self, h: PropValue) -> Self {
        super::set_prop(&mut self.props, "handle", h);
        self
    }
    /// Configure trailing ellipsis for truncated text.
    pub fn ellipsis(mut self, e: Ellipsis) -> Self {
        super::set_prop(&mut self.props, "ellipsis", e.wire_encode());
        self
    }
    /// Configure `menu_style`.
    pub fn menu_style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(
            &mut self.props,
            "menu_style",
            super::style_to_value(&s.into()),
        );
        self
    }
    /// Handler for open events.
    pub fn on_open(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_open", enabled);
        self
    }
    /// Handler for close events.
    pub fn on_close(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_close", enabled);
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second (0 = unbounded).
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    /// Mark this field as required. Flows into `a11y.required`.
    pub fn required(mut self, required: bool) -> Self {
        super::set_prop(&mut self.props, "required", required);
        self
    }
    /// Set the form-validation state. Flows into `a11y.invalid` and
    /// `a11y.error_message`.
    pub fn validation(mut self, v: Validation) -> Self {
        super::set_prop(&mut self.props, "validation", v.wire_encode());
        self
    }
    /// Attach accessibility metadata.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<PickListBuilder> for View {
    fn from(b: PickListBuilder) -> View {
        super::view_leaf(b.id, "pick_list", b.props)
    }
}

// ---------------------------------------------------------------------------
// combo_box
// ---------------------------------------------------------------------------

/// Builder for a searchable combo box (dropdown with text input).
pub struct ComboBoxBuilder {
    id: String,
    props: PropMap,
}

/// Create a combo box with the given ID, options, and current text value.
///
/// ```ignore
/// combo_box("lang", &["Rust", "Elixir", "Python"], "Rust")
///     .placeholder("Search languages...")
/// ```
pub fn combo_box(id: &str, options: &[&str], value: &str) -> ComboBoxBuilder {
    combo_box_with_selected(id, options, Some(value))
}

fn combo_box_with_selected(id: &str, options: &[&str], selected: Option<&str>) -> ComboBoxBuilder {
    let mut props = PropMap::new();
    let opts: Vec<PropValue> = options
        .iter()
        .map(|s| PropValue::Str(s.to_string()))
        .collect();
    super::set_prop(&mut props, "options", PropValue::Array(opts));
    super::set_opt(
        &mut props,
        "selected",
        selected.map(|s| PropValue::Str(s.to_string())),
    );
    ComboBoxBuilder {
        id: id.to_string(),
        props,
    }
}

/// Create a combo box whose current value comes from a [`Selection`].
///
/// Empty and multi-item selections leave the combo box unselected.
pub fn combo_box_for_selection(
    id: &str,
    options: &[&str],
    selection: &Selection,
) -> ComboBoxBuilder {
    combo_box_with_selected(id, options, selection.selected_value())
}

impl ComboBoxBuilder {
    /// Set the placeholder text shown when empty.
    pub fn placeholder(mut self, p: &str) -> Self {
        super::set_prop(&mut self.props, "placeholder", p);
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    /// Set the inner padding.
    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(
            &mut self.props,
            "padding",
            super::padding_to_value(p.into()),
        );
        self
    }
    /// Set the font size in pixels.
    pub fn size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "size", s.into().wire_encode());
        self
    }
    /// Set the font family and weight.
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
        self
    }
    /// Line height (pixels or multiplier of font size).
    pub fn line_height(mut self, lh: impl Into<Animatable<LineHeight>>) -> Self {
        super::set_prop(&mut self.props, "line_height", lh.into().wire_encode());
        self
    }
    /// Configure `menu_height`.
    pub fn menu_height(mut self, h: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "menu_height", h.into().wire_encode());
        self
    }
    /// Set the window icon from a file path.
    pub fn icon(mut self, icon: PropValue) -> Self {
        super::set_prop(&mut self.props, "icon", icon);
        self
    }
    /// Handler for option-hovered events.
    pub fn on_option_hovered(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_option_hovered", enabled);
        self
    }
    /// Handler for open events.
    pub fn on_open(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_open", enabled);
        self
    }
    /// Handler for close events.
    pub fn on_close(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_close", enabled);
        self
    }
    /// Text shaping strategy (basic or advanced).
    pub fn shaping(mut self, s: Shaping) -> Self {
        super::set_prop(&mut self.props, "shaping", s.wire_encode());
        self
    }
    /// Configure trailing ellipsis for truncated text.
    pub fn ellipsis(mut self, e: Ellipsis) -> Self {
        super::set_prop(&mut self.props, "ellipsis", e.wire_encode());
        self
    }
    /// Configure `menu_style`.
    pub fn menu_style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(
            &mut self.props,
            "menu_style",
            super::style_to_value(&s.into()),
        );
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second (0 = unbounded).
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    /// Mark this combo box as required. Flows into `a11y.required`.
    pub fn required(mut self, required: bool) -> Self {
        super::set_prop(&mut self.props, "required", required);
        self
    }
    /// Set the form-validation state. Flows into `a11y.invalid` and
    /// `a11y.error_message`.
    pub fn validation(mut self, v: Validation) -> Self {
        super::set_prop(&mut self.props, "validation", v.wire_encode());
        self
    }
    /// Attach accessibility metadata.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<ComboBoxBuilder> for View {
    fn from(b: ComboBoxBuilder) -> View {
        super::view_leaf(b.id, "combo_box", b.props)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkbox_mnemonic_builder_sets_wire_prop() {
        let view: View = checkbox("agree", false).mnemonic('A').into();

        assert_eq!(view.type_name(), "checkbox");
        assert_eq!(view.props().get_str("mnemonic"), Some("A"));
    }

    #[test]
    fn checkbox_access_key_builder_uses_mnemonic_wire_prop() {
        let view: View = checkbox("agree", false).access_key('A').into();

        assert_eq!(view.props().get_str("mnemonic"), Some("A"));
    }

    #[test]
    fn radio_mnemonic_builder_sets_wire_prop() {
        let view: View = radio("small", "small", None).mnemonic('S').into();

        assert_eq!(view.type_name(), "radio");
        assert_eq!(view.props().get_str("mnemonic"), Some("S"));
    }

    #[test]
    fn radio_access_key_builder_uses_mnemonic_wire_prop() {
        let view: View = radio("large", "large", None).access_key('L').into();

        assert_eq!(view.props().get_str("mnemonic"), Some("L"));
    }

    #[test]
    fn slider_keyboard_step_builder_sets_wire_prop() {
        let view: View = slider("volume", (0.0, 100.0), 40.0)
            .step(1.0)
            .keyboard_step(5.0)
            .into();

        assert_eq!(view.type_name(), "slider");
        assert_eq!(view.props().get_f64("step"), Some(1.0));
        assert_eq!(view.props().get_f64("keyboard_step"), Some(5.0));
    }

    #[test]
    fn vertical_slider_keyboard_step_builder_sets_wire_prop() {
        let view: View = vertical_slider("volume", (0.0, 100.0), 40.0)
            .step(1.0)
            .keyboard_step(5.0)
            .into();

        assert_eq!(view.type_name(), "vertical_slider");
        assert_eq!(view.props().get_f64("step"), Some(1.0));
        assert_eq!(view.props().get_f64("keyboard_step"), Some(5.0));
    }
}
