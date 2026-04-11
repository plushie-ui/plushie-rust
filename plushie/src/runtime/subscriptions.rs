//! Subscription lifecycle management.
//!
//! Diffs active subscriptions against newly declared ones and returns
//! operations to apply. Used by both the direct and wire runners to
//! keep the renderer's subscription state in sync with
//! `App::subscribe()`.

use std::collections::HashSet;

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
    /// Renderer-side subscriptions produce `Subscribe`/`Unsubscribe`
    /// ops. Timer subscriptions (`SubscriptionKind::Every`) produce
    /// `StartTimer`/`StopTimer` ops since they're handled SDK-side.
    pub fn sync(&mut self, new: Vec<Subscription>) -> Vec<SubOp> {
        let mut ops = Vec::new();

        let old_keys: HashSet<(&str, &str)> = self.active.iter()
            .map(|s| s.diff_key())
            .collect();
        let new_keys: HashSet<(&str, &str)> = new.iter()
            .map(|s| s.diff_key())
            .collect();

        // Stop/unsubscribe removed subscriptions.
        for sub in &self.active {
            let key = sub.diff_key();
            if !new_keys.contains(&key) {
                if matches!(sub.kind, SubscriptionKind::Every(_)) {
                    ops.push(SubOp::StopTimer { tag: sub.tag.clone() });
                } else {
                    ops.push(SubOp::Unsubscribe {
                        kind: sub.wire_kind().to_string(),
                        tag: sub.tag.clone(),
                    });
                }
            }
        }

        // Start/subscribe new subscriptions.
        for sub in &new {
            let key = sub.diff_key();
            if !old_keys.contains(&key) {
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
