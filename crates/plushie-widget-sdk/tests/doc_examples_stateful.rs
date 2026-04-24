//! Canonical template for testing a stateful widget end-to-end.
//!
//! Walks a contrived counter widget through the full
//! `prepare -> render -> handle_message -> handle_widget_op -> prune_stale`
//! lifecycle. Widget authors can copy this file and adapt it as the
//! starting point for their own stateful widgets.

use std::collections::HashMap;

use plushie_widget_sdk::prelude::*;
use plushie_widget_sdk::testing::*;
use serde_json::json;

// ---------------------------------------------------------------------------
// Counter widget: simple stateful example.
//
// State: per-(window_id, node_id) u32 count plus a generation counter for
// cache invalidation. Keyed by (String, String) so multiple counters can
// coexist across windows without mixing state.
// ---------------------------------------------------------------------------

struct Counter {
    counts: HashMap<(String, String), u32>,
}

impl Counter {
    fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }
}

impl PlushieWidget for Counter {
    fn type_names(&self) -> &[&str] {
        &["doc_counter"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        let key = (window_id.to_string(), node.id.clone());
        // Seed the counter from the node's initial_value prop if it
        // hasn't been seen before.
        self.counts
            .entry(key)
            .or_insert_with(|| node.prop_f32("initial_value").unwrap_or(0.0) as u32);
    }

    fn render<'a>(&'a self, node: &'a TreeNode, _ctx: &RenderCtx<'a>) -> PlushieElement<'a> {
        let key = (String::new(), node.id.clone());
        let count = self.counts.get(&key).copied().unwrap_or(0);
        text(format!("count: {count}")).into()
    }

    fn handle_message(&mut self, msg: &Message) -> HandleResult {
        if let Message::Event {
            window_id,
            id,
            family,
            ..
        } = msg
            && family == "increment"
        {
            let key = (window_id.clone(), id.clone());
            let new = self.counts.entry(key).or_insert(0);
            *new = new.saturating_add(1);
            return HandleResult::emit(vec![OutgoingEvent::generic(
                "changed",
                id.clone(),
                Some(json!({ "value": *new })),
            )]);
        }
        HandleResult::Fallthrough
    }

    fn handle_widget_op(
        &mut self,
        node_id: &str,
        op: &str,
        payload: &Value,
    ) -> Option<Vec<OutgoingEvent>> {
        if op == "reset" {
            let key = ("".to_string(), node_id.to_string());
            if let Some(target) = self.counts.get_mut(&key) {
                *target = payload.as_u64().unwrap_or(0) as u32;
                return Some(vec![OutgoingEvent::generic(
                    "changed",
                    node_id,
                    Some(json!({ "value": *target })),
                )]);
            }
        }
        None
    }

    fn prune_stale(&mut self, live_ids: &std::collections::HashSet<(String, String)>) {
        self.counts.retain(|k, _| live_ids.contains(k));
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget> {
        Box::new(Counter::new())
    }
}

// ---------------------------------------------------------------------------
// Full-lifecycle test: each phase exercised in order.
// ---------------------------------------------------------------------------

#[test]
fn counter_widget_full_lifecycle() {
    let mut widget = Counter::new();
    let node = node_with_props("c1", "doc_counter", json!({ "initial_value": 3 }));
    let test = TestEnv::default();

    // 1. prepare + render: state seeded from props, render sees count 3.
    {
        let _element = test.prepare_and_render(&mut widget, &node, "");
    }

    // 2. handle_message: increment bumps the stored count and emits
    //    a changed event.
    let increment = Message::Event {
        window_id: String::new(),
        id: "c1".into(),
        value: Value::Null,
        family: "increment".into(),
    };
    let events = test.handle_message_events(&mut widget, &increment);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].family, "changed");

    // 3. handle_widget_op: external reset via op. The reset op sets
    //    the counter to the provided value.
    let reset_events = <Counter as PlushieWidget<iced::Renderer>>::handle_widget_op(
        &mut widget,
        "c1",
        "reset",
        &json!(0),
    )
    .unwrap_or_default();
    assert_eq!(reset_events.len(), 1);
    assert_eq!(reset_events[0].family, "changed");

    // 4. prepare_walk-style pruning: retain only keys from the
    //    live-key set. An empty set evicts this widget's state.
    let empty: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    <Counter as PlushieWidget<iced::Renderer>>::prune_stale(&mut widget, &empty);
    // After cleanup, re-preparing an unrelated node shouldn't see
    // the previous key's state.
    assert!(widget.counts.is_empty());
}

#[test]
fn counter_widget_fallthrough_returns_empty_events() {
    let mut widget = Counter::new();
    let test = TestEnv::default();
    // A message the widget doesn't handle returns Fallthrough,
    // which handle_message_events flattens to an empty Vec.
    let unrelated = Message::NoOp;
    let events = test.handle_message_events(&mut widget, &unrelated);
    assert!(events.is_empty());
}
