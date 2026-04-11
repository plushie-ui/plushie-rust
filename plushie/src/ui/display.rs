//! Display widget builders (leaf nodes, auto-ID).
//!
//! These widgets display content but don't accept user input.
//! Content or source is the first argument; IDs are auto-generated
//! from the call site and can be overridden with `.id()`.

use super::PropMap;
use serde_json::{Value, json};

use crate::View;
use crate::types::*;

// ---------------------------------------------------------------------------
// text
// ---------------------------------------------------------------------------

/// Builder for a static text label.
pub struct TextBuilder {
    id: String,
    props: PropMap,
}

/// Create a text widget displaying `content`.
///
/// ```ignore
/// text("Hello, world!").size(18.0).color(Color::red())
/// ```
#[track_caller]
pub fn text(content: &str) -> TextBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "content", content);
    TextBuilder { id: super::auto_id("text"), props }
}

impl TextBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "size", s); self }
    pub fn color(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "color", super::color_to_value(&c.into())); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    pub fn align_x(mut self, a: Align) -> Self { super::set_prop(&mut self.props, "align_x", super::halign_to_value(a)); self }
    pub fn align_y(mut self, a: Align) -> Self { super::set_prop(&mut self.props, "align_y", super::valign_to_value(a)); self }
    pub fn wrapping(mut self, w: &str) -> Self { super::set_prop(&mut self.props, "wrapping", w); self }
    pub fn shaping(mut self, s: &str) -> Self { super::set_prop(&mut self.props, "shaping", s); self }
    pub fn line_height(mut self, lh: f32) -> Self { super::set_prop(&mut self.props, "line_height", lh); self }
    pub fn ellipsis(mut self, e: &str) -> Self { super::set_prop(&mut self.props, "ellipsis", e); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }

    /// Animate a property with a timed transition.
    ///
    /// The transition descriptor replaces the prop value. The renderer
    /// interpolates from the current value to the transition's `to`.
    ///
    /// ```ignore
    /// text("value")
    ///     .transition("size", Transition::new(300, 24.0).easing(Easing::EaseOut))
    /// ```
    pub fn transition(mut self, prop: &str, t: crate::animation::Transition) -> Self {
        super::set_prop(&mut self.props, prop, serde_json::to_value(&t).unwrap_or_default());
        self
    }

    /// Animate a property with spring physics.
    pub fn spring(mut self, prop: &str, s: crate::animation::Spring) -> Self {
        super::set_prop(&mut self.props, prop, serde_json::to_value(&s).unwrap_or_default());
        self
    }

    /// Animate a property with a sequence of steps.
    pub fn sequence(mut self, prop: &str, seq: crate::animation::Sequence) -> Self {
        super::set_prop(&mut self.props, prop, serde_json::to_value(&seq).unwrap_or_default());
        self
    }
}

impl From<TextBuilder> for View {
    fn from(b: TextBuilder) -> View {
        super::view_leaf(b.id, "text", b.props)
    }
}

// ---------------------------------------------------------------------------
// rich_text
// ---------------------------------------------------------------------------

/// Builder for a rich text widget with individually styled spans.
pub struct RichTextBuilder {
    id: String,
    props: PropMap,
}

/// Create a rich text widget with an auto-generated ID.
///
/// Use `.spans()` to provide styled text segments.
#[track_caller]
pub fn rich_text() -> RichTextBuilder {
    RichTextBuilder { id: super::auto_id("rich_text"), props: PropMap::new() }
}

/// Create a rich text widget with an explicit ID.
pub fn rich_text_id(id: &str) -> RichTextBuilder {
    RichTextBuilder { id: id.to_string(), props: PropMap::new() }
}

impl RichTextBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn spans(mut self, spans: Vec<Value>) -> Self {
        let pv: Vec<super::PropValue> = spans.into_iter().map(super::PropValue::from).collect();
        super::set_prop(&mut self.props, "spans", super::PropValue::Array(pv)); self
    }
    pub fn size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "size", s); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    pub fn color(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "color", super::color_to_value(&c.into())); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    pub fn line_height(mut self, lh: f32) -> Self { super::set_prop(&mut self.props, "line_height", lh); self }
    pub fn wrapping(mut self, w: &str) -> Self { super::set_prop(&mut self.props, "wrapping", w); self }
    pub fn ellipsis(mut self, e: &str) -> Self { super::set_prop(&mut self.props, "ellipsis", e); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }
}

impl From<RichTextBuilder> for View {
    fn from(b: RichTextBuilder) -> View {
        super::view_leaf(b.id, "rich_text", b.props)
    }
}

// ---------------------------------------------------------------------------
// space
// ---------------------------------------------------------------------------

/// Builder for an invisible spacer widget.
pub struct SpaceBuilder {
    id: String,
    props: PropMap,
}

/// Create an empty space widget for layout purposes.
#[track_caller]
pub fn space() -> SpaceBuilder {
    SpaceBuilder { id: super::auto_id("space"), props: PropMap::new() }
}

impl SpaceBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }
}

impl From<SpaceBuilder> for View {
    fn from(b: SpaceBuilder) -> View {
        super::view_leaf(b.id, "space", b.props)
    }
}

// ---------------------------------------------------------------------------
// rule
// ---------------------------------------------------------------------------

/// Builder for a horizontal or vertical divider line.
pub struct RuleBuilder {
    id: String,
    props: PropMap,
}

/// Create a rule (divider) widget.
#[track_caller]
pub fn rule() -> RuleBuilder {
    RuleBuilder { id: super::auto_id("rule"), props: PropMap::new() }
}

impl RuleBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn width(mut self, w: f32) -> Self { super::set_prop(&mut self.props, "width", w); self }
    pub fn height(mut self, h: f32) -> Self { super::set_prop(&mut self.props, "height", h); self }
    pub fn direction(mut self, d: &str) -> Self { super::set_prop(&mut self.props, "direction", d); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }
}

impl From<RuleBuilder> for View {
    fn from(b: RuleBuilder) -> View {
        super::view_leaf(b.id, "rule", b.props)
    }
}

// ---------------------------------------------------------------------------
// progress_bar
// ---------------------------------------------------------------------------

/// Builder for a progress bar.
pub struct ProgressBarBuilder {
    id: String,
    props: PropMap,
}

/// Create a progress bar with the given `(min, max)` range and current value.
///
/// ```ignore
/// progress_bar((0.0, 100.0), 42.0).style(Style::primary())
/// ```
#[track_caller]
pub fn progress_bar(range: (f32, f32), value: f32) -> ProgressBarBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "range", json!([range.0, range.1]));
    super::set_prop(&mut props, "value", value);
    ProgressBarBuilder { id: super::auto_id("progress_bar"), props }
}

impl ProgressBarBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    /// Render as a vertical bar instead of horizontal.
    pub fn vertical(mut self, v: bool) -> Self { super::set_prop(&mut self.props, "vertical", v); self }
    /// Accessible label for the progress bar.
    pub fn label(mut self, l: &str) -> Self { super::set_prop(&mut self.props, "label", l); self }
    pub fn style(mut self, s: impl Into<Style>) -> Self { super::set_prop(&mut self.props, "style", super::style_to_value(&s.into())); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }

    /// Animate a property with a timed transition.
    pub fn transition(mut self, prop: &str, t: crate::animation::Transition) -> Self {
        super::set_prop(&mut self.props, prop, serde_json::to_value(&t).unwrap_or_default());
        self
    }

    /// Animate a property with spring physics.
    pub fn spring(mut self, prop: &str, s: crate::animation::Spring) -> Self {
        super::set_prop(&mut self.props, prop, serde_json::to_value(&s).unwrap_or_default());
        self
    }

    /// Animate a property with a sequence of steps.
    pub fn sequence(mut self, prop: &str, seq: crate::animation::Sequence) -> Self {
        super::set_prop(&mut self.props, prop, serde_json::to_value(&seq).unwrap_or_default());
        self
    }
}

impl From<ProgressBarBuilder> for View {
    fn from(b: ProgressBarBuilder) -> View {
        super::view_leaf(b.id, "progress_bar", b.props)
    }
}

// ---------------------------------------------------------------------------
// image
// ---------------------------------------------------------------------------

/// Builder for a raster image.
pub struct ImageBuilder {
    id: String,
    props: PropMap,
}

/// Create an image widget from a file path or handle name.
///
/// ```ignore
/// image("assets/logo.png").width(200.0).content_fit("contain")
/// ```
#[track_caller]
pub fn image(source: &str) -> ImageBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "source", source);
    ImageBuilder { id: super::auto_id("image"), props }
}

impl ImageBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    pub fn content_fit(mut self, fit: &str) -> Self { super::set_prop(&mut self.props, "content_fit", fit); self }
    pub fn filter_method(mut self, method: &str) -> Self { super::set_prop(&mut self.props, "filter_method", method); self }
    pub fn rotation(mut self, degrees: f32) -> Self { super::set_prop(&mut self.props, "rotation", degrees); self }
    pub fn opacity(mut self, o: f32) -> Self { super::set_prop(&mut self.props, "opacity", o); self }
    /// Corner radius for rounded image borders.
    pub fn border_radius(mut self, r: f32) -> Self { super::set_prop(&mut self.props, "border_radius", r); self }
    /// Expand the image to fill available space.
    pub fn expand(mut self, v: bool) -> Self { super::set_prop(&mut self.props, "expand", v); self }
    /// Scale factor applied to the image.
    pub fn scale(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "scale", s); self }
    /// Crop to a pixel rectangle within the source image.
    pub fn crop(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        super::set_prop(&mut self.props, "crop", json!({"x": x, "y": y, "width": width, "height": height}));
        self
    }
    pub fn alt(mut self, alt: &str) -> Self { super::set_prop(&mut self.props, "alt", alt); self }
    /// Extended accessible description (longer than `alt`).
    pub fn description(mut self, desc: &str) -> Self { super::set_prop(&mut self.props, "description", desc); self }
    /// When true, hides the image from assistive technology.
    pub fn decorative(mut self, v: bool) -> Self { super::set_prop(&mut self.props, "decorative", v); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }

    /// Animate a property with a timed transition.
    pub fn transition(mut self, prop: &str, t: crate::animation::Transition) -> Self {
        super::set_prop(&mut self.props, prop, serde_json::to_value(&t).unwrap_or_default());
        self
    }

    /// Animate a property with spring physics.
    pub fn spring(mut self, prop: &str, s: crate::animation::Spring) -> Self {
        super::set_prop(&mut self.props, prop, serde_json::to_value(&s).unwrap_or_default());
        self
    }

    /// Animate a property with a sequence of steps.
    pub fn sequence(mut self, prop: &str, seq: crate::animation::Sequence) -> Self {
        super::set_prop(&mut self.props, prop, serde_json::to_value(&seq).unwrap_or_default());
        self
    }
}

impl From<ImageBuilder> for View {
    fn from(b: ImageBuilder) -> View {
        super::view_leaf(b.id, "image", b.props)
    }
}

// ---------------------------------------------------------------------------
// svg
// ---------------------------------------------------------------------------

/// Builder for a vector SVG image.
pub struct SvgBuilder {
    id: String,
    props: PropMap,
}

/// Create an SVG widget from a file path.
///
/// ```ignore
/// svg("assets/icon.svg").width(24.0).color(Color::white())
/// ```
#[track_caller]
pub fn svg(source: &str) -> SvgBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "source", source);
    SvgBuilder { id: super::auto_id("svg"), props }
}

impl SvgBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    pub fn color(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "color", super::color_to_value(&c.into())); self }
    pub fn content_fit(mut self, fit: &str) -> Self { super::set_prop(&mut self.props, "content_fit", fit); self }
    pub fn rotation(mut self, degrees: f32) -> Self { super::set_prop(&mut self.props, "rotation", degrees); self }
    pub fn opacity(mut self, o: f32) -> Self { super::set_prop(&mut self.props, "opacity", o); self }
    pub fn alt(mut self, alt: &str) -> Self { super::set_prop(&mut self.props, "alt", alt); self }
    /// Extended accessible description (longer than `alt`).
    pub fn description(mut self, desc: &str) -> Self { super::set_prop(&mut self.props, "description", desc); self }
    /// When true, hides the SVG from assistive technology.
    pub fn decorative(mut self, v: bool) -> Self { super::set_prop(&mut self.props, "decorative", v); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }
}

impl From<SvgBuilder> for View {
    fn from(b: SvgBuilder) -> View {
        super::view_leaf(b.id, "svg", b.props)
    }
}

// ---------------------------------------------------------------------------
// markdown
// ---------------------------------------------------------------------------

/// Builder for a rendered markdown document.
pub struct MarkdownBuilder {
    id: String,
    props: PropMap,
}

/// Create a markdown widget from raw markdown text.
///
/// ```ignore
/// markdown("# Hello\n\nSome **bold** text.").width(Length::Fill)
/// ```
#[track_caller]
pub fn markdown(content: &str) -> MarkdownBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "content", content);
    MarkdownBuilder { id: super::auto_id("markdown"), props }
}

impl MarkdownBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn text_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "text_size", s); self }
    pub fn h1_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "h1_size", s); self }
    pub fn h2_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "h2_size", s); self }
    pub fn h3_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "h3_size", s); self }
    pub fn code_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "code_size", s); self }
    pub fn spacing(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "spacing", s); self }
    pub fn link_color(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "link_color", super::color_to_value(&c.into())); self }
    pub fn code_theme(mut self, theme: &str) -> Self { super::set_prop(&mut self.props, "code_theme", theme); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }
}

impl From<MarkdownBuilder> for View {
    fn from(b: MarkdownBuilder) -> View {
        super::view_leaf(b.id, "markdown", b.props)
    }
}

// ---------------------------------------------------------------------------
// qr_code
// ---------------------------------------------------------------------------

/// Builder for a QR code.
pub struct QrCodeBuilder {
    id: String,
    props: PropMap,
}

/// Create a QR code widget encoding the given data string.
///
/// ```ignore
/// qr_code("https://example.com").cell_size(6.0)
/// ```
#[track_caller]
pub fn qr_code(data: &str) -> QrCodeBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "data", data);
    QrCodeBuilder { id: super::auto_id("qr_code"), props }
}

impl QrCodeBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn cell_size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "cell_size", s); self }
    pub fn width(mut self, w: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "width", super::length_to_value(w.into())); self }
    pub fn height(mut self, h: impl Into<Length>) -> Self { super::set_prop(&mut self.props, "height", super::length_to_value(h.into())); self }
    pub fn error_correction(mut self, level: &str) -> Self { super::set_prop(&mut self.props, "error_correction", level); self }
    pub fn cell_color(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "cell_color", super::color_to_value(&c.into())); self }
    pub fn background(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "background", super::color_to_value(&c.into())); self }
    /// Accessible label for the QR code.
    pub fn alt(mut self, alt: &str) -> Self { super::set_prop(&mut self.props, "alt", alt); self }
    /// Extended accessible description.
    pub fn description(mut self, desc: &str) -> Self { super::set_prop(&mut self.props, "description", desc); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }
}

impl From<QrCodeBuilder> for View {
    fn from(b: QrCodeBuilder) -> View {
        super::view_leaf(b.id, "qr_code", b.props)
    }
}
