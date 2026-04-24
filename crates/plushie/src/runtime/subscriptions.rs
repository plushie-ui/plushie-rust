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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubOp {
    /// Tell the renderer to start receiving this event kind.
    Subscribe {
        /// Wire event-kind string (e.g. `"key_press"`).
        kind: String,
        /// Subscription tag supplied by the app.
        tag: String,
        /// Optional maximum delivery rate (events per second).
        max_rate: Option<u32>,
        /// Optional window filter; `None` means all windows.
        window_id: Option<String>,
    },
    /// Tell the renderer to stop receiving this event kind.
    Unsubscribe {
        /// Wire event-kind string matching the prior subscribe.
        kind: String,
        /// Subscription tag matching the prior subscribe.
        tag: String,
    },
    /// Start an SDK-side timer (SubscriptionKind::Every).
    StartTimer {
        /// Timer tag used to correlate ticks with the originating subscription.
        tag: String,
        /// Interval between ticks.
        interval: std::time::Duration,
    },
    /// Stop an SDK-side timer.
    StopTimer {
        /// Timer tag to stop.
        tag: String,
    },
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionManager {
    /// Create a new manager with no active subscriptions.
    pub fn new() -> Self {
        Self { active: Vec::new() }
    }

    /// Snapshot of the currently active subscriptions.
    ///
    /// Exposed so [`TestSession`](crate::test::TestSession) can
    /// assert subscription state without reaching into private
    /// fields. Returned slice reflects the state after the last
    /// [`sync`](Self::sync) call.
    pub fn active(&self) -> &[Subscription] {
        &self.active
    }

    /// Diff new subscriptions against active and return ops to apply.
    ///
    /// Detects three kinds of changes:
    /// - Added: key present in new but not active -> Subscribe/StartTimer
    /// - Removed: key present in active but not new -> Unsubscribe/StopTimer
    /// - Changed: same key but different max_rate or window_id ->
    ///   `UpdateSubscribe` (re-register with new parameters, matches
    ///   Elixir's in-place behaviour; no gap event window)
    ///
    /// Renderer-side subscriptions produce `Subscribe`/`Unsubscribe`
    /// ops. Timer subscriptions (`SubscriptionKind::Every`) produce
    /// `StartTimer`/`StopTimer` ops since they're handled SDK-side.
    ///
    /// `self.active` is updated to reflect the new set. The wire and
    /// direct runners today consume every op this function returns
    /// synchronously and cannot fail mid-batch, so the active snapshot
    /// cannot drift from the renderer's view. If that ever changes
    /// (e.g. a runner that defers ops while the renderer is paused),
    /// the active-set update here should move to per-op success.
    pub fn sync(&mut self, new: Vec<Subscription>) -> Vec<SubOp> {
        let mut ops = Vec::new();

        // Build lookup maps keyed by (kind, tag).
        let old_map: HashMap<(&str, &str), &Subscription> =
            self.active.iter().map(|s| (s.diff_key(), s)).collect();
        let new_map: HashMap<(&str, &str), &Subscription> =
            new.iter().map(|s| (s.diff_key(), s)).collect();

        // Removed or changed subscriptions.
        for sub in &self.active {
            let key = sub.diff_key();
            match new_map.get(&key) {
                None => {
                    // Removed entirely.
                    if matches!(sub.kind, SubscriptionKind::Every(_)) {
                        ops.push(SubOp::StopTimer {
                            tag: sub.tag.clone(),
                        });
                    } else {
                        ops.push(SubOp::Unsubscribe {
                            kind: sub.kind().to_string(),
                            tag: sub.tag.clone(),
                        });
                    }
                }
                Some(new_sub) => {
                    // Present in both. Check if parameters changed.
                    let params_changed =
                        sub.max_rate != new_sub.max_rate || sub.window_id != new_sub.window_id;

                    // For timers, also check if the interval changed.
                    let interval_changed = match (&sub.kind, &new_sub.kind) {
                        (SubscriptionKind::Every(old), SubscriptionKind::Every(new)) => old != new,
                        _ => false,
                    };

                    if interval_changed {
                        // Timer interval changed: stop old, start new.
                        ops.push(SubOp::StopTimer {
                            tag: sub.tag.clone(),
                        });
                        if let SubscriptionKind::Every(interval) = new_sub.kind {
                            ops.push(SubOp::StartTimer {
                                tag: new_sub.tag.clone(),
                                interval,
                            });
                        }
                    } else if params_changed && !matches!(sub.kind, SubscriptionKind::Every(_)) {
                        // Renderer subscription parameters changed.
                        // Matching Elixir's send_subscribe(...) pattern,
                        // re-send Subscribe with the same (kind, tag)
                        // key so the renderer updates in place. Halves
                        // wire traffic vs. Unsubscribe+Subscribe and
                        // removes the event-delivery gap during the
                        // transition.
                        ops.push(SubOp::Subscribe {
                            kind: new_sub.kind().to_string(),
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
                        kind: sub.kind().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subscription::Subscription;

    #[test]
    fn window_scoped_renderer_subscriptions_do_not_collide_with_global_ones() {
        let mut manager = SubscriptionManager::new();

        let ops = manager.sync(vec![
            Subscription::on_key_press(),
            Subscription::on_key_press().for_window("main"),
        ]);

        assert_eq!(
            ops,
            vec![
                SubOp::Subscribe {
                    kind: "on_key_press".to_string(),
                    tag: "on_key_press".to_string(),
                    max_rate: None,
                    window_id: None,
                },
                SubOp::Subscribe {
                    kind: "on_key_press".to_string(),
                    tag: "main#on_key_press".to_string(),
                    max_rate: None,
                    window_id: Some("main".to_string()),
                },
            ]
        );
    }

    #[test]
    fn on_event_subscribes_to_catch_all_kind() {
        let mut manager = SubscriptionManager::new();

        let ops = manager.sync(vec![Subscription::on_event()]);

        assert_eq!(
            ops,
            vec![SubOp::Subscribe {
                kind: "on_event".to_string(),
                tag: "on_event".to_string(),
                max_rate: None,
                window_id: None,
            }]
        );
    }

    #[test]
    fn renderer_subscription_max_rate_change_resubscribes_existing_key() {
        let mut manager = SubscriptionManager::new();

        manager.sync(vec![Subscription::on_pointer_move().max_rate(30)]);
        let ops = manager.sync(vec![Subscription::on_pointer_move().max_rate(60)]);

        assert_eq!(
            ops,
            vec![SubOp::Subscribe {
                kind: "on_pointer_move".to_string(),
                tag: "on_pointer_move".to_string(),
                max_rate: Some(60),
                window_id: None,
            }]
        );
    }

    #[test]
    fn renderer_subscription_same_key_window_change_resubscribes() {
        let mut manager = SubscriptionManager::new();
        let mut scoped = Subscription::on_pointer_move();
        scoped.window_id = Some("main".to_string());

        manager.sync(vec![Subscription::on_pointer_move()]);
        let ops = manager.sync(vec![scoped]);

        assert_eq!(
            ops,
            vec![SubOp::Subscribe {
                kind: "on_pointer_move".to_string(),
                tag: "on_pointer_move".to_string(),
                max_rate: None,
                window_id: Some("main".to_string()),
            }]
        );
    }

    #[test]
    fn timer_interval_change_restarts_timer() {
        let mut manager = SubscriptionManager::new();

        manager.sync(vec![Subscription::every(
            std::time::Duration::from_millis(16),
            "tick",
        )]);
        let ops = manager.sync(vec![Subscription::every(
            std::time::Duration::from_millis(33),
            "tick",
        )]);

        assert_eq!(
            ops,
            vec![
                SubOp::StopTimer {
                    tag: "tick".to_string(),
                },
                SubOp::StartTimer {
                    tag: "tick".to_string(),
                    interval: std::time::Duration::from_millis(33),
                },
            ]
        );
    }
}
