//! Theme resolution: wire JSON into iced [`Theme`] via the typed core.
//!
//! Takes a JSON value produced by a host SDK (either a built-in theme
//! name or a custom palette object), parses it through
//! [`plushie_core::types::Theme::wire_decode`], and converts the typed
//! result into an [`iced::Theme`] via [`iced_convert::theme`].
//!
//! This module is the single wire-parse path. There is no second
//! hex-parser here; all hex validation goes through
//! [`plushie_core::types::Color`]'s strict wire decoder.

use iced::{Color, Theme};
use serde_json::Value;

use plushie_core::types::{PlushieType, Theme as CoreTheme};

use crate::iced_convert;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Resolve a JSON value into an iced [`Theme`].
///
/// Routes through the typed [`CoreTheme`] wire decoder so there is
/// exactly one hex-validation path. Unknown or unparseable values fall
/// back to [`Theme::Dark`].
pub fn resolve_theme(value: &Value) -> Theme {
    resolve_theme_with_chrome(value).0
}

/// Theme-level chrome colors that iced does not store in [`Theme`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ThemeChrome {
    pub cursor_color: Option<Color>,
    pub scrollbar_color: Option<Color>,
    pub scroller_color: Option<Color>,
}

impl ThemeChrome {
    pub fn is_empty(self) -> bool {
        self.cursor_color.is_none()
            && self.scrollbar_color.is_none()
            && self.scroller_color.is_none()
    }
}

/// Resolve a JSON value into an iced [`Theme`] plus renderer chrome tokens.
pub fn resolve_theme_with_chrome(value: &Value) -> (Theme, ThemeChrome) {
    match CoreTheme::wire_decode(value) {
        Some(CoreTheme::System) => (Theme::Dark, ThemeChrome::default()),
        Some(CoreTheme::Named(name)) => (resolve_builtin(&name), ThemeChrome::default()),
        Some(CoreTheme::Custom(c)) => {
            let chrome = ThemeChrome {
                cursor_color: c.colors.get("cursor_color").map(iced_convert::color),
                scrollbar_color: c.colors.get("scrollbar_color").map(iced_convert::color),
                scroller_color: c.colors.get("scroller_color").map(iced_convert::color),
            };
            (iced_convert::custom_theme(&c), chrome)
        }
        None => (Theme::Dark, ThemeChrome::default()),
    }
}

/// Resolve a theme value, returning `None` for `"system"` (follow OS preference).
pub fn resolve_theme_only(value: &Value) -> Option<Theme> {
    resolve_theme_and_chrome_only(value).map(|(theme, _)| theme)
}

/// Resolve a theme value with chrome, returning `None` for `"system"`.
pub fn resolve_theme_and_chrome_only(value: &Value) -> Option<(Theme, ThemeChrome)> {
    if let Some(s) = value.as_str()
        && s.eq_ignore_ascii_case("system")
    {
        return None;
    }
    Some(resolve_theme_with_chrome(value))
}

// ---------------------------------------------------------------------------
// Built-in theme resolution
// ---------------------------------------------------------------------------

/// Map a string name to a built-in iced theme variant.
pub(crate) fn resolve_builtin(s: &str) -> Theme {
    match s.to_ascii_lowercase().as_str() {
        "light" => Theme::Light,
        "dark" => Theme::Dark,
        "dracula" => Theme::Dracula,
        "nord" => Theme::Nord,
        "solarized_light" => Theme::SolarizedLight,
        "solarized_dark" => Theme::SolarizedDark,
        "gruvbox_light" => Theme::GruvboxLight,
        "gruvbox_dark" => Theme::GruvboxDark,
        "catppuccin_latte" => Theme::CatppuccinLatte,
        "catppuccin_frappe" => Theme::CatppuccinFrappe,
        "catppuccin_macchiato" => Theme::CatppuccinMacchiato,
        "catppuccin_mocha" => Theme::CatppuccinMocha,
        "tokyo_night" => Theme::TokyoNight,
        "tokyo_night_storm" => Theme::TokyoNightStorm,
        "tokyo_night_light" => Theme::TokyoNightLight,
        "kanagawa_wave" => Theme::KanagawaWave,
        "kanagawa_dragon" => Theme::KanagawaDragon,
        "kanagawa_lotus" => Theme::KanagawaLotus,
        "moonfly" => Theme::Moonfly,
        "nightfly" => Theme::Nightfly,
        "oxocarbon" => Theme::Oxocarbon,
        "ferra" => Theme::Ferra,
        _ => Theme::Dark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Color;
    use iced::theme::palette;
    use serde_json::json;

    #[test]
    fn resolve_builtin_themes() {
        assert!(matches!(resolve_theme(&json!("Dark")), Theme::Dark));
        assert!(matches!(resolve_theme(&json!("nord")), Theme::Nord));
        assert!(matches!(
            resolve_theme(&json!("CATPPUCCIN_MOCHA")),
            Theme::CatppuccinMocha
        ));
    }

    #[test]
    fn system_theme_returns_none() {
        assert!(resolve_theme_only(&json!("system")).is_none());
        assert!(resolve_theme_only(&json!("System")).is_none());
    }

    #[test]
    fn non_system_returns_some() {
        assert!(resolve_theme_only(&json!("Dark")).is_some());
        assert!(resolve_theme_only(&json!({"primary": "#ff0000"})).is_some());
    }

    #[test]
    fn unknown_string_falls_back_to_dark() {
        assert!(matches!(resolve_theme(&json!("neon_pink")), Theme::Dark));
    }

    #[test]
    fn custom_theme_minimal() {
        let val = json!({"name": "Mine"});
        let result = resolve_theme(&val);
        assert_eq!(format!("{}", result), "Mine");
    }

    #[test]
    fn custom_theme_with_colors() {
        let val = json!({
            "name": "Tokyo Remix",
            "background": "#1a1b26",
            "text": "#c0caf5",
            "primary": "#7aa2f7",
            "success": "#9ece6a",
            "danger": "#f7768e"
        });
        let result = resolve_theme(&val);
        let seed = result.seed();
        assert_eq!(seed.background, Color::from_rgb8(0x1a, 0x1b, 0x26));
        assert_eq!(seed.text, Color::from_rgb8(0xc0, 0xca, 0xf5));
        assert_eq!(seed.primary, Color::from_rgb8(0x7a, 0xa2, 0xf7));
        assert_eq!(seed.success, Color::from_rgb8(0x9e, 0xce, 0x6a));
        assert_eq!(seed.danger, Color::from_rgb8(0xf7, 0x76, 0x8e));
    }

    #[test]
    fn custom_theme_with_warning_color() {
        let val = json!({"warning": "#f9e2af"});
        let result = resolve_theme(&val);
        let seed = result.seed();
        assert_eq!(seed.warning, Color::from_rgb8(0xf9, 0xe2, 0xaf));
    }

    #[test]
    fn custom_theme_with_base() {
        let val = json!({"base": "Nord", "primary": "#88c0d0"});
        let result = resolve_theme(&val);
        let seed = result.seed();
        // Primary should be overridden.
        assert_eq!(seed.primary, Color::from_rgb8(0x88, 0xc0, 0xd0));
        // Background should come from Nord's seed.
        let nord_bg = Theme::Nord.seed().background;
        assert_eq!(seed.background, nord_bg);
    }

    #[test]
    fn custom_theme_defaults_name_to_custom() {
        let val = json!({"primary": "#ff0000"});
        let result = resolve_theme(&val);
        assert_eq!(format!("{}", result), "Custom");
    }

    #[test]
    fn short_hex_is_rejected_silently() {
        // Strict wire decoder rejects short hex. The field is dropped
        // entirely, falling back to the base seed color.
        let val = json!({"background": "#f80", "text": "#ffffff"});
        let result = resolve_theme(&val);
        let seed = result.seed();
        assert_eq!(seed.text, Color::from_rgb8(0xff, 0xff, 0xff));
        assert_eq!(seed.background, palette::Seed::DARK.background);
    }

    #[test]
    fn bad_color_field_is_ignored() {
        let val = json!({"background": "not-a-color", "text": "#ffffff"});
        let result = resolve_theme(&val);
        let seed = result.seed();
        // text should be set, background should remain the dark default.
        assert_eq!(seed.text, Color::from_rgb8(0xff, 0xff, 0xff));
        assert_eq!(seed.background, palette::Seed::DARK.background);
    }

    #[test]
    fn custom_theme_with_shade_override() {
        let val = json!({
            "primary": "#5865f2",
            "primary_strong": "#1a5276"
        });
        let result = resolve_theme(&val);
        let pal = result.palette();
        assert_eq!(pal.primary.strong.color, Color::from_rgb8(0x1a, 0x52, 0x76));
    }

    #[test]
    fn custom_theme_with_text_override() {
        let val = json!({
            "primary": "#5865f2",
            "primary_strong_text": "#ffffff"
        });
        let result = resolve_theme(&val);
        let pal = result.palette();
        assert_eq!(pal.primary.strong.text, Color::from_rgb8(0xff, 0xff, 0xff));
    }

    #[test]
    fn custom_theme_without_shades_uses_standard() {
        // No shade keys: should use Theme::custom (standard generation).
        let val = json!({"primary": "#ff0000"});
        let result = resolve_theme(&val);
        let pal = result.palette();
        // The generated palette should match what Palette::generate
        // produces for the same seed.
        let expected = palette::Palette::generate(result.seed());
        assert_eq!(pal.primary.strong.color, expected.primary.strong.color);
        assert_eq!(pal.primary.weak.color, expected.primary.weak.color);
    }

    #[test]
    fn custom_theme_background_shade_override() {
        let val = json!({
            "background": "#1a1a2e",
            "background_weakest": "#0d0d1a",
            "background_weakest_text": "#aaaaaa"
        });
        let result = resolve_theme(&val);
        let pal = result.palette();
        assert_eq!(
            pal.background.weakest.color,
            Color::from_rgb8(0x0d, 0x0d, 0x1a)
        );
        assert_eq!(
            pal.background.weakest.text,
            Color::from_rgb8(0xaa, 0xaa, 0xaa)
        );
    }

    #[test]
    fn custom_theme_chrome_tokens_are_resolved_outside_iced_theme() {
        let val = json!({
            "cursor_color": "#112233",
            "scrollbar_color": "#445566",
            "scroller_color": "#778899"
        });

        let (_, chrome) = resolve_theme_with_chrome(&val);

        assert_eq!(
            chrome.cursor_color,
            Some(Color::from_rgb8(0x11, 0x22, 0x33))
        );
        assert_eq!(
            chrome.scrollbar_color,
            Some(Color::from_rgb8(0x44, 0x55, 0x66))
        );
        assert_eq!(
            chrome.scroller_color,
            Some(Color::from_rgb8(0x77, 0x88, 0x99))
        );
    }

    #[test]
    fn built_in_theme_has_no_chrome_tokens() {
        let (_, chrome) = resolve_theme_with_chrome(&json!("dark"));

        assert!(chrome.is_empty());
    }
}
