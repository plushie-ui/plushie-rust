//! Display widget builders (leaf nodes, auto-ID).
//!
//! These widgets display content but don't accept user input.
//! Content or source is the first argument; IDs are auto-generated
//! from the call site and can be overridden with `.id()`.

use super::{PropMap, PropValue};

use crate::View;
use crate::derive_support::PlushieType;
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
    TextBuilder {
        id: super::auto_id("text"),
        props,
    }
}

impl TextBuilder {
    /// Override the auto-generated node ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    /// Set the font size in pixels.
    pub fn size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "size", s.into().wire_encode());
        self
    }
    /// Set the text color.
    pub fn color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "color", c.into().wire_encode());
        self
    }
    /// Set the font family and weight.
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
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
    /// Set the horizontal alignment within the text bounding box.
    pub fn align_x(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_x", super::halign_to_value(a));
        self
    }
    /// Set the vertical alignment within the text bounding box.
    pub fn align_y(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_y", super::valign_to_value(a));
        self
    }
    /// Set the line-wrap strategy for long content.
    pub fn wrapping(mut self, w: Wrapping) -> Self {
        super::set_prop(&mut self.props, "wrapping", w.wire_encode());
        self
    }
    /// Set the text shaping strategy (basic or advanced).
    pub fn shaping(mut self, s: Shaping) -> Self {
        super::set_prop(&mut self.props, "shaping", s.wire_encode());
        self
    }
    /// Set the line height (absolute pixels or a multiplier of the font size).
    pub fn line_height(mut self, lh: impl Into<Animatable<LineHeight>>) -> Self {
        super::set_prop(&mut self.props, "line_height", lh.into().wire_encode());
        self
    }
    /// Configure the trailing ellipsis for truncated text.
    pub fn ellipsis(mut self, e: Ellipsis) -> Self {
        super::set_prop(&mut self.props, "ellipsis", e.wire_encode());
        self
    }
    /// Apply a named or custom style to the text.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second emitted by this widget (0 = unbounded).
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

impl From<TextBuilder> for View {
    fn from(b: TextBuilder) -> View {
        super::view_leaf(b.id, "text", b.props)
    }
}

// ---------------------------------------------------------------------------
// rich_text
// ---------------------------------------------------------------------------

/// Highlight backdrop drawn behind a [`Span`]'s text.
///
/// `background` paints a solid colour; `border` adds a stroke around
/// the highlighted region. Both fields are optional.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpanHighlight {
    /// Solid background colour.
    pub background: Option<Color>,
    /// Border drawn around the highlighted region.
    pub border: Option<Border>,
}

impl SpanHighlight {
    /// Empty highlight; both fields default to `None`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the background colour.
    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.background = Some(color.into());
        self
    }

    /// Set the border.
    pub fn border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    fn wire_encode(&self) -> PropValue {
        let mut map = PropMap::new();
        if let Some(ref bg) = self.background {
            map.insert("background", bg.wire_encode());
        }
        if let Some(ref b) = self.border {
            map.insert("border", b.wire_encode());
        }
        PropValue::Object(map)
    }
}

/// A typed span for the [`rich_text`] widget.
///
/// Each span carries one segment of text with its own optional
/// styling. Unset fields fall back to the rich_text widget's
/// defaults.
///
/// ```ignore
/// use plushie::ui::Span;
///
/// let title = Span::new("Build ").color(Color::black());
/// let ok = Span::new("ok").color(Color::green()).underline(true);
/// rich_text().spans(vec![title, ok]);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Span {
    /// Required text content for this span.
    pub text: String,
    /// Font size in pixels.
    pub size: Option<f32>,
    /// Font family and weight.
    pub font: Option<Font>,
    /// Text colour.
    pub color: Option<Color>,
    /// Line height for this span.
    pub line_height: Option<LineHeight>,
    /// Hyperlink URL; clicks emit a `link_click` event with this value.
    pub link: Option<String>,
    /// Underline decoration.
    pub underline: Option<bool>,
    /// Strikethrough decoration.
    pub strikethrough: Option<bool>,
    /// Padding around the span's text.
    pub padding: Option<Padding>,
    /// Highlight backdrop.
    pub highlight: Option<SpanHighlight>,
}

impl Span {
    /// Build a span with the given text and no style overrides.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// Set the font size in pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    /// Set the font.
    pub fn font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
    }

    /// Set the text colour.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Set the line height.
    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        self.line_height = Some(line_height.into());
        self
    }

    /// Set the hyperlink URL.
    pub fn link(mut self, url: impl Into<String>) -> Self {
        self.link = Some(url.into());
        self
    }

    /// Set the underline decoration.
    pub fn underline(mut self, on: bool) -> Self {
        self.underline = Some(on);
        self
    }

    /// Set the strikethrough decoration.
    pub fn strikethrough(mut self, on: bool) -> Self {
        self.strikethrough = Some(on);
        self
    }

    /// Set the padding around the span's text.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = Some(padding.into());
        self
    }

    /// Set the highlight backdrop.
    pub fn highlight(mut self, highlight: SpanHighlight) -> Self {
        self.highlight = Some(highlight);
        self
    }

    /// Encode the span to its wire `PropValue` form.
    pub fn wire_encode(&self) -> PropValue {
        let mut map = PropMap::new();
        map.insert("text", PropValue::Str(self.text.clone()));
        if let Some(size) = self.size {
            map.insert("size", PropValue::F64(size as f64));
        }
        if let Some(ref font) = self.font {
            map.insert("font", font.wire_encode());
        }
        if let Some(ref color) = self.color {
            map.insert("color", color.wire_encode());
        }
        if let Some(lh) = self.line_height {
            map.insert("line_height", lh.wire_encode());
        }
        if let Some(ref link) = self.link {
            map.insert("link", PropValue::Str(link.clone()));
        }
        if let Some(u) = self.underline {
            map.insert("underline", PropValue::Bool(u));
        }
        if let Some(s) = self.strikethrough {
            map.insert("strikethrough", PropValue::Bool(s));
        }
        if let Some(ref p) = self.padding {
            map.insert("padding", p.wire_encode());
        }
        if let Some(ref h) = self.highlight {
            map.insert("highlight", h.wire_encode());
        }
        PropValue::Object(map)
    }
}

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
    RichTextBuilder {
        id: super::auto_id("rich_text"),
        props: PropMap::new(),
    }
}

/// Create a rich text widget with an explicit ID.
pub fn rich_text_id(id: &str) -> RichTextBuilder {
    RichTextBuilder {
        id: id.to_string(),
        props: PropMap::new(),
    }
}

impl RichTextBuilder {
    /// Override the node ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    /// Set the ordered list of typed [`Span`]s that make up the text.
    pub fn spans(mut self, spans: Vec<Span>) -> Self {
        let encoded: Vec<PropValue> = spans.iter().map(Span::wire_encode).collect();
        super::set_prop(&mut self.props, "spans", PropValue::Array(encoded));
        self
    }
    /// Default font size for spans that don't override it (pixels).
    pub fn size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "size", s.into().wire_encode());
        self
    }
    /// Default font for spans that don't override it.
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
        self
    }
    /// Default text color for spans that don't override it.
    pub fn color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "color", c.into().wire_encode());
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
    /// Set the line height (absolute pixels or a multiplier of the font size).
    pub fn line_height(mut self, lh: impl Into<Animatable<LineHeight>>) -> Self {
        super::set_prop(&mut self.props, "line_height", lh.into().wire_encode());
        self
    }
    /// Set the line-wrap strategy for long content.
    pub fn wrapping(mut self, w: Wrapping) -> Self {
        super::set_prop(&mut self.props, "wrapping", w.wire_encode());
        self
    }
    /// Configure the trailing ellipsis for truncated text.
    pub fn ellipsis(mut self, e: Ellipsis) -> Self {
        super::set_prop(&mut self.props, "ellipsis", e.wire_encode());
        self
    }
    /// Maximum events per second emitted by this widget (0 = unbounded).
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
    SpaceBuilder {
        id: super::auto_id("space"),
        props: PropMap::new(),
    }
}

impl SpaceBuilder {
    /// Override the auto-generated node ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
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
    /// Maximum events per second emitted by this widget (0 = unbounded).
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
    RuleBuilder {
        id: super::auto_id("rule"),
        props: PropMap::new(),
    }
}

impl RuleBuilder {
    /// Override the auto-generated node ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    /// Set the rule width in pixels (for vertical rules) or as a length.
    pub fn width(mut self, w: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "width", w.into().wire_encode());
        self
    }
    /// Set the rule height in pixels (for horizontal rules) or as a length.
    pub fn height(mut self, h: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "height", h.into().wire_encode());
        self
    }
    /// Set the rule direction (horizontal or vertical).
    pub fn direction(mut self, d: Direction) -> Self {
        super::set_prop(&mut self.props, "direction", d.wire_encode());
        self
    }
    /// Set the rule thickness in pixels.
    ///
    /// Direction-agnostic alias: horizontal rules use this as height,
    /// vertical rules use this as width. Explicit `width` / `height`
    /// take precedence for the matching axis.
    pub fn thickness(mut self, t: f32) -> Self {
        super::set_prop(&mut self.props, "thickness", t);
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second emitted by this widget (0 = unbounded).
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
    super::set_prop(
        &mut props,
        "range",
        PropValue::Array(vec![
            PropValue::F64(range.0 as f64),
            PropValue::F64(range.1 as f64),
        ]),
    );
    super::set_prop(&mut props, "value", value);
    ProgressBarBuilder {
        id: super::auto_id("progress_bar"),
        props,
    }
}

impl ProgressBarBuilder {
    /// Override the auto-generated node ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
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
    /// Render as a vertical bar instead of horizontal.
    pub fn vertical(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "vertical", v);
        self
    }
    /// Accessible label for the progress bar.
    pub fn label(mut self, l: &str) -> Self {
        super::set_prop(&mut self.props, "label", l);
        self
    }
    /// Apply a named or custom style.
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }
    /// Maximum events per second emitted by this widget (0 = unbounded).
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
/// image("assets/logo.png").width(200.0).content_fit(ContentFit::Contain)
/// ```
#[track_caller]
pub fn image(source: &str) -> ImageBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "source", source);
    ImageBuilder {
        id: super::auto_id("image"),
        props,
    }
}

impl ImageBuilder {
    /// Override the auto-generated node ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
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
    /// Control how the image scales to fit its bounds.
    pub fn content_fit(mut self, fit: ContentFit) -> Self {
        super::set_prop(&mut self.props, "content_fit", fit.wire_encode());
        self
    }
    /// Select the pixel interpolation method (nearest, linear).
    pub fn filter_method(mut self, method: FilterMethod) -> Self {
        super::set_prop(&mut self.props, "filter_method", method.wire_encode());
        self
    }
    /// Rotation angle. Supports animation via [`Animatable`].
    ///
    /// Bare numbers are degrees (matching the cross-SDK convention):
    /// ```ignore
    /// image("photo", "/img/hero.png").rotation(45.0)
    /// image("photo", "/img/hero.png").rotation(Angle::rad(PI / 4.0))
    /// image("photo", "/img/hero.png").rotation(Transition::new(Angle::deg(90.0), 300))
    /// ```
    pub fn rotation(mut self, angle: impl Into<Animatable<Angle>>) -> Self {
        super::set_prop(&mut self.props, "rotation", angle.into().wire_encode());
        self
    }
    /// Alpha multiplier in the range `0.0..=1.0`.
    pub fn opacity(mut self, o: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "opacity", o.into().wire_encode());
        self
    }
    /// Corner radius for rounded image borders.
    pub fn border_radius(mut self, r: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "border_radius", r.into().wire_encode());
        self
    }
    /// Expand the image to fill available space.
    pub fn expand(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "expand", v);
        self
    }
    /// Scale factor applied to the image.
    pub fn scale(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "scale", s.into().wire_encode());
        self
    }
    /// Crop to a pixel rectangle within the source image.
    pub fn crop(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        let mut crop = PropMap::new();
        crop.insert("x", PropValue::F64(x as f64));
        crop.insert("y", PropValue::F64(y as f64));
        crop.insert("width", PropValue::F64(width as f64));
        crop.insert("height", PropValue::F64(height as f64));
        super::set_prop(&mut self.props, "crop", PropValue::Object(crop));
        self
    }
    /// Short accessible description (alt text). Flows into the a11y label.
    pub fn alt(mut self, alt: &str) -> Self {
        super::set_prop(&mut self.props, "alt", alt);
        self
    }
    /// Extended accessible description (longer than `alt`).
    pub fn description(mut self, desc: &str) -> Self {
        super::set_prop(&mut self.props, "description", desc);
        self
    }
    /// When true, hides the image from assistive technology.
    pub fn decorative(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "decorative", v);
        self
    }
    /// Maximum events per second emitted by this widget (0 = unbounded).
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
    SvgBuilder {
        id: super::auto_id("svg"),
        props,
    }
}

impl SvgBuilder {
    /// Override the auto-generated node ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
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
    /// Tint color. Replaces the SVG's painted color with this value.
    pub fn color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "color", c.into().wire_encode());
        self
    }
    /// Control how the SVG scales to fit its bounds.
    pub fn content_fit(mut self, fit: ContentFit) -> Self {
        super::set_prop(&mut self.props, "content_fit", fit.wire_encode());
        self
    }
    /// Rotation angle. Supports animation via [`Animatable`].
    ///
    /// Bare numbers are degrees (matching the cross-SDK convention):
    /// ```ignore
    /// image("photo", "/img/hero.png").rotation(45.0)
    /// image("photo", "/img/hero.png").rotation(Angle::rad(PI / 4.0))
    /// image("photo", "/img/hero.png").rotation(Transition::new(Angle::deg(90.0), 300))
    /// ```
    pub fn rotation(mut self, angle: impl Into<Animatable<Angle>>) -> Self {
        super::set_prop(&mut self.props, "rotation", angle.into().wire_encode());
        self
    }
    /// Alpha multiplier in the range `0.0..=1.0`.
    pub fn opacity(mut self, o: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "opacity", o.into().wire_encode());
        self
    }
    /// Short accessible description (alt text). Flows into the a11y label.
    pub fn alt(mut self, alt: &str) -> Self {
        super::set_prop(&mut self.props, "alt", alt);
        self
    }
    /// Extended accessible description (longer than `alt`).
    pub fn description(mut self, desc: &str) -> Self {
        super::set_prop(&mut self.props, "description", desc);
        self
    }
    /// When true, hides the SVG from assistive technology.
    pub fn decorative(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "decorative", v);
        self
    }
    /// Maximum events per second emitted by this widget (0 = unbounded).
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
    MarkdownBuilder {
        id: super::auto_id("markdown"),
        props,
    }
}

impl MarkdownBuilder {
    /// Override the auto-generated node ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    /// Set the preferred width.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    /// Body text size in pixels.
    pub fn text_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "text_size", s.into().wire_encode());
        self
    }
    /// Size in pixels for `#` headings.
    pub fn h1_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "h1_size", s.into().wire_encode());
        self
    }
    /// Size in pixels for `##` headings.
    pub fn h2_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "h2_size", s.into().wire_encode());
        self
    }
    /// Size in pixels for `###` and deeper headings.
    pub fn h3_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "h3_size", s.into().wire_encode());
        self
    }
    /// Size in pixels for inline and fenced code.
    pub fn code_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "code_size", s.into().wire_encode());
        self
    }
    /// Vertical spacing between blocks, in pixels.
    pub fn spacing(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "spacing", s.into().wire_encode());
        self
    }
    /// Color applied to hyperlink runs.
    pub fn link_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "link_color", c.into().wire_encode());
        self
    }
    /// Syntax-highlight theme name for fenced code blocks.
    pub fn code_theme(mut self, theme: &str) -> Self {
        super::set_prop(&mut self.props, "code_theme", theme);
        self
    }
    /// Maximum events per second emitted by this widget (0 = unbounded).
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
    QrCodeBuilder {
        id: super::auto_id("qr_code"),
        props,
    }
}

impl QrCodeBuilder {
    /// Override the auto-generated node ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    /// Side length of an individual QR module (cell) in pixels.
    pub fn cell_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "cell_size", s.into().wire_encode());
        self
    }
    /// Total rendered side length in pixels.
    pub fn total_size(mut self, s: f32) -> Self {
        super::set_prop(&mut self.props, "total_size", s);
        self
    }
    /// Set the QR error-correction level.
    pub fn error_correction(mut self, level: ErrorCorrection) -> Self {
        super::set_prop(&mut self.props, "error_correction", level.wire_encode());
        self
    }
    /// Color of the filled cells (typically the dark color).
    pub fn cell_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "cell_color", c.into().wire_encode());
        self
    }
    /// Background color (typically the light color).
    pub fn background(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "background", c.into().wire_encode());
        self
    }
    /// Accessible label for the QR code.
    pub fn alt(mut self, alt: &str) -> Self {
        super::set_prop(&mut self.props, "alt", alt);
        self
    }
    /// Extended accessible description.
    pub fn description(mut self, desc: &str) -> Self {
        super::set_prop(&mut self.props, "description", desc);
        self
    }
    /// Maximum events per second emitted by this widget (0 = unbounded).
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

impl From<QrCodeBuilder> for View {
    fn from(b: QrCodeBuilder) -> View {
        super::view_leaf(b.id, "qr_code", b.props)
    }
}
