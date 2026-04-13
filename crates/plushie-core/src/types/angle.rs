//! Angle type for rotation and arc parameters.
//!
//! Stores the original unit (degrees or radians) to avoid precision
//! loss from unnecessary conversions. Wire protocol uses degrees.

use serde_json::Value;

use crate::protocol::PropValue;

use super::PlushieType;

/// An angle value that preserves its original unit.
///
/// Bare numbers are treated as **degrees** (matching the Elixir SDK
/// convention). Use [`Angle::rad`] when you need explicit radians.
///
/// The wire protocol transmits angles in degrees. Conversion to
/// radians happens at the rendering boundary via [`Angle::radians`].
///
/// ```
/// use plushie_core::types::Angle;
///
/// // Bare float = degrees (cross-SDK convention)
/// let a: Angle = 45.0.into();
/// assert_eq!(a.degrees(), 45.0);
///
/// // Explicit constructors
/// let a = Angle::deg(90.0);
/// let a = Angle::rad(std::f32::consts::FRAC_PI_2);
///
/// // Comparison accounts for float imprecision
/// assert_eq!(Angle::deg(180.0), Angle::rad(std::f32::consts::PI));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Angle(AngleRepr);

#[derive(Debug, Clone, Copy)]
enum AngleRepr {
    Degrees(f32),
    Radians(f32),
}

impl Angle {
    /// Create an angle from degrees.
    pub fn deg(degrees: f32) -> Self {
        Self(AngleRepr::Degrees(degrees))
    }

    /// Create an angle from radians.
    pub fn rad(radians: f32) -> Self {
        Self(AngleRepr::Radians(radians))
    }

    /// The angle in degrees.
    ///
    /// Exact when the angle was constructed with [`deg`](Self::deg)
    /// or [`From<f32>`]. One conversion when constructed with
    /// [`rad`](Self::rad). The result is clamped to finite f32 range.
    pub fn degrees(self) -> f32 {
        match self.0 {
            AngleRepr::Degrees(d) => d,
            AngleRepr::Radians(r) => clamp_finite(r.to_degrees()),
        }
    }

    /// The angle in radians.
    ///
    /// Exact when the angle was constructed with [`rad`](Self::rad).
    /// One conversion when constructed with [`deg`](Self::deg) or
    /// [`From<f32>`]. The result is clamped to finite f32 range.
    pub fn radians(self) -> f32 {
        match self.0 {
            AngleRepr::Degrees(d) => clamp_finite(d.to_radians()),
            AngleRepr::Radians(r) => r,
        }
    }

    /// Whether two angles represent the same rotation, accounting
    /// for float imprecision from unit conversion.
    pub fn approx_eq(self, other: Angle) -> bool {
        (self.radians() - other.radians()).abs() < 1e-6
    }
}

/// Bare `f32` is degrees (matching the Elixir SDK convention where
/// bare numbers are degrees).
impl From<f32> for Angle {
    fn from(degrees: f32) -> Self {
        Self::deg(degrees)
    }
}

/// Bare `i32` is degrees.
impl From<i32> for Angle {
    fn from(degrees: i32) -> Self {
        Self::deg(degrees as f32)
    }
}

impl Default for Angle {
    fn default() -> Self {
        Self::deg(0.0)
    }
}

/// Approximate equality: compares in radians with 1e-6 tolerance.
///
/// This means `Angle::deg(180.0) == Angle::rad(PI)` is true,
/// which is the expected behavior for angle comparisons.
///
/// Note: does not implement [`Eq`] because approximate equality
/// is not transitive.
impl PartialEq for Angle {
    fn eq(&self, other: &Self) -> bool {
        self.approx_eq(*other)
    }
}

impl PlushieType for Angle {
    fn wire_decode(value: &Value) -> Option<Self> {
        let v = value.as_f64()?;
        if !v.is_finite() {
            return None;
        }
        // Clamp to f32 range before converting.
        let clamped = v.clamp(f32::MIN as f64, f32::MAX as f64) as f32;
        Some(Angle::deg(clamped))
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::F64(self.degrees() as f64)
    }

    fn type_name() -> &'static str {
        "angle"
    }
}

/// Clamp a conversion result to finite f32 range.
/// Converts infinity/NaN from overflow to the nearest finite value.
fn clamp_finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else if v.is_nan() {
        0.0
    } else {
        v.signum() * f32::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    #[test]
    fn deg_preserves_exact_value() {
        let a = Angle::deg(45.0);
        assert_eq!(a.degrees(), 45.0);
    }

    #[test]
    fn rad_preserves_exact_value() {
        let a = Angle::rad(1.0);
        assert_eq!(a.radians(), 1.0);
    }

    #[test]
    fn from_f32_is_degrees() {
        let a: Angle = 90.0.into();
        assert_eq!(a.degrees(), 90.0);
    }

    #[test]
    fn from_i32_is_degrees() {
        let a: Angle = Angle::from(45i32);
        assert_eq!(a.degrees(), 45.0);
    }

    #[test]
    fn deg_and_rad_compare_equal() {
        assert_eq!(Angle::deg(180.0), Angle::rad(PI));
        assert_eq!(Angle::deg(90.0), Angle::rad(FRAC_PI_2));
        assert_eq!(Angle::deg(45.0), Angle::rad(FRAC_PI_4));
    }

    #[test]
    fn different_angles_compare_not_equal() {
        assert_ne!(Angle::deg(45.0), Angle::deg(90.0));
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Angle::default(), Angle::deg(0.0));
    }

    #[test]
    fn wire_encode_uses_degrees() {
        let a = Angle::rad(PI);
        let encoded = a.wire_encode();
        match encoded {
            PropValue::F64(v) => assert!((v - 180.0).abs() < 0.001),
            _ => panic!("expected F64"),
        }
    }

    #[test]
    fn wire_decode_treats_as_degrees() {
        let a = Angle::wire_decode(&serde_json::json!(90.0)).unwrap();
        assert_eq!(a, Angle::deg(90.0));
    }

    #[test]
    fn wire_decode_rejects_nan() {
        assert!(Angle::wire_decode(&serde_json::json!(f64::NAN)).is_none());
    }

    #[test]
    fn wire_decode_rejects_infinity() {
        assert!(Angle::wire_decode(&serde_json::json!(f64::INFINITY)).is_none());
    }

    #[test]
    fn wire_decode_clamps_large_values() {
        let a = Angle::wire_decode(&serde_json::json!(1e300)).unwrap();
        assert!(a.degrees().is_finite());
    }

    #[test]
    fn extreme_radian_to_degrees_stays_finite() {
        let a = Angle::rad(f32::MAX);
        assert!(a.degrees().is_finite());
    }

    #[test]
    fn negative_angles_work() {
        let a = Angle::deg(-90.0);
        assert_eq!(a.degrees(), -90.0);
        assert_eq!(a, Angle::rad(-FRAC_PI_2));
    }
}
