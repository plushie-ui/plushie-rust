//! Tests for property types: Color, Length, Padding, Style, etc.

use plushie::types::*;

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

#[test]
fn color_from_hex_preserves_value() {
    let c = Color::hex("#3498db");
    assert_eq!(c.as_hex(), "#3498db");
}

#[test]
fn color_from_rgb_produces_hex() {
    let c = Color::rgb(1.0, 0.0, 0.0);
    assert_eq!(c.as_hex(), "#ff0000");
}

#[test]
fn color_from_rgba_includes_alpha() {
    let c = Color::rgba(1.0, 1.0, 1.0, 0.5);
    assert_eq!(c.as_hex(), "#ffffff7f");
}

#[test]
fn color_rgb_clamps_out_of_range() {
    let c = Color::rgb(2.0, -1.0, 0.5);
    assert_eq!(c.as_hex(), "#ff007f");
}

#[test]
fn color_named_constructors() {
    assert_eq!(Color::red().as_hex(), "#ff0000");
    assert_eq!(Color::blue().as_hex(), "#0000ff");
    assert_eq!(Color::white().as_hex(), "#ffffff");
    assert_eq!(Color::black().as_hex(), "#000000");
    assert_eq!(Color::transparent().as_hex(), "#00000000");
}

#[test]
fn color_from_str() {
    let c: Color = "#abcdef".into();
    assert_eq!(c.as_hex(), "#abcdef");
}

// ---------------------------------------------------------------------------
// Length
// ---------------------------------------------------------------------------

#[test]
fn length_from_f32_is_fixed() {
    let l: Length = 200.0.into();
    assert_eq!(l, Length::Fixed(200.0));
}

#[test]
fn length_from_i32_is_fixed() {
    let l: Length = 100i32.into();
    assert_eq!(l, Length::Fixed(100.0));
}

#[test]
fn length_fill_and_shrink_are_distinct() {
    assert_ne!(Length::Fill, Length::Shrink);
}

// ---------------------------------------------------------------------------
// Padding
// ---------------------------------------------------------------------------

#[test]
fn padding_uniform_from_f32() {
    let p: Padding = 16.0.into();
    assert_eq!(p, Padding::new(16.0, 16.0, 16.0, 16.0));
}

#[test]
fn padding_axes_from_tuple() {
    let p: Padding = (16.0, 8.0).into();
    assert_eq!(p, Padding::new(16.0, 8.0, 16.0, 8.0));
}

#[test]
fn padding_four_sides_from_tuple() {
    let p: Padding = (1.0, 2.0, 3.0, 4.0).into();
    assert_eq!(p, Padding::new(1.0, 2.0, 3.0, 4.0));
}

#[test]
fn padding_all_is_uniform() {
    assert_eq!(Padding::all(10.0), Padding::new(10.0, 10.0, 10.0, 10.0));
}

// ---------------------------------------------------------------------------
// Border
// ---------------------------------------------------------------------------

#[test]
fn border_fluent_builder() {
    let b = Border::new()
        .color(Color::red())
        .width(2.0)
        .radius(8.0);

    assert_eq!(b.color, Some(Color::red()));
    assert_eq!(b.width, 2.0);
    assert_eq!(b.radius, 8.0);
}

#[test]
fn border_default_is_invisible() {
    let b = Border::default();
    assert!(b.color.is_none());
    assert_eq!(b.width, 0.0);
    assert_eq!(b.radius, 0.0);
}

// ---------------------------------------------------------------------------
// Shadow
// ---------------------------------------------------------------------------

#[test]
fn shadow_fluent_builder() {
    let s = Shadow::new()
        .color(Color::hex("#00000040"))
        .offset(2.0, 4.0)
        .blur_radius(8.0);

    assert_eq!(s.offset_x, 2.0);
    assert_eq!(s.offset_y, 4.0);
    assert_eq!(s.blur_radius, 8.0);
}

// ---------------------------------------------------------------------------
// Style
// ---------------------------------------------------------------------------

#[test]
fn style_preset_from_str() {
    let s: Style = "primary".into();
    assert!(matches!(s, Style::Preset(name) if name == "primary"));
}

#[test]
fn style_named_constructors() {
    assert!(matches!(Style::primary(), Style::Preset(n) if n == "primary"));
    assert!(matches!(Style::danger(), Style::Preset(n) if n == "danger"));
}

#[test]
fn style_custom_builder() {
    let s: Style = Style::custom()
        .background(Color::red())
        .text_color(Color::white())
        .into();

    match s {
        Style::Custom(map) => {
            assert_eq!(map.background, Some(Color::red()));
            assert_eq!(map.text_color, Some(Color::white()));
        }
        _ => panic!("expected Custom"),
    }
}

// ---------------------------------------------------------------------------
// StyleMap
// ---------------------------------------------------------------------------

#[test]
fn style_map_hovered_override() {
    let m = StyleMap::new()
        .background(Color::blue())
        .hovered(|s| s.background(Color::hex("#0000cc")));

    assert!(m.hovered.is_some());
    let hover = m.hovered.unwrap();
    assert_eq!(hover.background, Some(Color::hex("#0000cc")));
}

#[test]
fn style_map_base_preset() {
    let m = StyleMap::new().base("primary").text_color(Color::white());
    assert_eq!(m.base, Some("primary".to_string()));
    assert_eq!(m.text_color, Some(Color::white()));
}

// ---------------------------------------------------------------------------
// Font
// ---------------------------------------------------------------------------

#[test]
fn font_monospace_shorthand() {
    let f = Font::monospace();
    assert_eq!(f.family, Some("monospace".to_string()));
}

#[test]
fn font_fluent_builder() {
    let f = Font::new()
        .family("Inter")
        .weight(FontWeight::Bold)
        .style(FontStyle::Italic);

    assert_eq!(f.family, Some("Inter".to_string()));
    assert_eq!(f.weight, Some(FontWeight::Bold));
    assert_eq!(f.style, Some(FontStyle::Italic));
}

// ---------------------------------------------------------------------------
// Gradient
// ---------------------------------------------------------------------------

#[test]
fn gradient_linear_with_stops() {
    let g = Gradient::linear(45.0, vec![
        (0.0, Color::red()),
        (1.0, Color::blue()),
    ]);

    assert_eq!(g.angle, 45.0);
    assert_eq!(g.stops.len(), 2);
    assert_eq!(g.stops[0].offset, 0.0);
    assert_eq!(g.stops[1].color, Color::blue());
}

// ---------------------------------------------------------------------------
// KeyModifiers
// ---------------------------------------------------------------------------

#[test]
fn key_modifiers_default_is_all_false() {
    let m = KeyModifiers::default();
    assert!(!m.ctrl);
    assert!(!m.shift);
    assert!(!m.alt);
    assert!(!m.logo);
    assert!(!m.command);
}
