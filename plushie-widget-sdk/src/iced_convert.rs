//! Conversion layer from plushie-core types to iced types.
//!
//! This is the single location where plushie-core's domain types meet
//! iced's rendering types. Neither crate is owned by plushie-widget-sdk,
//! so Rust's orphan rule prevents `From` impls. Instead we provide
//! named conversion functions.

use iced::advanced::widget::operation::accessible;
use iced::widget::canvas;
use iced::widget::text;

use crate::theming::parse_hex_color;
use crate::widget::helpers::intern_font_family;

use plushie_core::types;

// -------------------------------------------------------------------------
// Color
// -------------------------------------------------------------------------

/// Convert a plushie-core Color (hex string) to an iced Color.
/// Returns `iced::Color::TRANSPARENT` for unparseable hex values.
pub fn color(c: &types::Color) -> iced::Color {
    parse_hex_color(c.as_hex()).unwrap_or(iced::Color::TRANSPARENT)
}

// -------------------------------------------------------------------------
// Length
// -------------------------------------------------------------------------

pub fn length(l: &types::Length) -> iced::Length {
    match *l {
        types::Length::Fill => iced::Length::Fill,
        types::Length::Shrink => iced::Length::Shrink,
        types::Length::FillPortion(n) => iced::Length::FillPortion(n),
        types::Length::Fixed(f) => iced::Length::Fixed(f),
    }
}

// -------------------------------------------------------------------------
// Padding
// -------------------------------------------------------------------------

pub fn padding(p: &types::Padding) -> iced::Padding {
    iced::Padding {
        top: p.top,
        right: p.right,
        bottom: p.bottom,
        left: p.left,
    }
}

// -------------------------------------------------------------------------
// FontWeight
// -------------------------------------------------------------------------

pub fn font_weight(w: types::FontWeight) -> iced::font::Weight {
    match w {
        types::FontWeight::Thin => iced::font::Weight::Thin,
        types::FontWeight::ExtraLight => iced::font::Weight::ExtraLight,
        types::FontWeight::Light => iced::font::Weight::Light,
        types::FontWeight::Normal => iced::font::Weight::Normal,
        types::FontWeight::Medium => iced::font::Weight::Medium,
        types::FontWeight::SemiBold => iced::font::Weight::Semibold,
        types::FontWeight::Bold => iced::font::Weight::Bold,
        types::FontWeight::ExtraBold => iced::font::Weight::ExtraBold,
        types::FontWeight::Black => iced::font::Weight::Black,
    }
}

// -------------------------------------------------------------------------
// FontStyle
// -------------------------------------------------------------------------

pub fn font_style(s: types::FontStyle) -> iced::font::Style {
    match s {
        types::FontStyle::Normal => iced::font::Style::Normal,
        types::FontStyle::Italic => iced::font::Style::Italic,
        types::FontStyle::Oblique => iced::font::Style::Oblique,
    }
}

// -------------------------------------------------------------------------
// FontStretch
// -------------------------------------------------------------------------

pub fn font_stretch(s: types::FontStretch) -> iced::font::Stretch {
    match s {
        types::FontStretch::UltraCondensed => iced::font::Stretch::UltraCondensed,
        types::FontStretch::ExtraCondensed => iced::font::Stretch::ExtraCondensed,
        types::FontStretch::Condensed => iced::font::Stretch::Condensed,
        types::FontStretch::SemiCondensed => iced::font::Stretch::SemiCondensed,
        types::FontStretch::Normal => iced::font::Stretch::Normal,
        types::FontStretch::SemiExpanded => iced::font::Stretch::SemiExpanded,
        types::FontStretch::Expanded => iced::font::Stretch::Expanded,
        types::FontStretch::ExtraExpanded => iced::font::Stretch::ExtraExpanded,
        types::FontStretch::UltraExpanded => iced::font::Stretch::UltraExpanded,
    }
}

// -------------------------------------------------------------------------
// Font
// -------------------------------------------------------------------------

/// Convert a plushie-core Font to an iced Font.
///
/// Shorthand strings: "default"/"sans_serif" -> `Font::DEFAULT`,
/// "monospace" -> `Font::MONOSPACE`. Custom family names are interned
/// for the `'static` lifetime iced requires.
pub fn font(f: &types::Font) -> iced::Font {
    // Bare shorthand: no weight/style/stretch overrides.
    if f.weight.is_none() && f.style.is_none() && f.stretch.is_none() {
        match f.family.as_deref() {
            None | Some("default") | Some("sans_serif") => return iced::Font::DEFAULT,
            Some("monospace") => return iced::Font::MONOSPACE,
            _ => {}
        }
    }

    let mut out = iced::Font::DEFAULT;

    if let Some(ref family) = f.family {
        out.family = match family.as_str() {
            "monospace" => iced::font::Family::Monospace,
            "serif" => iced::font::Family::Serif,
            "cursive" => iced::font::Family::Cursive,
            "fantasy" => iced::font::Family::Fantasy,
            "default" | "sans_serif" | "" => iced::font::Family::SansSerif,
            other => iced::font::Family::Name(intern_font_family(other)),
        };
    }

    if let Some(w) = f.weight {
        out.weight = font_weight(w);
    }
    if let Some(s) = f.style {
        out.style = font_style(s);
    }
    if let Some(s) = f.stretch {
        out.stretch = font_stretch(s);
    }

    out
}

// -------------------------------------------------------------------------
// Radius
// -------------------------------------------------------------------------

pub fn radius(r: types::Radius) -> iced::border::Radius {
    match r {
        types::Radius::Uniform(v) => v.into(),
        types::Radius::PerCorner {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        } => iced::border::Radius {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        },
    }
}

// -------------------------------------------------------------------------
// Border
// -------------------------------------------------------------------------

pub fn border(b: &types::Border) -> iced::Border {
    iced::Border {
        color: b
            .color
            .as_ref()
            .map(color)
            .unwrap_or(iced::Color::TRANSPARENT),
        width: b.width,
        radius: radius(b.radius),
    }
}

// -------------------------------------------------------------------------
// Shadow
// -------------------------------------------------------------------------

pub fn shadow(s: &types::Shadow) -> iced::Shadow {
    iced::Shadow {
        color: color(&s.color),
        offset: iced::Vector::new(s.offset_x, s.offset_y),
        blur_radius: s.blur_radius,
    }
}

// -------------------------------------------------------------------------
// HorizontalAlignment
// -------------------------------------------------------------------------

pub fn horizontal_alignment(a: types::HorizontalAlignment) -> iced::alignment::Horizontal {
    match a {
        types::HorizontalAlignment::Left => iced::alignment::Horizontal::Left,
        types::HorizontalAlignment::Center => iced::alignment::Horizontal::Center,
        types::HorizontalAlignment::Right => iced::alignment::Horizontal::Right,
    }
}

// -------------------------------------------------------------------------
// VerticalAlignment
// -------------------------------------------------------------------------

pub fn vertical_alignment(a: types::VerticalAlignment) -> iced::alignment::Vertical {
    match a {
        types::VerticalAlignment::Top => iced::alignment::Vertical::Top,
        types::VerticalAlignment::Center => iced::alignment::Vertical::Center,
        types::VerticalAlignment::Bottom => iced::alignment::Vertical::Bottom,
    }
}

// -------------------------------------------------------------------------
// Wrapping
// -------------------------------------------------------------------------

pub fn wrapping(w: types::Wrapping) -> text::Wrapping {
    match w {
        types::Wrapping::None => text::Wrapping::None,
        types::Wrapping::Word => text::Wrapping::Word,
        types::Wrapping::Glyph => text::Wrapping::Glyph,
        types::Wrapping::WordOrGlyph => text::Wrapping::WordOrGlyph,
    }
}

// -------------------------------------------------------------------------
// Shaping
// -------------------------------------------------------------------------

pub fn shaping(s: types::Shaping) -> text::Shaping {
    match s {
        types::Shaping::Basic => text::Shaping::Basic,
        types::Shaping::Advanced => text::Shaping::Advanced,
        types::Shaping::Auto => text::Shaping::Auto,
    }
}

// -------------------------------------------------------------------------
// Ellipsis
// -------------------------------------------------------------------------

pub fn ellipsis(e: types::Ellipsis) -> text::Ellipsis {
    match e {
        types::Ellipsis::None => text::Ellipsis::None,
        types::Ellipsis::Start => text::Ellipsis::Start,
        types::Ellipsis::Middle => text::Ellipsis::Middle,
        types::Ellipsis::End => text::Ellipsis::End,
    }
}

// -------------------------------------------------------------------------
// ContentFit
// -------------------------------------------------------------------------

pub fn content_fit(f: types::ContentFit) -> iced::ContentFit {
    match f {
        types::ContentFit::Contain => iced::ContentFit::Contain,
        types::ContentFit::Cover => iced::ContentFit::Cover,
        types::ContentFit::Fill => iced::ContentFit::Fill,
        types::ContentFit::ScaleDown => iced::ContentFit::ScaleDown,
        types::ContentFit::None => iced::ContentFit::None,
    }
}

// -------------------------------------------------------------------------
// LineHeight
// -------------------------------------------------------------------------

pub fn line_height(lh: types::LineHeight) -> text::LineHeight {
    match lh {
        types::LineHeight::Relative(r) => text::LineHeight::Relative(r),
        types::LineHeight::Absolute(a) => text::LineHeight::Absolute(iced::Pixels(a)),
    }
}

// -------------------------------------------------------------------------
// CursorStyle
// -------------------------------------------------------------------------

pub fn cursor_style(c: types::CursorStyle) -> iced::mouse::Interaction {
    match c {
        types::CursorStyle::Pointer => iced::mouse::Interaction::Pointer,
        types::CursorStyle::Grab => iced::mouse::Interaction::Grab,
        types::CursorStyle::Grabbing => iced::mouse::Interaction::Grabbing,
        types::CursorStyle::Crosshair => iced::mouse::Interaction::Crosshair,
        types::CursorStyle::Text => iced::mouse::Interaction::Text,
        types::CursorStyle::Move => iced::mouse::Interaction::Move,
        types::CursorStyle::NotAllowed => iced::mouse::Interaction::NotAllowed,
        types::CursorStyle::Progress => iced::mouse::Interaction::Progress,
        types::CursorStyle::Wait => iced::mouse::Interaction::Wait,
        types::CursorStyle::Help => iced::mouse::Interaction::Help,
        types::CursorStyle::Cell => iced::mouse::Interaction::Cell,
        types::CursorStyle::Copy => iced::mouse::Interaction::Copy,
        types::CursorStyle::Alias => iced::mouse::Interaction::Alias,
        types::CursorStyle::NoDrop => iced::mouse::Interaction::NoDrop,
        types::CursorStyle::AllScroll => iced::mouse::Interaction::AllScroll,
        types::CursorStyle::ZoomIn => iced::mouse::Interaction::ZoomIn,
        types::CursorStyle::ZoomOut => iced::mouse::Interaction::ZoomOut,
        types::CursorStyle::ContextMenu => iced::mouse::Interaction::ContextMenu,
        types::CursorStyle::ResizingHorizontally => iced::mouse::Interaction::ResizingHorizontally,
        types::CursorStyle::ResizingVertically => iced::mouse::Interaction::ResizingVertically,
        types::CursorStyle::ResizingDiagonallyUp => iced::mouse::Interaction::ResizingDiagonallyUp,
        types::CursorStyle::ResizingDiagonallyDown => {
            iced::mouse::Interaction::ResizingDiagonallyDown
        }
        types::CursorStyle::ResizingColumn => iced::mouse::Interaction::ResizingColumn,
        types::CursorStyle::ResizingRow => iced::mouse::Interaction::ResizingRow,
    }
}

// -------------------------------------------------------------------------
// InputPurpose
// -------------------------------------------------------------------------

pub fn input_purpose(p: types::InputPurpose) -> iced::advanced::input_method::Purpose {
    use iced::advanced::input_method::Purpose;
    match p {
        types::InputPurpose::Normal => Purpose::Normal,
        types::InputPurpose::Secure => Purpose::Secure,
        types::InputPurpose::Terminal => Purpose::Terminal,
        types::InputPurpose::Number => Purpose::Number,
        types::InputPurpose::Decimal => Purpose::Decimal,
        types::InputPurpose::Phone => Purpose::Phone,
        types::InputPurpose::Email => Purpose::Email,
        types::InputPurpose::Url => Purpose::Url,
        types::InputPurpose::Search => Purpose::Search,
    }
}

// -------------------------------------------------------------------------
// Anchor
// -------------------------------------------------------------------------

pub fn anchor(a: types::Anchor) -> iced::widget::scrollable::Anchor {
    match a {
        types::Anchor::Start => iced::widget::scrollable::Anchor::Start,
        types::Anchor::End => iced::widget::scrollable::Anchor::End,
    }
}

// -------------------------------------------------------------------------
// Direction (scrollable)
// -------------------------------------------------------------------------

/// Build a scrollable direction from a plushie-core Direction and scrollbar.
///
/// Direction maps to iced's `scrollable::Direction` enum, which carries the
/// scrollbar configuration as data. `Both` reuses the same scrollbar for
/// both axes.
pub fn scrollable_direction(
    d: types::Direction,
    scrollbar: iced::widget::scrollable::Scrollbar,
) -> iced::widget::scrollable::Direction {
    match d {
        types::Direction::Horizontal => iced::widget::scrollable::Direction::Horizontal(scrollbar),
        types::Direction::Both => iced::widget::scrollable::Direction::Both {
            vertical: scrollbar,
            horizontal: scrollbar,
        },
        types::Direction::Vertical => iced::widget::scrollable::Direction::Vertical(scrollbar),
    }
}

// -------------------------------------------------------------------------
// Background
// -------------------------------------------------------------------------

pub fn background(bg: &types::Background) -> iced::Background {
    match bg {
        types::Background::Color(c) => iced::Background::Color(color(c)),
        types::Background::Gradient(g) => iced::Background::Gradient(gradient(g)),
    }
}

// -------------------------------------------------------------------------
// Gradient (background gradient, angle-based)
// -------------------------------------------------------------------------

/// Convert a plushie-core Gradient to an iced background Gradient.
///
/// Background gradients in iced use an angle (Radians) derived from
/// the start/end coordinate pair via atan2.
pub fn gradient(g: &types::Gradient) -> iced::Gradient {
    let dx = g.end.0 - g.start.0;
    let dy = g.end.1 - g.start.1;
    let angle = iced::Radians(dy.atan2(dx));
    let mut linear = iced::gradient::Linear::new(angle);
    for stop in &g.stops {
        linear = linear.add_stop(stop.offset, color(&stop.color));
    }
    iced::Gradient::Linear(linear)
}

// -------------------------------------------------------------------------
// Canvas: FillRule
// -------------------------------------------------------------------------

pub fn fill_rule(r: types::canvas::FillRule) -> canvas::fill::Rule {
    match r {
        types::canvas::FillRule::NonZero => canvas::fill::Rule::NonZero,
        types::canvas::FillRule::EvenOdd => canvas::fill::Rule::EvenOdd,
    }
}

/// Convert a plushie-core Gradient to an iced canvas gradient.
///
/// Canvas gradients use start/end points (not an angle), so
/// coordinates are passed through directly.
pub fn canvas_gradient(g: &types::Gradient) -> canvas::Gradient {
    let start = iced::Point::new(g.start.0, g.start.1);
    let end = iced::Point::new(g.end.0, g.end.1);
    let mut linear = canvas::gradient::Linear::new(start, end);
    for stop in &g.stops {
        linear = linear.add_stop(stop.offset, color(&stop.color));
    }
    canvas::Gradient::Linear(linear)
}

// -------------------------------------------------------------------------
// Canvas: LineCap
// -------------------------------------------------------------------------

pub fn line_cap(c: types::canvas::LineCap) -> canvas::LineCap {
    match c {
        types::canvas::LineCap::Butt => canvas::LineCap::Butt,
        types::canvas::LineCap::Round => canvas::LineCap::Round,
        types::canvas::LineCap::Square => canvas::LineCap::Square,
    }
}

// -------------------------------------------------------------------------
// Canvas: LineJoin
// -------------------------------------------------------------------------

pub fn line_join(j: types::canvas::LineJoin) -> canvas::LineJoin {
    match j {
        types::canvas::LineJoin::Miter => canvas::LineJoin::Miter,
        types::canvas::LineJoin::Round => canvas::LineJoin::Round,
        types::canvas::LineJoin::Bevel => canvas::LineJoin::Bevel,
    }
}

// -------------------------------------------------------------------------
// FilterMethod
// -------------------------------------------------------------------------

pub fn filter_method(f: types::FilterMethod) -> iced::widget::image::FilterMethod {
    match f {
        types::FilterMethod::Nearest => iced::widget::image::FilterMethod::Nearest,
        types::FilterMethod::Linear => iced::widget::image::FilterMethod::Linear,
    }
}

// -------------------------------------------------------------------------
// A11y: Role
// -------------------------------------------------------------------------

pub fn a11y_role(r: &types::Role) -> accessible::Role {
    match r {
        types::Role::Alert => accessible::Role::Alert,
        types::Role::AlertDialog => accessible::Role::AlertDialog,
        types::Role::Button => accessible::Role::Button,
        types::Role::Canvas => accessible::Role::Canvas,
        types::Role::CheckBox => accessible::Role::CheckBox,
        types::Role::Cell => accessible::Role::Cell,
        types::Role::ColumnHeader => accessible::Role::ColumnHeader,
        types::Role::ComboBox => accessible::Role::ComboBox,
        types::Role::Dialog => accessible::Role::Dialog,
        types::Role::Document => accessible::Role::Document,
        types::Role::GenericContainer => accessible::Role::GenericContainer,
        types::Role::Group => accessible::Role::Group,
        types::Role::Heading => accessible::Role::Heading,
        types::Role::Image => accessible::Role::Image,
        types::Role::Label => accessible::Role::Label,
        types::Role::Link => accessible::Role::Link,
        types::Role::List => accessible::Role::List,
        types::Role::ListItem => accessible::Role::ListItem,
        types::Role::Menu => accessible::Role::Menu,
        types::Role::MenuBar => accessible::Role::MenuBar,
        types::Role::MenuItem => accessible::Role::MenuItem,
        types::Role::Meter => accessible::Role::Meter,
        types::Role::MultilineTextInput => accessible::Role::MultilineTextInput,
        types::Role::Navigation => accessible::Role::Navigation,
        types::Role::ProgressIndicator => accessible::Role::ProgressIndicator,
        types::Role::RadioButton => accessible::Role::RadioButton,
        types::Role::RadioGroup => accessible::Role::RadioGroup,
        types::Role::Region => accessible::Role::Region,
        types::Role::Row => accessible::Role::Row,
        types::Role::ScrollBar => accessible::Role::ScrollBar,
        types::Role::ScrollView => accessible::Role::ScrollView,
        types::Role::Search => accessible::Role::Search,
        types::Role::Separator => accessible::Role::Separator,
        types::Role::Slider => accessible::Role::Slider,
        types::Role::StaticText => accessible::Role::StaticText,
        types::Role::Status => accessible::Role::Status,
        types::Role::Switch => accessible::Role::Switch,
        types::Role::Tab => accessible::Role::Tab,
        types::Role::TabList => accessible::Role::TabList,
        types::Role::TabPanel => accessible::Role::TabPanel,
        types::Role::Table => accessible::Role::Table,
        types::Role::TextInput => accessible::Role::TextInput,
        types::Role::Toolbar => accessible::Role::Toolbar,
        types::Role::Tooltip => accessible::Role::Tooltip,
        types::Role::Tree => accessible::Role::Tree,
        types::Role::TreeItem => accessible::Role::TreeItem,
        types::Role::Window => accessible::Role::Window,
    }
}

// -------------------------------------------------------------------------
// A11y: Live
// -------------------------------------------------------------------------

pub fn a11y_live(l: &types::Live) -> accessible::Live {
    match l {
        types::Live::Polite => accessible::Live::Polite,
        types::Live::Assertive => accessible::Live::Assertive,
    }
}

// -------------------------------------------------------------------------
// A11y: Orientation
// -------------------------------------------------------------------------

pub fn a11y_orientation(o: &types::Orientation) -> accessible::Orientation {
    match o {
        types::Orientation::Horizontal => accessible::Orientation::Horizontal,
        types::Orientation::Vertical => accessible::Orientation::Vertical,
    }
}

// -------------------------------------------------------------------------
// A11y: HasPopup
// -------------------------------------------------------------------------

pub fn a11y_has_popup(h: &types::HasPopup) -> accessible::HasPopup {
    match h {
        types::HasPopup::Listbox => accessible::HasPopup::Listbox,
        types::HasPopup::Menu => accessible::HasPopup::Menu,
        types::HasPopup::Dialog => accessible::HasPopup::Dialog,
        types::HasPopup::Tree => accessible::HasPopup::Tree,
        types::HasPopup::Grid => accessible::HasPopup::Grid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_hex_rgb() {
        let c = types::Color::hex("#ff8040");
        let ic = color(&c);
        assert!((ic.r - 1.0).abs() < 0.01);
        assert!((ic.g - 0.502).abs() < 0.01);
        assert!((ic.b - 0.251).abs() < 0.01);
        assert!((ic.a - 1.0).abs() < 0.001);
    }

    #[test]
    fn color_hex_rgba() {
        let c = types::Color::hex("#ff000080");
        let ic = color(&c);
        assert!((ic.r - 1.0).abs() < 0.01);
        assert!((ic.a - 0.502).abs() < 0.01);
    }

    #[test]
    fn color_invalid_falls_back_to_transparent() {
        let c = types::Color::hex("not-a-color");
        let ic = color(&c);
        assert_eq!(ic, iced::Color::TRANSPARENT);
    }

    #[test]
    fn length_variants() {
        assert_eq!(length(&types::Length::Fill), iced::Length::Fill);
        assert_eq!(length(&types::Length::Shrink), iced::Length::Shrink);
        assert_eq!(
            length(&types::Length::FillPortion(3)),
            iced::Length::FillPortion(3)
        );
        assert_eq!(
            length(&types::Length::Fixed(42.0)),
            iced::Length::Fixed(42.0)
        );
    }

    #[test]
    fn padding_fields() {
        let p = types::Padding::new(1.0, 2.0, 3.0, 4.0);
        let ip = padding(&p);
        assert_eq!(ip.top, 1.0);
        assert_eq!(ip.right, 2.0);
        assert_eq!(ip.bottom, 3.0);
        assert_eq!(ip.left, 4.0);
    }

    #[test]
    fn font_default_and_monospace() {
        let f = types::Font::new();
        assert_eq!(font(&f), iced::Font::DEFAULT);

        let f = types::Font::monospace();
        assert_eq!(font(&f), iced::Font::MONOSPACE);
    }

    #[test]
    fn font_with_weight() {
        let f = types::Font::new().weight(types::FontWeight::Bold);
        let if_ = font(&f);
        assert_eq!(if_.weight, iced::font::Weight::Bold);
    }

    #[test]
    fn font_custom_family() {
        let f = types::Font::new().family("Fira Code");
        let if_ = font(&f);
        assert!(matches!(if_.family, iced::font::Family::Name("Fira Code")));
    }

    #[test]
    fn border_conversion() {
        let b = types::Border::new().color("#ff0000").width(2.0).radius(8.0);
        let ib = border(&b);
        assert_eq!(ib.width, 2.0);
        assert!((ib.color.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn shadow_conversion() {
        let s = types::Shadow::new()
            .color("#000000")
            .offset(5.0, 10.0)
            .blur_radius(3.0);
        let is = shadow(&s);
        assert_eq!(is.offset, iced::Vector::new(5.0, 10.0));
        assert_eq!(is.blur_radius, 3.0);
    }

    #[test]
    fn cursor_style_mapping() {
        assert_eq!(
            cursor_style(types::CursorStyle::Pointer),
            iced::mouse::Interaction::Pointer
        );
        assert_eq!(
            cursor_style(types::CursorStyle::ZoomIn),
            iced::mouse::Interaction::ZoomIn
        );
    }

    #[test]
    fn wrapping_mapping() {
        assert_eq!(
            wrapping(types::Wrapping::WordOrGlyph),
            text::Wrapping::WordOrGlyph
        );
    }

    #[test]
    fn shaping_mapping() {
        assert_eq!(shaping(types::Shaping::Advanced), text::Shaping::Advanced);
    }

    #[test]
    fn ellipsis_mapping() {
        assert_eq!(ellipsis(types::Ellipsis::Middle), text::Ellipsis::Middle);
    }

    #[test]
    fn content_fit_mapping() {
        assert_eq!(
            content_fit(types::ContentFit::Cover),
            iced::ContentFit::Cover
        );
    }

    #[test]
    fn line_height_relative() {
        assert_eq!(
            line_height(types::LineHeight::Relative(1.5)),
            text::LineHeight::Relative(1.5)
        );
    }

    #[test]
    fn line_height_absolute() {
        assert_eq!(
            line_height(types::LineHeight::Absolute(24.0)),
            text::LineHeight::Absolute(iced::Pixels(24.0))
        );
    }

    #[test]
    fn canvas_fill_rule_mapping() {
        assert_eq!(
            fill_rule(types::canvas::FillRule::EvenOdd),
            canvas::fill::Rule::EvenOdd
        );
    }

    #[test]
    fn canvas_line_cap_mapping() {
        assert!(matches!(
            line_cap(types::canvas::LineCap::Butt),
            canvas::LineCap::Butt
        ));
        assert!(matches!(
            line_cap(types::canvas::LineCap::Round),
            canvas::LineCap::Round
        ));
        assert!(matches!(
            line_cap(types::canvas::LineCap::Square),
            canvas::LineCap::Square
        ));
    }

    #[test]
    fn canvas_line_join_mapping() {
        assert!(matches!(
            line_join(types::canvas::LineJoin::Miter),
            canvas::LineJoin::Miter
        ));
        assert!(matches!(
            line_join(types::canvas::LineJoin::Round),
            canvas::LineJoin::Round
        ));
        assert!(matches!(
            line_join(types::canvas::LineJoin::Bevel),
            canvas::LineJoin::Bevel
        ));
    }

    #[test]
    fn background_color() {
        let bg = types::Background::Color(types::Color::hex("#ff0000"));
        let ibg = background(&bg);
        match ibg {
            iced::Background::Color(c) => assert!((c.r - 1.0).abs() < 0.01),
            _ => panic!("expected Color background"),
        }
    }

    #[test]
    fn gradient_angle_vertical() {
        // start=(0,0) end=(0,1): straight down, angle = pi/2
        let g = types::Gradient::linear(
            (0.0, 0.0),
            (0.0, 1.0),
            vec![(0.0, types::Color::black()), (1.0, types::Color::white())],
        );
        let ig = gradient(&g);
        match ig {
            iced::Gradient::Linear(_) => {}
        }
    }

    #[test]
    fn filter_method_mapping() {
        assert_eq!(
            filter_method(types::FilterMethod::Nearest),
            iced::widget::image::FilterMethod::Nearest
        );
    }

    #[test]
    fn input_purpose_mapping() {
        use iced::advanced::input_method::Purpose;
        assert_eq!(input_purpose(types::InputPurpose::Email), Purpose::Email);
    }

    #[test]
    fn anchor_mapping() {
        assert_eq!(
            anchor(types::Anchor::Start),
            iced::widget::scrollable::Anchor::Start
        );
        assert_eq!(
            anchor(types::Anchor::End),
            iced::widget::scrollable::Anchor::End
        );
    }

    #[test]
    fn scrollable_direction_vertical() {
        let sb = iced::widget::scrollable::Scrollbar::default();
        let d = scrollable_direction(types::Direction::Vertical, sb);
        assert!(matches!(
            d,
            iced::widget::scrollable::Direction::Vertical(_)
        ));
    }

    #[test]
    fn scrollable_direction_horizontal() {
        let sb = iced::widget::scrollable::Scrollbar::default();
        let d = scrollable_direction(types::Direction::Horizontal, sb);
        assert!(matches!(
            d,
            iced::widget::scrollable::Direction::Horizontal(_)
        ));
    }

    #[test]
    fn scrollable_direction_both() {
        let sb = iced::widget::scrollable::Scrollbar::default();
        let d = scrollable_direction(types::Direction::Both, sb);
        assert!(matches!(
            d,
            iced::widget::scrollable::Direction::Both { .. }
        ));
    }
}
