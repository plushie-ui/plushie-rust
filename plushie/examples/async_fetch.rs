//! Async command example: a button that triggers background work.
//!
//! Demonstrates Command::Async for off-thread work, pattern matching
//! on AsyncEvent for success/error, and loading state management.
//!
//! The direct runner does not execute async tasks yet. Clicking
//! "Fetch Data" transitions to loading state and constructs the
//! correct Command::Async, but the async result will not arrive
//! until the runner gains async support. The AsyncEvent handling
//! code is complete and ready for when that lands.
//!
//! Run with: `cargo run -p plushie --example async_fetch`

use std::thread;
use std::time::Duration;

use plushie::prelude::*;

struct FetchApp {
    status: Status,
    result: Option<String>,
    error: Option<String>,
}

#[derive(PartialEq)]
enum Status {
    Idle,
    Loading,
    Done,
    Error,
}

/// Simulated async fetch task. In a real app this would do network
/// I/O, database queries, or other blocking work. The closure is
/// boxed as `dyn Any + Send` for the Command::Async variant.
fn fetch_task() -> Box<dyn std::any::Any + Send> {
    Box::new(move || -> Result<String, String> {
        thread::sleep(Duration::from_millis(500));
        Ok(format!("Fetched at {:?}", std::time::SystemTime::now()))
    })
}

impl App for FetchApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (FetchApp {
            status: Status::Idle,
            result: None,
            error: None,
        }, Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Click("fetch")) => {
                model.status = Status::Loading;
                model.result = None;
                model.error = None;
                // The runner will execute this task on a background
                // thread and deliver the result as an AsyncEvent
                // once async command support is complete.
                return Command::Async {
                    tag: "fetch_result".to_string(),
                    task: fetch_task(),
                };
            }
            _ => {}
        }

        if let Some(a) = event.as_async() {
            if a.tag == "fetch_result" {
                match &a.result {
                    Ok(value) => {
                        model.status = Status::Done;
                        model.result = value.as_str().map(String::from);
                    }
                    Err(reason) => {
                        model.status = Status::Error;
                        model.error = reason.as_str().map(|s| format!("Error: {s}"));
                    }
                }
            }
        }

        Command::none()
    }

    fn view(model: &Self) -> View {
        window("main").title("Async Fetch").child(
            column().padding(24).spacing(16.0).width(Fill)
                .child(text("Async Command Demo").id("header").size(20.0))
                .child(button("fetch", "Fetch Data"))
                .child(status_message(model))
        ).into()
    }
}

fn status_message(model: &FetchApp) -> View {
    match model.status {
        Status::Idle => {
            text("Press the button to start")
                .id("status")
                .color(Color::hex("#888888"))
                .into()
        }
        Status::Loading => {
            text("Loading...")
                .id("status")
                .color(Color::hex("#cc8800"))
                .into()
        }
        Status::Done => {
            let result = model.result.as_deref().unwrap_or("");
            column().spacing(4.0)
                .child(text("Result:").id("label").size(14.0))
                .child(text(result).id("result").color(Color::hex("#22aa44")))
                .into()
        }
        Status::Error => {
            let error = model.error.as_deref().unwrap_or("");
            text(error)
                .id("error")
                .color(Color::hex("#cc2222"))
                .into()
        }
    }
}

fn main() -> plushie::Result {
    plushie::run::<FetchApp>()
}
