//! Timer-driven clock showing the current time, updated every second.
//!
//! Demonstrates:
//! - `Subscription::every` for timer-based updates
//! - Matching `Timer("tick")` in the update function
//! - Formatting time from a system clock
//!
//! Run with: `cargo run -p plushie --example clock`

use std::time::Duration;

use plushie::prelude::*;

struct Clock {
    time: String,
}

impl Clock {
    fn current_time() -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let secs = now % 60;
        let mins = (now / 60) % 60;
        let hours = (now / 3600) % 24;

        format!("{hours:02}:{mins:02}:{secs:02}")
    }
}

impl App for Clock {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Clock { time: Self::current_time() }, Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Timer("tick")) => model.time = Self::current_time(),
            _ => {}
        }
        Command::none()
    }

    fn subscribe(_model: &Self) -> Vec<Subscription> {
        vec![Subscription::every(Duration::from_secs(1), "tick")]
    }

    fn view(model: &Self) -> View {
        window("main").title("Clock").child(
            column().spacing(16.0).padding(24).width(Fill).align_x(Align::Center)
                .child(text(&model.time).id("clock_display").size(48.0))
                .child(
                    text("Updates every second")
                        .id("subtitle")
                        .size(12.0)
                        .color(Color::hex("#888888")),
                ),
        ).into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<Clock>()
}
