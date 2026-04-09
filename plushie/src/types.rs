//! Shared property types for building views.
//!
//! These types represent widget properties like colors, dimensions,
//! fonts, and styles. Each type has ergonomic constructors and
//! `From` implementations for common conversions.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// A color value, stored as a canonical hex string.
///
/// Construct with named colors, hex strings, or RGB values:
///
/// ```
/// use plushie::types::Color;
///
/// let red = Color::red();
/// let custom = Color::hex("#3498db");
/// let rgb = Color::rgb(0.5, 0.8, 1.0);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Color(String);

impl Color {
    /// Create a color from a hex string (e.g. `"#ff0000"` or `"#ff000080"`).
    pub fn hex(s: &str) -> Self {
        Self(s.to_string())
    }

    /// Create a color from RGB floats (0.0 to 1.0).
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        let r = (r.clamp(0.0, 1.0) * 255.0) as u8;
        let g = (g.clamp(0.0, 1.0) * 255.0) as u8;
        let b = (b.clamp(0.0, 1.0) * 255.0) as u8;
        Self(format!("#{r:02x}{g:02x}{b:02x}"))
    }

    /// Create a color from RGBA floats (0.0 to 1.0).
    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        let r = (r.clamp(0.0, 1.0) * 255.0) as u8;
        let g = (g.clamp(0.0, 1.0) * 255.0) as u8;
        let b = (b.clamp(0.0, 1.0) * 255.0) as u8;
        let a = (a.clamp(0.0, 1.0) * 255.0) as u8;
        Self(format!("#{r:02x}{g:02x}{b:02x}{a:02x}"))
    }

    /// The hex string representation of this color.
    pub fn as_hex(&self) -> &str {
        &self.0
    }

    // --- Named CSS colors (most commonly used) ---

    pub fn transparent() -> Self { Self::hex("#00000000") }

    pub fn red() -> Self { Self::hex("#ff0000") }
    pub fn green() -> Self { Self::hex("#008000") }
    pub fn blue() -> Self { Self::hex("#0000ff") }
    pub fn white() -> Self { Self::hex("#ffffff") }
    pub fn black() -> Self { Self::hex("#000000") }
    pub fn yellow() -> Self { Self::hex("#ffff00") }
    pub fn orange() -> Self { Self::hex("#ffa500") }
    pub fn purple() -> Self { Self::hex("#800080") }
    pub fn pink() -> Self { Self::hex("#ffc0cb") }
    pub fn gray() -> Self { Self::hex("#808080") }
    pub fn cyan() -> Self { Self::hex("#00ffff") }
    pub fn magenta() -> Self { Self::hex("#ff00ff") }
    pub fn brown() -> Self { Self::hex("#a52a2a") }
    pub fn navy() -> Self { Self::hex("#000080") }
    pub fn teal() -> Self { Self::hex("#008080") }
    pub fn coral() -> Self { Self::hex("#ff7f50") }
    pub fn salmon() -> Self { Self::hex("#fa8072") }
    pub fn gold() -> Self { Self::hex("#ffd700") }
    pub fn silver() -> Self { Self::hex("#c0c0c0") }
    pub fn indigo() -> Self { Self::hex("#4b0082") }
}

impl From<&str> for Color {
    fn from(s: &str) -> Self {
        Self::hex(s)
    }
}

impl From<String> for Color {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// ---------------------------------------------------------------------------
// Length
// ---------------------------------------------------------------------------

/// How a widget should be sized along an axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Length {
    /// Fill all available space.
    Fill,
    /// Take only the space needed by the content.
    Shrink,
    /// Fill a weighted portion of available space.
    FillPortion(u16),
    /// A fixed size in logical pixels.
    Fixed(f32),
}

impl From<f32> for Length {
    fn from(v: f32) -> Self {
        Length::Fixed(v)
    }
}

impl From<i32> for Length {
    fn from(v: i32) -> Self {
        Length::Fixed(v as f32)
    }
}

impl From<u32> for Length {
    fn from(v: u32) -> Self {
        Length::Fixed(v as f32)
    }
}

// ---------------------------------------------------------------------------
// Padding
// ---------------------------------------------------------------------------

/// Spacing between a widget's border and its content.
///
/// Construct uniformly, by axis, or per-side:
///
/// ```
/// use plushie::types::Padding;
///
/// let uniform = Padding::from(16.0);
/// let axis = Padding::from((16.0, 8.0));       // vertical, horizontal
/// let full = Padding::new(16.0, 8.0, 16.0, 8.0); // top, right, bottom, left
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Padding {
    /// Create padding with all four sides specified.
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self { top, right, bottom, left }
    }

    /// Create uniform padding on all sides.
    pub fn all(value: f32) -> Self {
        Self { top: value, right: value, bottom: value, left: value }
    }

    /// Create padding with vertical and horizontal values.
    pub fn axes(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

impl From<f32> for Padding {
    fn from(v: f32) -> Self {
        Self::all(v)
    }
}

impl From<i32> for Padding {
    fn from(v: i32) -> Self {
        Self::all(v as f32)
    }
}

impl From<(f32, f32)> for Padding {
    fn from((v, h): (f32, f32)) -> Self {
        Self::axes(v, h)
    }
}

impl From<(f32, f32, f32, f32)> for Padding {
    fn from((t, r, b, l): (f32, f32, f32, f32)) -> Self {
        Self::new(t, r, b, l)
    }
}

// ---------------------------------------------------------------------------
// Alignment
// ---------------------------------------------------------------------------

/// Horizontal or vertical alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
    Start,
    Center,
    End,
}

// ---------------------------------------------------------------------------
// Border
// ---------------------------------------------------------------------------

/// A widget border with color, width, and corner radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Border {
    pub color: Option<Color>,
    pub width: f32,
    pub radius: f32,
}

impl Border {
    pub fn new() -> Self {
        Self { color: None, width: 0.0, radius: 0.0 }
    }

    pub fn color(mut self, c: impl Into<Color>) -> Self {
        self.color = Some(c.into());
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
}

impl Default for Border {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Shadow
// ---------------------------------------------------------------------------

/// A drop shadow effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub color: Color,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
}

impl Shadow {
    pub fn new() -> Self {
        Self {
            color: Color::black(),
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
        }
    }

    pub fn color(mut self, c: impl Into<Color>) -> Self {
        self.color = c.into();
        self
    }

    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset_x = x;
        self.offset_y = y;
        self
    }

    pub fn blur_radius(mut self, r: f32) -> Self {
        self.blur_radius = r;
        self
    }
}

impl Default for Shadow {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Font
// ---------------------------------------------------------------------------

/// A font specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Font {
    pub family: Option<String>,
    pub weight: Option<FontWeight>,
    pub style: Option<FontStyle>,
}

impl Font {
    pub fn new() -> Self {
        Self { family: None, weight: None, style: None }
    }

    pub fn family(mut self, f: &str) -> Self {
        self.family = Some(f.to_string());
        self
    }

    pub fn weight(mut self, w: FontWeight) -> Self {
        self.weight = Some(w);
        self
    }

    pub fn style(mut self, s: FontStyle) -> Self {
        self.style = Some(s);
        self
    }

    /// Shorthand for a monospace font.
    pub fn monospace() -> Self {
        Self::new().family("monospace")
    }
}

impl Default for Font {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontWeight {
    Thin,
    ExtraLight,
    Light,
    Normal,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

// ---------------------------------------------------------------------------
// Style
// ---------------------------------------------------------------------------

/// A widget style: either a named preset or a custom style map.
///
/// ```
/// use plushie::types::Style;
///
/// let preset = Style::primary();
/// let custom = Style::custom()
///     .background(plushie::types::Color::red())
///     .text_color(plushie::types::Color::white());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Style {
    Preset(String),
    Custom(StyleMap),
}

impl Style {
    pub fn primary() -> Self { Self::Preset("primary".into()) }
    pub fn secondary() -> Self { Self::Preset("secondary".into()) }
    pub fn success() -> Self { Self::Preset("success".into()) }
    pub fn danger() -> Self { Self::Preset("danger".into()) }
    pub fn warning() -> Self { Self::Preset("warning".into()) }
    pub fn text() -> Self { Self::Preset("text".into()) }

    /// Start building a custom style.
    pub fn custom() -> StyleMap {
        StyleMap::new()
    }
}

impl From<&str> for Style {
    fn from(s: &str) -> Self {
        Self::Preset(s.to_string())
    }
}

impl From<StyleMap> for Style {
    fn from(m: StyleMap) -> Self {
        Self::Custom(m)
    }
}

// ---------------------------------------------------------------------------
// StyleMap
// ---------------------------------------------------------------------------

/// A custom style with explicit visual properties and per-status overrides.
///
/// Build fluently:
/// ```
/// use plushie::types::{StyleMap, Color, Border};
///
/// let style = StyleMap::new()
///     .background(Color::hex("#3498db"))
///     .text_color(Color::white())
///     .border(Border::new().radius(8.0))
///     .hovered(|s| s.background(Color::hex("#2980b9")));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StyleMap {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Shadow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hovered: Option<Box<StyleMap>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pressed: Option<Box<StyleMap>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<Box<StyleMap>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused: Option<Box<StyleMap>>,
}

impl StyleMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base(mut self, preset: &str) -> Self {
        self.base = Some(preset.to_string());
        self
    }

    pub fn background(mut self, c: impl Into<Color>) -> Self {
        self.background = Some(c.into());
        self
    }

    pub fn text_color(mut self, c: impl Into<Color>) -> Self {
        self.text_color = Some(c.into());
        self
    }

    pub fn border(mut self, b: Border) -> Self {
        self.border = Some(b);
        self
    }

    pub fn shadow(mut self, s: Shadow) -> Self {
        self.shadow = Some(s);
        self
    }

    pub fn hovered(mut self, f: impl FnOnce(StyleMap) -> StyleMap) -> Self {
        self.hovered = Some(Box::new(f(StyleMap::new())));
        self
    }

    pub fn pressed(mut self, f: impl FnOnce(StyleMap) -> StyleMap) -> Self {
        self.pressed = Some(Box::new(f(StyleMap::new())));
        self
    }

    pub fn disabled(mut self, f: impl FnOnce(StyleMap) -> StyleMap) -> Self {
        self.disabled = Some(Box::new(f(StyleMap::new())));
        self
    }

    pub fn focused(mut self, f: impl FnOnce(StyleMap) -> StyleMap) -> Self {
        self.focused = Some(Box::new(f(StyleMap::new())));
        self
    }
}

// ---------------------------------------------------------------------------
// Gradient
// ---------------------------------------------------------------------------

/// A linear gradient fill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub angle: f32,
    pub stops: Vec<GradientStop>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

impl Gradient {
    pub fn linear(angle: f32, stops: Vec<(f32, Color)>) -> Self {
        Self {
            angle,
            stops: stops
                .into_iter()
                .map(|(offset, color)| GradientStop { offset, color })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// KeyModifiers
// ---------------------------------------------------------------------------

/// Keyboard modifier state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
    pub command: bool,
}
