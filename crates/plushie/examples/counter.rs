//! Minimal counter example.
//!
//! Demonstrates button click handling, model updates from events,
//! and basic column/row layout.
//!
//! Run with: `cargo run -p plushie --example counter`

use plushie::prelude::*;

struct Counter {
    count: i32,
}

impl App for Counter {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Counter { count: 0 }, Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Click("inc")) => model.count += 1,
            Some(Click("dec")) => model.count -= 1,
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> Option<View> {
        Some(
            window("main")
                .title("Counter")
                .child(
                    column()
                        .padding(16)
                        .spacing(8.0)
                        .child(text(&format!("Count: {}", model.count)).id("count"))
                        .child(
                            row()
                                .spacing(8.0)
                                .children([button("inc", "+"), button("dec", "-")]),
                        ),
                )
                .into(),
        )
    }
}

fn main() -> plushie::Result {
    plushie::run::<Counter>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie::test::TestSession;

    #[test]
    fn starts_with_count_of_zero() {
        let session = TestSession::<Counter>::start();
        assert_eq!(session.model().count, 0);
    }

    #[test]
    fn displays_initial_count() {
        let session = TestSession::<Counter>::start();
        session.assert_text("count", "Count: 0");
    }

    #[test]
    fn increment_increases_count() {
        let mut session = TestSession::<Counter>::start();
        session.click("inc");
        assert_eq!(session.model().count, 1);
        session.assert_text("count", "Count: 1");
    }

    #[test]
    fn decrement_decreases_count() {
        let mut session = TestSession::<Counter>::start();
        session.click("dec");
        assert_eq!(session.model().count, -1);
        session.assert_text("count", "Count: -1");
    }

    #[test]
    fn multiple_clicks_accumulate() {
        let mut session = TestSession::<Counter>::start();
        session.click("inc");
        session.click("inc");
        session.click("inc");
        session.click("dec");
        assert_eq!(session.model().count, 2);
        session.assert_text("count", "Count: 2");
    }
}
