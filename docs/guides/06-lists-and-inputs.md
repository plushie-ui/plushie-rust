# Lists and Inputs

The counter in [chapter 5](05-events.md) handled exactly one widget per
interaction. Real apps render many copies of the same widget from a
collection and read values back from inputs of several kinds. This
chapter builds a todo list from scratch to cover both: rendering a
dynamic list, wiring scoped IDs so per-item events route cleanly, and
reading text from `text_input`, booleans from `checkbox` and `toggler`,
and one-of-many selection from `radio`.

The finished example lives in
`crates/plushie/examples/todo.rs`. Feel free to run it alongside this
chapter:

```bash
cargo run -p plushie --example todo
```

## The model

A todo app needs a collection of items plus a scratch buffer for the
text the user is currently typing:

```rust
use plushie::prelude::*;

struct TodoApp {
    todos: Vec<TodoItem>,
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
                todos: vec![],
                input: String::new(),
                next_id: 1,
            },
            Command::none(),
        )
    }

    fn update(_model: &mut Self, _event: Event) -> Command {
        Command::none()
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main").title("Todos").into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<TodoApp>()
}
```

Each `TodoItem` carries its own stable `id` field. That ID is what we
will feed into the scoped-ID machinery so the delete button for item
`todo_3` is distinguishable from the delete button for `todo_7`.
`next_id` is a monotonic counter so new items get fresh, non-colliding
identifiers without having to scan the list.

## Rendering items

Start with a single column that maps over `model.todos`:

```rust
fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
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
                    column()
                        .id("list")
                        .spacing(4.0)
                        .children(model.todos.iter().map(todo_row)),
                ),
        )
        .into()
}

fn todo_row(todo: &TodoItem) -> View {
    container()
        .id(&todo.id)
        .child(
            row()
                .spacing(8.0)
                .child(checkbox("toggle", todo.done))
                .child(text(&todo.text))
                .child(button("delete", "x")),
        )
        .into()
}
```

`children` takes anything that implements `IntoIterator<Item = View>`,
so `model.todos.iter().map(todo_row)` works directly: the helper
returns `View` and the iterator streams them in.

Wrapping the list in a `scrollable` once it grows past the window is a
one-line change, because `scrollable` takes a single child:

```rust
scrollable()
    .id("scroll")
    .height(Length::Fill)
    .child(
        column()
            .id("list")
            .spacing(4.0)
            .children(model.todos.iter().map(todo_row)),
    )
```

For lists whose items are added, removed, or reordered at runtime,
reach for `keyed_column` instead of `column`. A plain `column` diffs
children by position, which means inserting a row at the top shifts
every subsequent row's renderer-side state (focus, text cursor, scroll
offset) by one slot. `keyed_column` uses each child's scoped ID as the
diff key, so items keep their state regardless of position:

```rust
keyed_column()
    .id("list")
    .spacing(4.0)
    .children(model.todos.iter().map(todo_row))
```

Use `keyed_column` for dynamic lists, `column` for static layouts.

## Stable IDs per row

The row helper wraps its contents in `container().id(&todo.id)`. That
wrapping is load-bearing. Without it, every row renders a button with
the same `"delete"` ID and a checkbox with the same `"toggle"` ID.
Siblings with colliding explicit IDs produce a `duplicate_id`
diagnostic, and even if they did not, the update handler would have no
way to tell which row's button was clicked.

The `container` turns the item's `id` into a scope. On the wire, the
delete button for todo `todo_3` becomes `list/todo_3/delete`, and the
checkbox becomes `list/todo_3/toggle`. The inner IDs (`toggle`,
`delete`) stay local and can repeat across every row. Auto-ID layout
widgets like the wrapping `row()` are transparent: they do not appear
in the scope chain.

When the click arrives, `event.scope()` returns the reversed ancestor
chain. The nearest named container is first:

```rust
use plushie::event::WidgetMatch::*;

match event.widget_match() {
    Some(Click("delete")) => {
        if let Some(item_id) = event.scope().and_then(|s| s.first()) {
            model.todos.retain(|t| t.id != *item_id);
        }
    }
    _ => {}
}
```

The `Click("delete")` pattern matches any delete button in the tree.
The scope head narrows it to the row that owns the event. See
[scoped IDs](../reference/scoped-ids.md) for the resolution rules and
the list of diagnostics that fire when IDs collide.

## Text input

To add items, we need a text field. `text_input` is single-line and
takes an ID and the current value; the value comes from the model so
the renderer and the app stay in sync:

```rust
text_input("new_todo", &model.input)
    .placeholder("What needs doing?")
    .on_submit(true)
```

Two event shapes come out of a `text_input`:

- `Input(id, value)` fires on every keystroke. The value is a `&str`
  borrow into the event; convert to `String` when the model needs
  ownership.
- `Submit(id, value)` fires when the user presses Enter, but only if
  `on_submit(true)` was set. The value is the committed text at the
  moment of submission.

Wire both into `update`:

```rust
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
        _ => {}
    }
    Command::none()
}
```

`Command::focus` takes the scoped form of a widget ID, slash-joined.
Here the input lives under the `app` column, so `"app/new_todo"` is
the canonical path. Returning it puts keyboard focus back on the
field so the user can type the next item without reaching for the
mouse. See [commands](../reference/commands.md) for the full list of
widget-targeted commands.

Reading `value` out of `Submit` is often unnecessary when the model
already tracks the input buffer: the buffer is authoritative, and the
submitted value is the same text. Use `_` to ignore it, or bind it
when the arm needs the exact text the renderer committed.

## Checkbox

`checkbox` is the canonical boolean toggle. It takes an ID and the
current checked state:

```rust
checkbox("toggle", todo.done)
```

It emits `Toggle(id, value)` with the new boolean state after the
user clicks. In the todo app, each checkbox lives inside a row scope,
so the arm combines the local ID match with the scope head:

```rust
Some(Toggle("toggle", _)) => {
    if let Some(todo_id) = event.scope().and_then(|s| s.first())
        && let Some(item) = model.todos.iter_mut().find(|i| i.id == *todo_id)
    {
        item.done = !item.done;
    }
}
```

The new state arrives in the event, but flipping `item.done` directly
is equivalent and avoids naming the bound value. When the model is
the source of truth for the checked state, either shape is fine; when
the checkbox drives an independent flag, bind the value:

```rust
Some(Toggle("auto_save", value)) => model.auto_save = value,
```

## Toggler and radio

`toggler` is `checkbox` rendered as a switch. The constructor and
event shape are the same: `toggler(id, is_toggled)` emits
`Toggle(id, value)`. Pick between them on visual grounds; the update
handler does not change.

```rust
toggler("dark_mode", model.dark_mode).label("Dark mode")
```

```rust
Some(Toggle("dark_mode", on)) => model.dark_mode = on,
```

`radio` is one-of-many selection. It takes an ID, the value this
radio represents, and the currently selected value (or `None`):

```rust
row()
    .spacing(12.0)
    .child(radio("all", "All", model.filter.as_deref()).label("All"))
    .child(radio("active", "Active", model.filter.as_deref()).label("Active"))
    .child(radio("done", "Done", model.filter.as_deref()).label("Done"))
```

Radios emit `Select(id, value)` where `value` is the chosen option:

```rust
Some(Select(_, value)) => {
    model.filter = Some(value.to_string());
}
```

The ID that appears in the event is the ID of the radio that was
clicked. When several radios form a group, matching on `value` alone
is usually what you want; the ID distinguishes radios across groups
that happen to share option names.

## Form state on the model

The model holds one field per piece of in-flight user input. For the
todo app that is just `input: String`, but forms grow quickly:

```rust
struct NewUserForm {
    name: String,
    email: String,
    subscribe: bool,
    plan: Option<String>,
}
```

Each field is bound to its widget's current value in `view`, and each
widget's event arm writes back to the same field:

```rust
column()
    .id("form")
    .spacing(8.0)
    .child(text_input("name", &model.form.name).placeholder("Name"))
    .child(
        text_input("email", &model.form.email)
            .placeholder("Email")
            .on_submit(true),
    )
    .child(checkbox("subscribe", model.form.subscribe).label("Subscribe"))
    .child(
        row()
            .spacing(12.0)
            .child(radio("free", "free", model.form.plan.as_deref()).label("Free"))
            .child(radio("pro", "pro", model.form.plan.as_deref()).label("Pro")),
    )
```

```rust
match event.widget_match() {
    Some(Input("name", text)) => model.form.name = text.to_string(),
    Some(Input("email", text)) => model.form.email = text.to_string(),
    Some(Toggle("subscribe", value)) => model.form.subscribe = value,
    Some(Select(_, value)) => model.form.plan = Some(value.to_string()),
    Some(Submit("email", _)) => return submit_form(model),
    _ => {}
}
```

This is the canonical "controlled input" shape: the model holds the
truth, `view` renders the current truth, and `update` reconciles user
intent back into it. The renderer's own cursor and selection are
still preserved across renders because `text_input` keys its internal
state by ID.

## What's next

The todo list renders many widgets from a collection, routes events
through scoped IDs, and reads text, booleans, and selections back
onto the model. Everything visible so far has been wrapped in a
single `column` with default spacing. The next chapter covers layout
in depth: `Length::Fill` and `Length::FillPortion`, padding and
spacing, alignment, and when to reach for `row`, `column`, `stack`,
or `grid`. Continue to [Layout](07-layout.md).
