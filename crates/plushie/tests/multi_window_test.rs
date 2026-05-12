//! Multi-window TestSession coverage.
//!
//! Drives a two-window app through TestSession and asserts per-window
//! state: the modal's view contents, per-window lifecycle events, and
//! clicks scoped via `session.window("main")` / `session.window("modal")`.

use plushie::event::WindowEventType;
use plushie::prelude::*;
use plushie::test::TestSession;

// ---------------------------------------------------------------------------
// Two-window app: main window has a button that opens the modal; the
// modal has a button that closes itself.
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct TwoWindowApp {
    modal_open: bool,
    last_main_focus: Option<WindowEventType>,
    last_modal_resize: Option<(f32, f32)>,
    last_closed: Option<String>,
}

impl App for TwoWindowApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Self::default(), Command::none())
    }

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        match event.widget_match() {
            Some(Click("open_modal")) => next.modal_open = true,
            Some(Click("close_modal")) => next.modal_open = false,
            _ => {}
        }
        if let Event::Window(w) = &event {
            match w.event_type {
                WindowEventType::Focused | WindowEventType::Unfocused if w.window_id == "main" => {
                    next.last_main_focus = Some(w.event_type);
                }
                WindowEventType::Resized if w.window_id == "modal" => {
                    next.last_modal_resize = w.width.zip(w.height);
                }
                WindowEventType::Closed => {
                    next.last_closed = Some(w.window_id.clone());
                }
                _ => {}
            }
        }
        (next, Command::none())
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        let mut views: Vec<View> = vec![
            window("main")
                .title("Main")
                .child(
                    column()
                        .child(text("home").id("home"))
                        .child(button("open_modal", "Open Modal")),
                )
                .into(),
        ];
        if model.modal_open {
            views.push(
                window("modal")
                    .title("Modal")
                    .child(
                        column()
                            .child(text("modal body").id("modal_body"))
                            .child(button("close_modal", "Close")),
                    )
                    .into(),
            );
        }
        // Wrap both windows in a synthetic container so normalize can
        // walk the tree uniformly; the renderer splits top-level
        // windows out in the real runner.
        row().children(views).into()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn main_window_click_flips_modal_state() {
    let mut s = TestSession::<TwoWindowApp>::start();
    assert!(!s.model().modal_open);

    // Click "Open Modal" scoped to the main window.
    s.window("main").click("open_modal");
    assert!(s.model().modal_open);

    // The modal now appears in the tree.
    s.assert_exists("modal_body");
}

#[test]
fn modal_window_click_closes_itself() {
    let mut s = TestSession::<TwoWindowApp>::start();
    s.window("main").click("open_modal");
    assert!(s.model().modal_open);

    s.window("modal").click("close_modal");
    assert!(!s.model().modal_open);
    s.assert_not_exists("modal_body");
}

#[test]
fn window_lifecycle_events_carry_their_window_id() {
    let mut s = TestSession::<TwoWindowApp>::start();

    s.window("main").focused();
    assert_eq!(
        s.model().last_main_focus,
        Some(WindowEventType::Focused),
        "main.focused should land as Focused"
    );

    s.window("main").unfocused();
    assert_eq!(
        s.model().last_main_focus,
        Some(WindowEventType::Unfocused),
        "main.unfocused should land as Unfocused"
    );

    // Open the modal and resize it.
    s.window("main").click("open_modal");
    s.window("modal").resized(640.0, 480.0);
    assert_eq!(s.model().last_modal_resize, Some((640.0, 480.0)));

    // Closing the modal delivers both CloseRequested and Closed; we
    // only track Closed in this fixture.
    s.window("modal").closed();
    assert_eq!(s.model().last_closed.as_deref(), Some("modal"));
}
