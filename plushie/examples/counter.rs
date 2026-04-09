//! A simple counter app demonstrating the plushie SDK.
//!
//! Two buttons increment and decrement a counter. The count is
//! displayed as text. This is the "Hello World" of plushie apps.
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

    fn view(model: &Self) -> View {
        window("main").title("Counter").child(
            column().spacing(8.0).padding(16)
                .child(text(&format!("Count: {}", model.count)).size(24.0))
                .child(row().spacing(8.0).children([
                    button("inc", "+").style(Style::primary()),
                    button("dec", "-").style(Style::danger()),
                ]))
        ).into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<Counter>()
}
