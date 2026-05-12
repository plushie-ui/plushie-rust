#![cfg(not(any(feature = "direct", feature = "wire")))]

use plushie::automation::file;
use plushie::command::Command;
use plushie::event::Event;
use plushie::{App, Error, ViewList};

struct NoRunnerApp;

impl App for NoRunnerApp {
    type Model = ();

    fn init() -> (Self::Model, Command) {
        ((), Command::none())
    }

    fn update(_model: &Self::Model, _event: Event) -> (Self::Model, Command) {
        ((), Command::none())
    }

    fn view(_model: &Self::Model, _widgets: &mut plushie::widget::WidgetRegistrar) -> ViewList {
        ViewList::new()
    }
}

#[test]
fn run_reports_missing_runner_feature() {
    let err = plushie::run::<NoRunnerApp>().expect_err("run should require a runner feature");

    assert!(matches!(err, Error::NoRunnerFeature));
    assert!(err.to_string().contains("enable at least one"));
}

#[test]
fn windowed_automation_reports_missing_runner_feature() {
    let script = file::parse("app: NoRunnerApp\nbackend: windowed\n-----\nwait 1\n")
        .expect("script should parse");
    let err = plushie::automation::runner::run_with_backend::<NoRunnerApp>(&script)
        .expect_err("windowed automation should require wire");

    assert!(matches!(err, Error::NoRunnerFeature));
}
