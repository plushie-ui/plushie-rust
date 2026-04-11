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

use serde::Serialize;

// ---------------------------------------------------------------------------
// Transition
// ---------------------------------------------------------------------------

/// A timed interpolation between values.
///
/// The descriptor is set as a widget prop value. The renderer detects
/// it by the `"type": "transition"` field and interpolates from the
/// current (or `from`) value to `to` over `duration` milliseconds.
///
/// ```
/// use plushie_core::animation::{Transition, Easing};
///
/// let t = Transition::new(500, 24.0)
///     .easing(Easing::EaseOutCubic)
///     .delay(100);
/// ```
#[derive(Debug, Clone)]
pub struct Transition {
    pub to: serde_json::Value,
    pub duration: u64,
    pub easing: Easing,
    pub delay: u64,
    pub from: Option<serde_json::Value>,
    pub repeat: Option<Repeat>,
    pub auto_reverse: bool,
    pub on_complete: Option<String>,
}

impl Serialize for Transition {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", "transition")?;
        map.serialize_entry("to", &self.to)?;
        map.serialize_entry("duration", &self.duration)?;
        map.serialize_entry("easing", &self.easing)?;
        if self.delay > 0 { map.serialize_entry("delay", &self.delay)?; }
        if let Some(ref from) = self.from { map.serialize_entry("from", from)?; }
        if let Some(ref repeat) = self.repeat {
            let wire_val: i64 = match repeat {
                Repeat::Forever => -1,
                Repeat::Times(n) => *n as i64,
            };
            map.serialize_entry("repeat", &wire_val)?;
        }
        if self.auto_reverse { map.serialize_entry("auto_reverse", &true)?; }
        if let Some(ref tag) = self.on_complete { map.serialize_entry("on_complete", tag)?; }
        map.end()
    }
}

impl Transition {
    /// Create a transition with the given duration and target value.
    pub fn new(duration_ms: u64, to: impl Into<serde_json::Value>) -> Self {
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
    pub fn to(mut self, v: impl Into<serde_json::Value>) -> Self { self.to = v.into(); self }
    pub fn easing(mut self, e: Easing) -> Self { self.easing = e; self }
    pub fn delay(mut self, ms: u64) -> Self { self.delay = ms; self }
    pub fn from(mut self, v: impl Into<serde_json::Value>) -> Self {
        self.from = Some(v.into()); self
    }
    pub fn repeat(mut self, n: u32) -> Self { self.repeat = Some(Repeat::Times(n)); self }
    pub fn repeat_forever(mut self) -> Self { self.repeat = Some(Repeat::Forever); self }
    pub fn auto_reverse(mut self, v: bool) -> Self { self.auto_reverse = v; self }
    pub fn on_complete(mut self, tag: &str) -> Self { self.on_complete = Some(tag.into()); self }

    /// Create a looping transition (repeat forever, auto-reverse).
    pub fn looping(duration_ms: u64, to: impl Into<serde_json::Value>) -> Self {
        Self::new(duration_ms, to)
            .repeat_forever()
            .auto_reverse(true)
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
/// let s = Spring::new(1.05).stiffness(200.0).damping(20.0);
/// let bouncy = Spring::bouncy(1.05);
/// ```
#[derive(Debug, Clone)]
pub struct Spring {
    pub to: serde_json::Value,
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
    pub velocity: f64,
    pub from: Option<serde_json::Value>,
    pub on_complete: Option<String>,
}

impl Serialize for Spring {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", "spring")?;
        map.serialize_entry("to", &self.to)?;
        map.serialize_entry("stiffness", &self.stiffness)?;
        map.serialize_entry("damping", &self.damping)?;
        if (self.mass - 1.0).abs() > f64::EPSILON { map.serialize_entry("mass", &self.mass)?; }
        if self.velocity.abs() > f64::EPSILON { map.serialize_entry("velocity", &self.velocity)?; }
        if let Some(ref from) = self.from { map.serialize_entry("from", from)?; }
        if let Some(ref tag) = self.on_complete { map.serialize_entry("on_complete", tag)?; }
        map.end()
    }
}

impl Spring {
    /// Create a spring targeting the given value.
    pub fn new(to: impl Into<serde_json::Value>) -> Self {
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
    pub fn to(mut self, v: impl Into<serde_json::Value>) -> Self { self.to = v.into(); self }
    pub fn stiffness(mut self, s: f64) -> Self { self.stiffness = s; self }
    pub fn damping(mut self, d: f64) -> Self { self.damping = d; self }
    pub fn mass(mut self, m: f64) -> Self { self.mass = m; self }
    pub fn velocity(mut self, v: f64) -> Self { self.velocity = v; self }
    pub fn from(mut self, v: impl Into<serde_json::Value>) -> Self {
        self.from = Some(v.into()); self
    }
    pub fn on_complete(mut self, tag: &str) -> Self { self.on_complete = Some(tag.into()); self }

    // Named presets matching the Elixir SDK.

    /// Slow, smooth, no overshoot.
    pub fn gentle(to: impl Into<serde_json::Value>) -> Self { Self::new(to).stiffness(120.0).damping(14.0) }
    /// Quick with visible overshoot.
    pub fn bouncy(to: impl Into<serde_json::Value>) -> Self { Self::new(to).stiffness(300.0).damping(10.0) }
    /// Very quick, crisp stop.
    pub fn stiff(to: impl Into<serde_json::Value>) -> Self { Self::new(to).stiffness(400.0).damping(30.0) }
    /// Quick, minimal overshoot.
    pub fn snappy(to: impl Into<serde_json::Value>) -> Self { Self::new(to).stiffness(200.0).damping(20.0) }
    /// Slow, heavy, deliberate.
    pub fn molasses(to: impl Into<serde_json::Value>) -> Self { Self::new(to).stiffness(60.0).damping(12.0) }
}

// ---------------------------------------------------------------------------
// Sequence
// ---------------------------------------------------------------------------

/// A chain of animation steps executed in order.
///
/// The descriptor is set as a widget prop value. The renderer detects
/// it by the `"type": "sequence"` field and runs each step in turn.
#[derive(Debug, Clone)]
pub struct Sequence {
    pub steps: Vec<AnimationStep>,
    pub on_complete: Option<String>,
}

impl Serialize for Sequence {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", "sequence")?;
        map.serialize_entry("steps", &self.steps)?;
        if let Some(ref tag) = self.on_complete { map.serialize_entry("on_complete", tag)?; }
        map.end()
    }
}

/// A single step in a sequence.
#[derive(Debug, Clone)]
pub enum AnimationStep {
    Transition(Transition),
    Spring(Spring),
}

impl Serialize for AnimationStep {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            AnimationStep::Transition(t) => t.serialize(serializer),
            AnimationStep::Spring(s) => s.serialize(serializer),
        }
    }
}

impl Sequence {
    pub fn new(steps: Vec<AnimationStep>) -> Self {
        Self { steps, on_complete: None }
    }

    pub fn on_complete(mut self, tag: &str) -> Self {
        self.on_complete = Some(tag.into());
        self
    }
}

impl From<Transition> for AnimationStep {
    fn from(t: Transition) -> Self { AnimationStep::Transition(t) }
}

impl From<Spring> for AnimationStep {
    fn from(s: Spring) -> Self { AnimationStep::Spring(s) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_preset_values_match_elixir_sdk() {
        let g = Spring::gentle(1.0);
        assert_eq!(g.stiffness, 120.0);
        assert_eq!(g.damping, 14.0);

        let b = Spring::bouncy(1.0);
        assert_eq!(b.stiffness, 300.0);
        assert_eq!(b.damping, 10.0);

        let st = Spring::stiff(1.0);
        assert_eq!(st.stiffness, 400.0);
        assert_eq!(st.damping, 30.0);

        let sn = Spring::snappy(1.0);
        assert_eq!(sn.stiffness, 200.0);
        assert_eq!(sn.damping, 20.0);

        let m = Spring::molasses(1.0);
        assert_eq!(m.stiffness, 60.0);
        assert_eq!(m.damping, 12.0);
    }

    #[test]
    fn transition_serializes_as_descriptor() {
        let t = Transition::new(300, 24.0).easing(Easing::EaseOut);
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json["type"], "transition");
        assert_eq!(json["to"], 24.0);
        assert_eq!(json["duration"], 300);
        assert_eq!(json["easing"], "ease_out");
    }

    #[test]
    fn transition_repeat_serializes_as_integer() {
        let t = Transition::new(300, 1.0).repeat(3);
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json["repeat"], 3);

        let t = Transition::looping(300, 1.0);
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json["repeat"], -1);
        assert_eq!(json["auto_reverse"], true);
    }

    #[test]
    fn spring_serializes_as_descriptor() {
        let s = Spring::bouncy(1.05);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["type"], "spring");
        assert_eq!(json["to"], 1.05);
        assert_eq!(json["stiffness"], 300.0);
    }

    #[test]
    fn sequence_serializes_as_descriptor() {
        let seq = Sequence::new(vec![
            Transition::new(200, 1.0).into(),
            Spring::new(0.0).stiffness(200.0).into(),
        ]);
        let json = serde_json::to_value(&seq).unwrap();
        assert_eq!(json["type"], "sequence");
        let steps = json["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["type"], "transition");
        assert_eq!(steps[1]["type"], "spring");
    }
}
