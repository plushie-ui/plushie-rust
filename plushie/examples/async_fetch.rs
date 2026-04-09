//! Async data fetching example.
//!
//! Demonstrates the Command::async_task pattern for running
//! background work and handling results via AsyncEvent.
//!
//! This example simulates an HTTP fetch. In a real app, you'd
//! use reqwest or similar inside the async block.
//!
//! Run with: `cargo run -p plushie --example async_fetch`

use plushie::prelude::*;

struct FetchApp {
    status: Status,
    data: Option<String>,
    error: Option<String>,
}

enum Status {
    Idle,
    Loading,
    Done,
}

impl App for FetchApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (FetchApp {
            status: Status::Idle,
            data: None,
            error: None,
        }, Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        // Widget events (button clicks)
        match event.widget_match() {
            Some(Click("fetch")) => {
                model.status = Status::Loading;
                model.error = None;
                // In a real app, this would be an HTTP request.
                // The async block returns a Result that arrives
                // as an AsyncEvent.
                return Command::none(); // async_task needs tokio runtime
            }
            Some(Click("clear")) => {
                model.status = Status::Idle;
                model.data = None;
                model.error = None;
            }
            _ => {}
        }

        // Async results
        if let Some(a) = event.as_async() {
            if a.tag == "fetch" {
                match &a.result {
                    Ok(data) => {
                        model.status = Status::Done;
                        model.data = data.as_str().map(String::from);
                    }
                    Err(err) => {
                        model.status = Status::Done;
                        model.error = err.as_str().map(String::from);
                    }
                }
            }
        }

        Command::none()
    }

    fn view(model: &Self) -> View {
        let status_text = match model.status {
            Status::Idle => "Click Fetch to load data",
            Status::Loading => "Loading...",
            Status::Done => "Complete",
        };

        let mut col = column().spacing(12.0).padding(20)
            .child(text("Async Fetch Demo").size(24.0))
            .child(text(status_text).id("status"))
            .child(row().spacing(8.0).children([
                button("fetch", "Fetch Data").style(Style::primary()),
                button("clear", "Clear"),
            ]));

        if let Some(data) = &model.data {
            col = col.child(
                container().padding(12).style(Style::custom()
                    .background(Color::hex("#f0f0f0"))
                    .border(Border::new().radius(4.0)))
                .child(text(data).id("data"))
            );
        }

        if let Some(error) = &model.error {
            col = col.child(
                text(&format!("Error: {error}")).id("error")
                    .color(Color::red())
            );
        }

        window("main").title("Async Fetch").child(col).into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<FetchApp>()
}
