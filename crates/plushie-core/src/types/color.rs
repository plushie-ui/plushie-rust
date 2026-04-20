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
/// let short = Color::hex("#f0f");       // expands to #ff00ff
/// let rgb = Color::rgb(0.5, 0.8, 1.0);
/// ```
///
/// # Named color catalog
///
/// All 148 CSS Color Module Level 4 names are exposed as constructor
/// methods alongside `transparent`. Method names match the canonical
/// lowercase CSS identifier (`Color::aliceblue()`, `Color::cornflowerblue()`,
/// `Color::rebeccapurple()`, etc.). The full list lives below in the
/// `impl Color` block and is kept in sync with the other host SDKs.
#[derive(Debug, Clone, PartialEq)]
pub struct Color(String);

impl Color {
    /// Create a color from a hex string.
    ///
    /// Accepts 3, 4, 6, or 8 hex digit formats (with or without
    /// `#` prefix). Short forms are expanded automatically:
    ///
    /// - `#rgb` / `rgb` expands to `#rrggbb`
    /// - `#rgba` / `rgba` expands to `#rrggbbaa`
    /// - `#rrggbb` / `rrggbb` stored as `#rrggbb`
    /// - `#rrggbbaa` / `rrggbbaa` stored as `#rrggbbaa`
    ///
    /// Use [`try_hex`](Self::try_hex) for a fallible variant that
    /// returns `Option<Color>`.
    ///
    /// # Panics
    ///
    /// Panics on invalid hex characters, unsupported lengths, or
    /// empty input. Intended for hard-coded hex literals where the
    /// input is known at compile time.
    pub fn hex(s: &str) -> Self {
        match Self::try_hex(s) {
            Some(c) => c,
            None => panic!("invalid hex color: \"{s}\""),
        }
    }

    /// Fallible version of [`hex`](Self::hex).
    ///
    /// Returns `None` for invalid hex characters, unsupported
    /// lengths, or empty input.
    pub fn try_hex(s: &str) -> Option<Self> {
        let digits = s.strip_prefix('#').unwrap_or(s);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        // Normalize to lowercase for consistent PartialEq behavior
        // (rgb()/rgba() produce lowercase hex, so hex() should too).
        let lower = digits.to_ascii_lowercase();
        let canonical = match lower.len() {
            3 | 4 => {
                let mut out = String::with_capacity(1 + lower.len() * 2);
                out.push('#');
                for c in lower.chars() {
                    out.push(c);
                    out.push(c);
                }
                out
            }
            6 | 8 => format!("#{lower}"),
            _ => return None,
        };
        Some(Self(canonical))
    }

    /// Create a color from RGB floats (0.0 to 1.0).
    ///
    /// Values outside the range are clamped.
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        let r = (r.clamp(0.0, 1.0) * 255.0) as u8;
        let g = (g.clamp(0.0, 1.0) * 255.0) as u8;
        let b = (b.clamp(0.0, 1.0) * 255.0) as u8;
        Self(format!("#{r:02x}{g:02x}{b:02x}"))
    }

    /// Create a color from RGBA floats (0.0 to 1.0).
    ///
    /// Values outside the range are clamped.
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

    /// CSS named color `aliceblue` (#f0f8ff).
    pub fn aliceblue() -> Self {
        Self::hex("#f0f8ff")
    }
    /// CSS named color `antiquewhite` (#faebd7).
    pub fn antiquewhite() -> Self {
        Self::hex("#faebd7")
    }
    /// CSS named color `aqua` (#00ffff).
    pub fn aqua() -> Self {
        Self::hex("#00ffff")
    }
    /// CSS named color `aquamarine` (#7fffd4).
    pub fn aquamarine() -> Self {
        Self::hex("#7fffd4")
    }
    /// CSS named color `azure` (#f0ffff).
    pub fn azure() -> Self {
        Self::hex("#f0ffff")
    }
    /// CSS named color `beige` (#f5f5dc).
    pub fn beige() -> Self {
        Self::hex("#f5f5dc")
    }
    /// CSS named color `bisque` (#ffe4c4).
    pub fn bisque() -> Self {
        Self::hex("#ffe4c4")
    }
    /// CSS named color `black` (#000000).
    pub fn black() -> Self {
        Self::hex("#000000")
    }
    /// CSS named color `blanchedalmond` (#ffebcd).
    pub fn blanchedalmond() -> Self {
        Self::hex("#ffebcd")
    }
    /// CSS named color `blue` (#0000ff).
    pub fn blue() -> Self {
        Self::hex("#0000ff")
    }
    /// CSS named color `blueviolet` (#8a2be2).
    pub fn blueviolet() -> Self {
        Self::hex("#8a2be2")
    }
    /// CSS named color `brown` (#a52a2a).
    pub fn brown() -> Self {
        Self::hex("#a52a2a")
    }
    /// CSS named color `burlywood` (#deb887).
    pub fn burlywood() -> Self {
        Self::hex("#deb887")
    }
    /// CSS named color `cadetblue` (#5f9ea0).
    pub fn cadetblue() -> Self {
        Self::hex("#5f9ea0")
    }
    /// CSS named color `chartreuse` (#7fff00).
    pub fn chartreuse() -> Self {
        Self::hex("#7fff00")
    }
    /// CSS named color `chocolate` (#d2691e).
    pub fn chocolate() -> Self {
        Self::hex("#d2691e")
    }
    /// CSS named color `coral` (#ff7f50).
    pub fn coral() -> Self {
        Self::hex("#ff7f50")
    }
    /// CSS named color `cornflowerblue` (#6495ed).
    pub fn cornflowerblue() -> Self {
        Self::hex("#6495ed")
    }
    /// CSS named color `cornsilk` (#fff8dc).
    pub fn cornsilk() -> Self {
        Self::hex("#fff8dc")
    }
    /// CSS named color `crimson` (#dc143c).
    pub fn crimson() -> Self {
        Self::hex("#dc143c")
    }
    /// CSS named color `cyan` (#00ffff).
    pub fn cyan() -> Self {
        Self::hex("#00ffff")
    }
    /// CSS named color `darkblue` (#00008b).
    pub fn darkblue() -> Self {
        Self::hex("#00008b")
    }
    /// CSS named color `darkcyan` (#008b8b).
    pub fn darkcyan() -> Self {
        Self::hex("#008b8b")
    }
    /// CSS named color `darkgoldenrod` (#b8860b).
    pub fn darkgoldenrod() -> Self {
        Self::hex("#b8860b")
    }
    /// CSS named color `darkgray` (#a9a9a9).
    pub fn darkgray() -> Self {
        Self::hex("#a9a9a9")
    }
    /// CSS named color `darkgreen` (#006400).
    pub fn darkgreen() -> Self {
        Self::hex("#006400")
    }
    /// CSS named color `darkgrey` (#a9a9a9).
    pub fn darkgrey() -> Self {
        Self::hex("#a9a9a9")
    }
    /// CSS named color `darkkhaki` (#bdb76b).
    pub fn darkkhaki() -> Self {
        Self::hex("#bdb76b")
    }
    /// CSS named color `darkmagenta` (#8b008b).
    pub fn darkmagenta() -> Self {
        Self::hex("#8b008b")
    }
    /// CSS named color `darkolivegreen` (#556b2f).
    pub fn darkolivegreen() -> Self {
        Self::hex("#556b2f")
    }
    /// CSS named color `darkorange` (#ff8c00).
    pub fn darkorange() -> Self {
        Self::hex("#ff8c00")
    }
    /// CSS named color `darkorchid` (#9932cc).
    pub fn darkorchid() -> Self {
        Self::hex("#9932cc")
    }
    /// CSS named color `darkred` (#8b0000).
    pub fn darkred() -> Self {
        Self::hex("#8b0000")
    }
    /// CSS named color `darksalmon` (#e9967a).
    pub fn darksalmon() -> Self {
        Self::hex("#e9967a")
    }
    /// CSS named color `darkseagreen` (#8fbc8f).
    pub fn darkseagreen() -> Self {
        Self::hex("#8fbc8f")
    }
    /// CSS named color `darkslateblue` (#483d8b).
    pub fn darkslateblue() -> Self {
        Self::hex("#483d8b")
    }
    /// CSS named color `darkslategray` (#2f4f4f).
    pub fn darkslategray() -> Self {
        Self::hex("#2f4f4f")
    }
    /// CSS named color `darkslategrey` (#2f4f4f).
    pub fn darkslategrey() -> Self {
        Self::hex("#2f4f4f")
    }
    /// CSS named color `darkturquoise` (#00ced1).
    pub fn darkturquoise() -> Self {
        Self::hex("#00ced1")
    }
    /// CSS named color `darkviolet` (#9400d3).
    pub fn darkviolet() -> Self {
        Self::hex("#9400d3")
    }
    /// CSS named color `deeppink` (#ff1493).
    pub fn deeppink() -> Self {
        Self::hex("#ff1493")
    }
    /// CSS named color `deepskyblue` (#00bfff).
    pub fn deepskyblue() -> Self {
        Self::hex("#00bfff")
    }
    /// CSS named color `dimgray` (#696969).
    pub fn dimgray() -> Self {
        Self::hex("#696969")
    }
    /// CSS named color `dimgrey` (#696969).
    pub fn dimgrey() -> Self {
        Self::hex("#696969")
    }
    /// CSS named color `dodgerblue` (#1e90ff).
    pub fn dodgerblue() -> Self {
        Self::hex("#1e90ff")
    }
    /// CSS named color `firebrick` (#b22222).
    pub fn firebrick() -> Self {
        Self::hex("#b22222")
    }
    /// CSS named color `floralwhite` (#fffaf0).
    pub fn floralwhite() -> Self {
        Self::hex("#fffaf0")
    }
    /// CSS named color `forestgreen` (#228b22).
    pub fn forestgreen() -> Self {
        Self::hex("#228b22")
    }
    /// CSS named color `fuchsia` (#ff00ff).
    pub fn fuchsia() -> Self {
        Self::hex("#ff00ff")
    }
    /// CSS named color `gainsboro` (#dcdcdc).
    pub fn gainsboro() -> Self {
        Self::hex("#dcdcdc")
    }
    /// CSS named color `ghostwhite` (#f8f8ff).
    pub fn ghostwhite() -> Self {
        Self::hex("#f8f8ff")
    }
    /// CSS named color `gold` (#ffd700).
    pub fn gold() -> Self {
        Self::hex("#ffd700")
    }
    /// CSS named color `goldenrod` (#daa520).
    pub fn goldenrod() -> Self {
        Self::hex("#daa520")
    }
    /// CSS named color `gray` (#808080).
    pub fn gray() -> Self {
        Self::hex("#808080")
    }
    /// CSS named color `green` (#008000).
    pub fn green() -> Self {
        Self::hex("#008000")
    }
    /// CSS named color `greenyellow` (#adff2f).
    pub fn greenyellow() -> Self {
        Self::hex("#adff2f")
    }
    /// CSS named color `grey` (#808080).
    pub fn grey() -> Self {
        Self::hex("#808080")
    }
    /// CSS named color `honeydew` (#f0fff0).
    pub fn honeydew() -> Self {
        Self::hex("#f0fff0")
    }
    /// CSS named color `hotpink` (#ff69b4).
    pub fn hotpink() -> Self {
        Self::hex("#ff69b4")
    }
    /// CSS named color `indianred` (#cd5c5c).
    pub fn indianred() -> Self {
        Self::hex("#cd5c5c")
    }
    /// CSS named color `indigo` (#4b0082).
    pub fn indigo() -> Self {
        Self::hex("#4b0082")
    }
    /// CSS named color `ivory` (#fffff0).
    pub fn ivory() -> Self {
        Self::hex("#fffff0")
    }
    /// CSS named color `khaki` (#f0e68c).
    pub fn khaki() -> Self {
        Self::hex("#f0e68c")
    }
    /// CSS named color `lavender` (#e6e6fa).
    pub fn lavender() -> Self {
        Self::hex("#e6e6fa")
    }
    /// CSS named color `lavenderblush` (#fff0f5).
    pub fn lavenderblush() -> Self {
        Self::hex("#fff0f5")
    }
    /// CSS named color `lawngreen` (#7cfc00).
    pub fn lawngreen() -> Self {
        Self::hex("#7cfc00")
    }
    /// CSS named color `lemonchiffon` (#fffacd).
    pub fn lemonchiffon() -> Self {
        Self::hex("#fffacd")
    }
    /// CSS named color `lightblue` (#add8e6).
    pub fn lightblue() -> Self {
        Self::hex("#add8e6")
    }
    /// CSS named color `lightcoral` (#f08080).
    pub fn lightcoral() -> Self {
        Self::hex("#f08080")
    }
    /// CSS named color `lightcyan` (#e0ffff).
    pub fn lightcyan() -> Self {
        Self::hex("#e0ffff")
    }
    /// CSS named color `lightgoldenrodyellow` (#fafad2).
    pub fn lightgoldenrodyellow() -> Self {
        Self::hex("#fafad2")
    }
    /// CSS named color `lightgray` (#d3d3d3).
    pub fn lightgray() -> Self {
        Self::hex("#d3d3d3")
    }
    /// CSS named color `lightgreen` (#90ee90).
    pub fn lightgreen() -> Self {
        Self::hex("#90ee90")
    }
    /// CSS named color `lightgrey` (#d3d3d3).
    pub fn lightgrey() -> Self {
        Self::hex("#d3d3d3")
    }
    /// CSS named color `lightpink` (#ffb6c1).
    pub fn lightpink() -> Self {
        Self::hex("#ffb6c1")
    }
    /// CSS named color `lightsalmon` (#ffa07a).
    pub fn lightsalmon() -> Self {
        Self::hex("#ffa07a")
    }
    /// CSS named color `lightseagreen` (#20b2aa).
    pub fn lightseagreen() -> Self {
        Self::hex("#20b2aa")
    }
    /// CSS named color `lightskyblue` (#87cefa).
    pub fn lightskyblue() -> Self {
        Self::hex("#87cefa")
    }
    /// CSS named color `lightslategray` (#778899).
    pub fn lightslategray() -> Self {
        Self::hex("#778899")
    }
    /// CSS named color `lightslategrey` (#778899).
    pub fn lightslategrey() -> Self {
        Self::hex("#778899")
    }
    /// CSS named color `lightsteelblue` (#b0c4de).
    pub fn lightsteelblue() -> Self {
        Self::hex("#b0c4de")
    }
    /// CSS named color `lightyellow` (#ffffe0).
    pub fn lightyellow() -> Self {
        Self::hex("#ffffe0")
    }
    /// CSS named color `lime` (#00ff00).
    pub fn lime() -> Self {
        Self::hex("#00ff00")
    }
    /// CSS named color `limegreen` (#32cd32).
    pub fn limegreen() -> Self {
        Self::hex("#32cd32")
    }
    /// CSS named color `linen` (#faf0e6).
    pub fn linen() -> Self {
        Self::hex("#faf0e6")
    }
    /// CSS named color `magenta` (#ff00ff).
    pub fn magenta() -> Self {
        Self::hex("#ff00ff")
    }
    /// CSS named color `maroon` (#800000).
    pub fn maroon() -> Self {
        Self::hex("#800000")
    }
    /// CSS named color `mediumaquamarine` (#66cdaa).
    pub fn mediumaquamarine() -> Self {
        Self::hex("#66cdaa")
    }
    /// CSS named color `mediumblue` (#0000cd).
    pub fn mediumblue() -> Self {
        Self::hex("#0000cd")
    }
    /// CSS named color `mediumorchid` (#ba55d3).
    pub fn mediumorchid() -> Self {
        Self::hex("#ba55d3")
    }
    /// CSS named color `mediumpurple` (#9370db).
    pub fn mediumpurple() -> Self {
        Self::hex("#9370db")
    }
    /// CSS named color `mediumseagreen` (#3cb371).
    pub fn mediumseagreen() -> Self {
        Self::hex("#3cb371")
    }
    /// CSS named color `mediumslateblue` (#7b68ee).
    pub fn mediumslateblue() -> Self {
        Self::hex("#7b68ee")
    }
    /// CSS named color `mediumspringgreen` (#00fa9a).
    pub fn mediumspringgreen() -> Self {
        Self::hex("#00fa9a")
    }
    /// CSS named color `mediumturquoise` (#48d1cc).
    pub fn mediumturquoise() -> Self {
        Self::hex("#48d1cc")
    }
    /// CSS named color `mediumvioletred` (#c71585).
    pub fn mediumvioletred() -> Self {
        Self::hex("#c71585")
    }
    /// CSS named color `midnightblue` (#191970).
    pub fn midnightblue() -> Self {
        Self::hex("#191970")
    }
    /// CSS named color `mintcream` (#f5fffa).
    pub fn mintcream() -> Self {
        Self::hex("#f5fffa")
    }
    /// CSS named color `mistyrose` (#ffe4e1).
    pub fn mistyrose() -> Self {
        Self::hex("#ffe4e1")
    }
    /// CSS named color `moccasin` (#ffe4b5).
    pub fn moccasin() -> Self {
        Self::hex("#ffe4b5")
    }
    /// CSS named color `navajowhite` (#ffdead).
    pub fn navajowhite() -> Self {
        Self::hex("#ffdead")
    }
    /// CSS named color `navy` (#000080).
    pub fn navy() -> Self {
        Self::hex("#000080")
    }
    /// CSS named color `oldlace` (#fdf5e6).
    pub fn oldlace() -> Self {
        Self::hex("#fdf5e6")
    }
    /// CSS named color `olive` (#808000).
    pub fn olive() -> Self {
        Self::hex("#808000")
    }
    /// CSS named color `olivedrab` (#6b8e23).
    pub fn olivedrab() -> Self {
        Self::hex("#6b8e23")
    }
    /// CSS named color `orange` (#ffa500).
    pub fn orange() -> Self {
        Self::hex("#ffa500")
    }
    /// CSS named color `orangered` (#ff4500).
    pub fn orangered() -> Self {
        Self::hex("#ff4500")
    }
    /// CSS named color `orchid` (#da70d6).
    pub fn orchid() -> Self {
        Self::hex("#da70d6")
    }
    /// CSS named color `palegoldenrod` (#eee8aa).
    pub fn palegoldenrod() -> Self {
        Self::hex("#eee8aa")
    }
    /// CSS named color `palegreen` (#98fb98).
    pub fn palegreen() -> Self {
        Self::hex("#98fb98")
    }
    /// CSS named color `paleturquoise` (#afeeee).
    pub fn paleturquoise() -> Self {
        Self::hex("#afeeee")
    }
    /// CSS named color `palevioletred` (#db7093).
    pub fn palevioletred() -> Self {
        Self::hex("#db7093")
    }
    /// CSS named color `papayawhip` (#ffefd5).
    pub fn papayawhip() -> Self {
        Self::hex("#ffefd5")
    }
    /// CSS named color `peachpuff` (#ffdab9).
    pub fn peachpuff() -> Self {
        Self::hex("#ffdab9")
    }
    /// CSS named color `peru` (#cd853f).
    pub fn peru() -> Self {
        Self::hex("#cd853f")
    }
    /// CSS named color `pink` (#ffc0cb).
    pub fn pink() -> Self {
        Self::hex("#ffc0cb")
    }
    /// CSS named color `plum` (#dda0dd).
    pub fn plum() -> Self {
        Self::hex("#dda0dd")
    }
    /// CSS named color `powderblue` (#b0e0e6).
    pub fn powderblue() -> Self {
        Self::hex("#b0e0e6")
    }
    /// CSS named color `purple` (#800080).
    pub fn purple() -> Self {
        Self::hex("#800080")
    }
    /// CSS named color `rebeccapurple` (#663399).
    pub fn rebeccapurple() -> Self {
        Self::hex("#663399")
    }
    /// CSS named color `red` (#ff0000).
    pub fn red() -> Self {
        Self::hex("#ff0000")
    }
    /// CSS named color `rosybrown` (#bc8f8f).
    pub fn rosybrown() -> Self {
        Self::hex("#bc8f8f")
    }
    /// CSS named color `royalblue` (#4169e1).
    pub fn royalblue() -> Self {
        Self::hex("#4169e1")
    }
    /// CSS named color `saddlebrown` (#8b4513).
    pub fn saddlebrown() -> Self {
        Self::hex("#8b4513")
    }
    /// CSS named color `salmon` (#fa8072).
    pub fn salmon() -> Self {
        Self::hex("#fa8072")
    }
    /// CSS named color `sandybrown` (#f4a460).
    pub fn sandybrown() -> Self {
        Self::hex("#f4a460")
    }
    /// CSS named color `seagreen` (#2e8b57).
    pub fn seagreen() -> Self {
        Self::hex("#2e8b57")
    }
    /// CSS named color `seashell` (#fff5ee).
    pub fn seashell() -> Self {
        Self::hex("#fff5ee")
    }
    /// CSS named color `sienna` (#a0522d).
    pub fn sienna() -> Self {
        Self::hex("#a0522d")
    }
    /// CSS named color `silver` (#c0c0c0).
    pub fn silver() -> Self {
        Self::hex("#c0c0c0")
    }
    /// CSS named color `skyblue` (#87ceeb).
    pub fn skyblue() -> Self {
        Self::hex("#87ceeb")
    }
    /// CSS named color `slateblue` (#6a5acd).
    pub fn slateblue() -> Self {
        Self::hex("#6a5acd")
    }
    /// CSS named color `slategray` (#708090).
    pub fn slategray() -> Self {
        Self::hex("#708090")
    }
    /// CSS named color `slategrey` (#708090).
    pub fn slategrey() -> Self {
        Self::hex("#708090")
    }
    /// CSS named color `snow` (#fffafa).
    pub fn snow() -> Self {
        Self::hex("#fffafa")
    }
    /// CSS named color `springgreen` (#00ff7f).
    pub fn springgreen() -> Self {
        Self::hex("#00ff7f")
    }
    /// CSS named color `steelblue` (#4682b4).
    pub fn steelblue() -> Self {
        Self::hex("#4682b4")
    }
    /// CSS named color `tan` (#d2b48c).
    pub fn tan() -> Self {
        Self::hex("#d2b48c")
    }
    /// CSS named color `teal` (#008080).
    pub fn teal() -> Self {
        Self::hex("#008080")
    }
    /// CSS named color `thistle` (#d8bfd8).
    pub fn thistle() -> Self {
        Self::hex("#d8bfd8")
    }
    /// CSS named color `tomato` (#ff6347).
    pub fn tomato() -> Self {
        Self::hex("#ff6347")
    }
    /// Transparent (zero alpha).
    pub fn transparent() -> Self {
        Self::hex("#00000000")
    }
    /// CSS named color `turquoise` (#40e0d0).
    pub fn turquoise() -> Self {
        Self::hex("#40e0d0")
    }
    /// CSS named color `violet` (#ee82ee).
    pub fn violet() -> Self {
        Self::hex("#ee82ee")
    }
    /// CSS named color `wheat` (#f5deb3).
    pub fn wheat() -> Self {
        Self::hex("#f5deb3")
    }
    /// CSS named color `white` (#ffffff).
    pub fn white() -> Self {
        Self::hex("#ffffff")
    }
    /// CSS named color `whitesmoke` (#f5f5f5).
    pub fn whitesmoke() -> Self {
        Self::hex("#f5f5f5")
    }
    /// CSS named color `yellow` (#ffff00).
    pub fn yellow() -> Self {
        Self::hex("#ffff00")
    }
    /// CSS named color `yellowgreen` (#9acd32).
    pub fn yellowgreen() -> Self {
        Self::hex("#9acd32")
    }
}

impl Color {
    /// Strict hex parser used by the wire decoder.
    ///
    /// Accepts only `#rrggbb` and `#rrggbbaa`. Short forms (`#rgb`,
    /// `#rgba`) are rejected because the wire protocol requires
    /// canonical hex. Host SDKs must normalize before sending.
    fn try_hex_strict(s: &str) -> Option<Self> {
        let digits = s.strip_prefix('#').unwrap_or(s);
        if !matches!(digits.len(), 6 | 8) {
            return None;
        }
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self(format!("#{}", digits.to_ascii_lowercase())))
    }
}

impl PlushieType for Color {
    fn wire_decode(value: &Value) -> Option<Self> {
        value.as_str().and_then(Color::try_hex_strict)
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(self.0.clone())
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        props.get_str(key).and_then(Color::try_hex_strict)
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
        Self::hex(&s)
    }
}

// Re-import Props for the extract override.
use crate::protocol::Props;
