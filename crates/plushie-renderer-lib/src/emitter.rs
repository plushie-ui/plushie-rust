//! Rate-limited event emission with coalescing.
//!
//! Buffers high-frequency events (mouse moves, scroll, animation frames)
//! and emits them at a configurable rate. Non-coalescable events (clicks,
//! key presses) flush the buffer immediately before emitting.
//!
//! The host controls rates via three mechanisms (highest priority first):
//! 1. Per-widget `event_rate` prop
//! 2. Per-subscription `max_rate` field on Subscribe
//! 3. Global `default_event_rate` in Settings

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use iced::time::{Duration, Instant};

use iced::Task;

use plushie_widget_sdk::message::Message;
use plushie_widget_sdk::protocol::{CoalesceHint, OutgoingEvent};

use crate::emitters::EventSink;

// ---------------------------------------------------------------------------
// Platform-aware sleep
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
async fn platform_sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_arch = "wasm32")]
async fn platform_sleep(duration: Duration) {
    wasmtimer::tokio::sleep(duration).await;
}

// ---------------------------------------------------------------------------
// Coalesce key
// ---------------------------------------------------------------------------

/// Identifies a stream of events that can be coalesced together.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum CoalesceKey {
    /// Subscription event keyed by entry tag (e.g. "on_pointer_move" or
    /// "on_pointer_move:main" for window-scoped subscriptions).
    Subscription(String),
    /// Widget event keyed by (widget_id, event_family).
    Widget(String, String),
}

// ---------------------------------------------------------------------------
// Pending event buffer
// ---------------------------------------------------------------------------

enum PendingEvent {
    /// Latest-value-wins: only the most recent event is kept.
    Replace(OutgoingEvent),
    /// Named-field accumulation: the listed fields in `data` are summed
    /// across arrivals. All other fields keep the latest event's values.
    Accumulate {
        base: OutgoingEvent,
        fields: Vec<String>,
        totals: HashMap<String, f64>,
    },
}

impl PendingEvent {
    fn from_hint(event: OutgoingEvent, hint: &CoalesceHint) -> Self {
        match hint {
            CoalesceHint::Replace => PendingEvent::Replace(event),
            CoalesceHint::Accumulate(fields) => {
                let mut totals = HashMap::new();
                if let Some(value) = &event.value {
                    for field in fields {
                        if let Some(val) = value.get(field).and_then(|v| v.as_f64()) {
                            totals.insert(field.clone(), val);
                        }
                    }
                }
                PendingEvent::Accumulate {
                    base: event,
                    fields: fields.clone(),
                    totals,
                }
            }
        }
    }

    fn merge(&mut self, event: OutgoingEvent) {
        match self {
            PendingEvent::Replace(existing) => *existing = event,
            PendingEvent::Accumulate {
                base,
                fields,
                totals,
            } => {
                if let Some(value) = &event.value {
                    for field in fields.iter() {
                        if let Some(val) = value.get(field).and_then(|v| v.as_f64()) {
                            *totals.entry(field.clone()).or_insert(0.0) += val;
                        }
                    }
                }
                *base = event;
            }
        }
    }

    fn into_event(self) -> OutgoingEvent {
        match self {
            PendingEvent::Replace(ev) => ev,
            PendingEvent::Accumulate {
                mut base, totals, ..
            } => {
                // Patch accumulated totals back into the event's value.
                if let Some(ref mut value) = base.value
                    && let Some(obj) = value.as_object_mut()
                {
                    for (field, total) in totals {
                        obj.insert(field, serde_json::json!(total));
                    }
                }
                base
            }
        }
    }
}

// ---------------------------------------------------------------------------
// EventEmitter
// ---------------------------------------------------------------------------

/// Rate-limited event emission with coalescing.
///
/// Sits between the iced message handlers and the wire protocol. Events
/// classified as coalescable are buffered and emitted at a controlled
/// rate; non-coalescable events flush the buffer and emit immediately.
pub struct EventEmitter {
    /// The output sink for emitted events.
    sink: Arc<Mutex<Box<dyn EventSink>>>,
    /// Pending coalescable events, keyed by coalesce key.
    pending: HashMap<CoalesceKey, PendingEvent>,
    /// Timestamp of last emission per coalesce key.
    last_emits: HashMap<CoalesceKey, Instant>,
    /// Whether a `Message::FlushCoalesce` timer task is outstanding.
    flush_scheduled: bool,
    /// Global default rate from Settings. None = no limit.
    default_rate: Option<u32>,
    /// Per-subscription rates from Subscribe max_rate.
    subscription_rates: HashMap<String, u32>,
    /// Per-widget rates from event_rate prop.
    widget_rates: HashMap<String, u32>,
    /// Batch-suppression state. When the depth is > 0, outgoing
    /// events are buffered here in order; they flush when the
    /// outermost batch closes. Protected by a Mutex so the sink
    /// emit paths (which take `&self`) can still buffer safely.
    batch: Arc<Mutex<BatchState>>,
}

#[derive(Default)]
struct BatchState {
    depth: u32,
    buffer: Vec<OutgoingEvent>,
}

impl EventEmitter {
    /// Create a new EventEmitter that writes to the given sink.
    pub fn new(sink: Arc<Mutex<Box<dyn EventSink>>>) -> Self {
        Self {
            sink,
            pending: HashMap::new(),
            last_emits: HashMap::new(),
            flush_scheduled: false,
            default_rate: None,
            subscription_rates: HashMap::new(),
            widget_rates: HashMap::new(),
            batch: Arc::new(Mutex::new(BatchState::default())),
        }
    }

    /// Begin an atomic batch: outgoing events are buffered until
    /// [`end_batch`](Self::end_batch) is called at the matching
    /// depth. Nested calls are counted so callers don't have to
    /// coordinate.
    pub fn begin_batch(&self) {
        let mut state = self.batch.lock().unwrap_or_else(|e| e.into_inner());
        state.depth = state.depth.saturating_add(1);
    }

    /// End an atomic batch. When the outermost batch closes, all
    /// buffered events are emitted through the sink in order.
    pub fn end_batch(&self) {
        let buffered = {
            let mut state = self.batch.lock().unwrap_or_else(|e| e.into_inner());
            if state.depth == 0 {
                return;
            }
            state.depth -= 1;
            if state.depth == 0 {
                std::mem::take(&mut state.buffer)
            } else {
                Vec::new()
            }
        };
        for event in buffered {
            if let Err(e) = self.with_sink(|sink| sink.emit_event(event)) {
                log::error!("event sink write error: {e}");
            }
        }
    }

    /// Get a clone of the sink Arc for passing to async callbacks.
    pub fn sink(&self) -> Arc<Mutex<Box<dyn EventSink>>> {
        self.sink.clone()
    }

    /// Set the global default rate from Settings.
    pub fn set_default_rate(&mut self, rate: Option<u32>) {
        self.default_rate = rate;
    }

    /// Set (or update) the rate for a subscription kind.
    pub fn set_subscription_rate(&mut self, kind: &str, rate: u32) {
        self.subscription_rates.insert(kind.to_string(), rate);
    }

    /// Remove rate tracking for a subscription kind.
    pub fn remove_subscription_rate(&mut self, kind: &str) {
        self.subscription_rates.remove(kind);
    }

    /// Set the rate for a specific widget (from `event_rate` prop).
    pub fn set_widget_rate(&mut self, widget_id: &str, rate: u32) {
        self.widget_rates.insert(widget_id.to_string(), rate);
    }

    /// Clear all widget rates (called on Snapshot, tree replaced).
    pub fn clear_widget_rates(&mut self) {
        self.widget_rates.clear();
    }

    /// Check whether a widget rate is already cached.
    pub fn has_widget_rate(&self, widget_id: &str) -> bool {
        self.widget_rates.contains_key(widget_id)
    }

    /// Iterate over the subscription rate keys.
    pub fn subscription_rate_keys(&self) -> impl Iterator<Item = &str> {
        self.subscription_rates.keys().map(|s| s.as_str())
    }

    /// Resolve the effective rate for a given key, following the
    /// priority hierarchy: widget > subscription > global default.
    fn effective_rate(&self, key: &CoalesceKey) -> Option<u32> {
        match key {
            CoalesceKey::Widget(widget_id, _family) => {
                if let Some(&rate) = self.widget_rates.get(widget_id) {
                    return Some(rate);
                }
                self.default_rate
            }
            CoalesceKey::Subscription(tag) => {
                if let Some(&rate) = self.subscription_rates.get(tag) {
                    return Some(rate);
                }
                self.default_rate
            }
        }
    }

    /// Emit a coalescable event, buffering it if the rate limit has
    /// not elapsed. The coalescing strategy is read from the event's
    /// [`CoalesceHint`]. Returns a Task if a flush timer needs scheduling.
    pub fn coalesce(&mut self, key: CoalesceKey, mut event: OutgoingEvent) -> Task<Message> {
        // Take the hint out of the event: it's consumed by the emitter
        // and not needed downstream (not serialized to the wire).
        let hint = match event.take_coalesce() {
            Some(h) => h,
            None => {
                // No hint: treat as non-coalescable (immediate delivery).
                return self.emit_immediate(event);
            }
        };

        let rate = self.effective_rate(&key);

        // Zero rate = muted, silently drop.
        if rate == Some(0) {
            return Task::none();
        }

        // No rate limit = emit immediately.
        let Some(rate) = rate else {
            self.flush_key(&key);
            return self.do_emit(event);
        };

        let min_interval = Duration::from_secs_f64(1.0 / rate as f64);
        let now = Instant::now();

        let can_emit_now = self
            .last_emits
            .get(&key)
            .map(|last| now.duration_since(*last) >= min_interval)
            .unwrap_or(true);

        if can_emit_now {
            self.pending.remove(&key);
            self.last_emits.insert(key, now);
            return self.do_emit(event);
        }

        // Buffer the event.
        self.buffer_event(&key, event, &hint);

        // Schedule a flush timer if one isn't already running.
        if !self.flush_scheduled {
            self.flush_scheduled = true;
            let remaining = self
                .last_emits
                .get(&key)
                .map(|last| min_interval.saturating_sub(now.duration_since(*last)))
                .unwrap_or(min_interval);
            return Task::perform(
                async move {
                    platform_sleep(remaining).await;
                },
                |_| Message::FlushCoalesce,
            );
        }

        Task::none()
    }

    /// Emit a non-coalescable event immediately, flushing pending
    /// events first to preserve ordering.
    pub fn emit_immediate(&mut self, event: OutgoingEvent) -> Task<Message> {
        self.flush_all();
        self.do_emit(event)
    }

    /// Flush all pending events. Called by the `Message::FlushCoalesce`
    /// handler.
    pub fn flush(&mut self) -> Task<Message> {
        self.flush_scheduled = false;
        self.flush_all();
        Task::none()
    }

    /// Flush pending events for a specific key.
    pub fn flush_key(&mut self, key: &CoalesceKey) {
        if let Some(pending) = self.pending.remove(key) {
            let now = Instant::now();
            self.last_emits.insert(key.clone(), now);
            let _ = self.do_emit(pending.into_event());
        }
    }

    /// Flush all pending events (internal).
    fn flush_all(&mut self) {
        let keys: Vec<CoalesceKey> = self.pending.keys().cloned().collect();
        let now = Instant::now();
        for key in keys {
            if let Some(pending) = self.pending.remove(&key) {
                self.last_emits.insert(key, now);
                let _ = self.do_emit(pending.into_event());
            }
        }
    }

    /// Buffer an event under the given key.
    ///
    /// If the existing entry uses a different strategy (e.g. Replace vs
    /// Accumulate), the old entry is flushed first and a fresh buffer is
    /// started. This handles the edge case where a widget changes
    /// its coalesce hint between events for the same key.
    fn buffer_event(&mut self, key: &CoalesceKey, event: OutgoingEvent, hint: &CoalesceHint) {
        if let Some(existing) = self.pending.get_mut(key) {
            // Check for strategy mismatch. Replace-vs-Replace is always compatible.
            // Accumulate-vs-Accumulate is only compatible when the tracked field
            // set is identical: merging into a buffer that tracks different
            // fields would silently miscount totals.
            let compatible = match (&*existing, hint) {
                (PendingEvent::Replace(_), CoalesceHint::Replace) => true,
                (
                    PendingEvent::Accumulate {
                        fields: existing_fields,
                        ..
                    },
                    CoalesceHint::Accumulate(new_fields),
                ) => existing_fields == new_fields,
                _ => false,
            };
            if compatible {
                existing.merge(event);
                return;
            }
            // Strategy changed (or Accumulate field list changed); flush the old
            // entry and start fresh.
            self.flush_key(key);
        }
        self.pending
            .insert(key.clone(), PendingEvent::from_hint(event, hint));
    }

    /// Emit an event through the sink, returning a Result.
    ///
    /// Used by methods that return `io::Result` (e.g. event handlers
    /// in events.rs, apply.rs).
    pub fn emit_event(&self, event: OutgoingEvent) -> std::io::Result<()> {
        self.with_sink(|sink| sink.emit_event(event))
    }

    /// Write an event directly to the sink, bypassing rate limiting.
    ///
    /// Used for subscription events and system events that don't
    /// participate in widget-level coalescing. Returns Task::none()
    /// on success, iced::exit() on broken pipe.
    pub fn emit_direct(&self, event: OutgoingEvent) -> Task<Message> {
        self.do_emit(event)
    }

    /// Emit an effect response through the sink.
    pub fn emit_effect_response(
        &self,
        response: plushie_widget_sdk::protocol::EffectResponse,
    ) -> std::io::Result<()> {
        self.with_sink(|sink| sink.emit_effect_response(response))
    }

    /// Emit a query response through the sink.
    pub fn emit_query_response(
        &self,
        kind: &str,
        tag: &str,
        data: &serde_json::Value,
    ) -> std::io::Result<()> {
        self.with_sink(|sink| sink.emit_query_response(kind, tag, data))
    }

    /// Emit a screenshot response through the sink.
    pub fn emit_screenshot_response(
        &self,
        id: &str,
        name: &str,
        hash: &str,
        width: u32,
        height: u32,
        rgba_bytes: &[u8],
    ) -> std::io::Result<()> {
        self.with_sink(|sink| {
            sink.emit_screenshot_response(id, name, hash, width, height, rgba_bytes)
        })
    }

    /// Write pre-encoded bytes through the sink.
    pub fn write_raw(&self, bytes: &[u8]) -> std::io::Result<()> {
        self.with_sink(|sink| sink.write_raw(bytes))
    }

    fn do_emit(&self, event: OutgoingEvent) -> Task<Message> {
        {
            let mut state = self.batch.lock().unwrap_or_else(|e| e.into_inner());
            if state.depth > 0 {
                state.buffer.push(event);
                return Task::none();
            }
        }
        match self.with_sink(|sink| sink.emit_event(event)) {
            Ok(()) => Task::none(),
            Err(e) => {
                log::error!("write error: {e}");
                iced::exit()
            }
        }
    }

    fn with_sink<R>(
        &self,
        f: impl FnOnce(&mut dyn EventSink) -> std::io::Result<R>,
    ) -> std::io::Result<R> {
        let mut guard = self.sink.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut **guard)
    }
}

/// Build a CoalesceKey for a widget event.
pub fn widget_coalesce_key(event: &OutgoingEvent) -> CoalesceKey {
    CoalesceKey::Widget(event.id.clone(), event.family.clone())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use plushie_widget_sdk::protocol::{CoalesceHint, OutgoingEvent};
    use serde_json::json;

    /// No-op sink for unit tests that only exercise rate limiting
    /// and coalescing logic, not actual event delivery.
    struct NullSink;
    impl EventSink for NullSink {
        fn emit_event(&mut self, _: OutgoingEvent) -> std::io::Result<()> {
            Ok(())
        }
        fn emit_effect_response(
            &mut self,
            _: plushie_widget_sdk::protocol::EffectResponse,
        ) -> std::io::Result<()> {
            Ok(())
        }
        fn emit_query_response(
            &mut self,
            _: &str,
            _: &str,
            _: &serde_json::Value,
        ) -> std::io::Result<()> {
            Ok(())
        }
        fn emit_screenshot_response(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: u32,
            _: u32,
            _: &[u8],
        ) -> std::io::Result<()> {
            Ok(())
        }
        fn emit_hello(
            &mut self,
            _: &str,
            _: &str,
            _: &[&str],
            _: &[&str],
            _: &str,
        ) -> std::io::Result<()> {
            Ok(())
        }
        fn write_raw(&mut self, _: &[u8]) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn test_emitter() -> EventEmitter {
        let sink: Arc<Mutex<Box<dyn EventSink>>> = Arc::new(Mutex::new(Box::new(NullSink)));
        EventEmitter::new(sink)
    }

    fn make_event(family: &str, id: &str) -> OutgoingEvent {
        OutgoingEvent::widget_event(family, id, None)
    }

    fn make_event_with_data(family: &str, id: &str, data: serde_json::Value) -> OutgoingEvent {
        OutgoingEvent::widget_event(family, id, Some(data))
    }

    // -- effective_rate hierarchy --

    #[test]
    fn effective_rate_no_config_returns_none() {
        let emitter = test_emitter();
        let key = CoalesceKey::Subscription("on_pointer_move".into());
        assert_eq!(emitter.effective_rate(&key), None);
    }

    #[test]
    fn effective_rate_uses_default() {
        let mut emitter = test_emitter();
        emitter.set_default_rate(Some(60));
        let key = CoalesceKey::Subscription("on_pointer_move".into());
        assert_eq!(emitter.effective_rate(&key), Some(60));
    }

    #[test]
    fn effective_rate_subscription_overrides_default() {
        let mut emitter = test_emitter();
        emitter.set_default_rate(Some(60));
        emitter.set_subscription_rate("on_pointer_move", 30);
        let key = CoalesceKey::Subscription("on_pointer_move".into());
        assert_eq!(emitter.effective_rate(&key), Some(30));
    }

    #[test]
    fn effective_rate_widget_overrides_default() {
        let mut emitter = test_emitter();
        emitter.set_default_rate(Some(60));
        emitter.set_widget_rate("slider-1", 15);
        let key = CoalesceKey::Widget("slider-1".into(), "slide".into());
        assert_eq!(emitter.effective_rate(&key), Some(15));
    }

    #[test]
    fn effective_rate_widget_without_override_falls_to_default() {
        let mut emitter = test_emitter();
        emitter.set_default_rate(Some(60));
        let key = CoalesceKey::Widget("slider-1".into(), "slide".into());
        assert_eq!(emitter.effective_rate(&key), Some(60));
    }

    // -- clear_widget_rates --

    #[test]
    fn clear_widget_rates_removes_all() {
        let mut emitter = test_emitter();
        emitter.set_widget_rate("a", 10);
        emitter.set_widget_rate("b", 20);
        emitter.clear_widget_rates();
        assert!(emitter.widget_rates.is_empty());
    }

    // -- remove_subscription_rate --

    #[test]
    fn remove_subscription_rate_clears_rate() {
        let mut emitter = test_emitter();
        emitter.set_subscription_rate("on_pointer_move", 30);
        emitter.remove_subscription_rate("on_pointer_move");
        assert!(!emitter.subscription_rates.contains_key("on_pointer_move"));
    }

    // -- buffer_event --

    #[test]
    fn buffer_replace_keeps_latest() {
        let mut emitter = test_emitter();
        let key = CoalesceKey::Widget("w1".into(), "slide".into());
        let hint = CoalesceHint::Replace;

        let ev1 = make_event("slide", "w1");
        emitter.buffer_event(&key, ev1, &hint);

        let ev2 = make_event("slide", "w1");
        emitter.buffer_event(&key, ev2, &hint);

        assert_eq!(emitter.pending.len(), 1);
    }

    #[test]
    fn buffer_accumulate_sums_deltas() {
        let mut emitter = test_emitter();
        let key = CoalesceKey::Widget("ma1".into(), "scroll".into());
        let hint = CoalesceHint::Accumulate(vec!["delta_x".into(), "delta_y".into()]);

        let ev1 = make_event_with_data("scroll", "ma1", json!({"delta_x": 1.0, "delta_y": 2.0}));
        emitter.buffer_event(&key, ev1, &hint);

        let ev2 = make_event_with_data("scroll", "ma1", json!({"delta_x": 3.0, "delta_y": 4.0}));
        emitter.buffer_event(&key, ev2, &hint);

        match emitter.pending.get(&key).unwrap() {
            PendingEvent::Accumulate { totals, .. } => {
                assert!((totals["delta_x"] - 4.0).abs() < f64::EPSILON);
                assert!((totals["delta_y"] - 6.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected Accumulate variant"),
        }
    }

    // -- PendingEvent::into_event --

    #[test]
    fn accumulate_into_event_patches_totals() {
        let base = make_event_with_data(
            "canvas_scroll",
            "c1",
            json!({"delta_x": 1.0, "delta_y": 2.0, "x": 50.0}),
        );
        let mut totals = HashMap::new();
        totals.insert("delta_x".to_string(), 10.0);
        totals.insert("delta_y".to_string(), 20.0);
        let pending = PendingEvent::Accumulate {
            base,
            fields: vec!["delta_x".into(), "delta_y".into()],
            totals,
        };
        let event = pending.into_event();
        let value = event.value.unwrap();
        assert_eq!(value["delta_x"], 10.0);
        assert_eq!(value["delta_y"], 20.0);
        // Other fields preserved.
        assert_eq!(value["x"], 50.0);
    }

    // -- CoalesceHint on constructors --

    #[test]
    fn constructors_set_replace_hint() {
        let events = vec![
            OutgoingEvent::slide("s1".into(), 0.5),
            OutgoingEvent::cursor_moved("t".into(), 1.0, 2.0),
            OutgoingEvent::pointer_move(
                "m1".into(),
                1.0,
                2.0,
                "mouse",
                None,
                plushie_widget_sdk::protocol::KeyModifiers::default(),
            ),
            OutgoingEvent::resize("s1".into(), 100.0, 200.0),
            OutgoingEvent::pane_resized("p1".into(), "s0".into(), 0.5),
            OutgoingEvent::animation_frame("t".into(), 16000),
            OutgoingEvent::theme_changed("t".into(), "dark".into()),
            OutgoingEvent::finger_moved("t".into(), 1, 10.0, 20.0),
            OutgoingEvent::modifiers_changed(
                "t".into(),
                plushie_widget_sdk::protocol::KeyModifiers::default(),
            ),
            OutgoingEvent::scroll("s1".into(), 0.0, 0.0, 0.0, 0.0, 100.0, 200.0, 300.0, 400.0),
        ];
        for event in events {
            assert!(
                matches!(event.coalesce_hint(), Some(CoalesceHint::Replace)),
                "expected Replace hint on {}",
                event.family
            );
        }
    }

    #[test]
    fn constructors_set_accumulate_hint() {
        let events = vec![
            OutgoingEvent::wheel_scrolled("t".into(), 0.0, -3.0, "line"),
            OutgoingEvent::pointer_scroll(
                "m1".into(),
                0.0,
                0.0,
                0.0,
                -3.0,
                "mouse",
                plushie_widget_sdk::protocol::KeyModifiers::default(),
            ),
        ];
        for event in events {
            assert!(
                matches!(event.coalesce_hint(), Some(CoalesceHint::Accumulate(_))),
                "expected Accumulate hint on {}",
                event.family
            );
        }
    }

    #[test]
    fn constructors_set_no_hint_for_discrete() {
        let events = vec![
            OutgoingEvent::click("b1".into()),
            OutgoingEvent::input("i1".into(), "text".into()),
            OutgoingEvent::submit("f1".into(), "data".into()),
            OutgoingEvent::toggle("c1".into(), true),
            OutgoingEvent::select("p1".into(), "opt".into()),
            OutgoingEvent::paste("i1".into(), "text".into()),
            OutgoingEvent::slide_release("s1".into(), 0.5),
            OutgoingEvent::pointer_press(
                "c1".into(),
                1.0,
                2.0,
                "Left",
                "mouse",
                None,
                plushie_widget_sdk::protocol::KeyModifiers::default(),
            ),
            OutgoingEvent::pointer_release(
                "c1".into(),
                1.0,
                2.0,
                "Left",
                "mouse",
                None,
                plushie_widget_sdk::protocol::KeyModifiers::default(),
            ),
            OutgoingEvent::option_hovered("cb1".into(), "opt".into()),
            OutgoingEvent::cursor_entered("t".into()),
            OutgoingEvent::cursor_left("t".into()),
            OutgoingEvent::button_pressed("t".into(), "Left".into()),
            OutgoingEvent::button_released("t".into(), "Left".into()),
            OutgoingEvent::pointer_enter("m1".into()),
            OutgoingEvent::pointer_exit("m1".into()),
            OutgoingEvent::pane_clicked("pg1".into(), "pane_a".into()),
            OutgoingEvent::pane_focus_cycle("pg1".into(), "pane_a".into()),
            OutgoingEvent::pane_dragged("pg1".into(), "picked", "pane_a".into(), None, None, None),
        ];
        for event in events {
            assert!(
                event.coalesce_hint().is_none(),
                "expected no hint on {}",
                event.family
            );
        }
    }

    // -- Accumulate with missing fields --

    #[test]
    fn accumulate_missing_fields_graceful() {
        let hint = CoalesceHint::Accumulate(vec!["dx".into(), "dy".into()]);
        // Event only has dx, not dy.
        let ev = make_event_with_data("custom", "w1", json!({"dx": 5.0}));
        let pending = PendingEvent::from_hint(ev, &hint);
        match &pending {
            PendingEvent::Accumulate { totals, .. } => {
                assert_eq!(totals.get("dx"), Some(&5.0));
                assert_eq!(totals.get("dy"), None);
            }
            _ => panic!("expected Accumulate"),
        }
    }

    // -- Mixed hinted/unhinted events (ordering guarantee) --

    #[test]
    fn emit_immediate_flushes_pending_first() {
        let mut emitter = test_emitter();
        let key = CoalesceKey::Widget("w1".into(), "cursor_pos".into());
        let hint = CoalesceHint::Replace;

        // Buffer a coalescable event.
        let ev = make_event("cursor_pos", "w1");
        emitter.buffer_event(&key, ev, &hint);
        assert_eq!(emitter.pending.len(), 1);

        // emit_immediate should flush pending events first (even though
        // it can't actually write to stdout in tests, the flush clears
        // the pending buffer).
        let discrete = make_event("click", "w1");
        let _ = emitter.emit_immediate(discrete);

        // The pending buffer should be empty after flush.
        assert!(emitter.pending.is_empty());
    }

    // -- Strategy mismatch (widget changes hint between events) --

    #[test]
    fn buffer_event_flushes_on_strategy_mismatch() {
        let mut emitter = test_emitter();
        let key = CoalesceKey::Widget("w1".into(), "update".into());

        // Buffer a Replace event.
        let ev1 = make_event_with_data("update", "w1", json!({"x": 1.0}));
        emitter.buffer_event(&key, ev1, &CoalesceHint::Replace);
        assert_eq!(emitter.pending.len(), 1);

        // Buffer an Accumulate event with the same key (strategy mismatch).
        // The old Replace entry should be flushed and a new Accumulate started.
        let ev2 = make_event_with_data("update", "w1", json!({"dx": 5.0}));
        let acc_hint = CoalesceHint::Accumulate(vec!["dx".into()]);
        emitter.buffer_event(&key, ev2, &acc_hint);

        // Should still have one pending entry, but now it's Accumulate.
        assert_eq!(emitter.pending.len(), 1);
        assert!(matches!(
            emitter.pending.get(&key),
            Some(PendingEvent::Accumulate { .. })
        ));
    }

    // -- Accumulate with custom fields --

    #[test]
    fn accumulate_custom_fields() {
        let mut emitter = test_emitter();
        let key = CoalesceKey::Widget("w1".into(), "physics".into());
        let hint = CoalesceHint::Accumulate(vec!["impulse_x".into(), "impulse_y".into()]);

        let ev1 = make_event_with_data(
            "physics",
            "w1",
            json!({"x": 10.0, "y": 20.0, "impulse_x": 1.0, "impulse_y": 2.0}),
        );
        emitter.buffer_event(&key, ev1, &hint);

        let ev2 = make_event_with_data(
            "physics",
            "w1",
            json!({"x": 15.0, "y": 25.0, "impulse_x": 3.0, "impulse_y": 4.0}),
        );
        emitter.buffer_event(&key, ev2, &hint);

        let result = emitter.pending.remove(&key).unwrap().into_event();
        let value = result.value.unwrap();
        // Position fields: latest value wins.
        assert_eq!(value["x"], 15.0);
        assert_eq!(value["y"], 25.0);
        // Impulse fields: accumulated.
        assert_eq!(value["impulse_x"], 4.0);
        assert_eq!(value["impulse_y"], 6.0);
    }
}
