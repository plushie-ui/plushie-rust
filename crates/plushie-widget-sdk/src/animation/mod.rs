//! Animation system: renderer-side transitions, springs, sequences, and exit hooks.
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
//! - Exit ghost storage is present but disabled until removed nodes can be
//!   rendered, advanced, and pruned through the normal lifecycle.

pub mod color;
pub mod easing;
pub mod ghost;
pub mod spring;
pub mod timed;

use ghost::GhostManager;
use iced::animation::Easing;
use iced::time::Instant;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// The value being animated: either a number or a color.
#[derive(Clone, Debug)]
pub enum AnimValue {
    /// Scalar f32 value (pixels, angles, opacity, etc.).
    Number(f32),
    /// Color value interpolated in Oklch.
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
    /// Time-based tween with easing and optional loop/delay/reverse.
    Timed {
        /// Animation duration in milliseconds.
        duration_ms: f32,
        /// Easing curve.
        easing: Easing,
        /// Optional four-point bezier for custom easing.
        bezier: Option<[f32; 4]>,
        /// Delay before the tween starts, in milliseconds.
        delay_ms: f32,
        /// Repeat policy.
        repeat: RepeatMode,
        /// When true, each loop reverses direction.
        auto_reverse: bool,
    },
    /// Spring physics animation.
    Spring {
        /// Spring stiffness (higher is faster).
        stiffness: f32,
        /// Damping coefficient (higher resists oscillation).
        damping: f32,
        /// Mass of the animated "body" (higher feels heavier).
        mass: f32,
    },
}

/// Repeat mode for timed transitions.
#[derive(Clone, Debug)]
pub enum RepeatMode {
    /// Run once and stop.
    None,
    /// Repeat a fixed number of times.
    Count(u32),
    /// Repeat indefinitely.
    Forever,
}

/// State for a single animated property.
#[derive(Clone, Debug)]
pub struct TransitionState {
    /// Animation kind (timed or spring) with its parameters.
    pub kind: AnimationKind,
    /// Starting value.
    pub from: AnimValue,
    /// Target value.
    pub to: AnimValue,
    /// Current interpolated value.
    pub current: AnimValue,
    /// Current velocity (spring mode).
    pub velocity: f32,
    /// Elapsed milliseconds since the animation started.
    pub elapsed_ms: f32,
    /// Optional SDK completion tag.
    pub on_complete: Option<String>,
    /// True once the animation has reached its target.
    pub finished: bool,
}

/// State for a sequence of animations on one property.
#[derive(Clone, Debug)]
pub struct SequenceState {
    /// Ordered list of steps in the sequence.
    pub steps: Vec<TransitionState>,
    /// Index of the step currently running.
    pub current_step: usize,
    /// Optional SDK completion tag emitted when the sequence finishes.
    pub on_complete: Option<String>,
}

/// An active animation: either a single transition/spring or a sequence.
#[derive(Clone, Debug)]
pub enum ActiveAnimation {
    /// A single transition or spring.
    Single(TransitionState),
    /// A sequence of steps.
    Sequence(SequenceState),
}

/// Completion event to emit back to the SDK.
pub struct CompletionEvent {
    /// ID of the widget whose animation finished.
    pub widget_id: String,
    /// Name of the animated prop.
    pub prop_name: String,
    /// SDK completion tag (for correlation with `on_complete`).
    pub tag: String,
}

/// Main animation manager. Tracks all active animations and ghosts.
pub struct TransitionManager {
    /// Active animations nested as widget_id -> prop_name -> animation.
    /// Lookups dominate (per-frame `current_value`, scan-time cancel
    /// checks) so the nested layout lets cancel and current_value
    /// hit the inner map without allocating an owned `(String, String)`
    /// key just to probe a `HashMap`.
    active: HashMap<String, HashMap<String, ActiveAnimation>>,
    /// Set of widget IDs that have at least one active animation.
    /// Maintained incrementally on start/complete so the per-frame
    /// `interpolated_props.retain` filter doesn't have to rebuild
    /// it from `active.keys()` every advance. Equivalent to
    /// `active.keys()` by construction; kept as a separate set so
    /// the scan_node_inner short-circuit and the per-frame retain
    /// don't have to walk the outer map.
    active_widget_ids: HashSet<String>,
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
    /// Create an empty manager with no active animations.
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            active_widget_ids: HashSet::new(),
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
        self.active_widget_ids.clear();
        self.ghosts.clear();
        self.last_tick = None;
        self.last_headless_ms = None;
    }

    /// Removes animations and cached interpolated props for widgets that are
    /// no longer present in the live tree.
    pub(crate) fn prune_to_live_widgets(
        &mut self,
        live_ids: &HashSet<String>,
        interpolated_props: &mut HashMap<String, serde_json::Map<String, Value>>,
    ) {
        self.active
            .retain(|widget_id, _| live_ids.contains(widget_id));
        self.active_widget_ids
            .retain(|widget_id| live_ids.contains(widget_id));
        interpolated_props.retain(|widget_id, _| live_ids.contains(widget_id));
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

        // Advance every active animation. The bulk-iterate cost of the
        // nested for-loop is acceptable because lookups (current_value,
        // cancel) dominate at 60 FPS; the bulk path runs once per frame.
        for (widget_id, props) in &mut self.active {
            let entry = interpolated_props.entry(widget_id.clone()).or_default();
            for (prop_name, anim) in props.iter_mut() {
                let (value, finished) = advance_animation(anim, dt);
                entry.insert(prop_name.clone(), value.to_json());

                if finished && let Some(tag) = completion_tag(anim) {
                    completions.push(CompletionEvent {
                        widget_id: widget_id.clone(),
                        prop_name: prop_name.clone(),
                        tag,
                    });
                }
            }
        }

        // Remove finished animations and the corresponding widget
        // entries in `active_widget_ids` when no other animation
        // remains for that widget.
        self.retain_unfinished();

        // Clean up interpolated props for widgets with no active
        // animations. The set is maintained incrementally so the
        // per-frame retain doesn't pay for a fresh allocation.
        interpolated_props.retain(|widget_id, _| self.active_widget_ids.contains(widget_id));

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

        for (widget_id, props) in &mut self.active {
            let entry = interpolated_props.entry(widget_id.clone()).or_default();
            for (prop_name, anim) in props.iter_mut() {
                let (value, finished) = advance_animation(anim, dt);
                entry.insert(prop_name.clone(), value.to_json());

                if finished && let Some(tag) = completion_tag(anim) {
                    completions.push(CompletionEvent {
                        widget_id: widget_id.clone(),
                        prop_name: prop_name.clone(),
                        tag,
                    });
                }
            }
        }

        self.retain_unfinished();
        interpolated_props.retain(|widget_id, _| self.active_widget_ids.contains(widget_id));

        completions
    }

    /// Drop finished animations and rebuild `active_widget_ids`
    /// from the survivors. The retain happens once; the membership
    /// rebuild only runs when at least one animation finished, so
    /// the steady-state advance pays nothing extra.
    fn retain_unfinished(&mut self) {
        let mut had_completion = false;
        self.active.retain(|_, props| {
            let before = props.len();
            props.retain(|_, anim| !is_finished(anim));
            if props.len() != before {
                had_completion = true;
            }
            !props.is_empty()
        });
        if had_completion {
            self.active_widget_ids.clear();
            for wid in self.active.keys() {
                self.active_widget_ids.insert(wid.clone());
            }
        }
    }

    /// Registers a new animation from a detected descriptor.
    pub fn start_animation(
        &mut self,
        widget_id: String,
        prop_name: String,
        animation: ActiveAnimation,
    ) {
        self.active_widget_ids.insert(widget_id.clone());
        self.active
            .entry(widget_id)
            .or_default()
            .insert(prop_name, animation);
    }

    /// Cancels an animation on a specific widget+prop.
    pub fn cancel(&mut self, widget_id: &str, prop_name: &str) {
        if let Some(props) = self.active.get_mut(widget_id)
            && props.remove(prop_name).is_some()
            && props.is_empty()
        {
            self.active.remove(widget_id);
            self.active_widget_ids.remove(widget_id);
        }
    }

    /// Returns the current interpolated value for a widget+prop, if animating.
    pub fn current_value(&self, widget_id: &str, prop_name: &str) -> Option<&AnimValue> {
        self.active
            .get(widget_id)
            .and_then(|props| props.get(prop_name))
            .and_then(|anim| match anim {
                ActiveAnimation::Single(s) => Some(&s.current),
                ActiveAnimation::Sequence(seq) => seq
                    .steps
                    .get(seq.current_step)
                    .or_else(|| seq.steps.last())
                    .map(|step| &step.current),
            })
    }

    /// Scans a tree for animation descriptors and sets up/updates animations.
    ///
    /// Called after patch/snapshot application. For each prop that is an
    /// animation descriptor, starts a new animation or updates the target
    /// if it changed. For props that are raw values, cancels any active
    /// animation.
    pub fn scan_tree(&mut self, root: Option<&mut crate::protocol::TreeNode>) {
        if self.reduced_motion {
            return;
        }
        if let Some(node) = root {
            let mut transform = ScanTransform { manager: self };
            let mut ctx = plushie_core::tree_walk::WalkCtx::default();
            plushie_core::tree_walk::walk(node, &mut [&mut transform], &mut ctx);
        }
    }

    /// Apply the per-node scan for a single node. Separated so
    /// [`ScanTransform`] and the renderer's combined prepare+scan walk
    /// can both drive it.
    ///
    /// Iterates the [`PropMap`] in place rather than converting to a
    /// JSON `Value` first. Most nodes are not animated, and the JSON
    /// conversion was a per-node clone of the entire prop set just to
    /// check whether any value happened to be an animation descriptor.
    pub(crate) fn scan_node_inner(&mut self, node: &crate::protocol::TreeNode) {
        let prop_map = node.props.as_prop_map();
        for (key, value) in prop_map.iter() {
            // Skip internal props
            if key.starts_with("__") || key == "exit" {
                continue;
            }

            if is_descriptor_prop(value) {
                let old_value = self.current_value(&node.id, key).cloned();

                // Convert just the descriptor value once, so
                // parse_descriptor can keep working on
                // serde_json::Value without rewriting every parser.
                let json_value = serde_json::Value::from(value.clone());
                if let Some(new_anim) = parse_descriptor_with_old_value(&json_value, old_value) {
                    let target_same = self
                        .active
                        .get(node.id.as_str())
                        .and_then(|props| props.get(key))
                        .map(|existing| targets_match(existing, &new_anim))
                        .unwrap_or(false);

                    if !target_same {
                        self.start_animation(node.id.clone(), key.to_string(), new_anim);
                    }
                } else {
                    crate::diagnostics::warn(
                        plushie_core::Diagnostic::AnimationDescriptorInvalid {
                            id: node.id.clone(),
                            prop: key.to_string(),
                        },
                    );
                }
            } else if self.active_widget_ids.contains(node.id.as_str()) {
                // Raw value, and this widget has at least one active
                // animation: cancel any animation on this prop. The
                // active_widget_ids fast-path skips the inner map
                // probe for the overwhelmingly common case of a node
                // that isn't animating anything.
                self.cancel(&node.id, key);
            }
        }
    }
}

/// Tree transform that detects animation descriptors on each node's
/// props and starts, updates, or cancels the corresponding animations
/// on the owning [`TransitionManager`].
///
/// The transform does not mutate nodes; it reads props and updates
/// the manager's internal maps. It's decoupled from prepare so
/// renderer hosts can compose `[PrepareTransform, ScanTransform]` in
/// a single walk.
pub struct ScanTransform<'a> {
    /// Manager receiving animation start/update/cancel notifications.
    pub manager: &'a mut TransitionManager,
}

impl plushie_core::tree_walk::TreeTransform for ScanTransform<'_> {
    fn enter(
        &mut self,
        node: &mut crate::protocol::TreeNode,
        _ctx: &mut plushie_core::tree_walk::WalkCtx,
    ) {
        if self.manager.reduced_motion {
            return;
        }
        self.manager.scan_node_inner(node);
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

    match &state.kind {
        AnimationKind::Timed {
            duration_ms,
            easing,
            bezier,
            delay_ms,
            repeat: repeat_mode,
            auto_reverse,
        } => {
            state.elapsed_ms += dt * 1000.0;

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
            let params = spring::SpringParams {
                stiffness: *stiffness,
                damping: *damping,
                mass: *mass,
            };

            match (&state.from, &state.to, &state.current) {
                (_, AnimValue::Number(target), AnimValue::Number(current)) => {
                    let spring_state = spring::SpringState {
                        position: *current,
                        velocity: state.velocity,
                    };
                    let (new_state, settled) = spring::advance(spring_state, *target, &params, dt);

                    state.current = AnimValue::Number(new_state.position);
                    state.velocity = new_state.velocity;
                    state.finished = settled;
                    (state.current.clone(), settled)
                }
                (AnimValue::Color(from), AnimValue::Color(to), AnimValue::Color(_)) => {
                    let spring_state = spring::SpringState {
                        position: state.elapsed_ms,
                        velocity: state.velocity,
                    };
                    let (new_state, settled) = spring::advance(spring_state, 1.0, &params, dt);
                    let progress = new_state.position.clamp(0.0, 1.0);
                    state.elapsed_ms = new_state.position;
                    state.velocity = new_state.velocity;
                    state.finished = settled;
                    state.current = if settled {
                        AnimValue::Color(*to)
                    } else {
                        AnimValue::Color(color::interpolate(*from, *to, progress))
                    };
                    (state.current.clone(), settled)
                }
                _ => {
                    state.finished = true;
                    (state.to.clone(), true)
                }
            }
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

/// `is_descriptor` for native `PropValue` so the per-node scan
/// doesn't have to convert the whole prop map to JSON just to
/// detect descriptor objects.
pub(crate) fn is_descriptor_prop(value: &plushie_core::protocol::PropValue) -> bool {
    use plushie_core::protocol::PropValue;
    let PropValue::Object(map) = value else {
        return false;
    };
    matches!(
        map.get("type").and_then(PropValue::as_str),
        Some("transition" | "spring" | "sequence")
    )
}

/// Parses a transition descriptor from a prop value.
pub fn parse_descriptor(value: &Value, old_value: Option<f32>) -> Option<ActiveAnimation> {
    parse_descriptor_with_old_value(value, old_value.map(AnimValue::Number))
}

fn parse_descriptor_with_old_value(
    value: &Value,
    old_value: Option<AnimValue>,
) -> Option<ActiveAnimation> {
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
    old_value: Option<AnimValue>,
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
        .or_else(|| compatible_old_value(&to_val, old_value))
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
    old_value: Option<AnimValue>,
) -> Option<ActiveAnimation> {
    let to_val = parse_anim_value(obj.get("to")?)?;
    let stiffness = obj
        .get("stiffness")
        .and_then(|v| v.as_f64())
        .unwrap_or(100.0) as f32;
    let damping = obj.get("damping").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
    let mass = parse_spring_mass(obj)?;
    let velocity = obj.get("velocity").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let from_val = obj
        .get("from")
        .and_then(parse_anim_value)
        .or_else(|| compatible_old_value(&to_val, old_value))
        .unwrap_or_else(|| to_val.clone());
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
        from: from_val.clone(),
        to: to_val,
        current: from_val,
        velocity,
        elapsed_ms: 0.0,
        on_complete,
        finished: false,
    }))
}

fn parse_spring_mass(obj: &serde_json::Map<String, Value>) -> Option<f32> {
    match obj.get("mass") {
        Some(value) => {
            let mass = value.as_f64()? as f32;
            (mass.is_finite() && mass > 0.0).then_some(mass)
        }
        None => Some(1.0),
    }
}

fn compatible_old_value(to: &AnimValue, old_value: Option<AnimValue>) -> Option<AnimValue> {
    match (to, old_value) {
        (AnimValue::Number(_), Some(old @ AnimValue::Number(_)))
        | (AnimValue::Color(_), Some(old @ AnimValue::Color(_))) => Some(old),
        _ => None,
    }
}

fn parse_sequence(
    obj: &serde_json::Map<String, Value>,
    old_value: Option<AnimValue>,
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
            _ => None,
        }?;

        let ActiveAnimation::Single(state) = anim else {
            return None;
        };
        prev_to = Some(state.to.clone());
        steps.push(state);
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn spring_with_zero_mass_is_rejected() {
        let descriptor = json!({
            "type": "spring",
            "to": 10.0,
            "stiffness": 100.0,
            "damping": 10.0,
            "mass": 0.0
        });

        assert!(parse_descriptor(&descriptor, Some(0.0)).is_none());
    }

    #[test]
    fn spring_with_negative_mass_is_rejected() {
        let descriptor = json!({
            "type": "spring",
            "to": 10.0,
            "stiffness": 100.0,
            "damping": 10.0,
            "mass": -1.0
        });

        assert!(parse_descriptor(&descriptor, Some(0.0)).is_none());
    }

    #[test]
    fn spring_with_out_of_range_mass_is_rejected() {
        let descriptor = json!({
            "type": "spring",
            "to": 10.0,
            "stiffness": 100.0,
            "damping": 10.0,
            "mass": f64::MAX
        });

        assert!(parse_descriptor(&descriptor, Some(0.0)).is_none());
    }

    #[test]
    fn spring_without_mass_uses_default() {
        let descriptor = json!({
            "type": "spring",
            "to": 10.0,
            "stiffness": 100.0,
            "damping": 10.0
        });

        assert!(parse_descriptor(&descriptor, Some(0.0)).is_some());
    }

    #[test]
    fn transition_manager_spring_survives_large_headless_delta() {
        let descriptor = json!({
            "type": "spring",
            "to": 100.0,
            "stiffness": 4000.0,
            "damping": 80.0,
            "mass": 1.0
        });
        let animation = parse_descriptor(&descriptor, Some(0.0)).unwrap();
        let mut manager = TransitionManager::new();
        manager.start_animation("panel".to_string(), "width".to_string(), animation);
        let mut props = HashMap::new();

        manager.advance_with_timestamp(0, &mut props);
        manager.advance_with_timestamp(100, &mut props);

        let value = props["panel"]["width"].as_f64().unwrap();
        assert!(value.is_finite());
        assert!((0.0..=150.0).contains(&value));
    }

    #[test]
    fn current_value_for_finished_sequence_returns_last_step() {
        let mut manager = TransitionManager::new();
        manager.start_animation(
            "panel".to_string(),
            "opacity".to_string(),
            ActiveAnimation::Sequence(SequenceState {
                steps: vec![TransitionState {
                    kind: AnimationKind::Timed {
                        duration_ms: 100.0,
                        easing: Easing::Linear,
                        bezier: None,
                        delay_ms: 0.0,
                        repeat: RepeatMode::None,
                        auto_reverse: false,
                    },
                    from: AnimValue::Number(0.0),
                    to: AnimValue::Number(1.0),
                    current: AnimValue::Number(1.0),
                    velocity: 0.0,
                    elapsed_ms: 100.0,
                    on_complete: None,
                    finished: true,
                }],
                current_step: 1,
                on_complete: None,
            }),
        );

        let value = manager.current_value("panel", "opacity");

        assert!(matches!(value, Some(AnimValue::Number(1.0))));
    }

    #[test]
    fn sequence_with_unknown_step_type_is_rejected() {
        let descriptor = json!({
            "type": "sequence",
            "steps": [
                {"type": "transition", "to": 1.0, "duration": 100.0},
                {"type": "pause", "duration": 100.0}
            ]
        });

        assert!(parse_descriptor(&descriptor, Some(0.0)).is_none());
    }

    #[test]
    fn sequence_with_invalid_step_is_rejected() {
        let descriptor = json!({
            "type": "sequence",
            "steps": [
                {"type": "transition", "to": 1.0, "duration": 100.0},
                {"type": "transition", "duration": 100.0}
            ]
        });

        assert!(parse_descriptor(&descriptor, Some(0.0)).is_none());
    }

    #[test]
    fn color_transition_carries_current_value_when_retargeted() {
        let previous = AnimValue::Color(iced::Color::from_rgba(0.25, 0.5, 0.75, 1.0));
        let descriptor = json!({
            "type": "transition",
            "to": "#ff0000",
            "duration": 100.0
        });

        let animation = parse_descriptor_with_old_value(&descriptor, Some(previous.clone()));

        let Some(ActiveAnimation::Single(state)) = animation else {
            panic!("expected single transition");
        };
        match state.from {
            AnimValue::Color(color) => {
                let AnimValue::Color(previous_color) = previous else {
                    unreachable!();
                };
                assert!((color.r - previous_color.r).abs() < f32::EPSILON);
                assert!((color.g - previous_color.g).abs() < f32::EPSILON);
                assert!((color.b - previous_color.b).abs() < f32::EPSILON);
                assert!((color.a - previous_color.a).abs() < f32::EPSILON);
            }
            _ => panic!("expected color from value"),
        }
    }

    #[test]
    fn color_spring_descriptor_parses() {
        let descriptor = json!({
            "type": "spring",
            "from": "#000000",
            "to": "#ffffff",
            "stiffness": 100.0,
            "damping": 10.0
        });

        let animation = parse_descriptor(&descriptor, None);

        let Some(ActiveAnimation::Single(state)) = animation else {
            panic!("expected single spring");
        };
        assert!(matches!(state.from, AnimValue::Color(_)));
        assert!(matches!(state.to, AnimValue::Color(_)));
    }

    #[test]
    fn color_spring_advances_to_color_value() {
        let descriptor = json!({
            "type": "spring",
            "from": "#000000",
            "to": "#ffffff",
            "stiffness": 100.0,
            "damping": 10.0
        });
        let mut animation = parse_descriptor(&descriptor, None).unwrap();

        let (value, finished) = advance_animation(&mut animation, 0.016);

        assert!(!finished);
        assert!(matches!(value, AnimValue::Color(_)));
    }
}
