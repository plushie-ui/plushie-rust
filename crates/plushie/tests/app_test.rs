//! Integration tests exercising the full MVU cycle through
//! TestSession. Each test defines a small app and verifies
//! behavior through the public API.

use plushie::prelude::*;
use plushie::test::TestSession;

// ---------------------------------------------------------------------------
// Counter app
// ---------------------------------------------------------------------------

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
            Some(Click("reset")) => model.count = 0,
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main")
            .title("Counter")
            .child(
                column()
                    .spacing(8.0)
                    .padding(16)
                    .child(text(&format!("{}", model.count)).id("display"))
                    .child(row().spacing(4.0).children([
                        button("inc", "+"),
                        button("dec", "-"),
                        button("reset", "Reset"),
                    ])),
            )
            .into()
    }
}

#[test]
fn counter_starts_at_zero() {
    let session = TestSession::<Counter>::start();
    assert_eq!(session.model().count, 0);
}

#[test]
fn counter_increments_on_click() {
    let mut session = TestSession::<Counter>::start();
    session.click("inc");
    assert_eq!(session.model().count, 1);
}

#[test]
fn counter_decrements_on_click() {
    let mut session = TestSession::<Counter>::start();
    session.click("dec");
    assert_eq!(session.model().count, -1);
}

#[test]
fn counter_multiple_clicks() {
    let mut session = TestSession::<Counter>::start();
    session.click("inc");
    session.click("inc");
    session.click("inc");
    session.click("dec");
    assert_eq!(session.model().count, 2);
}

#[test]
fn counter_reset() {
    let mut session = TestSession::<Counter>::start();
    session.click("inc");
    session.click("inc");
    session.click("reset");
    assert_eq!(session.model().count, 0);
}

#[test]
fn counter_view_reflects_model() {
    let mut session = TestSession::<Counter>::start();
    session.assert_text("display", "0");
    session.click("inc");
    session.assert_text("display", "1");
    session.click("inc");
    session.assert_text("display", "2");
}

#[test]
fn counter_buttons_exist() {
    let session = TestSession::<Counter>::start();
    session.assert_exists("inc");
    session.assert_exists("dec");
    session.assert_exists("reset");
}

// ---------------------------------------------------------------------------
// Form app (text input + toggle + slider)
// ---------------------------------------------------------------------------

struct Form {
    name: String,
    agreed: bool,
    volume: f64,
}

impl App for Form {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            Form {
                name: String::new(),
                agreed: false,
                volume: 50.0,
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Input("name", text)) => model.name = text.to_string(),
            Some(Toggle("agree", on)) => model.agreed = on,
            Some(Slide("volume", vol)) => model.volume = vol,
            Some(Submit("name", text)) => {
                model.name = text.to_string();
            }
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main")
            .child(
                column()
                    .spacing(8.0)
                    .child(text_input("name", &model.name).placeholder("Your name"))
                    .child(checkbox("agree", model.agreed).label("I agree"))
                    .child(slider("volume", (0.0, 100.0), model.volume as f32))
                    .child(text(&format!("Name: {}", model.name)).id("name_display"))
                    .child(text(&format!("Agreed: {}", model.agreed)).id("agreed_display")),
            )
            .into()
    }
}

#[test]
fn form_text_input() {
    let mut session = TestSession::<Form>::start();
    session.type_text("name", "Alice");
    assert_eq!(session.model().name, "Alice");
    session.assert_text("name_display", "Name: Alice");
}

#[test]
fn form_toggle() {
    let mut session = TestSession::<Form>::start();
    assert!(!session.model().agreed);
    session.toggle("agree", true);
    assert!(session.model().agreed);
    session.assert_text("agreed_display", "Agreed: true");
}

#[test]
fn form_slider() {
    let mut session = TestSession::<Form>::start();
    session.slide("volume", 75.0);
    assert!((session.model().volume - 75.0).abs() < f64::EPSILON);
}

#[test]
fn form_submit() {
    let mut session = TestSession::<Form>::start();
    session.submit("name", "Bob");
    assert_eq!(session.model().name, "Bob");
}

// ---------------------------------------------------------------------------
// Todo app (dynamic list with scoped events)
// ---------------------------------------------------------------------------

struct TodoItem {
    id: String,
    text: String,
    done: bool,
}

struct TodoApp {
    items: Vec<TodoItem>,
    next_id: usize,
    input: String,
}

impl App for TodoApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            TodoApp {
                items: vec![
                    TodoItem {
                        id: "1".into(),
                        text: "Buy milk".into(),
                        done: false,
                    },
                    TodoItem {
                        id: "2".into(),
                        text: "Write tests".into(),
                        done: true,
                    },
                ],
                next_id: 3,
                input: String::new(),
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Input("new_todo", text)) => {
                model.input = text.to_string();
            }
            Some(Submit("new_todo", text)) => {
                let id = model.next_id.to_string();
                model.next_id += 1;
                model.items.push(TodoItem {
                    id,
                    text: text.to_string(),
                    done: false,
                });
                model.input.clear();
            }
            Some(Toggle("done", _)) => {
                if let Some(item_id) = event.scope().and_then(|s| s.first())
                    && let Some(item) = model.items.iter_mut().find(|i| i.id == *item_id)
                {
                    item.done = !item.done;
                }
            }
            Some(Click("delete")) => {
                if let Some(item_id) = event.scope().and_then(|s| s.first()) {
                    model.items.retain(|i| i.id != *item_id);
                }
            }
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main")
            .title("Todos")
            .child(
                column()
                    .spacing(8.0)
                    .padding(16)
                    .child(text_input("new_todo", &model.input).placeholder("Add todo..."))
                    .child(
                        column()
                            .id("list")
                            .spacing(4.0)
                            .children(model.items.iter().map(|item| {
                                row()
                                    .id(&item.id)
                                    .spacing(8.0)
                                    .child(checkbox("done", item.done).label(&item.text))
                                    .child(button("delete", "X"))
                            })),
                    ),
            )
            .into()
    }
}

#[test]
fn todo_starts_with_items() {
    let session = TestSession::<TodoApp>::start();
    assert_eq!(session.model().items.len(), 2);
}

#[test]
fn todo_add_item_via_submit() {
    let mut session = TestSession::<TodoApp>::start();
    session.submit("new_todo", "Learn Rust");
    assert_eq!(session.model().items.len(), 3);
    assert_eq!(session.model().items[2].text, "Learn Rust");
}

#[test]
fn todo_text_input_updates_model() {
    let mut session = TestSession::<TodoApp>::start();
    session.type_text("new_todo", "Draft");
    assert_eq!(session.model().input, "Draft");
}

// ---------------------------------------------------------------------------
// Command inspection
// ---------------------------------------------------------------------------

struct CommandApp {
    last_action: String,
}

impl App for CommandApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            CommandApp {
                last_action: String::new(),
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Click("focus_email")) => {
                model.last_action = "focus".into();
                Command::focus("email")
            }
            Some(Click("quit")) => {
                model.last_action = "quit".into();
                Command::exit()
            }
            _ => Command::none(),
        }
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main")
            .child(
                column()
                    .child(button("focus_email", "Focus Email"))
                    .child(button("quit", "Quit")),
            )
            .into()
    }
}

#[test]
fn command_app_updates_model_on_interaction() {
    let mut session = TestSession::<CommandApp>::start();
    session.click("focus_email");
    assert_eq!(session.model().last_action, "focus");
}

// ---------------------------------------------------------------------------
// Mixed event types via full Event match
// ---------------------------------------------------------------------------

struct MixedEventApp {
    clicks: usize,
    inputs: Vec<String>,
    selections: Vec<String>,
}

impl App for MixedEventApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            MixedEventApp {
                clicks: 0,
                inputs: vec![],
                selections: vec![],
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Event::Widget(w) = &event {
            match (&w.event_type, w.scoped_id.id.as_str()) {
                (EventType::Click, _) => model.clicks += 1,
                (EventType::Input, _) => {
                    if let Some(text) = w.value_string() {
                        model.inputs.push(text);
                    }
                }
                (EventType::Select, _) => {
                    if let Some(val) = w.value_string() {
                        model.selections.push(val);
                    }
                }
                _ => {}
            }
        }
        Command::none()
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main")
            .child(
                column()
                    .child(button("btn", "Click"))
                    .child(text_input("inp", ""))
                    .child(pick_list("sel", &["A", "B", "C"], None)),
            )
            .into()
    }
}

#[test]
fn mixed_events_via_full_match() {
    let mut session = TestSession::<MixedEventApp>::start();
    session.click("btn");
    session.click("btn");
    session.type_text("inp", "hello");
    session.select("sel", "B");

    assert_eq!(session.model().clicks, 2);
    assert_eq!(session.model().inputs, vec!["hello"]);
    assert_eq!(session.model().selections, vec!["B"]);
}

// ---------------------------------------------------------------------------
// View tree assertions
// ---------------------------------------------------------------------------

#[test]
fn assert_exists_finds_widget() {
    let session = TestSession::<Counter>::start();
    session.assert_exists("inc");
}

#[test]
#[should_panic(expected = "expected widget nonexistent to exist")]
fn assert_exists_panics_for_missing_widget() {
    let session = TestSession::<Counter>::start();
    session.assert_exists("nonexistent");
}

#[test]
fn assert_not_exists_for_missing_widget() {
    let session = TestSession::<Counter>::start();
    session.assert_not_exists("nonexistent");
}

#[test]
#[should_panic(expected = "expected widget inc to NOT exist")]
fn assert_not_exists_panics_for_existing_widget() {
    let session = TestSession::<Counter>::start();
    session.assert_not_exists("inc");
}

#[test]
fn prop_reads_widget_property() {
    let session = TestSession::<Form>::start();
    let placeholder = session.prop_str("name", "placeholder");
    assert_eq!(placeholder.as_deref(), Some("Your name"));
}
