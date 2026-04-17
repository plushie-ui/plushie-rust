//! Theme specification type.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::{Color, PlushieType};

/// Theme specification for the application or a widget subtree.
///
/// ## Wire format
///
/// - String `"system"` for OS-detected theme
/// - String `"dark"`, `"light"`, etc. for built-in themes
/// - JSON object for custom themes with palette colors and shade overrides
///
/// ## Custom themes
///
/// Custom themes start from a base built-in theme and override specific
/// colors. The `base` field selects the starting palette (default: "dark").
///
/// ```
/// use plushie_core::types::Theme;
///
/// let theme = Theme::custom("my-theme")
///     .base("dark")
///     .background("#1a1a2e")
///     .primary("#0f3460")
///     .primary_strong("#1a5276")
///     .background_weakest("#0d0d1a");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    /// A named built-in theme (e.g., "dark", "light", "dracula").
    Named(String),
    /// System theme (follows OS setting).
    System,
    /// Custom theme with palette overrides.
    Custom(CustomTheme),
}

/// A custom theme with seed colors and optional shade overrides.
///
/// Built via [`Theme::custom()`] and the builder methods. The
/// `base` field selects which built-in theme to start from
/// (default: "dark"). Seed colors (background, text, primary,
/// success, warning, danger) set the foundation. Shade keys
/// provide fine-grained control over the extended palette.
///
/// On the wire, encodes as a JSON object with all specified fields.
/// The renderer resolves unknown or missing fields from the base
/// theme's generated palette.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomTheme {
    /// Display name for the theme.
    pub name: String,
    /// Built-in theme to start from (e.g., "dark", "light").
    pub base: Option<String>,
    /// Color overrides: seed colors ("background", "text", "primary",
    /// "success", "warning", "danger") and shade keys
    /// ("primary_strong", "background_weakest", etc.).
    ///
    /// Values are typed [`Color`]s so storage is always validated hex.
    /// Constructed via [`Theme::color`] (which parses via [`Color::hex`])
    /// or via wire decoding (which parses strictly).
    pub colors: BTreeMap<String, Color>,
}

/// The 22 built-in theme names supported by the renderer.
///
/// Useful for theme picker UIs and validation.
pub const BUILTIN_THEMES: &[&str] = &[
    "light",
    "dark",
    "dracula",
    "nord",
    "solarized_light",
    "solarized_dark",
    "gruvbox_light",
    "gruvbox_dark",
    "catppuccin_latte",
    "catppuccin_frappe",
    "catppuccin_macchiato",
    "catppuccin_mocha",
    "tokyo_night",
    "tokyo_night_storm",
    "tokyo_night_light",
    "kanagawa_wave",
    "kanagawa_dragon",
    "kanagawa_lotus",
    "moonfly",
    "nightfly",
    "oxocarbon",
    "ferra",
];

impl Theme {
    /// The 22 built-in theme names supported by the renderer.
    pub fn builtin_names() -> &'static [&'static str] {
        BUILTIN_THEMES
    }

    /// Create a custom theme with the given display name.
    ///
    /// Use builder methods to set colors and shade overrides.
    pub fn custom(name: &str) -> Self {
        Self::Custom(CustomTheme {
            name: name.to_string(),
            base: None,
            colors: BTreeMap::new(),
        })
    }

    // -- Builder methods (delegate to Custom variant) --

    /// Set the base built-in theme to start from.
    pub fn base(mut self, theme: &str) -> Self {
        if let Self::Custom(ref mut c) = self {
            c.base = Some(theme.to_string());
        }
        self
    }

    // Seed colors
    pub fn background(self, hex: &str) -> Self {
        self.color("background", hex)
    }
    pub fn text(self, hex: &str) -> Self {
        self.color("text", hex)
    }
    pub fn primary(self, hex: &str) -> Self {
        self.color("primary", hex)
    }
    pub fn success(self, hex: &str) -> Self {
        self.color("success", hex)
    }
    pub fn warning(self, hex: &str) -> Self {
        self.color("warning", hex)
    }
    pub fn danger(self, hex: &str) -> Self {
        self.color("danger", hex)
    }

    // Primary family shades
    pub fn primary_base(self, hex: &str) -> Self {
        self.color("primary_base", hex)
    }
    pub fn primary_weak(self, hex: &str) -> Self {
        self.color("primary_weak", hex)
    }
    pub fn primary_strong(self, hex: &str) -> Self {
        self.color("primary_strong", hex)
    }
    pub fn primary_base_text(self, hex: &str) -> Self {
        self.color("primary_base_text", hex)
    }
    pub fn primary_weak_text(self, hex: &str) -> Self {
        self.color("primary_weak_text", hex)
    }
    pub fn primary_strong_text(self, hex: &str) -> Self {
        self.color("primary_strong_text", hex)
    }

    // Secondary family shades
    pub fn secondary_base(self, hex: &str) -> Self {
        self.color("secondary_base", hex)
    }
    pub fn secondary_weak(self, hex: &str) -> Self {
        self.color("secondary_weak", hex)
    }
    pub fn secondary_strong(self, hex: &str) -> Self {
        self.color("secondary_strong", hex)
    }
    pub fn secondary_base_text(self, hex: &str) -> Self {
        self.color("secondary_base_text", hex)
    }
    pub fn secondary_weak_text(self, hex: &str) -> Self {
        self.color("secondary_weak_text", hex)
    }
    pub fn secondary_strong_text(self, hex: &str) -> Self {
        self.color("secondary_strong_text", hex)
    }

    // Success family shades
    pub fn success_base(self, hex: &str) -> Self {
        self.color("success_base", hex)
    }
    pub fn success_weak(self, hex: &str) -> Self {
        self.color("success_weak", hex)
    }
    pub fn success_strong(self, hex: &str) -> Self {
        self.color("success_strong", hex)
    }
    pub fn success_base_text(self, hex: &str) -> Self {
        self.color("success_base_text", hex)
    }
    pub fn success_weak_text(self, hex: &str) -> Self {
        self.color("success_weak_text", hex)
    }
    pub fn success_strong_text(self, hex: &str) -> Self {
        self.color("success_strong_text", hex)
    }

    // Warning family shades
    pub fn warning_base(self, hex: &str) -> Self {
        self.color("warning_base", hex)
    }
    pub fn warning_weak(self, hex: &str) -> Self {
        self.color("warning_weak", hex)
    }
    pub fn warning_strong(self, hex: &str) -> Self {
        self.color("warning_strong", hex)
    }
    pub fn warning_base_text(self, hex: &str) -> Self {
        self.color("warning_base_text", hex)
    }
    pub fn warning_weak_text(self, hex: &str) -> Self {
        self.color("warning_weak_text", hex)
    }
    pub fn warning_strong_text(self, hex: &str) -> Self {
        self.color("warning_strong_text", hex)
    }

    // Danger family shades
    pub fn danger_base(self, hex: &str) -> Self {
        self.color("danger_base", hex)
    }
    pub fn danger_weak(self, hex: &str) -> Self {
        self.color("danger_weak", hex)
    }
    pub fn danger_strong(self, hex: &str) -> Self {
        self.color("danger_strong", hex)
    }
    pub fn danger_base_text(self, hex: &str) -> Self {
        self.color("danger_base_text", hex)
    }
    pub fn danger_weak_text(self, hex: &str) -> Self {
        self.color("danger_weak_text", hex)
    }
    pub fn danger_strong_text(self, hex: &str) -> Self {
        self.color("danger_strong_text", hex)
    }

    // Background level shades
    pub fn background_base(self, hex: &str) -> Self {
        self.color("background_base", hex)
    }
    pub fn background_weakest(self, hex: &str) -> Self {
        self.color("background_weakest", hex)
    }
    pub fn background_weaker(self, hex: &str) -> Self {
        self.color("background_weaker", hex)
    }
    pub fn background_weak(self, hex: &str) -> Self {
        self.color("background_weak", hex)
    }
    pub fn background_neutral(self, hex: &str) -> Self {
        self.color("background_neutral", hex)
    }
    pub fn background_strong(self, hex: &str) -> Self {
        self.color("background_strong", hex)
    }
    pub fn background_stronger(self, hex: &str) -> Self {
        self.color("background_stronger", hex)
    }
    pub fn background_strongest(self, hex: &str) -> Self {
        self.color("background_strongest", hex)
    }

    // Background text overrides
    pub fn background_base_text(self, hex: &str) -> Self {
        self.color("background_base_text", hex)
    }
    pub fn background_weakest_text(self, hex: &str) -> Self {
        self.color("background_weakest_text", hex)
    }
    pub fn background_weaker_text(self, hex: &str) -> Self {
        self.color("background_weaker_text", hex)
    }
    pub fn background_weak_text(self, hex: &str) -> Self {
        self.color("background_weak_text", hex)
    }
    pub fn background_neutral_text(self, hex: &str) -> Self {
        self.color("background_neutral_text", hex)
    }
    pub fn background_strong_text(self, hex: &str) -> Self {
        self.color("background_strong_text", hex)
    }
    pub fn background_stronger_text(self, hex: &str) -> Self {
        self.color("background_stronger_text", hex)
    }
    pub fn background_strongest_text(self, hex: &str) -> Self {
        self.color("background_strongest_text", hex)
    }

    /// Set an arbitrary color key by name.
    ///
    /// Used internally by the named builder methods. Can also be
    /// used directly for future shade keys or custom keys without
    /// waiting for a named builder method.
    ///
    /// `hex` is parsed through [`Color::hex`] which accepts short forms
    /// and expands them. The wire decoder is stricter and rejects
    /// short hex.
    pub fn color(mut self, key: &str, hex: &str) -> Self {
        if let Self::Custom(ref mut c) = self {
            c.colors.insert(key.to_string(), Color::hex(hex));
        }
        self
    }
}

impl PlushieType for Theme {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => {
                if s == "system" {
                    Some(Self::System)
                } else {
                    Some(Self::Named(s.to_string()))
                }
            }
            Value::Object(obj) => {
                let name = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Custom")
                    .to_string();
                let base = obj.get("base").and_then(|v| v.as_str()).map(String::from);
                let mut colors = BTreeMap::new();
                for (k, v) in obj {
                    if k == "name" || k == "base" {
                        continue;
                    }
                    // Each color field is validated strictly through
                    // Color::wire_decode. Off-canonical values (short hex,
                    // non-string values) are silently dropped here; higher
                    // layers may emit a diagnostic.
                    if let Some(color) = Color::wire_decode(v) {
                        colors.insert(k.clone(), color);
                    }
                }
                Some(Self::Custom(CustomTheme { name, base, colors }))
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::System => PropValue::Str("system".to_string()),
            Self::Named(name) => PropValue::Str(name.clone()),
            Self::Custom(c) => {
                let mut m = PropMap::new();
                m.insert("name", PropValue::Str(c.name.clone()));
                if let Some(ref base) = c.base {
                    m.insert("base", PropValue::Str(base.clone()));
                }
                for (k, v) in &c.colors {
                    m.insert(k, v.wire_encode());
                }
                PropValue::Object(m)
            }
        }
    }

    fn type_name() -> &'static str {
        "theme"
    }
}

impl From<&str> for Theme {
    fn from(s: &str) -> Self {
        if s == "system" {
            Theme::System
        } else {
            Theme::Named(s.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn named_theme_round_trip() {
        let theme = Theme::Named("dracula".to_string());
        let encoded = theme.wire_encode();
        let decoded = Theme::wire_decode(&json!("dracula")).unwrap();
        assert_eq!(decoded, theme);
        assert_eq!(encoded, PropValue::Str("dracula".into()));
    }

    #[test]
    fn system_theme_round_trip() {
        let decoded = Theme::wire_decode(&json!("system")).unwrap();
        assert_eq!(decoded, Theme::System);
    }

    #[test]
    fn custom_theme_builder() {
        let theme = Theme::custom("my-theme")
            .base("dark")
            .background("#1a1a2e")
            .primary("#0f3460")
            .primary_strong("#1a5276");

        if let Theme::Custom(c) = &theme {
            assert_eq!(c.name, "my-theme");
            assert_eq!(c.base.as_deref(), Some("dark"));
            assert_eq!(c.colors.get("background").unwrap().as_hex(), "#1a1a2e");
            assert_eq!(c.colors.get("primary").unwrap().as_hex(), "#0f3460");
            assert_eq!(c.colors.get("primary_strong").unwrap().as_hex(), "#1a5276");
        } else {
            panic!("expected Custom");
        }
    }

    #[test]
    fn custom_theme_wire_round_trip() {
        let theme = Theme::custom("test")
            .base("light")
            .primary("#ff0000")
            .danger_strong("#cc0000");

        let encoded = theme.wire_encode();
        let json_val = Value::from(encoded);
        let decoded = Theme::wire_decode(&json_val).unwrap();

        if let Theme::Custom(c) = decoded {
            assert_eq!(c.name, "test");
            assert_eq!(c.base.as_deref(), Some("light"));
            assert_eq!(c.colors.get("primary").unwrap().as_hex(), "#ff0000");
            assert_eq!(c.colors.get("danger_strong").unwrap().as_hex(), "#cc0000");
        } else {
            panic!("expected Custom");
        }
    }

    #[test]
    fn custom_theme_decode_from_json_object() {
        let val = json!({
            "name": "custom",
            "base": "dark",
            "background": "#111111",
            "text": "#eeeeee",
            "primary_strong": "#0000ff"
        });
        let theme = Theme::wire_decode(&val).unwrap();
        if let Theme::Custom(c) = theme {
            assert_eq!(c.colors.get("background").unwrap().as_hex(), "#111111");
            assert_eq!(c.colors.get("primary_strong").unwrap().as_hex(), "#0000ff");
        } else {
            panic!("expected Custom");
        }
    }

    #[test]
    fn custom_theme_decode_rejects_short_hex() {
        let val = json!({
            "name": "custom",
            "background": "#f00",
            "text": "#ffffff",
        });
        let theme = Theme::wire_decode(&val).unwrap();
        if let Theme::Custom(c) = theme {
            // Short-hex background is dropped by the strict wire decoder;
            // only the canonical text survives.
            assert!(!c.colors.contains_key("background"));
            assert_eq!(c.colors.get("text").unwrap().as_hex(), "#ffffff");
        } else {
            panic!("expected Custom");
        }
    }

    #[test]
    fn from_str_named() {
        let t: Theme = "dark".into();
        assert_eq!(t, Theme::Named("dark".to_string()));
    }

    #[test]
    fn from_str_system() {
        let t: Theme = "system".into();
        assert_eq!(t, Theme::System);
    }
}
