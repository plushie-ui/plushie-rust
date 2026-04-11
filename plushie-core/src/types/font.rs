//! Font types for text rendering.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue, Props};

use super::PlushieType;

/// Font weight (CSS numeric equivalents in parentheses).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl PlushieType for FontWeight {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "thin" => Some(Self::Thin),
            "extra_light" => Some(Self::ExtraLight),
            "light" => Some(Self::Light),
            "normal" => Some(Self::Normal),
            "medium" => Some(Self::Medium),
            "semi_bold" => Some(Self::SemiBold),
            "bold" => Some(Self::Bold),
            "extra_bold" => Some(Self::ExtraBold),
            "black" => Some(Self::Black),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Thin => "thin",
                Self::ExtraLight => "extra_light",
                Self::Light => "light",
                Self::Normal => "normal",
                Self::Medium => "medium",
                Self::SemiBold => "semi_bold",
                Self::Bold => "bold",
                Self::ExtraBold => "extra_bold",
                Self::Black => "black",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "thin" => Some(Self::Thin),
            "extra_light" => Some(Self::ExtraLight),
            "light" => Some(Self::Light),
            "normal" => Some(Self::Normal),
            "medium" => Some(Self::Medium),
            "semi_bold" => Some(Self::SemiBold),
            "bold" => Some(Self::Bold),
            "extra_bold" => Some(Self::ExtraBold),
            "black" => Some(Self::Black),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "font_weight"
    }
}

/// Font style (upright, italic, or oblique).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    /// Upright (roman) style.
    Normal,
    /// Italic style, using the font's italic glyphs.
    Italic,
    /// Oblique style, a slanted version of the upright glyphs.
    Oblique,
}

impl PlushieType for FontStyle {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "normal" => Some(Self::Normal),
            "italic" => Some(Self::Italic),
            "oblique" => Some(Self::Oblique),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Normal => "normal",
                Self::Italic => "italic",
                Self::Oblique => "oblique",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "normal" => Some(Self::Normal),
            "italic" => Some(Self::Italic),
            "oblique" => Some(Self::Oblique),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "font_style"
    }
}

/// Font stretch (width), from ultra-condensed to ultra-expanded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl PlushieType for FontStretch {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "ultra_condensed" => Some(Self::UltraCondensed),
            "extra_condensed" => Some(Self::ExtraCondensed),
            "condensed" => Some(Self::Condensed),
            "semi_condensed" => Some(Self::SemiCondensed),
            "normal" => Some(Self::Normal),
            "semi_expanded" => Some(Self::SemiExpanded),
            "expanded" => Some(Self::Expanded),
            "extra_expanded" => Some(Self::ExtraExpanded),
            "ultra_expanded" => Some(Self::UltraExpanded),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::UltraCondensed => "ultra_condensed",
                Self::ExtraCondensed => "extra_condensed",
                Self::Condensed => "condensed",
                Self::SemiCondensed => "semi_condensed",
                Self::Normal => "normal",
                Self::SemiExpanded => "semi_expanded",
                Self::Expanded => "expanded",
                Self::ExtraExpanded => "extra_expanded",
                Self::UltraExpanded => "ultra_expanded",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "ultra_condensed" => Some(Self::UltraCondensed),
            "extra_condensed" => Some(Self::ExtraCondensed),
            "condensed" => Some(Self::Condensed),
            "semi_condensed" => Some(Self::SemiCondensed),
            "normal" => Some(Self::Normal),
            "semi_expanded" => Some(Self::SemiExpanded),
            "expanded" => Some(Self::Expanded),
            "extra_expanded" => Some(Self::ExtraExpanded),
            "ultra_expanded" => Some(Self::UltraExpanded),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "font_stretch"
    }
}

/// A font specification.
///
/// ## Wire format
///
/// A plain string for shorthand (`"default"`, `"monospace"`, or a family name),
/// or an object with optional keys:
///
/// ```json
/// {"family": "Fira Code", "weight": "bold", "style": "italic", "stretch": "condensed"}
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    /// Font family name. `None` or `"default"` uses the system sans-serif.
    /// `"monospace"` selects the system monospace font.
    pub family: Option<String>,
    /// Font weight (CSS 100-900 equivalent). `None` inherits the default (normal/400).
    pub weight: Option<FontWeight>,
    /// Font style (normal, italic, oblique). `None` inherits the default (normal).
    pub style: Option<FontStyle>,
    /// Font stretch (ultra-condensed through ultra-expanded). `None` inherits the default (normal).
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

impl PlushieType for Font {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => match s.as_str() {
                "default" => Some(Self::new()),
                "monospace" => Some(Self::monospace()),
                family => Some(Self::new().family(family)),
            },
            Value::Object(obj) => {
                let mut font = Self::new();

                if let Some(family) = obj.get("family").and_then(|v| v.as_str()) {
                    font.family = Some(family.to_string());
                }
                if let Some(weight_val) = obj.get("weight") {
                    font.weight = FontWeight::wire_decode(weight_val);
                }
                if let Some(style_val) = obj.get("style") {
                    font.style = FontStyle::wire_decode(style_val);
                }
                if let Some(stretch_val) = obj.get("stretch") {
                    font.stretch = FontStretch::wire_decode(stretch_val);
                }

                Some(font)
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        // Simple cases: if only family is set, encode as string.
        if self.weight.is_none() && self.style.is_none() && self.stretch.is_none() {
            match &self.family {
                None => return PropValue::Str("default".into()),
                Some(f) if f == "monospace" => return PropValue::Str("monospace".into()),
                Some(f) => return PropValue::Str(f.clone()),
            }
        }

        let mut m = PropMap::new();
        if let Some(ref family) = self.family {
            m.insert("family", PropValue::Str(family.clone()));
        }
        if let Some(ref weight) = self.weight {
            m.insert("weight", weight.wire_encode());
        }
        if let Some(ref style) = self.style {
            m.insert("style", style.wire_encode());
        }
        if let Some(ref stretch) = self.stretch {
            m.insert("stretch", stretch.wire_encode());
        }
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "font"
    }
}
