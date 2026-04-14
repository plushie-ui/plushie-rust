//! Async command example: a button that triggers background work.
//!
//! Demonstrates `Command::async_task` for off-thread work, pattern
//! matching on `AsyncEvent` for success/error, and loading state
//! management.
//!
//! Run with: `cargo run -p plushie --example async_fetch`

use std::time::Duration;

use plushie::prelude::*;
use serde_json::json;

struct FetchApp {
    status: Status,
    result: Option<String>,
    error: Option<String>,
}

#[derive(Debug, PartialEq)]
enum Status {
    Idle,
    Loading,
    Done,
    Error,
}

impl App for FetchApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            FetchApp {
                status: Status::Idle,
                result: None,
                error: None,
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Some(Click("fetch")) = event.widget_match() {
            model.status = Status::Loading;
            model.result = None;
            model.error = None;
            // The runner will execute this task on a background
            // thread and deliver the result as an AsyncEvent
            // once async command support is complete.
            return Command::async_task("fetch_result", || async {
                // Simulate network delay.
                std::thread::sleep(Duration::from_millis(500));
                Ok(json!(format!(
                    "Fetched at {:?}",
                    std::time::SystemTime::now()
                )))
            });
        }

        if let Some(a) = event.as_async()
            && a.tag == "fetch_result"
        {
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

        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main")
            .title("Async Fetch")
            .child(
                column()
                    .padding(24)
                    .spacing(16.0)
                    .width(Fill)
                    .child(text("Async Command Demo").id("header").size(20.0))
                    .child(button("fetch", "Fetch Data"))
                    .child(status_message(model)),
            )
            .into()
    }
}

fn status_message(model: &FetchApp) -> View {
    match model.status {
        Status::Idle => text("Press the button to start")
            .id("status")
            .color(Color::hex("#888888"))
            .into(),
        Status::Loading => text("Loading...")
            .id("status")
            .color(Color::hex("#cc8800"))
            .into(),
        Status::Done => {
            let result = model.result.as_deref().unwrap_or("");
            column()
                .spacing(4.0)
                .child(text("Result:").id("label").size(14.0))
                .child(text(result).id("result").color(Color::hex("#22aa44")))
                .into()
        }
        Status::Error => {
            let error = model.error.as_deref().unwrap_or("");
            text(error).id("error").color(Color::hex("#cc2222")).into()
        }
    }
}

fn main() -> plushie::Result {
    plushie::run::<FetchApp>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie::test::TestSession;

    #[test]
    fn starts_in_idle_state() {
        let session = TestSession::<FetchApp>::start();
        assert_eq!(session.model().status, Status::Idle);
        assert!(session.model().result.is_none());
    }

    #[test]
    fn fetch_button_exists() {
        let session = TestSession::<FetchApp>::start();
        session.assert_exists("fetch");
    }

    #[test]
    fn displays_idle_status_message() {
        let session = TestSession::<FetchApp>::start();
        session.assert_text("status", "Press the button to start");
    }

    #[test]
    fn clicking_fetch_triggers_async_work() {
        let mut session = TestSession::<FetchApp>::start();
        session.click("fetch");
        // TestSession executes async tasks synchronously, so result
        // is available immediately after the click.
        assert_eq!(session.model().status, Status::Done);
    }

    #[test]
    fn async_fetch_produces_a_result() {
        let mut session = TestSession::<FetchApp>::start();
        session.click("fetch");
        assert_eq!(session.model().status, Status::Done);
        assert!(session.model().result.is_some());
        session.assert_exists("result");
    }
}
