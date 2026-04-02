//! Animation system: renderer-side transitions, springs, sequences, and exit ghosts.
//!
//! The SDK sends animation descriptors as prop values. The renderer detects
//! them during patch application, sets up internal animation state, and
//! interpolates on each frame. Zero wire traffic during active animations.
//!
//! # Architecture
//!
//! - `TransitionManager` is the main entry point, stored on the renderer App.
//! - On patch application, `detect_descriptors` scans prop values for
//!   `{"type": "transition"|"spring"|"sequence"}` maps.
//! - On each frame, `advance_all` progresses all active animations and writes
//!   interpolated values to the `interpolated_props` cache.
//! - Widget render functions use `prop_animated_*` helpers that check the
//!   cache before falling back to tree props.
//! - Ghost management handles exit animations by keeping removed nodes in
//!   layout flow with index adjustment on patches.

pub mod color;
pub mod easing;
pub mod ghost;
pub mod spring;
pub mod timed;

use ghost::GhostManager;
use iced::animation::Easing;
use iced::time::Instant;
use serde_json::Value;
use std::collections::HashMap;

/// The value being animated: either a number or a color.
#[derive(Clone, Debug)]
pub enum AnimValue {
    Number(f32),
    Color(iced::Color),
}

impl AnimValue {
    /// Converts to a JSON value for the interpolated_props cache.
    pub fn to_json(&self) -> Value {
        match self {
            AnimValue::Number(n) => Value::Number(
                serde_json::Number::from_f64(*n as f64).unwrap_or(serde_json::Number::from(0)),
            ),
            AnimValue::Color(c) => Value::String(color::color_to_hex(*c)),
        }
    }
}

/// The kind of animation being performed.
#[derive(Clone, Debug)]
pub enum AnimationKind {
    Timed {
        duration_ms: f32,
        easing: Easing,
        bezier: Option<[f32; 4]>,
        delay_ms: f32,
        repeat: RepeatMode,
        auto_reverse: bool,
    },
    Spring {
        stiffness: f32,
        damping: f32,
        mass: f32,
    },
}

/// Repeat mode for timed transitions.
#[derive(Clone, Debug)]
pub enum RepeatMode {
    None,
    Count(u32),
    Forever,
}

/// State for a single animated property.
#[derive(Clone, Debug)]
pub struct TransitionState {
    pub kind: AnimationKind,
    pub from: AnimValue,
    pub to: AnimValue,
    pub current: AnimValue,
    pub velocity: f32,
    pub elapsed_ms: f32,
    pub on_complete: Option<String>,
    pub finished: bool,
}

/// State for a sequence of animations on one property.
#[derive(Clone, Debug)]
pub struct SequenceState {
    pub steps: Vec<TransitionState>,
    pub current_step: usize,
    pub on_complete: Option<String>,
}

/// An active animation: either a single transition/spring or a sequence.
#[derive(Clone, Debug)]
pub enum ActiveAnimation {
    Single(TransitionState),
    Sequence(SequenceState),
}

/// Completion event to emit back to the SDK.
pub struct CompletionEvent {
    pub widget_id: String,
    pub prop_name: String,
    pub tag: String,
}

/// Main animation manager. Tracks all active animations and ghosts.
pub struct TransitionManager {
    /// Active animations keyed by (widget_id, prop_name).
    active: HashMap<(String, String), ActiveAnimation>,
    /// Ghost manager for exit animations.
    pub ghosts: GhostManager,
    /// Last frame timestamp for delta calculation (daemon mode).
    last_tick: Option<Instant>,
    /// Last headless timestamp for delta calculation (headless mode).
    last_headless_ms: Option<u64>,
    /// Whether reduced motion is active (skip all animations).
    pub reduced_motion: bool,
}

impl Default for TransitionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TransitionManager {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            ghosts: GhostManager::new(),
            last_tick: None,
            last_headless_ms: None,
            reduced_motion: false,
        }
    }

    /// Returns true if any animations or ghosts are active.
    pub fn has_active(&self) -> bool {
        !self.active.is_empty() || self.ghosts.has_active()
    }

    /// Clears all state (used on snapshot/reset).
    pub fn clear(&mut self) {
        self.active.clear();
        self.ghosts.clear();
        self.last_tick = None;
        self.last_headless_ms = None;
    }

    /// Advances all active animations by one frame.
    ///
    /// Returns completion events to emit to the SDK.
    pub fn advance_all(
        &mut self,
        now: Instant,
        interpolated_props: &mut HashMap<String, serde_json::Map<String, Value>>,
    ) -> Vec<CompletionEvent> {
        let dt = match self.last_tick {
            Some(prev) => now.duration_since(prev).as_secs_f32(),
            None => 1.0 / 60.0, // assume 60fps for first frame
        };
        self.last_tick = Some(now);

        let mut completions = Vec::new();

        // Advance each active animation
        for ((widget_id, prop_name), anim) in &mut self.active {
            let (value, finished) = advance_animation(anim, dt);

            // Write to interpolated props cache
            interpolated_props
                .entry(widget_id.clone())
                .or_default()
                .insert(prop_name.clone(), value.to_json());

            if finished && let Some(tag) = completion_tag(anim) {
                completions.push(CompletionEvent {
                    widget_id: widget_id.clone(),
                    prop_name: prop_name.clone(),
                    tag,
                });
            }
        }

        // Remove finished animations
        self.active.retain(|_, anim| !is_finished(anim));

        // Clean up interpolated props for widgets with no active animations
        let active_widgets: std::collections::HashSet<&String> =
            self.active.keys().map(|(wid, _)| wid).collect();
        interpolated_props.retain(|widget_id, _| active_widgets.contains(widget_id));

        completions
    }

    /// Advances using a host-provided timestamp (headless/test mode).
    ///
    /// Computes delta from the previous headless timestamp for
    /// deterministic, testable animation advancement.
    pub fn advance_with_timestamp(
        &mut self,
        timestamp_ms: u64,
        interpolated_props: &mut HashMap<String, serde_json::Map<String, Value>>,
    ) -> Vec<CompletionEvent> {
        let prev = self.last_headless_ms.unwrap_or(timestamp_ms);
        let delta_ms = timestamp_ms.saturating_sub(prev);
        self.last_headless_ms = Some(timestamp_ms);

        let dt = delta_ms as f32 / 1000.0;

        let mut completions = Vec::new();

        for ((widget_id, prop_name), anim) in &mut self.active {
            let (value, finished) = advance_animation(anim, dt);

            interpolated_props
                .entry(widget_id.clone())
                .or_default()
                .insert(prop_name.clone(), value.to_json());

            if finished && let Some(tag) = completion_tag(anim) {
                completions.push(CompletionEvent {
                    widget_id: widget_id.clone(),
                    prop_name: prop_name.clone(),
                    tag,
                });
            }
        }

        self.active.retain(|_, anim| !is_finished(anim));

        // Clean up interpolated props for widgets with no active animations
        let active_widgets: std::collections::HashSet<&String> =
            self.active.keys().map(|(wid, _)| wid).collect();
        interpolated_props.retain(|widget_id, _| active_widgets.contains(widget_id));

        completions
    }

    /// Registers a new animation from a detected descriptor.
    pub fn start_animation(
        &mut self,
        widget_id: String,
        prop_name: String,
        animation: ActiveAnimation,
    ) {
        self.active.insert((widget_id, prop_name), animation);
    }

    /// Cancels an animation on a specific widget+prop.
    pub fn cancel(&mut self, widget_id: &str, prop_name: &str) {
        self.active
            .remove(&(widget_id.to_string(), prop_name.to_string()));
    }

    /// Returns the current interpolated value for a widget+prop, if animating.
    pub fn current_value(&self, widget_id: &str, prop_name: &str) -> Option<&AnimValue> {
        self.active
            .get(&(widget_id.to_string(), prop_name.to_string()))
            .map(|anim| match anim {
                ActiveAnimation::Single(s) => &s.current,
                ActiveAnimation::Sequence(seq) => &seq.steps[seq.current_step].current,
            })
    }

    /// Scans a tree for animation descriptors and sets up/updates animations.
    ///
    /// Called after patch/snapshot application. For each prop that is an
    /// animation descriptor, starts a new animation or updates the target
    /// if it changed. For props that are raw values, cancels any active
    /// animation.
    pub fn scan_tree(&mut self, root: Option<&crate::protocol::TreeNode>) {
        if self.reduced_motion {
            return;
        }
        if let Some(node) = root {
            self.scan_node(node);
        }
    }

    fn scan_node(&mut self, node: &crate::protocol::TreeNode) {
        if let Some(props) = node.props.as_object() {
            for (key, value) in props {
                // Skip internal props
                if key.starts_with("__") || key == "exit" {
                    continue;
                }

                if is_descriptor(value) {
                    let old_value = self.current_value(&node.id, key).and_then(|v| match v {
                        AnimValue::Number(n) => Some(*n),
                        _ => None,
                    });

                    if let Some(new_anim) = parse_descriptor(value, old_value) {
                        let target_same = self
                            .active
                            .get(&(node.id.clone(), key.clone()))
                            .map(|existing| targets_match(existing, &new_anim))
                            .unwrap_or(false);

                        if !target_same {
                            self.start_animation(node.id.clone(), key.clone(), new_anim);
                        }
                    }
                } else {
                    // Raw value: cancel any active animation for this prop
                    let anim_key = (node.id.clone(), key.clone());
                    if self.active.contains_key(&anim_key) {
                        self.active.remove(&anim_key);
                    }
                }
            }
        }

        for child in &node.children {
            self.scan_node(child);
        }
    }
}

// -- Internal animation advancement --

fn advance_animation(anim: &mut ActiveAnimation, dt: f32) -> (AnimValue, bool) {
    match anim {
        ActiveAnimation::Single(state) => advance_single(state, dt),
        ActiveAnimation::Sequence(seq) => advance_sequence(seq, dt),
    }
}

fn advance_single(state: &mut TransitionState, dt: f32) -> (AnimValue, bool) {
    if state.finished {
        return (state.current.clone(), true);
    }

    state.elapsed_ms += dt * 1000.0;

    match &state.kind {
        AnimationKind::Timed {
            duration_ms,
            easing,
            bezier,
            delay_ms,
            repeat: repeat_mode,
            auto_reverse,
        } => {
            // Clone values upfront to avoid borrow conflicts when
            // mutating state for repeat/auto_reverse
            let dur = *duration_ms;
            let del = *delay_ms;
            let eas = *easing;
            let bez = *bezier;
            let repeat = repeat_mode.clone();
            let auto_rev = *auto_reverse;
            let (eased_t, cycle_done) = timed::progress(state.elapsed_ms, dur, del, eas, bez);

            // Interpolate based on value type
            let value = match (&state.from, &state.to) {
                (AnimValue::Number(f), AnimValue::Number(t)) => {
                    if cycle_done {
                        state.to.clone()
                    } else {
                        AnimValue::Number(f + (t - f) * eased_t)
                    }
                }
                (AnimValue::Color(f), AnimValue::Color(t)) => {
                    if cycle_done {
                        state.to.clone()
                    } else {
                        AnimValue::Color(color::interpolate(*f, *t, eased_t))
                    }
                }
                _ => {
                    state.finished = true;
                    return (state.to.clone(), true);
                }
            };

            state.current = value.clone();

            if cycle_done {
                // Handle repeat/auto_reverse
                match repeat {
                    RepeatMode::None => {
                        state.finished = true;
                        (value, true)
                    }
                    RepeatMode::Forever => {
                        if auto_rev {
                            std::mem::swap(&mut state.from, &mut state.to);
                        }
                        state.elapsed_ms -= dur + del;
                        (value, false)
                    }
                    RepeatMode::Count(n) => {
                        if n <= 1 {
                            state.finished = true;
                            (value, true)
                        } else {
                            if let AnimationKind::Timed { repeat, .. } = &mut state.kind {
                                *repeat = RepeatMode::Count(n - 1);
                            }
                            if auto_rev {
                                std::mem::swap(&mut state.from, &mut state.to);
                            }
                            state.elapsed_ms -= dur + del;
                            (value, false)
                        }
                    }
                }
            } else {
                (value, false)
            }
        }

        AnimationKind::Spring {
            stiffness,
            damping,
            mass,
        } => {
            let target = match &state.to {
                AnimValue::Number(t) => *t,
                _ => {
                    state.finished = true;
                    return (state.to.clone(), true);
                }
            };

            let current = match &state.current {
                AnimValue::Number(c) => *c,
                _ => target,
            };

            let params = spring::SpringParams {
                stiffness: *stiffness,
                damping: *damping,
                mass: *mass,
            };

            let spring_state = spring::SpringState {
                position: current,
                velocity: state.velocity,
            };

            let (new_state, settled) = spring::advance(spring_state, target, &params, dt);

            state.current = AnimValue::Number(new_state.position);
            state.velocity = new_state.velocity;
            state.finished = settled;
            (state.current.clone(), settled)
        }
    }
}

fn advance_sequence(seq: &mut SequenceState, dt: f32) -> (AnimValue, bool) {
    if seq.steps.is_empty() {
        return (AnimValue::Number(0.0), true);
    }
    if seq.current_step >= seq.steps.len() {
        let last = &seq.steps[seq.steps.len() - 1];
        return (last.current.clone(), true);
    }

    let step = &mut seq.steps[seq.current_step];
    let (value, step_finished) = advance_single(step, dt);

    if step_finished && seq.current_step + 1 < seq.steps.len() {
        // Move to next step, carrying over the final value as from
        seq.current_step += 1;
        let next = &mut seq.steps[seq.current_step];
        next.from = value.clone();
        next.current = value.clone();
        (value, false)
    } else if step_finished {
        // Last step finished
        (value, true)
    } else {
        (value, false)
    }
}

fn is_finished(anim: &ActiveAnimation) -> bool {
    match anim {
        ActiveAnimation::Single(s) => s.finished,
        ActiveAnimation::Sequence(seq) => {
            seq.current_step >= seq.steps.len()
                || (seq.current_step == seq.steps.len() - 1 && seq.steps[seq.current_step].finished)
        }
    }
}

fn targets_match(existing: &ActiveAnimation, new: &ActiveAnimation) -> bool {
    let existing_to = match existing {
        ActiveAnimation::Single(s) => &s.to,
        ActiveAnimation::Sequence(seq) => match seq.steps.last() {
            Some(s) => &s.to,
            None => return false,
        },
    };
    let new_to = match new {
        ActiveAnimation::Single(s) => &s.to,
        ActiveAnimation::Sequence(seq) => match seq.steps.last() {
            Some(s) => &s.to,
            None => return false,
        },
    };
    match (existing_to, new_to) {
        (AnimValue::Number(a), AnimValue::Number(b)) => (a - b).abs() < f32::EPSILON,
        (AnimValue::Color(a), AnimValue::Color(b)) => {
            (a.r - b.r).abs() < f32::EPSILON
                && (a.g - b.g).abs() < f32::EPSILON
                && (a.b - b.b).abs() < f32::EPSILON
                && (a.a - b.a).abs() < f32::EPSILON
        }
        _ => false,
    }
}

fn completion_tag(anim: &ActiveAnimation) -> Option<String> {
    match anim {
        ActiveAnimation::Single(s) => s.on_complete.clone(),
        ActiveAnimation::Sequence(seq) => seq.on_complete.clone(),
    }
}

/// Checks if a prop value is an animation descriptor.
pub fn is_descriptor(value: &Value) -> bool {
    value
        .as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(|t| t.as_str())
        .map(|t| matches!(t, "transition" | "spring" | "sequence"))
        .unwrap_or(false)
}

/// Parses a transition descriptor from a prop value.
pub fn parse_descriptor(value: &Value, old_value: Option<f32>) -> Option<ActiveAnimation> {
    let obj = value.as_object()?;
    let desc_type = obj.get("type")?.as_str()?;

    match desc_type {
        "transition" => parse_timed(obj, old_value),
        "spring" => parse_spring(obj, old_value),
        "sequence" => parse_sequence(obj, old_value),
        _ => None,
    }
}

/// Parses a value that could be a number or a color string into AnimValue.
fn parse_anim_value(value: &Value) -> Option<AnimValue> {
    if let Some(n) = value.as_f64() {
        Some(AnimValue::Number(n as f32))
    } else if value.as_str().is_some() {
        color::parse_color(value).map(AnimValue::Color)
    } else {
        None
    }
}

fn parse_timed(
    obj: &serde_json::Map<String, Value>,
    old_value: Option<f32>,
) -> Option<ActiveAnimation> {
    let to_val = parse_anim_value(obj.get("to")?)?;
    let duration = obj.get("duration")?.as_f64()? as f32;
    let easing_val = obj.get("easing").unwrap_or(&Value::Null);
    let (easing, bezier) = if easing_val.is_null() {
        (Easing::EaseInOut, None)
    } else if let Some(points) = easing_val
        .as_object()
        .and_then(|o| o.get("cubic_bezier"))
        .and_then(|v| v.as_array())
    {
        if points.len() == 4 {
            let x1 = points[0].as_f64().unwrap_or(0.0) as f32;
            let y1 = points[1].as_f64().unwrap_or(0.0) as f32;
            let x2 = points[2].as_f64().unwrap_or(1.0) as f32;
            let y2 = points[3].as_f64().unwrap_or(1.0) as f32;
            (Easing::Linear, Some([x1, y1, x2, y2]))
        } else {
            (Easing::EaseInOut, None)
        }
    } else {
        (easing::resolve(easing_val), None)
    };
    let delay = obj.get("delay").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let from_val = obj
        .get("from")
        .and_then(parse_anim_value)
        .or_else(|| old_value.map(AnimValue::Number))
        .unwrap_or_else(|| to_val.clone());
    let repeat = match obj.get("repeat").and_then(|v| v.as_i64()) {
        Some(-1) => RepeatMode::Forever,
        Some(n) if n > 0 => RepeatMode::Count(n as u32),
        _ => RepeatMode::None,
    };
    let auto_reverse = obj
        .get("auto_reverse")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let on_complete = obj
        .get("on_complete")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(ActiveAnimation::Single(TransitionState {
        kind: AnimationKind::Timed {
            duration_ms: duration,
            easing,
            bezier,
            delay_ms: delay,
            repeat,
            auto_reverse,
        },
        from: from_val.clone(),
        to: to_val,
        current: from_val,
        velocity: 0.0,
        elapsed_ms: 0.0,
        on_complete,
        finished: false,
    }))
}

fn parse_spring(
    obj: &serde_json::Map<String, Value>,
    old_value: Option<f32>,
) -> Option<ActiveAnimation> {
    let to = obj.get("to")?.as_f64()? as f32;
    let stiffness = obj
        .get("stiffness")
        .and_then(|v| v.as_f64())
        .unwrap_or(100.0) as f32;
    let damping = obj.get("damping").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
    let mass = obj.get("mass").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let velocity = obj.get("velocity").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let from = obj
        .get("from")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .or(old_value)
        .unwrap_or(to);
    let on_complete = obj
        .get("on_complete")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(ActiveAnimation::Single(TransitionState {
        kind: AnimationKind::Spring {
            stiffness,
            damping,
            mass,
        },
        from: AnimValue::Number(from),
        to: AnimValue::Number(to),
        current: AnimValue::Number(from),
        velocity,
        elapsed_ms: 0.0,
        on_complete,
        finished: false,
    }))
}

fn parse_sequence(
    obj: &serde_json::Map<String, Value>,
    old_value: Option<f32>,
) -> Option<ActiveAnimation> {
    let steps_val = obj.get("steps")?.as_array()?;
    let on_complete = obj
        .get("on_complete")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut steps = Vec::new();
    let mut prev_to = old_value;

    for step_val in steps_val {
        let step_obj = step_val.as_object()?;
        let step_type = step_obj.get("type")?.as_str()?;

        let anim = match step_type {
            "transition" => parse_timed(step_obj, prev_to),
            "spring" => parse_spring(step_obj, prev_to),
            _ => continue,
        };

        if let Some(ActiveAnimation::Single(state)) = anim {
            prev_to = match &state.to {
                AnimValue::Number(n) => Some(*n),
                _ => None,
            };
            steps.push(state);
        }
    }

    if steps.is_empty() {
        return None;
    }

    Some(ActiveAnimation::Sequence(SequenceState {
        steps,
        current_step: 0,
        on_complete,
    }))
}
