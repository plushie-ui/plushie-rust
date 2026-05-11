//! Color interpolation using Oklch for perceptually uniform transitions.
//!
//! Oklch interpolation produces smoother, more vibrant color transitions
//! than sRGB: no "muddy middle," no desaturation through gray, and
//! linear perceived brightness change.

use iced::Color;
use iced::color::Oklch;

use plushie_core::types::{Color as CoreColor, PlushieType};

use crate::iced_convert;

/// Interpolates between two colors in Oklch color space at progress `t`.
///
/// Uses shortest-hue-arc interpolation: a red-to-blue transition goes
/// through purple (short arc), not through green/yellow (long arc).
///
/// Achromatic handling: when a color has near-zero chroma, its hue is
/// undefined. We use the other color's hue to avoid jumps.
pub fn interpolate(from: Color, to: Color, t: f32) -> Color {
    let from_oklch = from.into_oklch();
    let to_oklch = to.into_oklch();

    let l = lerp(from_oklch.l, to_oklch.l, t);
    let c = lerp(from_oklch.c, to_oklch.c, t);
    let a = lerp(from_oklch.a, to_oklch.a, t);

    // Hue interpolation: shortest arc with achromatic handling
    let h = hue_lerp(from_oklch, to_oklch, t);

    Color::from_oklch(Oklch { l, c, h, a })
}

/// Linear interpolation between two floats.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Shortest-arc hue interpolation with achromatic handling.
///
/// When one color is achromatic (chroma near zero), its hue is undefined.
/// We use the other color's hue to prevent jumps.
fn hue_lerp(from: Oklch, to: Oklch, t: f32) -> f32 {
    const ACHROMATIC_THRESHOLD: f32 = 0.001;
    let pi = std::f32::consts::PI;

    let from_achromatic = from.c < ACHROMATIC_THRESHOLD;
    let to_achromatic = to.c < ACHROMATIC_THRESHOLD;

    if from_achromatic && to_achromatic {
        // Both achromatic: hue doesn't matter
        0.0
    } else if from_achromatic {
        // Use the target's hue
        to.h
    } else if to_achromatic {
        // Use the source's hue
        from.h
    } else {
        // Both chromatic: shortest arc
        let mut diff = to.h - from.h;
        if diff > pi {
            diff -= 2.0 * pi;
        } else if diff < -pi {
            diff += 2.0 * pi;
        }
        from.h + diff * t
    }
}

/// Attempts to parse a JSON value as a color.
pub fn parse_color(value: &serde_json::Value) -> Option<Color> {
    CoreColor::wire_decode(value).map(|c| iced_convert::color(&c))
}

/// Converts a Color back to a hex string for the interpolated props cache.
pub fn color_to_hex(c: Color) -> String {
    let r = (c.r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (c.g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (c.b.clamp(0.0, 1.0) * 255.0).round() as u8;
    if (c.a - 1.0).abs() < f32::EPSILON {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        let a = (c.a.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_color_close(actual: Color, expected: Color) {
        assert!((actual.r - expected.r).abs() < 1.0e-6);
        assert!((actual.g - expected.g).abs() < 1.0e-6);
        assert!((actual.b - expected.b).abs() < 1.0e-6);
        assert!((actual.a - expected.a).abs() < 1.0e-6);
    }

    #[test]
    fn interpolation_returns_endpoints() {
        let from = Color::from_rgba(1.0, 0.0, 0.0, 0.5);
        let to = Color::from_rgba(0.0, 0.0, 1.0, 1.0);

        assert_color_close(interpolate(from, to, 0.0), from);
        assert_color_close(interpolate(from, to, 1.0), to);
    }

    #[test]
    fn interpolation_lerps_alpha() {
        let from = Color::from_rgba(1.0, 0.0, 0.0, 0.25);
        let to = Color::from_rgba(0.0, 0.0, 1.0, 0.75);
        let midpoint = interpolate(from, to, 0.5);

        assert!((midpoint.a - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn achromatic_to_chromatic_interpolation_is_finite() {
        let from = Color::from_rgb(0.5, 0.5, 0.5);
        let to = Color::from_rgb(1.0, 0.0, 0.0);
        let midpoint = interpolate(from, to, 0.5);

        assert!(midpoint.r.is_finite());
        assert!(midpoint.g.is_finite());
        assert!(midpoint.b.is_finite());
        assert!(midpoint.a.is_finite());
    }
}
