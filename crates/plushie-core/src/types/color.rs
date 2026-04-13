//! Color type for widget properties.

use serde_json::Value;

use crate::protocol::PropValue;

use super::PlushieType;

/// A color value, stored as a canonical hex string.
///
/// Wire format: a JSON string in `#rrggbb` or `#rrggbbaa` hex notation.
/// Named color constructors produce canonical hex strings internally.
///
/// Construct with named colors, hex strings, or RGB values:
///
/// ```
/// use plushie_core::types::Color;
///
/// let red = Color::red();
/// let custom = Color::hex("#3498db");
/// let rgb = Color::rgb(0.5, 0.8, 1.0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Color(String);

impl Color {
    /// Create a color from a hex string.
    ///
    /// The string should be in `#rrggbb` or `#rrggbbaa` format.
    /// No validation is performed; invalid formats will be
    /// silently ignored by the renderer.
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

    pub fn aliceblue() -> Self {
        Self::hex("#f0f8ff")
    }
    pub fn antiquewhite() -> Self {
        Self::hex("#faebd7")
    }
    pub fn aqua() -> Self {
        Self::hex("#00ffff")
    }
    pub fn aquamarine() -> Self {
        Self::hex("#7fffd4")
    }
    pub fn azure() -> Self {
        Self::hex("#f0ffff")
    }
    pub fn beige() -> Self {
        Self::hex("#f5f5dc")
    }
    pub fn bisque() -> Self {
        Self::hex("#ffe4c4")
    }
    pub fn black() -> Self {
        Self::hex("#000000")
    }
    pub fn blanchedalmond() -> Self {
        Self::hex("#ffebcd")
    }
    pub fn blue() -> Self {
        Self::hex("#0000ff")
    }
    pub fn blueviolet() -> Self {
        Self::hex("#8a2be2")
    }
    pub fn brown() -> Self {
        Self::hex("#a52a2a")
    }
    pub fn burlywood() -> Self {
        Self::hex("#deb887")
    }
    pub fn cadetblue() -> Self {
        Self::hex("#5f9ea0")
    }
    pub fn chartreuse() -> Self {
        Self::hex("#7fff00")
    }
    pub fn chocolate() -> Self {
        Self::hex("#d2691e")
    }
    pub fn coral() -> Self {
        Self::hex("#ff7f50")
    }
    pub fn cornflowerblue() -> Self {
        Self::hex("#6495ed")
    }
    pub fn cornsilk() -> Self {
        Self::hex("#fff8dc")
    }
    pub fn crimson() -> Self {
        Self::hex("#dc143c")
    }
    pub fn cyan() -> Self {
        Self::hex("#00ffff")
    }
    pub fn darkblue() -> Self {
        Self::hex("#00008b")
    }
    pub fn darkcyan() -> Self {
        Self::hex("#008b8b")
    }
    pub fn darkgoldenrod() -> Self {
        Self::hex("#b8860b")
    }
    pub fn darkgray() -> Self {
        Self::hex("#a9a9a9")
    }
    pub fn darkgreen() -> Self {
        Self::hex("#006400")
    }
    pub fn darkgrey() -> Self {
        Self::hex("#a9a9a9")
    }
    pub fn darkkhaki() -> Self {
        Self::hex("#bdb76b")
    }
    pub fn darkmagenta() -> Self {
        Self::hex("#8b008b")
    }
    pub fn darkolivegreen() -> Self {
        Self::hex("#556b2f")
    }
    pub fn darkorange() -> Self {
        Self::hex("#ff8c00")
    }
    pub fn darkorchid() -> Self {
        Self::hex("#9932cc")
    }
    pub fn darkred() -> Self {
        Self::hex("#8b0000")
    }
    pub fn darksalmon() -> Self {
        Self::hex("#e9967a")
    }
    pub fn darkseagreen() -> Self {
        Self::hex("#8fbc8f")
    }
    pub fn darkslateblue() -> Self {
        Self::hex("#483d8b")
    }
    pub fn darkslategray() -> Self {
        Self::hex("#2f4f4f")
    }
    pub fn darkslategrey() -> Self {
        Self::hex("#2f4f4f")
    }
    pub fn darkturquoise() -> Self {
        Self::hex("#00ced1")
    }
    pub fn darkviolet() -> Self {
        Self::hex("#9400d3")
    }
    pub fn deeppink() -> Self {
        Self::hex("#ff1493")
    }
    pub fn deepskyblue() -> Self {
        Self::hex("#00bfff")
    }
    pub fn dimgray() -> Self {
        Self::hex("#696969")
    }
    pub fn dimgrey() -> Self {
        Self::hex("#696969")
    }
    pub fn dodgerblue() -> Self {
        Self::hex("#1e90ff")
    }
    pub fn firebrick() -> Self {
        Self::hex("#b22222")
    }
    pub fn floralwhite() -> Self {
        Self::hex("#fffaf0")
    }
    pub fn forestgreen() -> Self {
        Self::hex("#228b22")
    }
    pub fn fuchsia() -> Self {
        Self::hex("#ff00ff")
    }
    pub fn gainsboro() -> Self {
        Self::hex("#dcdcdc")
    }
    pub fn ghostwhite() -> Self {
        Self::hex("#f8f8ff")
    }
    pub fn gold() -> Self {
        Self::hex("#ffd700")
    }
    pub fn goldenrod() -> Self {
        Self::hex("#daa520")
    }
    pub fn gray() -> Self {
        Self::hex("#808080")
    }
    pub fn green() -> Self {
        Self::hex("#008000")
    }
    pub fn greenyellow() -> Self {
        Self::hex("#adff2f")
    }
    pub fn grey() -> Self {
        Self::hex("#808080")
    }
    pub fn honeydew() -> Self {
        Self::hex("#f0fff0")
    }
    pub fn hotpink() -> Self {
        Self::hex("#ff69b4")
    }
    pub fn indianred() -> Self {
        Self::hex("#cd5c5c")
    }
    pub fn indigo() -> Self {
        Self::hex("#4b0082")
    }
    pub fn ivory() -> Self {
        Self::hex("#fffff0")
    }
    pub fn khaki() -> Self {
        Self::hex("#f0e68c")
    }
    pub fn lavender() -> Self {
        Self::hex("#e6e6fa")
    }
    pub fn lavenderblush() -> Self {
        Self::hex("#fff0f5")
    }
    pub fn lawngreen() -> Self {
        Self::hex("#7cfc00")
    }
    pub fn lemonchiffon() -> Self {
        Self::hex("#fffacd")
    }
    pub fn lightblue() -> Self {
        Self::hex("#add8e6")
    }
    pub fn lightcoral() -> Self {
        Self::hex("#f08080")
    }
    pub fn lightcyan() -> Self {
        Self::hex("#e0ffff")
    }
    pub fn lightgoldenrodyellow() -> Self {
        Self::hex("#fafad2")
    }
    pub fn lightgray() -> Self {
        Self::hex("#d3d3d3")
    }
    pub fn lightgreen() -> Self {
        Self::hex("#90ee90")
    }
    pub fn lightgrey() -> Self {
        Self::hex("#d3d3d3")
    }
    pub fn lightpink() -> Self {
        Self::hex("#ffb6c1")
    }
    pub fn lightsalmon() -> Self {
        Self::hex("#ffa07a")
    }
    pub fn lightseagreen() -> Self {
        Self::hex("#20b2aa")
    }
    pub fn lightskyblue() -> Self {
        Self::hex("#87cefa")
    }
    pub fn lightslategray() -> Self {
        Self::hex("#778899")
    }
    pub fn lightslategrey() -> Self {
        Self::hex("#778899")
    }
    pub fn lightsteelblue() -> Self {
        Self::hex("#b0c4de")
    }
    pub fn lightyellow() -> Self {
        Self::hex("#ffffe0")
    }
    pub fn lime() -> Self {
        Self::hex("#00ff00")
    }
    pub fn limegreen() -> Self {
        Self::hex("#32cd32")
    }
    pub fn linen() -> Self {
        Self::hex("#faf0e6")
    }
    pub fn magenta() -> Self {
        Self::hex("#ff00ff")
    }
    pub fn maroon() -> Self {
        Self::hex("#800000")
    }
    pub fn mediumaquamarine() -> Self {
        Self::hex("#66cdaa")
    }
    pub fn mediumblue() -> Self {
        Self::hex("#0000cd")
    }
    pub fn mediumorchid() -> Self {
        Self::hex("#ba55d3")
    }
    pub fn mediumpurple() -> Self {
        Self::hex("#9370db")
    }
    pub fn mediumseagreen() -> Self {
        Self::hex("#3cb371")
    }
    pub fn mediumslateblue() -> Self {
        Self::hex("#7b68ee")
    }
    pub fn mediumspringgreen() -> Self {
        Self::hex("#00fa9a")
    }
    pub fn mediumturquoise() -> Self {
        Self::hex("#48d1cc")
    }
    pub fn mediumvioletred() -> Self {
        Self::hex("#c71585")
    }
    pub fn midnightblue() -> Self {
        Self::hex("#191970")
    }
    pub fn mintcream() -> Self {
        Self::hex("#f5fffa")
    }
    pub fn mistyrose() -> Self {
        Self::hex("#ffe4e1")
    }
    pub fn moccasin() -> Self {
        Self::hex("#ffe4b5")
    }
    pub fn navajowhite() -> Self {
        Self::hex("#ffdead")
    }
    pub fn navy() -> Self {
        Self::hex("#000080")
    }
    pub fn oldlace() -> Self {
        Self::hex("#fdf5e6")
    }
    pub fn olive() -> Self {
        Self::hex("#808000")
    }
    pub fn olivedrab() -> Self {
        Self::hex("#6b8e23")
    }
    pub fn orange() -> Self {
        Self::hex("#ffa500")
    }
    pub fn orangered() -> Self {
        Self::hex("#ff4500")
    }
    pub fn orchid() -> Self {
        Self::hex("#da70d6")
    }
    pub fn palegoldenrod() -> Self {
        Self::hex("#eee8aa")
    }
    pub fn palegreen() -> Self {
        Self::hex("#98fb98")
    }
    pub fn paleturquoise() -> Self {
        Self::hex("#afeeee")
    }
    pub fn palevioletred() -> Self {
        Self::hex("#db7093")
    }
    pub fn papayawhip() -> Self {
        Self::hex("#ffefd5")
    }
    pub fn peachpuff() -> Self {
        Self::hex("#ffdab9")
    }
    pub fn peru() -> Self {
        Self::hex("#cd853f")
    }
    pub fn pink() -> Self {
        Self::hex("#ffc0cb")
    }
    pub fn plum() -> Self {
        Self::hex("#dda0dd")
    }
    pub fn powderblue() -> Self {
        Self::hex("#b0e0e6")
    }
    pub fn purple() -> Self {
        Self::hex("#800080")
    }
    pub fn rebeccapurple() -> Self {
        Self::hex("#663399")
    }
    pub fn red() -> Self {
        Self::hex("#ff0000")
    }
    pub fn rosybrown() -> Self {
        Self::hex("#bc8f8f")
    }
    pub fn royalblue() -> Self {
        Self::hex("#4169e1")
    }
    pub fn saddlebrown() -> Self {
        Self::hex("#8b4513")
    }
    pub fn salmon() -> Self {
        Self::hex("#fa8072")
    }
    pub fn sandybrown() -> Self {
        Self::hex("#f4a460")
    }
    pub fn seagreen() -> Self {
        Self::hex("#2e8b57")
    }
    pub fn seashell() -> Self {
        Self::hex("#fff5ee")
    }
    pub fn sienna() -> Self {
        Self::hex("#a0522d")
    }
    pub fn silver() -> Self {
        Self::hex("#c0c0c0")
    }
    pub fn skyblue() -> Self {
        Self::hex("#87ceeb")
    }
    pub fn slateblue() -> Self {
        Self::hex("#6a5acd")
    }
    pub fn slategray() -> Self {
        Self::hex("#708090")
    }
    pub fn slategrey() -> Self {
        Self::hex("#708090")
    }
    pub fn snow() -> Self {
        Self::hex("#fffafa")
    }
    pub fn springgreen() -> Self {
        Self::hex("#00ff7f")
    }
    pub fn steelblue() -> Self {
        Self::hex("#4682b4")
    }
    pub fn tan() -> Self {
        Self::hex("#d2b48c")
    }
    pub fn teal() -> Self {
        Self::hex("#008080")
    }
    pub fn thistle() -> Self {
        Self::hex("#d8bfd8")
    }
    pub fn tomato() -> Self {
        Self::hex("#ff6347")
    }
    pub fn transparent() -> Self {
        Self::hex("#00000000")
    }
    pub fn turquoise() -> Self {
        Self::hex("#40e0d0")
    }
    pub fn violet() -> Self {
        Self::hex("#ee82ee")
    }
    pub fn wheat() -> Self {
        Self::hex("#f5deb3")
    }
    pub fn white() -> Self {
        Self::hex("#ffffff")
    }
    pub fn whitesmoke() -> Self {
        Self::hex("#f5f5f5")
    }
    pub fn yellow() -> Self {
        Self::hex("#ffff00")
    }
    pub fn yellowgreen() -> Self {
        Self::hex("#9acd32")
    }
}

impl PlushieType for Color {
    fn wire_decode(value: &Value) -> Option<Self> {
        value.as_str().map(Color::hex)
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(self.0.clone())
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        props.get_str(key).map(Color::hex)
    }

    fn type_name() -> &'static str {
        "color"
    }
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

// Re-import Props for the extract override.
use crate::protocol::Props;
