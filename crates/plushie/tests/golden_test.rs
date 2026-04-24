//! Golden-file regression tests for canonical view trees.
//!
//! Each test drives a small app into a known state and compares the
//! resulting `tree_hash()` against a checked-in `.sha256` file under
//! `tests/golden/`. Rebuild the hashes with
//! `PLUSHIE_UPDATE_SNAPSHOTS=1 cargo test -p plushie --test golden_test`
//! after intentional tree-shape changes.

use plushie::prelude::*;
use plushie::test::{TestSession, assert_tree_hash};

const GOLDEN_DIR: &str = "tests/golden";

// ---------------------------------------------------------------------------
// Counter
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
        if let Some(Click("inc")) = event.widget_match() {
            model.count += 1;
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .title("Counter")
            .child(
                column()
                    .spacing(8.0)
                    .padding(16)
                    .child(text(&format!("{}", model.count)).id("display"))
                    .child(button("inc", "+")),
            )
            .into()
    }
}

#[test]
fn counter_after_two_inc_clicks() {
    let mut session = TestSession::<Counter>::start();
    session.click("inc");
    session.click("inc");
    session.assert_text("display", "2");
    assert_tree_hash(&session, "counter_after_two_inc", GOLDEN_DIR);
}

#[test]
fn test_session_tree_hash_matches_canonical_tree_node_hash() {
    let mut session = TestSession::<Counter>::start();
    session.click("inc");
    session.click("inc");

    let hash = session.tree_hash();
    assert_eq!(hash.len(), 64);
    assert_eq!(hash, session.tree().canonical_hash().unwrap());
}

// ---------------------------------------------------------------------------
// Todo (add + complete an item)
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
                items: Vec::new(),
                next_id: 1,
                input: String::new(),
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Input("new_todo", t)) => {
                model.input = t.to_string();
            }
            Some(Submit("new_todo", t)) => {
                let id = model.next_id.to_string();
                model.next_id += 1;
                model.items.push(TodoItem {
                    id,
                    text: t.to_string(),
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
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .title("Todo")
            .child(
                column()
                    .spacing(8.0)
                    .padding(16)
                    .child(text_input("new_todo", &model.input).placeholder("Add todo"))
                    .child(
                        column()
                            .id("list")
                            .spacing(4.0)
                            .children(model.items.iter().map(|item| {
                                row()
                                    .id(&item.id)
                                    .spacing(8.0)
                                    .child(checkbox("done", item.done).label(&item.text))
                            })),
                    ),
            )
            .into()
    }
}

#[test]
fn todo_after_add_and_complete_item() {
    let mut session = TestSession::<TodoApp>::start();
    session.submit_with("new_todo", "Buy milk");
    session.set_toggle(plushie_core::Selector::id("done"), true);
    assert_eq!(session.model().items.len(), 1);
    assert!(session.model().items[0].done);
    assert_tree_hash(&session, "todo_after_add_and_complete", GOLDEN_DIR);
}

// ---------------------------------------------------------------------------
// Form (text input + toggle + slider)
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
            Some(Input("name", t)) => model.name = t.to_string(),
            Some(Toggle("agree", on)) => model.agreed = on,
            Some(Slide("volume", v)) => model.volume = v,
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .child(
                column()
                    .spacing(8.0)
                    .child(text_input("name", &model.name).placeholder("Your name"))
                    .child(checkbox("agree", model.agreed).label("I agree"))
                    .child(slider("volume", (0.0, 100.0), model.volume as f32))
                    .child(text(&format!("Name: {}", model.name)).id("name_display")),
            )
            .into()
    }
}

#[test]
fn form_after_filling_fields() {
    let mut session = TestSession::<Form>::start();
    session.type_text("name", "Ada");
    session.set_toggle("agree", true);
    session.slide("volume", 75.0);
    assert_tree_hash(&session, "form_after_filling_fields", GOLDEN_DIR);
}

// ---------------------------------------------------------------------------
// Async fetch (post-completion view)
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone, Copy)]
enum FetchStatus {
    Idle,
    Loading,
    Done,
}

struct FetchApp {
    status: FetchStatus,
    result: Option<String>,
}

impl App for FetchApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            FetchApp {
                status: FetchStatus::Idle,
                result: None,
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Some(Click("fetch")) = event.widget_match() {
            model.status = FetchStatus::Loading;
            return Command::task("fetch_result", || async {
                // Pinned literal to keep the resulting tree
                // deterministic across runs; the real async_fetch
                // example uses a timestamp that can't be hashed.
                Ok(serde_json::json!("hello"))
            });
        }
        if let Some(a) = event.as_async()
            && a.tag == "fetch_result"
            && let Ok(value) = &a.result
        {
            model.status = FetchStatus::Done;
            model.result = value.as_str().map(String::from);
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .child(
                column()
                    .spacing(8.0)
                    .child(button("fetch", "Fetch"))
                    .child(
                        text(match model.status {
                            FetchStatus::Idle => "idle",
                            FetchStatus::Loading => "loading",
                            FetchStatus::Done => "done",
                        })
                        .id("status"),
                    )
                    .child(text(model.result.as_deref().unwrap_or("")).id("result")),
            )
            .into()
    }
}

#[test]
fn async_fetch_after_completion() {
    let mut session = TestSession::<FetchApp>::start();
    session.click("fetch");
    session.assert_text("status", "done");
    assert_tree_hash(&session, "async_fetch_after_completion", GOLDEN_DIR);
}
