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

#[derive(Debug, PartialEq)]
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
                    model.todos.insert(
                        0,
                        TodoItem {
                            id,
                            text: model.input.clone(),
                            done: false,
                        },
                    );
                    model.input.clear();
                    return Command::focus("app/new_todo");
                }
            }
            Some(Toggle("toggle", _)) => {
                if let Some(todo_id) = event.scope().and_then(|s| s.first())
                    && let Some(item) = model.todos.iter_mut().find(|i| i.id == *todo_id)
                {
                    item.done = !item.done;
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
        let filtered: Vec<&TodoItem> = model
            .todos
            .iter()
            .filter(|t| match model.filter {
                Filter::All => true,
                Filter::Active => !t.done,
                Filter::Done => t.done,
            })
            .collect();

        window("main")
            .title("Todos")
            .child(
                column()
                    .id("app")
                    .padding(20)
                    .spacing(12.0)
                    .width(Fill)
                    .child(text("My Todos").id("title").size(24.0))
                    .child(
                        text_input("new_todo", &model.input)
                            .placeholder("What needs doing?")
                            .on_submit(true),
                    )
                    .child(row().spacing(8.0).children([
                        button("filter_all", "All"),
                        button("filter_active", "Active"),
                        button("filter_done", "Done"),
                    ]))
                    .child(
                        column()
                            .id("list")
                            .spacing(4.0)
                            .children(filtered.iter().map(|item| todo_row(item))),
                    ),
            )
            .into()
    }
}

fn todo_row(todo: &TodoItem) -> View {
    container()
        .id(&todo.id)
        .child(
            row()
                .spacing(8.0)
                .child(checkbox("toggle", todo.done).a11y(&A11y::new().label(&todo.text)))
                .child(text(&todo.text))
                .child(button("delete", "x")),
        )
        .into()
}

fn main() -> plushie::Result {
    plushie::run::<TodoApp>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie::test::TestSession;

    #[test]
    fn starts_with_empty_todo_list() {
        let session = TestSession::<TodoApp>::start();
        assert!(session.model().todos.is_empty());
        assert!(session.model().input.is_empty());
        assert_eq!(session.model().filter, Filter::All);
    }

    #[test]
    fn input_and_filter_buttons_exist() {
        let session = TestSession::<TodoApp>::start();
        session.assert_exists("new_todo");
        session.assert_exists("filter_all");
        session.assert_exists("filter_active");
        session.assert_exists("filter_done");
    }

    #[test]
    fn typing_updates_input_model() {
        let mut session = TestSession::<TodoApp>::start();
        session.type_text("new_todo", "Buy milk");
        assert_eq!(session.model().input, "Buy milk");
    }

    #[test]
    fn submitting_adds_todo_and_clears_input() {
        let mut session = TestSession::<TodoApp>::start();
        session.type_text("new_todo", "Buy milk");
        session.submit_with("new_todo", "Buy milk");
        assert!(session.model().input.is_empty());
        assert_eq!(session.model().todos.len(), 1);
        assert_eq!(session.model().todos[0].text, "Buy milk");
        assert!(!session.model().todos[0].done);
    }

    #[test]
    fn toggling_marks_todo_complete() {
        let mut session = TestSession::<TodoApp>::start();
        session.type_text("new_todo", "Test task");
        session.submit_with("new_todo", "Test task");
        // Find the toggle checkbox inside the todo item.
        let toggle = session
            .find(Selector::id("toggle"))
            .expect("toggle checkbox not found");
        let toggle_id = toggle.id().to_string();
        session.set_toggle(&*toggle_id, true);
        assert!(session.model().todos[0].done);
    }

    #[test]
    fn deleting_removes_todo() {
        let mut session = TestSession::<TodoApp>::start();
        session.type_text("new_todo", "Ephemeral");
        session.submit_with("new_todo", "Ephemeral");
        let delete = session
            .find(Selector::id("delete"))
            .expect("delete button not found");
        let delete_id = delete.id().to_string();
        session.click(&*delete_id);
        assert!(session.model().todos.is_empty());
    }

    #[test]
    fn filter_buttons_change_active_filter() {
        let mut session = TestSession::<TodoApp>::start();
        session.click("filter_active");
        assert_eq!(session.model().filter, Filter::Active);
        session.click("filter_done");
        assert_eq!(session.model().filter, Filter::Done);
        session.click("filter_all");
        assert_eq!(session.model().filter, Filter::All);
    }
}
