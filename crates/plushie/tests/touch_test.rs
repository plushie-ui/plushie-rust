//! Multi-finger touch tests via TestSession's canvas_touch_* methods.
//!
//! Builds a minimal canvas-backed app that records every pointer event
//! the update function sees, then asserts on finger IDs, pointer kinds,
//! and the overall press/move/release sequencing for a two-finger
//! interaction.

use plushie::prelude::*;
use plushie::test::TestSession;
use plushie_core::key::PointerKind;

// ---------------------------------------------------------------------------
// App: a single canvas widget that forwards all pointer events.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Record {
    kind: &'static str, // "press" | "release" | "move"
    finger: Option<u64>,
    pointer: PointerKind,
    x: f32,
    y: f32,
}

#[derive(Default)]
struct TouchApp {
    events: Vec<Record>,
}

impl App for TouchApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Self::default(), Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Some(w) = event.as_widget() {
            match event.widget_match() {
                Some(Press("pad", p)) => model.events.push(Record {
                    kind: "press",
                    finger: p.finger,
                    pointer: p.pointer,
                    x: p.x,
                    y: p.y,
                }),
                Some(Release("pad", p)) => model.events.push(Record {
                    kind: "release",
                    finger: p.finger,
                    pointer: p.pointer,
                    x: p.x,
                    y: p.y,
                }),
                Some(Move("pad", p)) => model.events.push(Record {
                    kind: "move",
                    finger: p.finger,
                    pointer: p.pointer,
                    x: p.x,
                    y: p.y,
                }),
                _ => {
                    // Non-matching widget event; ignore.
                    let _ = w;
                }
            }
        }
        Command::none()
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main")
            .child(canvas("pad").width(Fill).height(Fill))
            .into()
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[test]
fn two_finger_touch_sequence_preserves_finger_ids_and_pointer_kind() {
    let mut s = TestSession::<TouchApp>::start();

    // Finger 1 presses at (10, 20).
    s.canvas_touch_press("pad", 10.0, 20.0, 1);
    // Finger 2 presses at (40, 60).
    s.canvas_touch_press("pad", 40.0, 60.0, 2);
    // Finger 1 drags to (15, 25).
    s.canvas_touch_move("pad", 15.0, 25.0, 1);
    // Finger 1 releases at (15, 25).
    s.canvas_touch_release("pad", 15.0, 25.0, 1);
    // Finger 2 releases at (40, 60).
    s.canvas_touch_release("pad", 40.0, 60.0, 2);

    let evs = &s.model().events;
    assert_eq!(evs.len(), 5, "expected 5 recorded events, got: {evs:#?}");

    // All must arrive as Touch pointer kind with the finger set.
    for (i, ev) in evs.iter().enumerate() {
        assert_eq!(
            ev.pointer,
            PointerKind::Touch,
            "event {i} must carry PointerKind::Touch: {ev:?}"
        );
        assert!(
            ev.finger.is_some(),
            "event {i} must carry a finger id: {ev:?}"
        );
    }

    let kinds: Vec<_> = evs.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec!["press", "press", "move", "release", "release"],
        "event ordering should follow dispatched order"
    );

    let fingers: Vec<_> = evs.iter().map(|e| e.finger.unwrap()).collect();
    assert_eq!(
        fingers,
        vec![1, 2, 1, 1, 2],
        "finger ids must match dispatched order"
    );

    // Spot-check coordinates on the finger-1 release.
    let last_f1_release = evs
        .iter()
        .rev()
        .find(|e| e.kind == "release" && e.finger == Some(1))
        .expect("finger 1 release event");
    assert!((last_f1_release.x - 15.0).abs() < f32::EPSILON);
    assert!((last_f1_release.y - 25.0).abs() < f32::EPSILON);
}
