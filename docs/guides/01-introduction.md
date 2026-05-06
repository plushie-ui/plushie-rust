# Introduction

## What is Plushie?

Plushie is a native desktop GUI platform with SDKs for multiple
languages. This guide covers the Rust SDK.

When you build an app with Plushie, you get real native windows, not
Electron, not a web view. Your application is a Rust program that
owns all the state. The UI runs either embedded in the same process
or out-of-process against a separate renderer binary, and the choice
lives behind a single entry point.

## Native desktop, powered by iced

The renderer is built on [iced](https://github.com/iced-rs/iced), a
mature cross-platform GUI toolkit for Rust. It provides
GPU-accelerated rendering, a software fallback for headless
environments, and full accessibility support including keyboard
navigation and screen reader integration. Plushie wraps iced with a
widget surface, an event taxonomy, a command system, and a
subscription model, so apps are written against the Plushie API and
never touch iced directly.

## The Elm architecture in Rust

Plushie follows the Elm architecture, a pattern for building UIs
around one-way data flow. If you have used Elm, Redux, or similar
frameworks, the shape will feel familiar. An app is a type that
implements the `App` trait:

```rust
use plushie::prelude::*;

pub struct Counter;

impl App for Counter {
    type Model = i32;

    fn init() -> (Self::Model, Command) {
        (0, Command::none())
    }

    fn update(model: &mut Self::Model, event: Event) -> Command {
        if let Some(WidgetMatch::Click(id)) = event.widget_match() {
            if id == "inc" {
                *model += 1;
            }
        }
        Command::none()
    }

    fn view(model: &Self::Model, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .title("Counter")
            .child(
                column()
                    .spacing(8)
                    .padding(16)
                    .children([
                        text(&format!("Count: {model}")).into(),
                        button("inc", "Increment").into(),
                    ]),
            )
            .into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<Counter>()
}
```

Three pieces do all the work.

**Model** is your application state. Any `Send + 'static` Rust type
works: a struct, an enum, a primitive. Plushie does not impose a
schema. Whatever `init` returns becomes the initial model.

**Update** receives `&mut Self::Model` and an `Event`, mutates the
model in place, and returns a `Command`. The `&mut Model` shape is
the Rust-idiomatic way to express the state transition: ownership
stays with the runtime, the function borrows mutably for the
duration of the call, and the compiler enforces single-writer
access. Events come from user interaction, from the system, or from
your own async work. `Command::none()` means no side effect;
richer constructors run async work, open dialogs, drive windows,
or cancel in-flight tasks. Commands are how side effects stay
explicit and testable.

**View** takes `&Self::Model` and a `&mut WidgetRegistrar` and
returns a `ViewList` of top-level windows. The runtime calls `view`
after every successful update. You never mutate the UI directly;
you return a description of what the screen should look like based
on the current state. A single-window app returns a one-element
list. Returning an empty list closes every window and shuts the app
down cleanly.

The cycle looks like this:

```
event -> update -> new model -> view -> ViewList -> render
```

Events go in, state comes out, the view reflects it. There is no
two-way binding and no hidden mutation. When something looks wrong
on screen, you look at the model. When the model is wrong, you
look at the event that changed it. Every bug has a short trail.

Alongside `update`, Plushie supports
[subscriptions](../reference/subscriptions.md) for ongoing event
sources like timers, keyboard shortcuts, and window events. Your
app declares which subscriptions are active based on the current
model, and the runtime starts and stops them automatically. One-off
side effects (HTTP fetches, file dialogs, clipboard writes, window
operations) are expressed as [commands](../reference/commands.md)
returned from `update`; the runtime executes them and feeds results
back as events.

## Two rendering modes

Plushie ships two runners behind the same `plushie::run::<A>()`
entry point.

**Direct mode** (the default) embeds the renderer directly in the
application binary. The Elm loop and iced share a single process.
No wire encoding, no subprocess, no handshake. It is the smallest
runtime footprint and the right choice for most desktop apps.

**Wire mode** spawns the `plushie-renderer` binary as a subprocess
and talks to it over stdin/stdout using MessagePack. The app
process owns the Elm loop; the renderer owns iced, the GPU context,
and platform effects. Because the two sides communicate over a byte
stream, the renderer can run on a different host, in a browser
(via the WASM build), or simply in its own crash domain.

Quick decision:

- Default to direct mode.
- Switch to wire mode when you need crash isolation, remote
  rendering, or a renderer shared with a host SDK in another
  language.

The app code is the same either way. The widget builders, event
variants, commands, subscriptions, themes, and test harness behave
identically across both modes. See
[direct vs wire](../reference/direct-vs-wire.md) for the full
comparison, including feature flags, binary discovery, custom
widgets, and the WASM renderer story.

## What this guide series covers

The chapters build on each other. They start with a minimal
runnable app, then layer on the concepts you reach for as apps
grow: events and input, layout and styling, animation,
subscriptions, async work and commands, canvas drawing, custom
widgets, state management, testing, shared state, and deployment.
Each chapter ends where the next one begins.

The accompanying reference pages under `docs/reference/` are the
detailed source of truth for every type, method, and prop. Guides
explain how pieces fit together; references exhaustively list what
those pieces are. Cross-links in each chapter point at the
reference pages that correspond to the concepts being introduced.

## Conventions used in the guides

Code blocks are fenced with explicit languages: ` ```rust `,
` ```toml `, ` ```bash `. Rust imports use the grouping
`std`, then external crates, then `crate::`; snippets keep imports
minimal and show only what the example references. Most snippets
start with `use plushie::prelude::*;` which brings the widget
constructors, common types, and the `WidgetMatch` helper into
scope.

Type and field names in prose are backticked: `` the `Command`
type ``, `` the `modifiers` field ``. Qualified paths use `::`
as in source: `` `plushie::ui::button` ``, `` `Length::Fill` ``.
Widget calls are shown as a pipeline: constructor, chained setters,
final `.into()` at the container boundary.

When a snippet only works in one rendering mode the chapter calls
that out explicitly and links to
[direct vs wire](../reference/direct-vs-wire.md). When a feature
behaves identically in both modes, no annotation appears; assume
the example is mode-agnostic unless told otherwise.

## Prerequisites

You need a recent stable Rust toolchain. Install via
[rustup](https://rustup.rs/) if you do not have one:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The guide does not require nightly. A standard `cargo new` project
is enough to follow along; the next chapter walks through creating
one and adding the `plushie` dependency.

`cargo-plushie` is an optional companion CLI that builds a custom
renderer binary for wire mode, downloads stock release binaries,
and scaffolds custom widget crates. Install it only if and when a
chapter directs you to:

```bash
cargo install cargo-plushie
```

Direct-mode apps do not need `cargo-plushie` at all. See
[CLI commands](../reference/cli-commands.md) for the full set of
subcommands and what each one does.

## What's next

The next chapter walks through installing Plushie, creating a new
project, and running a minimal app end to end. Continue to
[Getting Started](02-getting-started.md).
