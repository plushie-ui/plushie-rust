//! Subscription lifecycle management.
//!
//! Diffs active subscriptions against newly declared ones and returns
//! operations to apply. Used by both the direct and wire runners to
//! keep the renderer's subscription state in sync with
//! `App::subscribe()`.

use std::collections::HashMap;

use crate::subscription::{Subscription, SubscriptionKind};

/// Manages the lifecycle of app subscriptions by diffing.
pub struct SubscriptionManager {
    active: Vec<Subscription>,
}

/// An operation to apply after a subscription diff.
pub enum SubOp {
    /// Tell the renderer to start receiving this event kind.
    Subscribe {
        kind: String,
        tag: String,
        max_rate: Option<u32>,
        window_id: Option<String>,
    },
    /// Tell the renderer to stop receiving this event kind.
    Unsubscribe {
        kind: String,
        tag: String,
    },
    /// Start an SDK-side timer (SubscriptionKind::Every).
    StartTimer {
        tag: String,
        #[allow(dead_code)] // Used when timer implementation lands.
        interval: std::time::Duration,
    },
    /// Stop an SDK-side timer.
    StopTimer {
        tag: String,
    },
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self { active: Vec::new() }
    }

    /// Diff new subscriptions against active and return ops to apply.
    ///
    /// Detects three kinds of changes:
    /// - Added: key present in new but not active -> Subscribe/StartTimer
    /// - Removed: key present in active but not new -> Unsubscribe/StopTimer
    /// - Changed: same key but different max_rate or window_id ->
    ///   Unsubscribe + Subscribe (re-register with new parameters)
    ///
    /// Renderer-side subscriptions produce `Subscribe`/`Unsubscribe`
    /// ops. Timer subscriptions (`SubscriptionKind::Every`) produce
    /// `StartTimer`/`StopTimer` ops since they're handled SDK-side.
    pub fn sync(&mut self, new: Vec<Subscription>) -> Vec<SubOp> {
        let mut ops = Vec::new();

        // Build lookup maps keyed by (wire_kind, tag).
        let old_map: HashMap<(&str, &str), &Subscription> = self.active.iter()
            .map(|s| (s.diff_key(), s))
            .collect();
        let new_map: HashMap<(&str, &str), &Subscription> = new.iter()
            .map(|s| (s.diff_key(), s))
            .collect();

        // Removed or changed subscriptions.
        for sub in &self.active {
            let key = sub.diff_key();
            match new_map.get(&key) {
                None => {
                    // Removed entirely.
                    if matches!(sub.kind, SubscriptionKind::Every(_)) {
                        ops.push(SubOp::StopTimer { tag: sub.tag.clone() });
                    } else {
                        ops.push(SubOp::Unsubscribe {
                            kind: sub.wire_kind().to_string(),
                            tag: sub.tag.clone(),
                        });
                    }
                }
                Some(new_sub) => {
                    // Present in both. Check if parameters changed.
                    let params_changed = sub.max_rate != new_sub.max_rate
                        || sub.window_id != new_sub.window_id;

                    // For timers, also check if the interval changed.
                    let interval_changed = match (&sub.kind, &new_sub.kind) {
                        (SubscriptionKind::Every(old), SubscriptionKind::Every(new)) => old != new,
                        _ => false,
                    };

                    if interval_changed {
                        // Timer interval changed: stop old, start new.
                        ops.push(SubOp::StopTimer { tag: sub.tag.clone() });
                        if let SubscriptionKind::Every(interval) = new_sub.kind {
                            ops.push(SubOp::StartTimer {
                                tag: new_sub.tag.clone(),
                                interval,
                            });
                        }
                    } else if params_changed && !matches!(sub.kind, SubscriptionKind::Every(_)) {
                        // Renderer subscription parameters changed.
                        ops.push(SubOp::Unsubscribe {
                            kind: sub.wire_kind().to_string(),
                            tag: sub.tag.clone(),
                        });
                        ops.push(SubOp::Subscribe {
                            kind: new_sub.wire_kind().to_string(),
                            tag: new_sub.tag.clone(),
                            max_rate: new_sub.max_rate,
                            window_id: new_sub.window_id.clone(),
                        });
                    }
                }
            }
        }

        // Newly added subscriptions.
        for sub in &new {
            let key = sub.diff_key();
            if !old_map.contains_key(&key) {
                if let SubscriptionKind::Every(interval) = sub.kind {
                    ops.push(SubOp::StartTimer {
                        tag: sub.tag.clone(),
                        interval,
                    });
                } else {
                    ops.push(SubOp::Subscribe {
                        kind: sub.wire_kind().to_string(),
                        tag: sub.tag.clone(),
                        max_rate: sub.max_rate,
                        window_id: sub.window_id.clone(),
                    });
                }
            }
        }

        self.active = new;
        ops
    }
}
