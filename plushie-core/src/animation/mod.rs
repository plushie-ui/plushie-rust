//! Declarative animation descriptor types.
//!
//! These types describe animations that the renderer executes.
//! They are set as widget prop values and detected by the renderer's
//! animation system via the `"type"` field.
//!
//! Three animation types:
//! - [`Transition`]: Timed interpolation with easing curves.
//! - [`Spring`]: Physics-based spring animation.
//! - [`Sequence`]: Chain multiple animation steps.

mod easing;

pub use easing::Easing;

use crate::protocol::{PropMap, PropValue};
use crate::types::PlushieType;

use serde_json::Value;

// ---------------------------------------------------------------------------
// Transition
// ---------------------------------------------------------------------------

/// A timed interpolation between values.
///
/// The descriptor is set as a widget prop value. The renderer detects
/// it by the `"type": "transition"` field and interpolates from the
/// current (or `from`) value to `to` over `duration` milliseconds.
///
/// The type parameter `T` determines what kind of value is animated.
/// It defaults to `PropValue` for untyped use (e.g. renderer-side
/// decoding), but SDK builders use concrete types like `f32` or
/// `Color` for type safety.
///
/// ```
/// use plushie_core::animation::{Transition, Easing};
///
/// let t = Transition::new(500, 24.0_f32)
///     .easing(Easing::EaseOutCubic)
///     .delay(100);
/// ```
#[derive(Debug, Clone)]
pub struct Transition<T: PlushieType = PropValue> {
    /// Target value to animate towards.
    pub to: T,
    /// Animation duration in milliseconds.
    pub duration: u64,
    /// Easing function controlling the acceleration curve.
    pub easing: Easing,
    /// Delay before the animation starts, in milliseconds.
    pub delay: u64,
    /// Starting value. If None, animates from the current value.
    pub from: Option<T>,
    /// Number of times to repeat, or forever.
    pub repeat: Option<Repeat>,
    /// Whether to reverse direction on each repeat cycle.
    pub auto_reverse: bool,
    /// Event tag emitted when the animation finishes.
    pub on_complete: Option<String>,
}

impl<T: PlushieType> Transition<T> {
    /// Create a transition with the given duration and target value.
    pub fn new(duration_ms: u64, to: impl Into<T>) -> Self {
        Self {
            to: to.into(),
            duration: duration_ms,
            easing: Easing::EaseInOut,
            delay: 0,
            from: None,
            repeat: None,
            auto_reverse: false,
            on_complete: None,
        }
    }

    /// Set the target value.
    pub fn to(mut self, v: impl Into<T>) -> Self {
        self.to = v.into();
        self
    }
    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }
    pub fn delay(mut self, ms: u64) -> Self {
        self.delay = ms;
        self
    }
    pub fn from(mut self, v: impl Into<T>) -> Self {
        self.from = Some(v.into());
        self
    }
    pub fn repeat(mut self, n: u32) -> Self {
        self.repeat = Some(Repeat::Times(n));
        self
    }
    pub fn repeat_forever(mut self) -> Self {
        self.repeat = Some(Repeat::Forever);
        self
    }
    pub fn auto_reverse(mut self, v: bool) -> Self {
        self.auto_reverse = v;
        self
    }
    pub fn on_complete(mut self, tag: &str) -> Self {
        self.on_complete = Some(tag.into());
        self
    }

    /// Create a looping transition (repeat forever, auto-reverse).
    pub fn looping(duration_ms: u64, to: impl Into<T>) -> Self {
        Self::new(duration_ms, to)
            .repeat_forever()
            .auto_reverse(true)
    }
}

impl<T: PlushieType> PlushieType for Transition<T> {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        if obj.get("type")?.as_str()? != "transition" {
            return None;
        }
        let to_val = obj.get("to")?;
        let to = T::wire_decode(to_val)?;
        let duration = obj.get("duration")?.as_u64()?;
        let easing = obj
            .get("easing")
            .and_then(Easing::wire_decode)
            .unwrap_or(Easing::EaseInOut);
        let delay = obj.get("delay").and_then(|v| v.as_u64()).unwrap_or(0);
        let from = obj.get("from").and_then(T::wire_decode);
        let repeat = obj.get("repeat").and_then(|v| {
            let n = v.as_i64()?;
            if n < 0 {
                Some(Repeat::Forever)
            } else {
                Some(Repeat::Times(n as u32))
            }
        });
        let auto_reverse = obj
            .get("auto_reverse")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let on_complete = obj
            .get("on_complete")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Some(Self {
            to,
            duration,
            easing,
            delay,
            from,
            repeat,
            auto_reverse,
            on_complete,
        })
    }

    fn wire_encode(&self) -> PropValue {
        let mut map = PropMap::with_capacity(8);
        map.insert("type", PropValue::Str("transition".to_string()));
        map.insert("to", self.to.wire_encode());
        map.insert("duration", PropValue::U64(self.duration));
        map.insert("easing", self.easing.wire_encode());
        if self.delay > 0 {
            map.insert("delay", PropValue::U64(self.delay));
        }
        if let Some(ref from) = self.from {
            map.insert("from", from.wire_encode());
        }
        if let Some(ref repeat) = self.repeat {
            let wire_val: i64 = match repeat {
                Repeat::Forever => -1,
                Repeat::Times(n) => *n as i64,
            };
            map.insert("repeat", PropValue::I64(wire_val));
        }
        if self.auto_reverse {
            map.insert("auto_reverse", PropValue::Bool(true));
        }
        if let Some(ref tag) = self.on_complete {
            map.insert("on_complete", PropValue::Str(tag.clone()));
        }
        PropValue::Object(map)
    }

    fn type_name() -> &'static str {
        "transition"
    }
}

/// How many times to repeat an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    Times(u32),
    Forever,
}

// ---------------------------------------------------------------------------
// Spring
// ---------------------------------------------------------------------------

/// A spring physics animation.
///
/// The descriptor is set as a widget prop value. The renderer detects
/// it by the `"type": "spring"` field and simulates a damped spring
/// from the current (or `from`) value to `to`.
///
/// ```
/// use plushie_core::animation::Spring;
///
/// let s = Spring::new(1.05_f32).stiffness(200.0).damping(20.0);
/// let bouncy = Spring::bouncy(1.05_f32);
/// ```
#[derive(Debug, Clone)]
pub struct Spring<T: PlushieType = PropValue> {
    /// Target value the spring settles towards.
    pub to: T,
    /// Spring constant. Higher values produce faster motion. Default: 100.
    pub stiffness: f64,
    /// Resistance force. Higher values reduce oscillation. Default: 10.
    pub damping: f64,
    /// Simulated mass. Higher values produce slower, heavier motion. Default: 1.0.
    pub mass: f64,
    /// Initial velocity. Default: 0.0.
    pub velocity: f64,
    /// Starting value. If None, starts from the current value.
    pub from: Option<T>,
    /// Event tag emitted when the spring settles.
    pub on_complete: Option<String>,
}

impl<T: PlushieType> Spring<T> {
    /// Create a spring targeting the given value.
    pub fn new(to: impl Into<T>) -> Self {
        Self {
            to: to.into(),
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
            velocity: 0.0,
            from: None,
            on_complete: None,
        }
    }

    /// Set the target value.
    pub fn to(mut self, v: impl Into<T>) -> Self {
        self.to = v.into();
        self
    }
    pub fn stiffness(mut self, s: f64) -> Self {
        self.stiffness = s;
        self
    }
    pub fn damping(mut self, d: f64) -> Self {
        self.damping = d;
        self
    }
    pub fn mass(mut self, m: f64) -> Self {
        self.mass = m;
        self
    }
    pub fn velocity(mut self, v: f64) -> Self {
        self.velocity = v;
        self
    }
    pub fn from(mut self, v: impl Into<T>) -> Self {
        self.from = Some(v.into());
        self
    }
    pub fn on_complete(mut self, tag: &str) -> Self {
        self.on_complete = Some(tag.into());
        self
    }

    // Named presets matching the Elixir SDK.

    /// Slow, smooth, no overshoot.
    pub fn gentle(to: impl Into<T>) -> Self {
        Self::new(to).stiffness(120.0).damping(14.0)
    }
    /// Quick with visible overshoot.
    pub fn bouncy(to: impl Into<T>) -> Self {
        Self::new(to).stiffness(300.0).damping(10.0)
    }
    /// Very quick, crisp stop.
    pub fn stiff(to: impl Into<T>) -> Self {
        Self::new(to).stiffness(400.0).damping(30.0)
    }
    /// Quick, minimal overshoot.
    pub fn snappy(to: impl Into<T>) -> Self {
        Self::new(to).stiffness(200.0).damping(20.0)
    }
    /// Slow, heavy, deliberate.
    pub fn molasses(to: impl Into<T>) -> Self {
        Self::new(to).stiffness(60.0).damping(12.0)
    }
}

impl<T: PlushieType> PlushieType for Spring<T> {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        if obj.get("type")?.as_str()? != "spring" {
            return None;
        }
        let to_val = obj.get("to")?;
        let to = T::wire_decode(to_val)?;
        let stiffness = obj.get("stiffness")?.as_f64()?;
        let damping = obj.get("damping")?.as_f64()?;
        let mass = obj.get("mass").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let velocity = obj.get("velocity").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let from = obj.get("from").and_then(T::wire_decode);
        let on_complete = obj
            .get("on_complete")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Some(Self {
            to,
            stiffness,
            damping,
            mass,
            velocity,
            from,
            on_complete,
        })
    }

    fn wire_encode(&self) -> PropValue {
        let mut map = PropMap::with_capacity(7);
        map.insert("type", PropValue::Str("spring".to_string()));
        map.insert("to", self.to.wire_encode());
        map.insert("stiffness", PropValue::F64(self.stiffness));
        map.insert("damping", PropValue::F64(self.damping));
        if (self.mass - 1.0).abs() > f64::EPSILON {
            map.insert("mass", PropValue::F64(self.mass));
        }
        if self.velocity.abs() > f64::EPSILON {
            map.insert("velocity", PropValue::F64(self.velocity));
        }
        if let Some(ref from) = self.from {
            map.insert("from", from.wire_encode());
        }
        if let Some(ref tag) = self.on_complete {
            map.insert("on_complete", PropValue::Str(tag.clone()));
        }
        PropValue::Object(map)
    }

    fn type_name() -> &'static str {
        "spring"
    }
}

// ---------------------------------------------------------------------------
// Sequence
// ---------------------------------------------------------------------------

/// A chain of animation steps executed in order.
///
/// The descriptor is set as a widget prop value. The renderer detects
/// it by the `"type": "sequence"` field and runs each step in turn.
#[derive(Debug, Clone)]
pub struct Sequence<T: PlushieType = PropValue> {
    /// Ordered list of animation steps to execute.
    pub steps: Vec<AnimationStep<T>>,
    /// Event tag emitted when all steps finish.
    pub on_complete: Option<String>,
}

/// A single step in a sequence.
#[derive(Debug, Clone)]
pub enum AnimationStep<T: PlushieType = PropValue> {
    Transition(Transition<T>),
    Spring(Spring<T>),
}

impl<T: PlushieType> PlushieType for AnimationStep<T> {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let step_type = obj.get("type")?.as_str()?;
        match step_type {
            "transition" => Transition::wire_decode(value).map(AnimationStep::Transition),
            "spring" => Spring::wire_decode(value).map(AnimationStep::Spring),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            AnimationStep::Transition(t) => t.wire_encode(),
            AnimationStep::Spring(s) => s.wire_encode(),
        }
    }

    fn type_name() -> &'static str {
        "animation_step"
    }
}

impl<T: PlushieType> Sequence<T> {
    pub fn new(steps: Vec<AnimationStep<T>>) -> Self {
        Self {
            steps,
            on_complete: None,
        }
    }

    pub fn on_complete(mut self, tag: &str) -> Self {
        self.on_complete = Some(tag.into());
        self
    }
}

impl<T: PlushieType> PlushieType for Sequence<T> {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        if obj.get("type")?.as_str()? != "sequence" {
            return None;
        }
        let steps_arr = obj.get("steps")?.as_array()?;
        let steps: Vec<AnimationStep<T>> = steps_arr
            .iter()
            .filter_map(AnimationStep::wire_decode)
            .collect();
        let on_complete = obj
            .get("on_complete")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Some(Self { steps, on_complete })
    }

    fn wire_encode(&self) -> PropValue {
        let mut map = PropMap::with_capacity(3);
        map.insert("type", PropValue::Str("sequence".to_string()));
        let steps: Vec<PropValue> = self.steps.iter().map(|s| s.wire_encode()).collect();
        map.insert("steps", PropValue::Array(steps));
        if let Some(ref tag) = self.on_complete {
            map.insert("on_complete", PropValue::Str(tag.clone()));
        }
        PropValue::Object(map)
    }

    fn type_name() -> &'static str {
        "sequence"
    }
}

impl<T: PlushieType> From<Transition<T>> for AnimationStep<T> {
    fn from(t: Transition<T>) -> Self {
        AnimationStep::Transition(t)
    }
}

impl<T: PlushieType> From<Spring<T>> for AnimationStep<T> {
    fn from(s: Spring<T>) -> Self {
        AnimationStep::Spring(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_preset_values_match_elixir_sdk() {
        let g: Spring<f64> = Spring::gentle(1.0);
        assert_eq!(g.stiffness, 120.0);
        assert_eq!(g.damping, 14.0);

        let b: Spring<f64> = Spring::bouncy(1.0);
        assert_eq!(b.stiffness, 300.0);
        assert_eq!(b.damping, 10.0);

        let st: Spring<f64> = Spring::stiff(1.0);
        assert_eq!(st.stiffness, 400.0);
        assert_eq!(st.damping, 30.0);

        let sn: Spring<f64> = Spring::snappy(1.0);
        assert_eq!(sn.stiffness, 200.0);
        assert_eq!(sn.damping, 20.0);

        let m: Spring<f64> = Spring::molasses(1.0);
        assert_eq!(m.stiffness, 60.0);
        assert_eq!(m.damping, 12.0);
    }

    #[test]
    fn transition_encodes_as_descriptor() {
        let t: Transition<f64> = Transition::new(300, 24.0).easing(Easing::EaseOut);
        let encoded = t.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["type"], "transition");
        assert_eq!(json["to"], 24.0);
        assert_eq!(json["duration"], 300);
        assert_eq!(json["easing"], "ease_out");
    }

    #[test]
    fn transition_repeat_encodes_as_integer() {
        let t: Transition<f64> = Transition::new(300, 1.0).repeat(3);
        let encoded = t.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["repeat"], 3);

        let t: Transition<f64> = Transition::looping(300, 1.0);
        let encoded = t.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["repeat"], -1);
        assert_eq!(json["auto_reverse"], true);
    }

    #[test]
    fn spring_encodes_as_descriptor() {
        let s: Spring<f64> = Spring::bouncy(1.05);
        let encoded = s.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["type"], "spring");
        assert_eq!(json["to"], 1.05);
        assert_eq!(json["stiffness"], 300.0);
    }

    #[test]
    fn sequence_encodes_as_descriptor() {
        let seq: Sequence<f64> = Sequence::new(vec![
            Transition::new(200, 1.0).into(),
            Spring::new(0.0).stiffness(200.0).into(),
        ]);
        let encoded = seq.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["type"], "sequence");
        let steps = json["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["type"], "transition");
        assert_eq!(steps[1]["type"], "spring");
    }

    #[test]
    fn transition_round_trips() {
        let orig: Transition<f64> = Transition::new(500, 24.0)
            .easing(Easing::EaseOutCubic)
            .delay(100)
            .from(0.0)
            .repeat(3)
            .auto_reverse(true)
            .on_complete("done");
        let json = serde_json::Value::from(orig.wire_encode());
        let decoded = Transition::<f64>::wire_decode(&json).unwrap();
        assert_eq!(decoded.duration, 500);
        assert_eq!(decoded.easing, Easing::EaseOutCubic);
        assert_eq!(decoded.delay, 100);
        assert!(decoded.from.is_some());
        assert_eq!(decoded.repeat, Some(Repeat::Times(3)));
        assert!(decoded.auto_reverse);
        assert_eq!(decoded.on_complete.as_deref(), Some("done"));
    }

    #[test]
    fn spring_round_trips() {
        let orig: Spring<f64> = Spring::new(1.05)
            .stiffness(200.0)
            .damping(20.0)
            .mass(2.0)
            .velocity(0.5)
            .from(0.0)
            .on_complete("bounce_done");
        let json = serde_json::Value::from(orig.wire_encode());
        let decoded = Spring::<f64>::wire_decode(&json).unwrap();
        assert_eq!(decoded.stiffness, 200.0);
        assert_eq!(decoded.damping, 20.0);
        assert_eq!(decoded.mass, 2.0);
        assert_eq!(decoded.velocity, 0.5);
        assert!(decoded.from.is_some());
        assert_eq!(decoded.on_complete.as_deref(), Some("bounce_done"));
    }

    #[test]
    fn sequence_round_trips() {
        let orig: Sequence<f64> = Sequence::new(vec![
            Transition::new(200, 1.0).into(),
            Spring::new(0.0).stiffness(200.0).into(),
        ])
        .on_complete("seq_done");
        let json = serde_json::Value::from(orig.wire_encode());
        let decoded = Sequence::<f64>::wire_decode(&json).unwrap();
        assert_eq!(decoded.steps.len(), 2);
        assert!(matches!(decoded.steps[0], AnimationStep::Transition(_)));
        assert!(matches!(decoded.steps[1], AnimationStep::Spring(_)));
        assert_eq!(decoded.on_complete.as_deref(), Some("seq_done"));
    }

    #[test]
    fn typed_transition_f32() {
        let t: Transition<f32> = Transition::new(300, 24.0_f32);
        let encoded = t.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["type"], "transition");
        // f32 encodes as F64 via PlushieType, so wire value is f64
        assert_eq!(json["to"], 24.0);
    }

    #[test]
    fn default_propvalue_transition_still_works() {
        let t: Transition<PropValue> = Transition::new(300, PropValue::F64(42.0));
        let encoded = t.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["to"], 42.0);
    }

    #[test]
    fn color_transition_round_trips() {
        use crate::types::Color;
        let t = Transition::<Color>::new(300, Color::hex("#ff0000"))
            .from(Color::hex("#0000ff"))
            .easing(Easing::EaseOut);
        let encoded = t.wire_encode();
        let decoded = Transition::<Color>::wire_decode(&serde_json::Value::from(encoded)).unwrap();
        assert_eq!(decoded.to.as_hex(), "#ff0000");
        assert_eq!(decoded.from.unwrap().as_hex(), "#0000ff");
        assert_eq!(decoded.duration, 300);
    }

    #[test]
    fn color_spring_round_trips() {
        use crate::types::Color;
        let s = Spring::<Color>::new(Color::hex("#00ff00"))
            .stiffness(200.0)
            .damping(15.0);
        let encoded = s.wire_encode();
        let decoded = Spring::<Color>::wire_decode(&serde_json::Value::from(encoded)).unwrap();
        assert_eq!(decoded.to.as_hex(), "#00ff00");
        assert_eq!(decoded.stiffness, 200.0);
    }
}
