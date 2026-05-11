//! Timed transition interpolation (duration + easing).

use iced::animation::Easing;

/// Computes the eased progress for a timed transition.
///
/// Returns `(eased_t, finished)` where `eased_t` is in 0.0..1.0+
/// (may exceed 1.0 for overshooting easings like back/elastic).
pub fn progress(
    elapsed_ms: f32,
    duration_ms: f32,
    delay_ms: f32,
    easing: Easing,
    bezier: Option<[f32; 4]>,
) -> (f32, bool) {
    let effective = (elapsed_ms - delay_ms).max(0.0);

    if elapsed_ms < delay_ms {
        return (0.0, false);
    }

    let t = (effective / duration_ms).min(1.0);
    let eased_t = match bezier {
        Some([x1, y1, x2, y2]) => cubic_bezier(t, x1, y1, x2, y2),
        None => easing.value(t),
    };

    (eased_t, t >= 1.0)
}

/// Advances a timed transition for a numeric value.
///
/// Returns `(new_value, finished)`.
pub fn advance(
    from: f32,
    to: f32,
    elapsed_ms: f32,
    duration_ms: f32,
    delay_ms: f32,
    easing: Easing,
    bezier: Option<[f32; 4]>,
) -> (f32, bool) {
    let (eased_t, finished) = progress(elapsed_ms, duration_ms, delay_ms, easing, bezier);

    if finished {
        (to, true)
    } else {
        let value = from + (to - from) * eased_t;
        (value, false)
    }
}

// ---------------------------------------------------------------------------
// Cubic bezier solver
// ---------------------------------------------------------------------------

/// Evaluates a cubic bezier easing curve at progress `t`.
///
/// Control points (x1, y1, x2, y2) match the CSS cubic-bezier() function.
/// Uses Newton-Raphson iteration to solve for the parameter.
fn cubic_bezier(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    // Newton-Raphson: solve bezier_x(s) == t for s, then evaluate bezier_y(s)
    let mut s = t; // initial guess
    for _ in 0..8 {
        let x = bezier_eval(s, x1, x2);
        let dx = bezier_derivative(s, x1, x2);

        if (x - t).abs() < 1.0e-6 || dx.abs() < 1.0e-6 {
            break;
        }

        s -= (x - t) / dx;
        s = s.clamp(0.0, 1.0);
    }

    bezier_eval(s, y1, y2)
}

/// Evaluates the cubic bezier polynomial for one axis.
/// B(s) = 3(1-s)^2*s*p1 + 3(1-s)*s^2*p2 + s^3
fn bezier_eval(s: f32, p1: f32, p2: f32) -> f32 {
    let s2 = s * s;
    let s3 = s2 * s;
    let inv = 1.0 - s;
    3.0 * inv * inv * s * p1 + 3.0 * inv * s2 * p2 + s3
}

/// Derivative of the cubic bezier polynomial for one axis.
fn bezier_derivative(s: f32, p1: f32, p2: f32) -> f32 {
    let inv = 1.0 - s;
    3.0 * inv * inv * p1 + 6.0 * inv * s * (p2 - p1) + 3.0 * s * s * (1.0 - p2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_bezier_clamps_endpoints() {
        assert_eq!(cubic_bezier(0.0, 0.42, 0.0, 0.58, 1.0), 0.0);
        assert_eq!(cubic_bezier(1.0, 0.42, 0.0, 0.58, 1.0), 1.0);
    }

    #[test]
    fn cubic_bezier_linear_curve_tracks_progress() {
        for t in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let eased = cubic_bezier(t, 0.0, 0.0, 1.0, 1.0);
            assert!((eased - t).abs() < 1.0e-5);
        }
    }

    #[test]
    fn cubic_bezier_handles_degenerate_x_curve() {
        let eased = cubic_bezier(0.5, 0.0, 0.25, 0.0, 0.75);

        assert!(eased.is_finite());
        assert!((0.0..=1.0).contains(&eased));
    }
}
