//! To-do list with add, toggle, delete, and filter.
//!
//! Demonstrates text_input with on_submit, scoped IDs for dynamic
//! list items, scope binding in update for item-level events,
//! Command::focus for refocusing, and filter buttons with
//! conditional list rendering.
//!
//! Run with: `cargo run -p plushie --example todo`

use plushie::prelude::*;

struct TodoApp {
    todos: Vec<TodoItem>,
    input: String,
    filter: Filter,
    next_id: usize,
}

struct TodoItem {
    id: String,
    text: String,
    done: bool,
}

#[derive(PartialEq)]
enum Filter {
    All,
    Active,
    Done,
}

impl App for TodoApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            TodoApp {
                todos: vec![],
                input: String::new(),
                filter: Filter::All,
                next_id: 1,
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Input("new_todo", text)) => {
                model.input = text.to_string();
            }
            Some(Submit("new_todo", _)) => {
                if !model.input.trim().is_empty() {
                    let id = format!("todo_{}", model.next_id);
                    model.next_id += 1;
                    model.todos.insert(0, TodoItem {
                        id,
                        text: model.input.clone(),
                        done: false,
                    });
                    model.input.clear();
                    return Command::focus("app/new_todo");
                }
            }
            Some(Toggle("toggle", _)) => {
                if let Some(todo_id) = event.scope().and_then(|s| s.first()) {
                    if let Some(item) = model.todos.iter_mut().find(|i| i.id == *todo_id) {
                        item.done = !item.done;
                    }
                }
            }
            Some(Click("delete")) => {
                if let Some(todo_id) = event.scope().and_then(|s| s.first()) {
                    model.todos.retain(|i| i.id != *todo_id);
                }
            }
            Some(Click("filter_all")) => model.filter = Filter::All,
            Some(Click("filter_active")) => model.filter = Filter::Active,
            Some(Click("filter_done")) => model.filter = Filter::Done,
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        let filtered: Vec<&TodoItem> = model.todos.iter().filter(|t| match model.filter {
            Filter::All => true,
            Filter::Active => !t.done,
            Filter::Done => t.done,
        }).collect();

        window("main").title("Todos").child(
            column().id("app").padding(20).spacing(12.0).width(Fill)
                .child(text("My Todos").id("title").size(24.0))
                .child(
                    text_input("new_todo", &model.input)
                        .placeholder("What needs doing?")
                        .on_submit(true)
                )
                .child(row().spacing(8.0).children([
                    button("filter_all", "All"),
                    button("filter_active", "Active"),
                    button("filter_done", "Done"),
                ]))
                .child(
                    column().id("list").spacing(4.0).children(
                        filtered.iter().map(|item| todo_row(item))
                    )
                )
        ).into()
    }
}

fn todo_row(todo: &TodoItem) -> View {
    container().id(&todo.id).child(
        row().spacing(8.0)
            .child(checkbox("toggle", todo.done))
            .child(text(&todo.text))
            .child(button("delete", "x"))
    ).into()
}

fn main() -> plushie::Result {
    plushie::run::<TodoApp>()
}
