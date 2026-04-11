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

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main").title("Counter").child(
            column().padding(16).spacing(8.0)
                .child(text(&format!("Count: {}", model.count)).id("count"))
                .child(row().spacing(8.0).children([
                    button("inc", "+"),
                    button("dec", "-"),
                ]))
        ).into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<Counter>()
}
