//! Subscription lifecycle tests driven through `TestSession`.
//!
//! The diff algorithm lives in `runtime::subscriptions::SubscriptionManager`,
//! but every path (add, remove, tag rename, `max_rate` change,
//! `window_id` change, timer interval change, empty<->non-empty) is
//! exercised end-to-end through `TestSession::advance_subscriptions`.
//! Those integration cases cover the real SDK entry point and stand in
//! for isolated unit tests of the diff.

use std::time::Duration;

use plushie::prelude::*;
use plushie::runtime_internals::SubOp;
use plushie::test::TestSession;

// ---------------------------------------------------------------------------
// Test app: subscriptions driven by model knobs
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SubscribeApp {
    ticking: bool,
    tick_interval: Duration,
    listen_keys: bool,
    key_window: Option<String>,
    pointer_max_rate: Option<u32>,
    extra_tag: Option<&'static str>,
}

impl App for SubscribeApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Self::default(), Command::none())
    }

    fn update(_model: &mut Self, _event: Event) -> Command {
        Command::none()
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main").child(text("")).into()
    }

    fn subscribe(model: &Self) -> Vec<Subscription> {
        let mut subs = Vec::new();
        if model.ticking {
            subs.push(Subscription::every(model.tick_interval, "tick"));
        }
        if model.listen_keys {
            let sub = Subscription::on_key_press();
            let sub = match model.key_window.as_deref() {
                Some(win) => sub.for_window(win),
                None => sub,
            };
            subs.push(sub);
        }
        if let Some(rate) = model.pointer_max_rate {
            subs.push(Subscription::on_pointer_move().max_rate(rate));
        }
        if let Some(tag) = model.extra_tag {
            // Rebind on_event to a custom tag by reconstructing it.
            // We can't rename a subscription's tag via the builder,
            // so model two distinct renderer subscriptions (on_event
            // vs on_animation_frame) to cover the "rename" case.
            if tag == "custom_event" {
                subs.push(Subscription::on_event());
            } else if tag == "anim" {
                subs.push(Subscription::on_animation_frame());
            }
        }
        subs
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_subscribe(op: &SubOp, kind: &str, tag: &str) -> bool {
    matches!(op, SubOp::Subscribe { kind: k, tag: t, .. } if k == kind && t == tag)
}

fn is_unsubscribe(op: &SubOp, kind: &str, tag: &str) -> bool {
    matches!(op, SubOp::Unsubscribe { kind: k, tag: t } if k == kind && t == tag)
}

fn is_stop_timer(op: &SubOp, tag: &str) -> bool {
    matches!(op, SubOp::StopTimer { tag: t } if t == tag)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn fresh_session_has_no_active_subscriptions() {
    let session = TestSession::<SubscribeApp>::start();
    assert!(
        session.active_subscriptions().is_empty(),
        "fresh session must not have active subscriptions: {:?}",
        session.active_subscriptions()
    );
    assert!(
        session.last_subscription_ops().is_empty(),
        "fresh session must not have accumulated sub ops"
    );
}

#[test]
fn adding_a_subscription_emits_subscribe_op() {
    let mut session = TestSession::<SubscribeApp>::start();
    session.model_mut().listen_keys = true;
    session.advance_subscriptions();

    let ops = session.last_subscription_ops();
    assert_eq!(ops.len(), 1, "expected one Subscribe op, got: {ops:?}");
    assert!(
        is_subscribe(&ops[0], "on_key_press", "on_key_press"),
        "expected Subscribe(on_key_press), got: {ops:?}",
    );

    let active = session.active_subscriptions();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].kind(), "on_key_press");
}

#[test]
fn removing_a_subscription_emits_unsubscribe_op() {
    let mut session = TestSession::<SubscribeApp>::start();
    session.model_mut().listen_keys = true;
    session.advance_subscriptions();

    session.model_mut().listen_keys = false;
    session.advance_subscriptions();

    let ops = session.last_subscription_ops();
    assert_eq!(
        ops.len(),
        1,
        "expected one Unsubscribe op after removal, got: {ops:?}"
    );
    assert!(
        is_unsubscribe(&ops[0], "on_key_press", "on_key_press"),
        "expected Unsubscribe(on_key_press), got: {ops:?}"
    );
    assert!(session.active_subscriptions().is_empty());
}

#[test]
fn renaming_kind_unsubscribes_old_and_subscribes_new() {
    // "Rename" is modelled as a different kind entirely. We swap
    // on_event for on_animation_frame through the `extra_tag` knob.
    let mut session = TestSession::<SubscribeApp>::start();
    session.model_mut().extra_tag = Some("custom_event");
    session.advance_subscriptions();
    assert_eq!(session.active_subscriptions().len(), 1);

    session.model_mut().extra_tag = Some("anim");
    session.advance_subscriptions();

    let ops = session.last_subscription_ops();
    assert!(
        ops.iter()
            .any(|op| is_unsubscribe(op, "on_event", "on_event")),
        "expected Unsubscribe(on_event) in: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| is_subscribe(op, "on_animation_frame", "on_animation_frame")),
        "expected Subscribe(on_animation_frame) in: {ops:?}"
    );
}

#[test]
fn max_rate_change_produces_inplace_resubscribe() {
    let mut session = TestSession::<SubscribeApp>::start();
    session.model_mut().pointer_max_rate = Some(30);
    session.advance_subscriptions();
    assert_eq!(
        session.active_subscriptions()[0].max_rate_hint(),
        Some(30),
        "first advance should store max_rate=30"
    );

    session.model_mut().pointer_max_rate = Some(60);
    session.advance_subscriptions();

    // Matches Elixir's in-place send_subscribe: a single Subscribe op
    // with the new parameters, keyed by the same (kind, tag) pair so
    // the renderer updates in place without a gap in event delivery.
    let ops = session.last_subscription_ops();
    assert_eq!(
        ops.len(),
        1,
        "max_rate change should re-send Subscribe in place, got: {ops:?}"
    );
    assert!(matches!(
        &ops[0],
        SubOp::Subscribe {
            max_rate: Some(60),
            ..
        }
    ));
}

#[test]
fn window_id_change_produces_inplace_resubscribe() {
    let mut session = TestSession::<SubscribeApp>::start();
    session.model_mut().listen_keys = true;
    session.model_mut().key_window = Some("main".into());
    session.advance_subscriptions();
    assert_eq!(
        session.active_subscriptions()[0].window_id(),
        Some("main"),
        "first advance should record window_id=main"
    );

    session.model_mut().key_window = Some("popup".into());
    session.advance_subscriptions();

    let ops = session.last_subscription_ops();
    assert_eq!(
        ops.len(),
        1,
        "window_id change should re-send Subscribe in place, got: {ops:?}"
    );
    assert!(matches!(&ops[0], SubOp::Subscribe { window_id: Some(w), .. } if w == "popup"));
}

#[test]
fn timer_interval_change_restarts_the_timer() {
    let mut session = TestSession::<SubscribeApp>::start();
    session.model_mut().ticking = true;
    session.model_mut().tick_interval = Duration::from_millis(16);
    session.advance_subscriptions();
    assert!(matches!(
        session.last_subscription_ops().first(),
        Some(SubOp::StartTimer { .. })
    ));

    session.model_mut().tick_interval = Duration::from_millis(100);
    session.advance_subscriptions();

    let ops = session.last_subscription_ops();
    assert_eq!(
        ops.len(),
        2,
        "interval change should stop and restart the timer, got: {ops:?}"
    );
    assert!(is_stop_timer(&ops[0], "tick"), "got: {ops:?}");
    assert!(
        matches!(&ops[1], SubOp::StartTimer { tag, interval } if tag == "tick" && *interval == Duration::from_millis(100)),
        "got: {ops:?}"
    );
}

#[test]
fn empty_to_nonempty_produces_only_additions() {
    let mut session = TestSession::<SubscribeApp>::start();
    session.advance_subscriptions();
    assert!(session.last_subscription_ops().is_empty());

    session.model_mut().listen_keys = true;
    session.model_mut().pointer_max_rate = Some(30);
    session.advance_subscriptions();

    let ops = session.last_subscription_ops();
    assert_eq!(ops.len(), 2, "got: {ops:?}");
    assert!(ops.iter().all(|op| matches!(op, SubOp::Subscribe { .. })));
}

#[test]
fn nonempty_to_empty_removes_all_subscriptions() {
    let mut session = TestSession::<SubscribeApp>::start();
    session.model_mut().ticking = true;
    session.model_mut().tick_interval = Duration::from_millis(16);
    session.model_mut().listen_keys = true;
    session.advance_subscriptions();
    assert_eq!(session.active_subscriptions().len(), 2);

    session.model_mut().ticking = false;
    session.model_mut().listen_keys = false;
    session.advance_subscriptions();

    let ops = session.last_subscription_ops();
    assert_eq!(ops.len(), 2, "got: {ops:?}");
    assert!(ops.iter().any(|op| is_stop_timer(op, "tick")));
    assert!(
        ops.iter()
            .any(|op| is_unsubscribe(op, "on_key_press", "on_key_press"))
    );
    assert!(session.active_subscriptions().is_empty());
}
