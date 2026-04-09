//! A todo list app demonstrating dynamic lists, scoped events,
//! text input with submit, and conditional rendering.
//!
//! Run with: `cargo run -p plushie --example todo`

use plushie::prelude::*;

struct TodoApp {
    items: Vec<TodoItem>,
    input: String,
    next_id: usize,
}

struct TodoItem {
    id: String,
    text: String,
    done: bool,
}

impl App for TodoApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            TodoApp {
                items: vec![
                    TodoItem { id: "1".into(), text: "Buy milk".into(), done: false },
                    TodoItem { id: "2".into(), text: "Write tests".into(), done: true },
                    TodoItem { id: "3".into(), text: "Ship it".into(), done: false },
                ],
                input: String::new(),
                next_id: 4,
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
                if !text.is_empty() {
                    let id = model.next_id.to_string();
                    model.next_id += 1;
                    model.items.push(TodoItem {
                        id,
                        text: text.to_string(),
                        done: false,
                    });
                    model.input.clear();
                }
            }
            Some(Toggle("done", _)) => {
                // Scoped event: scope[0] is the item's row ID,
                // scope[1] would be "list".
                if let Some(item_id) = event.scope().and_then(|s| s.first()) {
                    if let Some(item) = model.items.iter_mut().find(|i| i.id == *item_id) {
                        item.done = !item.done;
                    }
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

    fn view(model: &Self) -> View {
        let done_count = model.items.iter().filter(|i| i.done).count();

        window("main").title("Todo List").child(
            column().spacing(12.0).padding(16)
                .child(
                    text_input("new_todo", &model.input)
                        .placeholder("What needs doing?")
                        .on_submit(true),
                )
                .child(
                    column().id("list").spacing(4.0).children(
                        model.items.iter().map(|item| {
                            row().id(&item.id).spacing(8.0)
                                .child(checkbox("done", item.done).label(&item.text))
                                .child(button("delete", "X").style(Style::danger()))
                        }),
                    ),
                )
                .child(if !model.items.is_empty() {
                    View::from(
                        text(&format!(
                            "{done_count}/{} completed",
                            model.items.len(),
                        ))
                        .id("status")
                        .size(14.0),
                    )
                } else {
                    View::from(text("No todos yet").id("status").size(14.0))
                }),
        )
        .into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<TodoApp>()
}
