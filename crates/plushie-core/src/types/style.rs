//! Style and StyleMap types for widget visual customization.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::PlushieType;
use super::background::Background;
use super::border::Border;
use super::color::Color;
use super::shadow::Shadow;

/// Style preset name or custom style map.
#[derive(Debug, Clone, PartialEq)]
pub enum Style {
    /// A named preset style (e.g. "primary", "secondary").
    Preset(String),
    /// A fully custom style with explicit properties.
    Custom(StyleMap),
}

impl PlushieType for Style {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => Some(Self::Preset(s.clone())),
            Value::Object(_) => StyleMap::wire_decode(value).map(Self::Custom),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::Preset(name) => PropValue::Str(name.clone()),
            Self::Custom(map) => map.wire_encode(),
        }
    }

    fn type_name() -> &'static str {
        "style"
    }
}

/// Custom style with visual properties and per-status overrides.
///
/// Build fluently:
/// ```
/// use plushie_core::types::{Style, StyleMap, Color, Border};
///
/// let style = StyleMap::new()
///     .background(Color::hex("#3498db"))
///     .text_color(Color::white())
///     .border(Border::new().radius(8.0))
///     .hovered(|s| s.background(Color::hex("#2980b9")));
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleMap {
    pub base: Option<String>,
    pub background: Option<Background>,
    pub text_color: Option<Color>,
    pub border: Option<Border>,
    pub shadow: Option<Shadow>,
    pub hovered: Option<Box<StyleMap>>,
    pub pressed: Option<Box<StyleMap>>,
    pub disabled: Option<Box<StyleMap>>,
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

impl Style {
    pub fn primary() -> Self {
        Self::Preset("primary".into())
    }
    pub fn secondary() -> Self {
        Self::Preset("secondary".into())
    }
    pub fn success() -> Self {
        Self::Preset("success".into())
    }
    pub fn danger() -> Self {
        Self::Preset("danger".into())
    }
    pub fn warning() -> Self {
        Self::Preset("warning".into())
    }
    pub fn text() -> Self {
        Self::Preset("text".into())
    }
    /// Default widget appearance (renderer preset name `"default"`).
    pub fn default_style() -> Self {
        Self::Preset("default".into())
    }
    /// Dark variant.
    pub fn dark() -> Self {
        Self::Preset("dark".into())
    }
    /// Subdued/weak appearance.
    pub fn weak() -> Self {
        Self::Preset("weak".into())
    }
    /// Container with rounded border.
    pub fn rounded_box() -> Self {
        Self::Preset("rounded_box".into())
    }
    /// Container with square border.
    pub fn bordered_box() -> Self {
        Self::Preset("bordered_box".into())
    }
    /// Fully transparent background.
    pub fn transparent() -> Self {
        Self::Preset("transparent".into())
    }
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

impl PlushieType for StyleMap {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;

        let base = obj.get("base").and_then(|v| v.as_str()).map(str::to_owned);
        let background = obj.get("background").and_then(Background::wire_decode);
        let text_color = obj.get("text_color").and_then(Color::wire_decode);
        let border = obj.get("border").and_then(Border::wire_decode);
        let shadow = obj.get("shadow").and_then(Shadow::wire_decode);
        let hovered = obj
            .get("hovered")
            .and_then(StyleMap::wire_decode)
            .map(Box::new);
        let pressed = obj
            .get("pressed")
            .and_then(StyleMap::wire_decode)
            .map(Box::new);
        let disabled = obj
            .get("disabled")
            .and_then(StyleMap::wire_decode)
            .map(Box::new);
        let focused = obj
            .get("focused")
            .and_then(StyleMap::wire_decode)
            .map(Box::new);

        Some(Self {
            base,
            background,
            text_color,
            border,
            shadow,
            hovered,
            pressed,
            disabled,
            focused,
        })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        if let Some(ref base) = self.base {
            m.insert("base", PropValue::Str(base.clone()));
        }
        if let Some(ref bg) = self.background {
            m.insert("background", bg.wire_encode());
        }
        if let Some(ref tc) = self.text_color {
            m.insert("text_color", tc.wire_encode());
        }
        if let Some(ref border) = self.border {
            m.insert("border", border.wire_encode());
        }
        if let Some(ref shadow) = self.shadow {
            m.insert("shadow", shadow.wire_encode());
        }
        if let Some(ref hovered) = self.hovered {
            m.insert("hovered", hovered.wire_encode());
        }
        if let Some(ref pressed) = self.pressed {
            m.insert("pressed", pressed.wire_encode());
        }
        if let Some(ref disabled) = self.disabled {
            m.insert("disabled", disabled.wire_encode());
        }
        if let Some(ref focused) = self.focused {
            m.insert("focused", focused.wire_encode());
        }
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "style_map"
    }
}
