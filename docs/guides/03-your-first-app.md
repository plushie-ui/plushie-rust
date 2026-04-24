# Your first app

In the previous chapter we set up a new Plushie project and ran
the starter binary. Now we will write one from scratch: a counter
with two buttons and a label. By the end of this chapter you will
have a working `App` implementation, a feel for the
`init` / `view` / `update` loop, and a working mental model for
widget-scoped events. We will then clone the **Plushie Pad**, the
anchor project we will thread through the rest of the guide.

## The task

The counter is the simplest useful app: two buttons labelled `+`
and `-`, a text label showing a count that starts at zero, and an
`update` that adjusts the count when either button is clicked.
That is enough surface area to touch every part of the trait.

The finished example lives at
`crates/plushie/examples/counter.rs`. Run it with
`cargo run -p plushie --example counter` if you want to see it
before walking through the code.

## The `App` trait

Every Plushie app is a type that implements `plushie::App`. The
trait has one associated type and three required methods:

```rust
use plushie::prelude::*;

struct Counter { count: i32 }

impl App for Counter {
    type Model = Self;

    fn init() -> (Self, Command) { todo!() }

    fn update(model: &mut Self, event: Event) -> Command { todo!() }

    fn view(model: &Self, widgets: &mut WidgetRegistrar) -> ViewList {
        todo!()
    }
}
```

`Model` is the state the runtime owns and hands back to each
callback. Most apps set `type Model = Self;` so the app type and
the model type coincide. `init` returns the starting model and
any startup command. `update` folds one event into the model.
`view` turns the model into a tree of widgets. For the full
reference, see [App lifecycle](../reference/app-lifecycle.md).

## The model

The counter holds a single integer. Define it next to the trait
impl:

```rust
struct Counter {
    count: i32,
}
```

`init` returns the starting value and `Command::none()`, meaning
"no startup work":

```rust
fn init() -> (Self, Command) {
    (Counter { count: 0 }, Command::none())
}
```

`Command` is the SDK's effect type. It covers async tasks,
window operations, focus moves, file dialogs, and more. The
[Commands reference](../reference/commands.md) has the full
catalogue. For now we will return `Command::none()` from every
callback.

## The view

`view` is a pure function from the model to a `ViewList`, which
is the list of top-level windows to render. The builders live in
`plushie::ui` and are re-exported from the prelude.

```rust
fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
    window("main")
        .title("Counter")
        .child(
            column()
                .padding(16)
                .spacing(8.0)
                .child(text(&format!("Count: {}", model.count)).id("count"))
                .child(
                    row()
                        .spacing(8.0)
                        .children([button("inc", "+"), button("dec", "-")]),
                ),
        )
        .into()
}
```

A few things to notice:

- `window("main")` opens the builder chain. `"main"` is the
  window's stable ID. `.title(..)` sets the title bar text.
- `column()` stacks its children vertically; `row()` stacks them
  horizontally. Both accept `.padding(..)`, `.spacing(..)`, and
  either `.child(..)` for a single view or `.children([..])` for
  an array.
- `text(..)` takes the string to display. We give the label an
  explicit ID (`.id("count")`) so tests can target it by name.
  Interactive widgets need an ID to route their events; passive
  widgets only need one when something else is going to address
  them.
- `button("inc", "+")` takes the widget ID first, then the label
  shown on the button. The ID is what comes back on the wire
  when the button is clicked.
- The final `.into()` converts the `WindowBuilder` into a
  `ViewList`. The runner accepts anything that can convert into
  one, so a single window, a `Vec<View>`, or an array all work.

The [Built-in widgets reference](../reference/built-in-widgets.md)
covers the full catalogue and their props. We will come back to
layout and styling in later chapters.

## The update

`update` receives the current model by mutable reference and
one `Event`. Mutate the model in place, return a `Command`:

```rust
fn update(model: &mut Self, event: Event) -> Command {
    match event.widget_match() {
        Some(Click("inc")) => model.count += 1,
        Some(Click("dec")) => model.count -= 1,
        _ => {}
    }
    Command::none()
}
```

`Event` is the top-level enum the runtime hands us. For
widget-scoped interactions (clicks, input, toggles, slides, and
friends) the typed helper `event.widget_match()` returns
`Option<WidgetMatch<'_>>`. The `Click("inc")` pattern unpacks
a click whose widget ID is the string `"inc"`. Everything else
falls through the `_` arm, which keeps the runtime from
logging unhandled events. The
[Events reference](../reference/events.md) lists every
`WidgetMatch` variant and the raw `Event` variants underneath.

The `use plushie::prelude::*;` line at the top of the file brings
`Click` into scope. If you prefer not to glob-import the variants,
pattern on `WidgetMatch::Click(..)` directly.

Returning `Command::none()` tells the runtime there is no side
effect to schedule. The view runs after every `update`, whether
the model changed or not, so we do not need to do anything
special to trigger a re-render.

## Running it

`main` hands control to the runner:

```rust
fn main() -> plushie::Result {
    plushie::run::<Counter>()
}
```

`plushie::run::<Counter>()` starts the Elm loop: it calls
`Counter::init`, runs the first `view` pass, shows the window,
and then loops over events, dispatching each one to `update`.
It returns when the app exits, either because of a
`Command::Exit` or because the last window closed. Direct mode
is the default, so the runner embeds the iced renderer in the
app process and draws the window itself. See
[Direct vs wire](../reference/direct-vs-wire.md) for what
changes if you flip to the wire runner later.

Save the file as `src/main.rs`, run `cargo run`, and click the
buttons. The count updates on every click.

## The Plushie Pad

The counter is a one-shot. From the next chapter on we will
work inside a larger scaffold called the **Plushie Pad**: an
experiment gallery that lives at
`plushie-demos/rust/plushie_pad/`. The pad gives us a single
window with a sidebar of experiments, a source view, a live
preview, and a rolling event log. Swapping experiments is a
single click; interacting with the preview fires real events,
which the pad routes to the current experiment and echoes into
the log.

The other SDK pads (Elixir, Gleam, Ruby, TypeScript) compile
user-typed code at runtime so you can edit an experiment inside
the pad and see the new version immediately. Rust has no
standard `eval`, and the workable alternatives (shelling out to
`cargo build` plus `dlopen`, or interpreting a declarative
format) each come with a steep cost. The Rust pad takes a
different route: every experiment is a pre-compiled file under
`src/experiments/` that ships its own source via
`include_str!`, and switching between them is just swapping
which `Experiment` the pad delegates to. Editing an experiment
means editing the file and rebuilding the pad. The teaching
value still lives where it should: in the widget builders,
event routing, and model updates inside each experiment.

Clone the demos repo next to your plushie-rust checkout:

```bash
git clone https://github.com/plushie/plushie-demos.git
cd plushie-demos/rust/plushie_pad
cargo run
```

The `Cargo.toml` uses a path dependency on
`../../../plushie-rust/crates/plushie`. If your repositories
live somewhere else, point the path at wherever your
`plushie-rust` checkout sits.

A minimal experiment looks like this:

```rust
use plushie::prelude::*;
use super::Experiment;

#[derive(Default)]
pub struct Hello;

impl Experiment for Hello {
    fn name(&self) -> &'static str { "hello" }
    fn source(&self) -> &'static str { include_str!("hello.rs") }

    fn view(&self) -> View {
        text("Hello from my experiment").into()
    }
}
```

Experiments that need to react to events implement
`update(&mut self, event: &Event) -> bool`, returning `true` when
state changed. The pad forwards every event scoped under the
`preview` container to the active experiment. IDs declared inside
a view become `preview/your_id` on the wire; inside `update`,
`event.widget_match()` hands back the local ID with no scope
prefix, so the match arms read the same as they do in a
standalone app.

The pad's `README.md` walks through adding a new experiment
(create the file, register the module, add an entry to
`build_gallery()`) and lists the files that make up the
scaffold.

## What's next

The counter taught the trait; the pad is where we will live from
here on. The next chapter,
[The development loop](04-the-development-loop.md), brings
`cargo plushie run --watch` into the picture so edits to the pad
rebuild and relaunch automatically, then walks through the
feedback loop we will use for every remaining chapter.
