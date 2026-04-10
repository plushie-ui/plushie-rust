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

    // --- All 148 CSS Color Module Level 4 named colors + transparent ---

    pub fn aliceblue() -> Self { Self::hex("#f0f8ff") }
    pub fn antiquewhite() -> Self { Self::hex("#faebd7") }
    pub fn aqua() -> Self { Self::hex("#00ffff") }
    pub fn aquamarine() -> Self { Self::hex("#7fffd4") }
    pub fn azure() -> Self { Self::hex("#f0ffff") }
    pub fn beige() -> Self { Self::hex("#f5f5dc") }
    pub fn bisque() -> Self { Self::hex("#ffe4c4") }
    pub fn black() -> Self { Self::hex("#000000") }
    pub fn blanchedalmond() -> Self { Self::hex("#ffebcd") }
    pub fn blue() -> Self { Self::hex("#0000ff") }
    pub fn blueviolet() -> Self { Self::hex("#8a2be2") }
    pub fn brown() -> Self { Self::hex("#a52a2a") }
    pub fn burlywood() -> Self { Self::hex("#deb887") }
    pub fn cadetblue() -> Self { Self::hex("#5f9ea0") }
    pub fn chartreuse() -> Self { Self::hex("#7fff00") }
    pub fn chocolate() -> Self { Self::hex("#d2691e") }
    pub fn coral() -> Self { Self::hex("#ff7f50") }
    pub fn cornflowerblue() -> Self { Self::hex("#6495ed") }
    pub fn cornsilk() -> Self { Self::hex("#fff8dc") }
    pub fn crimson() -> Self { Self::hex("#dc143c") }
    pub fn cyan() -> Self { Self::hex("#00ffff") }
    pub fn darkblue() -> Self { Self::hex("#00008b") }
    pub fn darkcyan() -> Self { Self::hex("#008b8b") }
    pub fn darkgoldenrod() -> Self { Self::hex("#b8860b") }
    pub fn darkgray() -> Self { Self::hex("#a9a9a9") }
    pub fn darkgreen() -> Self { Self::hex("#006400") }
    pub fn darkgrey() -> Self { Self::hex("#a9a9a9") }
    pub fn darkkhaki() -> Self { Self::hex("#bdb76b") }
    pub fn darkmagenta() -> Self { Self::hex("#8b008b") }
    pub fn darkolivegreen() -> Self { Self::hex("#556b2f") }
    pub fn darkorange() -> Self { Self::hex("#ff8c00") }
    pub fn darkorchid() -> Self { Self::hex("#9932cc") }
    pub fn darkred() -> Self { Self::hex("#8b0000") }
    pub fn darksalmon() -> Self { Self::hex("#e9967a") }
    pub fn darkseagreen() -> Self { Self::hex("#8fbc8f") }
    pub fn darkslateblue() -> Self { Self::hex("#483d8b") }
    pub fn darkslategray() -> Self { Self::hex("#2f4f4f") }
    pub fn darkslategrey() -> Self { Self::hex("#2f4f4f") }
    pub fn darkturquoise() -> Self { Self::hex("#00ced1") }
    pub fn darkviolet() -> Self { Self::hex("#9400d3") }
    pub fn deeppink() -> Self { Self::hex("#ff1493") }
    pub fn deepskyblue() -> Self { Self::hex("#00bfff") }
    pub fn dimgray() -> Self { Self::hex("#696969") }
    pub fn dimgrey() -> Self { Self::hex("#696969") }
    pub fn dodgerblue() -> Self { Self::hex("#1e90ff") }
    pub fn firebrick() -> Self { Self::hex("#b22222") }
    pub fn floralwhite() -> Self { Self::hex("#fffaf0") }
    pub fn forestgreen() -> Self { Self::hex("#228b22") }
    pub fn fuchsia() -> Self { Self::hex("#ff00ff") }
    pub fn gainsboro() -> Self { Self::hex("#dcdcdc") }
    pub fn ghostwhite() -> Self { Self::hex("#f8f8ff") }
    pub fn gold() -> Self { Self::hex("#ffd700") }
    pub fn goldenrod() -> Self { Self::hex("#daa520") }
    pub fn gray() -> Self { Self::hex("#808080") }
    pub fn green() -> Self { Self::hex("#008000") }
    pub fn greenyellow() -> Self { Self::hex("#adff2f") }
    pub fn grey() -> Self { Self::hex("#808080") }
    pub fn honeydew() -> Self { Self::hex("#f0fff0") }
    pub fn hotpink() -> Self { Self::hex("#ff69b4") }
    pub fn indianred() -> Self { Self::hex("#cd5c5c") }
    pub fn indigo() -> Self { Self::hex("#4b0082") }
    pub fn ivory() -> Self { Self::hex("#fffff0") }
    pub fn khaki() -> Self { Self::hex("#f0e68c") }
    pub fn lavender() -> Self { Self::hex("#e6e6fa") }
    pub fn lavenderblush() -> Self { Self::hex("#fff0f5") }
    pub fn lawngreen() -> Self { Self::hex("#7cfc00") }
    pub fn lemonchiffon() -> Self { Self::hex("#fffacd") }
    pub fn lightblue() -> Self { Self::hex("#add8e6") }
    pub fn lightcoral() -> Self { Self::hex("#f08080") }
    pub fn lightcyan() -> Self { Self::hex("#e0ffff") }
    pub fn lightgoldenrodyellow() -> Self { Self::hex("#fafad2") }
    pub fn lightgray() -> Self { Self::hex("#d3d3d3") }
    pub fn lightgreen() -> Self { Self::hex("#90ee90") }
    pub fn lightgrey() -> Self { Self::hex("#d3d3d3") }
    pub fn lightpink() -> Self { Self::hex("#ffb6c1") }
    pub fn lightsalmon() -> Self { Self::hex("#ffa07a") }
    pub fn lightseagreen() -> Self { Self::hex("#20b2aa") }
    pub fn lightskyblue() -> Self { Self::hex("#87cefa") }
    pub fn lightslategray() -> Self { Self::hex("#778899") }
    pub fn lightslategrey() -> Self { Self::hex("#778899") }
    pub fn lightsteelblue() -> Self { Self::hex("#b0c4de") }
    pub fn lightyellow() -> Self { Self::hex("#ffffe0") }
    pub fn lime() -> Self { Self::hex("#00ff00") }
    pub fn limegreen() -> Self { Self::hex("#32cd32") }
    pub fn linen() -> Self { Self::hex("#faf0e6") }
    pub fn magenta() -> Self { Self::hex("#ff00ff") }
    pub fn maroon() -> Self { Self::hex("#800000") }
    pub fn mediumaquamarine() -> Self { Self::hex("#66cdaa") }
    pub fn mediumblue() -> Self { Self::hex("#0000cd") }
    pub fn mediumorchid() -> Self { Self::hex("#ba55d3") }
    pub fn mediumpurple() -> Self { Self::hex("#9370db") }
    pub fn mediumseagreen() -> Self { Self::hex("#3cb371") }
    pub fn mediumslateblue() -> Self { Self::hex("#7b68ee") }
    pub fn mediumspringgreen() -> Self { Self::hex("#00fa9a") }
    pub fn mediumturquoise() -> Self { Self::hex("#48d1cc") }
    pub fn mediumvioletred() -> Self { Self::hex("#c71585") }
    pub fn midnightblue() -> Self { Self::hex("#191970") }
    pub fn mintcream() -> Self { Self::hex("#f5fffa") }
    pub fn mistyrose() -> Self { Self::hex("#ffe4e1") }
    pub fn moccasin() -> Self { Self::hex("#ffe4b5") }
    pub fn navajowhite() -> Self { Self::hex("#ffdead") }
    pub fn navy() -> Self { Self::hex("#000080") }
    pub fn oldlace() -> Self { Self::hex("#fdf5e6") }
    pub fn olive() -> Self { Self::hex("#808000") }
    pub fn olivedrab() -> Self { Self::hex("#6b8e23") }
    pub fn orange() -> Self { Self::hex("#ffa500") }
    pub fn orangered() -> Self { Self::hex("#ff4500") }
    pub fn orchid() -> Self { Self::hex("#da70d6") }
    pub fn palegoldenrod() -> Self { Self::hex("#eee8aa") }
    pub fn palegreen() -> Self { Self::hex("#98fb98") }
    pub fn paleturquoise() -> Self { Self::hex("#afeeee") }
    pub fn palevioletred() -> Self { Self::hex("#db7093") }
    pub fn papayawhip() -> Self { Self::hex("#ffefd5") }
    pub fn peachpuff() -> Self { Self::hex("#ffdab9") }
    pub fn peru() -> Self { Self::hex("#cd853f") }
    pub fn pink() -> Self { Self::hex("#ffc0cb") }
    pub fn plum() -> Self { Self::hex("#dda0dd") }
    pub fn powderblue() -> Self { Self::hex("#b0e0e6") }
    pub fn purple() -> Self { Self::hex("#800080") }
    pub fn rebeccapurple() -> Self { Self::hex("#663399") }
    pub fn red() -> Self { Self::hex("#ff0000") }
    pub fn rosybrown() -> Self { Self::hex("#bc8f8f") }
    pub fn royalblue() -> Self { Self::hex("#4169e1") }
    pub fn saddlebrown() -> Self { Self::hex("#8b4513") }
    pub fn salmon() -> Self { Self::hex("#fa8072") }
    pub fn sandybrown() -> Self { Self::hex("#f4a460") }
    pub fn seagreen() -> Self { Self::hex("#2e8b57") }
    pub fn seashell() -> Self { Self::hex("#fff5ee") }
    pub fn sienna() -> Self { Self::hex("#a0522d") }
    pub fn silver() -> Self { Self::hex("#c0c0c0") }
    pub fn skyblue() -> Self { Self::hex("#87ceeb") }
    pub fn slateblue() -> Self { Self::hex("#6a5acd") }
    pub fn slategray() -> Self { Self::hex("#708090") }
    pub fn slategrey() -> Self { Self::hex("#708090") }
    pub fn snow() -> Self { Self::hex("#fffafa") }
    pub fn springgreen() -> Self { Self::hex("#00ff7f") }
    pub fn steelblue() -> Self { Self::hex("#4682b4") }
    pub fn tan() -> Self { Self::hex("#d2b48c") }
    pub fn teal() -> Self { Self::hex("#008080") }
    pub fn thistle() -> Self { Self::hex("#d8bfd8") }
    pub fn tomato() -> Self { Self::hex("#ff6347") }
    pub fn transparent() -> Self { Self::hex("#00000000") }
    pub fn turquoise() -> Self { Self::hex("#40e0d0") }
    pub fn violet() -> Self { Self::hex("#ee82ee") }
    pub fn wheat() -> Self { Self::hex("#f5deb3") }
    pub fn white() -> Self { Self::hex("#ffffff") }
    pub fn whitesmoke() -> Self { Self::hex("#f5f5f5") }
    pub fn yellow() -> Self { Self::hex("#ffff00") }
    pub fn yellowgreen() -> Self { Self::hex("#9acd32") }
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
    /// Align to the start (left or top).
    Start,
    /// Align to the center.
    Center,
    /// Align to the end (right or bottom).
    End,
}

// ---------------------------------------------------------------------------
// Border
// ---------------------------------------------------------------------------

/// Corner radius for a border: uniform or per-corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Radius {
    /// Same radius on all four corners.
    Uniform(f32),
    /// Individual radius for each corner.
    PerCorner {
        top_left: f32,
        top_right: f32,
        bottom_right: f32,
        bottom_left: f32,
    },
}

impl Default for Radius {
    fn default() -> Self {
        Self::Uniform(0.0)
    }
}

impl Serialize for Radius {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Uniform(r) => serializer.serialize_f32(*r),
            Self::PerCorner { top_left, top_right, bottom_right, bottom_left } => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(4))?;
                map.serialize_entry("top_left", top_left)?;
                map.serialize_entry("top_right", top_right)?;
                map.serialize_entry("bottom_right", bottom_right)?;
                map.serialize_entry("bottom_left", bottom_left)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Radius {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        struct RadiusVisitor;

        impl<'de> de::Visitor<'de> for RadiusVisitor {
            type Value = Radius;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a number or an object with top_left, top_right, bottom_right, bottom_left")
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Radius, E> {
                Ok(Radius::Uniform(v as f32))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Radius, E> {
                Ok(Radius::Uniform(v as f32))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Radius, E> {
                Ok(Radius::Uniform(v as f32))
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Radius, A::Error> {
                let mut tl: Option<f32> = None;
                let mut tr: Option<f32> = None;
                let mut br: Option<f32> = None;
                let mut bl: Option<f32> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "top_left" => tl = Some(map.next_value()?),
                        "top_right" => tr = Some(map.next_value()?),
                        "bottom_right" => br = Some(map.next_value()?),
                        "bottom_left" => bl = Some(map.next_value()?),
                        _ => { let _ = map.next_value::<serde::de::IgnoredAny>()?; }
                    }
                }

                Ok(Radius::PerCorner {
                    top_left: tl.unwrap_or(0.0),
                    top_right: tr.unwrap_or(0.0),
                    bottom_right: br.unwrap_or(0.0),
                    bottom_left: bl.unwrap_or(0.0),
                })
            }
        }

        deserializer.deserialize_any(RadiusVisitor)
    }
}

impl From<f32> for Radius {
    fn from(r: f32) -> Self {
        Self::Uniform(r)
    }
}

/// A widget border with color, width, and corner radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Border {
    pub color: Option<Color>,
    pub width: f32,
    pub radius: Radius,
}

impl Border {
    pub fn new() -> Self {
        Self { color: None, width: 0.0, radius: Radius::default() }
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
        self.radius = Radius::Uniform(r);
        self
    }

    pub fn radius_corners(mut self, tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        self.radius = Radius::PerCorner {
            top_left: tl,
            top_right: tr,
            bottom_right: br,
            bottom_left: bl,
        };
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
///
/// Serializes offset as `"offset": [x, y]` to match the wire format.
#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
}

impl Serialize for Shadow {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("color", &self.color)?;
        map.serialize_entry("offset", &[self.offset_x, self.offset_y])?;
        map.serialize_entry("blur_radius", &self.blur_radius)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for Shadow {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct ShadowHelper {
            color: Color,
            #[serde(default)]
            offset: Option<[f32; 2]>,
            #[serde(default)]
            offset_x: Option<f32>,
            #[serde(default)]
            offset_y: Option<f32>,
            #[serde(default)]
            blur_radius: f32,
        }
        let h = ShadowHelper::deserialize(deserializer)?;
        let (ox, oy) = match h.offset {
            Some([x, y]) => (x, y),
            None => (h.offset_x.unwrap_or(0.0), h.offset_y.unwrap_or(0.0)),
        };
        Ok(Shadow { color: h.color, offset_x: ox, offset_y: oy, blur_radius: h.blur_radius })
    }
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
    pub stretch: Option<FontStretch>,
}

impl Font {
    pub fn new() -> Self {
        Self { family: None, weight: None, style: None, stretch: None }
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

    pub fn stretch(mut self, s: FontStretch) -> Self {
        self.stretch = Some(s);
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
/// Font weight (CSS numeric equivalents in parentheses).
pub enum FontWeight {
    /// Weight 100.
    Thin,
    /// Weight 200.
    ExtraLight,
    /// Weight 300.
    Light,
    /// Weight 400 (default).
    Normal,
    /// Weight 500.
    Medium,
    /// Weight 600.
    SemiBold,
    /// Weight 700.
    Bold,
    /// Weight 800.
    ExtraBold,
    /// Weight 900.
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Font style (upright, italic, or oblique).
pub enum FontStyle {
    /// Upright (roman) style.
    Normal,
    /// Italic style, using the font's italic glyphs.
    Italic,
    /// Oblique style, a slanted version of the upright glyphs.
    Oblique,
}

/// Font stretch (width), from ultra-condensed to ultra-expanded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStretch {
    /// 50% of normal width.
    UltraCondensed,
    /// 62.5% of normal width.
    ExtraCondensed,
    /// 75% of normal width.
    Condensed,
    /// 87.5% of normal width.
    SemiCondensed,
    /// 100% (default width).
    Normal,
    /// 112.5% of normal width.
    SemiExpanded,
    /// 125% of normal width.
    Expanded,
    /// 150% of normal width.
    ExtraExpanded,
    /// 200% of normal width.
    UltraExpanded,
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
    /// Default widget appearance.
    pub fn default() -> Self { Self::Preset("default".into()) }
    /// Dark variant.
    pub fn dark() -> Self { Self::Preset("dark".into()) }
    /// Subdued/weak appearance.
    pub fn weak() -> Self { Self::Preset("weak".into()) }
    /// Container with rounded border.
    pub fn rounded_box() -> Self { Self::Preset("rounded_box".into()) }
    /// Container with square border.
    pub fn bordered_box() -> Self { Self::Preset("bordered_box".into()) }
    /// Fully transparent background.
    pub fn transparent() -> Self { Self::Preset("transparent".into()) }

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
// Background
// ---------------------------------------------------------------------------

/// A background fill: either a solid color or a gradient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Background {
    /// A solid color fill.
    Color(Color),
    /// A gradient fill.
    Gradient(Gradient),
}

impl From<Color> for Background {
    fn from(c: Color) -> Self {
        Self::Color(c)
    }
}

impl From<Gradient> for Background {
    fn from(g: Gradient) -> Self {
        Self::Gradient(g)
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
    pub background: Option<Background>,
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

    pub fn background(mut self, bg: impl Into<Background>) -> Self {
        self.background = Some(bg.into());
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
