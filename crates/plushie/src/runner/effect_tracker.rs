//! Effect lifecycle tracking.
//!
//! Manages in-flight effects with wire ID generation, one-per-tag
//! enforcement, deadline-based timeouts, and typed result parsing.
//!
//! Both the direct and wire runners use `EffectTracker` to decouple
//! the user-facing tag from the wire ID sent to the renderer. The
//! typical flow:
//!
//! ```ignore
//! // 1. Track: generates a unique wire ID and starts the deadline.
//! let wire_id = tracker.track("save_file", "file_save", Duration::from_secs(120));
//! // Send wire_id to the renderer as the effect's ID.
//!
//! // 2. Resolve: when the renderer responds, recover the user's tag
//! //    and the effect kind for typed result parsing.
//! if let Some((tag, kind)) = tracker.resolve(&wire_id) {
//!     let result = EffectResult::parse(&kind, status, value);
//!     // Deliver Event::Effect(EffectEvent { tag, result }) to the app.
//! }
//!
//! // 3. Timeouts: periodically check for expired effects.
//! for (tag, kind) in tracker.check_timeouts() {
//!     // Deliver Event::Effect(EffectEvent { tag, result: Timeout })
//! }
//!
//! // 4. Flush: on renderer restart, cancel all pending effects.
//! for (tag, kind) in tracker.flush_all() {
//!     // Deliver Event::Effect(EffectEvent { tag, result: RendererRestarted })
//! }
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Tracks in-flight effects and manages their lifecycle.
pub struct EffectTracker {
    pending: HashMap<String, PendingEffect>,
    next_id: u64,
}

struct PendingEffect {
    /// User-provided tag for matching events in update.
    tag: String,
    /// Effect kind (e.g. "file_open") for typed result parsing.
    kind: String,
    /// Deadline after which the effect times out.
    deadline: Instant,
}

#[allow(dead_code)] // Public API surface, used by apps
impl EffectTracker {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            next_id: 0,
        }
    }

    /// Track a new effect. Returns the generated wire ID.
    ///
    /// If an effect with the same tag already exists, it is
    /// silently replaced (one-per-tag enforcement).
    pub fn track(&mut self, tag: &str, kind: &str, timeout: Duration) -> String {
        // One-per-tag: cancel any existing effect with this tag.
        self.pending.retain(|_, e| e.tag != tag);

        let wire_id = format!("ef_{}", self.next_id);
        // wrapping_add for explicit defensive clarity: 2^64 increments
        // is unreachable in practice, but this removes the debug-build
        // overflow panic concern entirely.
        self.next_id = self.next_id.wrapping_add(1);

        self.pending.insert(
            wire_id.clone(),
            PendingEffect {
                tag: tag.to_string(),
                kind: kind.to_string(),
                deadline: Instant::now() + timeout,
            },
        );

        wire_id
    }

    /// Resolve a response by wire ID. Returns (tag, kind) if found.
    pub fn resolve(&mut self, wire_id: &str) -> Option<(String, String)> {
        self.pending.remove(wire_id).map(|e| (e.tag, e.kind))
    }

    /// Check for timed-out effects. Returns (tag, kind) pairs.
    pub fn check_timeouts(&mut self) -> Vec<(String, String)> {
        let now = Instant::now();
        let expired: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, e)| now >= e.deadline)
            .map(|(id, _)| id.clone())
            .collect();

        expired
            .into_iter()
            .filter_map(|id| self.pending.remove(&id))
            .map(|e| (e.tag, e.kind))
            .collect()
    }

    /// Flush all pending effects (e.g. on renderer restart).
    /// Returns (tag, kind) pairs for all flushed effects so the
    /// caller can deliver `RendererRestarted` events to the app.
    pub fn flush_all(&mut self) -> Vec<(String, String)> {
        self.pending.drain().map(|(_, e)| (e.tag, e.kind)).collect()
    }

    /// Count of pending effects, for diagnostic logging on shutdown.
    ///
    /// Purely informational: runtime teardown paths call this before
    /// invoking [`flush_all`] so the log message carries useful
    /// context about how many effects were dropped.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Number of in-flight effects.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Default timeout for each effect kind.
///
/// File dialogs get 120s because users interact with the native OS
/// picker at their own pace. Clipboard and notification effects get
/// 5s since they complete programmatically. Unknown kinds get a
/// conservative 30s fallback.
///
/// Callers can override per-effect via the `timeout` field on
/// `RendererOp::Effect`. This function is only consulted when no
/// explicit timeout is provided.
pub fn default_timeout(kind: &str) -> Duration {
    match kind {
        "file_open"
        | "file_open_multiple"
        | "file_save"
        | "directory_select"
        | "directory_select_multiple" => Duration::from_secs(120),
        "clipboard_read"
        | "clipboard_write"
        | "clipboard_read_html"
        | "clipboard_write_html"
        | "clipboard_clear"
        | "clipboard_read_primary"
        | "clipboard_write_primary" => Duration::from_secs(5),
        "notification" => Duration::from_secs(5),
        _ => Duration::from_secs(30),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn track_and_resolve() {
        let mut tracker = EffectTracker::new();
        let wire_id = tracker.track("save_file", "file_save", Duration::from_secs(30));

        assert_eq!(wire_id, "ef_0");
        assert_eq!(tracker.len(), 1);

        let (tag, kind) = tracker.resolve(&wire_id).unwrap();
        assert_eq!(tag, "save_file");
        assert_eq!(kind, "file_save");
        assert!(tracker.is_empty());
    }

    #[test]
    fn resolve_unknown_returns_none() {
        let mut tracker = EffectTracker::new();
        assert!(tracker.resolve("ef_999").is_none());
    }

    #[test]
    fn wire_ids_increment() {
        let mut tracker = EffectTracker::new();
        let id1 = tracker.track("a", "clipboard_read", Duration::from_secs(5));
        let id2 = tracker.track("b", "clipboard_write", Duration::from_secs(5));

        assert_eq!(id1, "ef_0");
        assert_eq!(id2, "ef_1");
        assert_eq!(tracker.len(), 2);
    }

    #[test]
    fn one_per_tag_replaces_existing() {
        let mut tracker = EffectTracker::new();
        let old_id = tracker.track("clipboard", "clipboard_read", Duration::from_secs(5));
        let new_id = tracker.track("clipboard", "clipboard_write", Duration::from_secs(5));

        assert_eq!(tracker.len(), 1);

        // Old wire ID should be gone.
        assert!(tracker.resolve(&old_id).is_none());

        // New wire ID should resolve to the replacement.
        let (tag, kind) = tracker.resolve(&new_id).unwrap();
        assert_eq!(tag, "clipboard");
        assert_eq!(kind, "clipboard_write");
    }

    #[test]
    fn check_timeouts_returns_expired() {
        let mut tracker = EffectTracker::new();
        tracker.track("fast", "clipboard_read", Duration::from_millis(1));
        tracker.track("slow", "file_open", Duration::from_secs(60));

        // Wait for the fast one to expire.
        thread::sleep(Duration::from_millis(10));

        let expired = tracker.check_timeouts();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].0, "fast");
        assert_eq!(expired[0].1, "clipboard_read");

        // Slow one is still pending.
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn flush_all_clears_tracker() {
        let mut tracker = EffectTracker::new();
        tracker.track("a", "file_open", Duration::from_secs(60));
        tracker.track("b", "clipboard_read", Duration::from_secs(5));

        let flushed = tracker.flush_all();
        assert_eq!(flushed.len(), 2);
        assert!(tracker.is_empty());
    }

    #[test]
    fn default_timeouts_are_sensible() {
        assert_eq!(default_timeout("file_open"), Duration::from_secs(120));
        assert_eq!(default_timeout("clipboard_read"), Duration::from_secs(5));
        assert_eq!(default_timeout("notification"), Duration::from_secs(5));
        assert_eq!(default_timeout("unknown_effect"), Duration::from_secs(30));
    }
}
