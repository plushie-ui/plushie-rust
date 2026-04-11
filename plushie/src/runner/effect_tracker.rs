//! Effect lifecycle tracking.
//!
//! Manages in-flight effects with wire ID generation, one-per-tag
//! enforcement, deadline-based timeouts, and typed result parsing.

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
        self.next_id += 1;

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
        self.pending
            .remove(wire_id)
            .map(|e| (e.tag, e.kind))
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
    /// Returns (tag, kind) pairs for all flushed effects.
    #[allow(dead_code)]
    pub fn flush_all(&mut self) -> Vec<(String, String)> {
        self.pending
            .drain()
            .map(|(_, e)| (e.tag, e.kind))
            .collect()
    }

    /// Number of in-flight effects.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Default timeout for each effect kind.
pub fn default_timeout(kind: &str) -> Duration {
    match kind {
        "file_open" | "file_open_multiple" | "file_save" | "directory_select"
        | "directory_select_multiple" => Duration::from_secs(120),
        "clipboard_read" | "clipboard_write" | "clipboard_read_html"
        | "clipboard_write_html" | "clipboard_clear" | "clipboard_read_primary"
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
