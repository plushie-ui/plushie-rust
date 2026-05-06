//! Conversion layer from plushie-core types to iced types.
//!
//! This is the single location where plushie-core's domain types meet
//! iced's rendering types. Neither crate is owned by plushie-widget-sdk,
//! so Rust's orphan rule prevents `From` impls. Instead we provide
//! named conversion functions.

use iced::advanced::widget::operation::accessible;
use iced::theme::palette;
use iced::widget::canvas;
use iced::widget::text;

use crate::theming::resolve_builtin;
use crate::widget::helpers::intern_font_family;

use plushie_core::types;

// -------------------------------------------------------------------------
// Color
// -------------------------------------------------------------------------

/// Convert a plushie-core Color (canonical hex string) to an iced Color.
///
/// The input has already been validated by [`types::Color`]'s constructor
/// or wire decoder, so its hex form is guaranteed to be `#rrggbb` or
/// `#rrggbbaa` with valid hex digits. If the invariant is violated the
/// conversion returns [`iced::Color::TRANSPARENT`] as a safe fallback.
pub fn color(c: &types::Color) -> iced::Color {
    hex_to_iced_color(c.as_hex()).unwrap_or(iced::Color::TRANSPARENT)
}

/// Parse a canonical hex string (`#rrggbb` or `#rrggbbaa`) into an iced Color.
///
/// Strict: rejects short forms and any non-canonical input. All hex
/// validation lives in `types::Color`; this helper only performs the
/// byte-to-iced color conversion for values that have already passed
/// `Color`'s validator.
///
/// Callers holding an already-typed [`types::Color`] should use
/// [`color`] instead. This raw entry point exists for a small number
/// of sites (animation interpolation, canvas palette lookup) where the
/// value is produced inline rather than round-tripped through
/// [`types::Color`].
pub fn hex_to_iced_color(hex: &str) -> Option<iced::Color> {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(iced::Color::from_rgb8(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(iced::Color::from_rgba8(r, g, b, a as f32 / 255.0))
        }
        _ => None,
    }
}

// -------------------------------------------------------------------------
// Theme
// -------------------------------------------------------------------------

/// Convert a plushie-core [`types::CustomTheme`] into an iced [`iced::Theme`].
///
/// Uses the `base` field to select the seed palette (default: dark),
/// overrides seed colors from the typed color map, and applies shade
/// overrides onto the generated palette when any shade keys are set.
pub fn custom_theme(c: &types::CustomTheme) -> iced::Theme {
    let base_theme = c
        .base
        .as_deref()
        .map(|base| {
            resolve_builtin(base).unwrap_or_else(|| {
                log::warn!("unknown custom theme base {base:?}; using dark base");
                iced::Theme::Dark
            })
        })
        .unwrap_or(iced::Theme::Dark);

    let mut seed = base_theme.seed();
    if let Some(col) = c.colors.get("background") {
        seed.background = color(col);
    }
    if let Some(col) = c.colors.get("text") {
        seed.text = color(col);
    }
    if let Some(col) = c.colors.get("primary") {
        seed.primary = color(col);
    }
    if let Some(col) = c.colors.get("success") {
        seed.success = color(col);
    }
    if let Some(col) = c.colors.get("warning") {
        seed.warning = color(col);
    }
    if let Some(col) = c.colors.get("danger") {
        seed.danger = color(col);
    }

    let name = if c.name.is_empty() {
        "Custom".to_string()
    } else {
        c.name.clone()
    };

    if has_shade_override(c) {
        let colors = c.colors.clone();
        iced::Theme::custom_with_fn(name, seed, move |s| {
            let mut pal = palette::Palette::generate(s);
            apply_shade_overrides(&mut pal, &colors);
            pal
        })
    } else {
        iced::Theme::custom(name, seed)
    }
}

/// Shade keys that trigger the `custom_with_fn` path.
const SHADE_KEYS: &[&str] = &[
    "primary_base",
    "primary_weak",
    "primary_strong",
    "secondary_base",
    "secondary_weak",
    "secondary_strong",
    "success_base",
    "success_weak",
    "success_strong",
    "warning_base",
    "warning_weak",
    "warning_strong",
    "danger_base",
    "danger_weak",
    "danger_strong",
    "background_base",
    "background_weakest",
    "background_weaker",
    "background_weak",
    "background_neutral",
    "background_strong",
    "background_stronger",
    "background_strongest",
];

fn has_shade_override(c: &types::CustomTheme) -> bool {
    SHADE_KEYS
        .iter()
        .any(|k| c.colors.contains_key(*k) || c.colors.contains_key(&format!("{}_text", k)))
}

fn apply_shade_overrides(
    pal: &mut palette::Palette,
    colors: &std::collections::BTreeMap<String, types::Color>,
) {
    override_pair(&mut pal.primary.base, colors, "primary_base");
    override_pair(&mut pal.primary.weak, colors, "primary_weak");
    override_pair(&mut pal.primary.strong, colors, "primary_strong");

    override_pair(&mut pal.secondary.base, colors, "secondary_base");
    override_pair(&mut pal.secondary.weak, colors, "secondary_weak");
    override_pair(&mut pal.secondary.strong, colors, "secondary_strong");

    override_pair(&mut pal.success.base, colors, "success_base");
    override_pair(&mut pal.success.weak, colors, "success_weak");
    override_pair(&mut pal.success.strong, colors, "success_strong");

    override_pair(&mut pal.warning.base, colors, "warning_base");
    override_pair(&mut pal.warning.weak, colors, "warning_weak");
    override_pair(&mut pal.warning.strong, colors, "warning_strong");

    override_pair(&mut pal.danger.base, colors, "danger_base");
    override_pair(&mut pal.danger.weak, colors, "danger_weak");
    override_pair(&mut pal.danger.strong, colors, "danger_strong");

    override_pair(&mut pal.background.base, colors, "background_base");
    override_pair(&mut pal.background.weakest, colors, "background_weakest");
    override_pair(&mut pal.background.weaker, colors, "background_weaker");
    override_pair(&mut pal.background.weak, colors, "background_weak");
    override_pair(&mut pal.background.neutral, colors, "background_neutral");
    override_pair(&mut pal.background.strong, colors, "background_strong");
    override_pair(&mut pal.background.stronger, colors, "background_stronger");
    override_pair(
        &mut pal.background.strongest,
        colors,
        "background_strongest",
    );
}

fn override_pair(
    pair: &mut palette::Pair,
    colors: &std::collections::BTreeMap<String, types::Color>,
    key: &str,
) {
    if let Some(c) = colors.get(key) {
        pair.color = color(c);
    }
    let text_key = format!("{}_text", key);
    if let Some(c) = colors.get(&text_key) {
        pair.text = color(c);
    }
}

// -------------------------------------------------------------------------
// Length
// -------------------------------------------------------------------------

/// Convert a plushie-core Length to an iced Length.
pub fn length(l: &types::Length) -> iced::Length {
    match *l {
        types::Length::Fill => iced::Length::Fill,
        types::Length::Shrink => iced::Length::Shrink,
        types::Length::FillPortion(n) => {
            iced::Length::FillPortion(u16::try_from(n).unwrap_or(u16::MAX))
        }
        types::Length::Fixed(f) => iced::Length::Fixed(f),
    }
}

// -------------------------------------------------------------------------
// Padding
// -------------------------------------------------------------------------

/// Convert a plushie-core Padding to an iced Padding.
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

/// Convert a plushie-core FontWeight to an iced font weight.
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

/// Convert a plushie-core FontStyle to an iced font style.
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

/// Convert a plushie-core FontStretch to an iced font stretch.
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
            other => match intern_font_family(other) {
                Some(name) => iced::font::Family::Name(name),
                None => iced::font::Family::SansSerif,
            },
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

/// Convert a plushie-core Radius to an iced border radius.
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

/// Convert a plushie-core Border to an iced Border.
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

/// Convert a plushie-core Shadow to an iced Shadow.
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

/// Convert a plushie-core HorizontalAlignment to an iced horizontal alignment.
pub fn horizontal_alignment(a: types::HorizontalAlignment) -> iced::alignment::Horizontal {
    match a {
        types::HorizontalAlignment::Left => iced::alignment::Horizontal::Left,
        types::HorizontalAlignment::Center => iced::alignment::Horizontal::Center,
        types::HorizontalAlignment::Right => iced::alignment::Horizontal::Right,
    }
}

// -------------------------------------------------------------------------
// TextAlignment
// -------------------------------------------------------------------------

/// Convert a plushie-core TextAlignment to an iced text alignment.
pub fn text_alignment(a: types::TextAlignment, direction: types::TextDirection) -> text::Alignment {
    match a {
        types::TextAlignment::Default => text::Alignment::Default,
        types::TextAlignment::Left => text::Alignment::Left,
        types::TextAlignment::Center => text::Alignment::Center,
        types::TextAlignment::Right => text::Alignment::Right,
        types::TextAlignment::Start => match direction {
            types::TextDirection::Auto => text::Alignment::Default,
            types::TextDirection::Ltr => text::Alignment::Left,
            types::TextDirection::Rtl => text::Alignment::Right,
        },
        types::TextAlignment::End => match direction {
            types::TextDirection::Auto => text::Alignment::Right,
            types::TextDirection::Ltr => text::Alignment::Right,
            types::TextDirection::Rtl => text::Alignment::Left,
        },
        types::TextAlignment::Justified => text::Alignment::Justified,
    }
}

// -------------------------------------------------------------------------
// VerticalAlignment
// -------------------------------------------------------------------------

/// Convert a plushie-core VerticalAlignment to an iced vertical alignment.
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

/// Convert a plushie-core Wrapping to an iced text wrapping.
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

/// Convert a plushie-core Shaping to an iced text shaping.
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

/// Convert a plushie-core Ellipsis to an iced text ellipsis.
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

/// Convert a plushie-core ContentFit to an iced ContentFit.
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

/// Convert a plushie-core LineHeight to an iced text line height.
pub fn line_height(lh: types::LineHeight) -> text::LineHeight {
    match lh {
        types::LineHeight::Relative(r) => text::LineHeight::Relative(r),
        types::LineHeight::Absolute(a) => text::LineHeight::Absolute(iced::Pixels(a)),
    }
}

// -------------------------------------------------------------------------
// CursorStyle
// -------------------------------------------------------------------------

/// Convert a plushie-core CursorStyle to an iced mouse Interaction.
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

/// Convert a plushie-core InputPurpose to an iced input-method Purpose.
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

/// Convert a plushie-core scrollable Anchor to an iced Anchor.
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

/// Convert a plushie-core Background to an iced Background.
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

/// Convert a plushie-core canvas FillRule to an iced canvas fill::Rule.
pub fn fill_rule(r: types::FillRule) -> canvas::fill::Rule {
    match r {
        types::FillRule::NonZero => canvas::fill::Rule::NonZero,
        types::FillRule::EvenOdd => canvas::fill::Rule::EvenOdd,
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

/// Convert a plushie-core canvas LineCap to an iced canvas LineCap.
pub fn line_cap(c: types::LineCap) -> canvas::LineCap {
    match c {
        types::LineCap::Butt => canvas::LineCap::Butt,
        types::LineCap::Round => canvas::LineCap::Round,
        types::LineCap::Square => canvas::LineCap::Square,
    }
}

// -------------------------------------------------------------------------
// Canvas: LineJoin
// -------------------------------------------------------------------------

/// Convert a plushie-core canvas LineJoin to an iced canvas LineJoin.
pub fn line_join(j: types::LineJoin) -> canvas::LineJoin {
    match j {
        types::LineJoin::Miter => canvas::LineJoin::Miter,
        types::LineJoin::Round => canvas::LineJoin::Round,
        types::LineJoin::Bevel => canvas::LineJoin::Bevel,
    }
}

// -------------------------------------------------------------------------
// FilterMethod
// -------------------------------------------------------------------------

/// Convert a plushie-core FilterMethod to an iced image filter method.
pub fn filter_method(f: types::FilterMethod) -> iced::widget::image::FilterMethod {
    match f {
        types::FilterMethod::Nearest => iced::widget::image::FilterMethod::Nearest,
        types::FilterMethod::Linear => iced::widget::image::FilterMethod::Linear,
    }
}

// -------------------------------------------------------------------------
// A11y: Role
// -------------------------------------------------------------------------

/// Convert a plushie-core Role to an iced accessible Role.
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

/// Convert a plushie-core Live to an iced accessible Live.
pub fn a11y_live(l: &types::Live) -> accessible::Live {
    match l {
        types::Live::Polite => accessible::Live::Polite,
        types::Live::Assertive => accessible::Live::Assertive,
    }
}

// -------------------------------------------------------------------------
// A11y: Orientation
// -------------------------------------------------------------------------

/// Convert a plushie-core Orientation to an iced accessible Orientation.
pub fn a11y_orientation(o: &types::Orientation) -> accessible::Orientation {
    match o {
        types::Orientation::Horizontal => accessible::Orientation::Horizontal,
        types::Orientation::Vertical => accessible::Orientation::Vertical,
    }
}

// -------------------------------------------------------------------------
// A11y: HasPopup
// -------------------------------------------------------------------------

/// Convert a plushie-core HasPopup to an iced accessible HasPopup.
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
    fn color_invalid_hex_returns_none() {
        // Color::hex() now validates, so invalid hex can't be constructed
        // via that entry point. The raw hex_to_iced_color helper still
        // needs to fail gracefully on bad input.
        assert!(hex_to_iced_color("not-a-color").is_none());
        assert!(hex_to_iced_color("#xyz").is_none());
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
    fn text_alignment_maps_physical_values() {
        assert_eq!(
            text_alignment(types::TextAlignment::Default, types::TextDirection::Auto),
            text::Alignment::Default
        );
        assert_eq!(
            text_alignment(types::TextAlignment::Left, types::TextDirection::Rtl),
            text::Alignment::Left
        );
        assert_eq!(
            text_alignment(types::TextAlignment::Center, types::TextDirection::Auto),
            text::Alignment::Center
        );
        assert_eq!(
            text_alignment(types::TextAlignment::Right, types::TextDirection::Ltr),
            text::Alignment::Right
        );
    }

    #[test]
    fn text_alignment_maps_logical_values_by_direction() {
        assert_eq!(
            text_alignment(types::TextAlignment::Start, types::TextDirection::Ltr),
            text::Alignment::Left
        );
        assert_eq!(
            text_alignment(types::TextAlignment::End, types::TextDirection::Ltr),
            text::Alignment::Right
        );
        assert_eq!(
            text_alignment(types::TextAlignment::Start, types::TextDirection::Rtl),
            text::Alignment::Right
        );
        assert_eq!(
            text_alignment(types::TextAlignment::End, types::TextDirection::Rtl),
            text::Alignment::Left
        );
    }

    #[test]
    fn text_alignment_maps_auto_and_justified() {
        assert_eq!(
            text_alignment(types::TextAlignment::Start, types::TextDirection::Auto),
            text::Alignment::Default
        );
        assert_eq!(
            text_alignment(types::TextAlignment::End, types::TextDirection::Auto),
            text::Alignment::Right
        );
        assert_eq!(
            text_alignment(types::TextAlignment::Justified, types::TextDirection::Rtl),
            text::Alignment::Justified
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
            fill_rule(types::FillRule::EvenOdd),
            canvas::fill::Rule::EvenOdd
        );
    }

    #[test]
    fn canvas_line_cap_mapping() {
        assert!(matches!(
            line_cap(types::LineCap::Butt),
            canvas::LineCap::Butt
        ));
        assert!(matches!(
            line_cap(types::LineCap::Round),
            canvas::LineCap::Round
        ));
        assert!(matches!(
            line_cap(types::LineCap::Square),
            canvas::LineCap::Square
        ));
    }

    #[test]
    fn canvas_line_join_mapping() {
        assert!(matches!(
            line_join(types::LineJoin::Miter),
            canvas::LineJoin::Miter
        ));
        assert!(matches!(
            line_join(types::LineJoin::Round),
            canvas::LineJoin::Round
        ));
        assert!(matches!(
            line_join(types::LineJoin::Bevel),
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
