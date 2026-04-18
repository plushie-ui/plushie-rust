# plushie

Build native desktop apps in Rust. **Pre-1.0**

Write your entire application in Rust (state, events, UI) and get
native windows on Linux, macOS, and Windows. The
[renderer](https://github.com/plushie-ui/plushie-rust) is built on
[iced](https://github.com/iced-rs/iced) and can run in-process (no
subprocess needed) or as a separate binary over stdin/stdout.

SDKs are also available for
[Elixir](https://github.com/plushie-ui/plushie-elixir),
[Gleam](https://github.com/plushie-ui/plushie-gleam),
[Python](https://github.com/plushie-ui/plushie-python),
[Ruby](https://github.com/plushie-ui/plushie-ruby), and
[TypeScript](https://github.com/plushie-ui/plushie-typescript).

## Quick start

```rust
use plushie::prelude::*;

struct Counter { count: i32 }

impl App for Counter {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Counter { count: 0 }, Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Click("inc")) => model.count += 1,
            Some(Click("dec")) => model.count -= 1,
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {
        window("main").title("Counter").child(
            column().spacing(8.0).padding(16)
                .child(text(&format!("Count: {}", model.count)).size(24.0))
                .child(row().spacing(8.0).children([
                    button("inc", "+").style(Style::primary()),
                    button("dec", "-").style(Style::danger()),
                ]))
        ).into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<Counter>()
}
```

```toml
[dependencies]
plushie = "0.6"
```

Direct mode (default) embeds the renderer. For wire-only (lighter build):

```toml
[dependencies]
plushie = { version = "0.6", default-features = false, features = ["wire"] }
```

Run the [examples](examples/) with `cargo run -p plushie --example counter`.

## How it works

Your Rust application and the renderer share a process by default
(direct mode). The SDK builds UI trees and handles events; the
renderer draws native windows and captures input.

The SDK diffs each new tree against the previous one and sends only
the changes. In wire mode, the renderer runs as a separate process
and communication happens over stdin/stdout, using the same protocol
that powers the Elixir, Gleam, Python, Ruby, and TypeScript SDKs.

## Features

- **Elm architecture** - init, update, view with typed events
  and commands
- **Built-in widgets** - layout, input, display, and interactive
  widgets out of the box
- **Canvas** - shapes, paths, gradients, transforms, and
  interactive elements for custom 2D drawing
- **Themes** - dark, light, nord, catppuccin, tokyo night, and
  more, with custom palettes and per-widget style overrides
- **Animation** - renderer-side transitions, springs, and
  sequences with no wire traffic per frame
- **Multi-window** - declare windows in your view; the framework
  manages the rest
- **Platform effects** - native file dialogs, clipboard, OS
  notifications
- **Accessibility** - keyboard navigation, screen readers, and
  focus management via [AccessKit](https://accesskit.dev)
- **Custom widgets** - compose existing widgets, draw on the
  canvas, or implement `PlushieWidget` in Rust
- **Two rendering modes** - direct (in-process, default) or wire
  (subprocess binary via stdin/stdout)

## Testing

Use `TestSession` for headless testing without a display:

```rust
use plushie::test::TestSession;

let mut session = TestSession::<Counter>::start();
session.click("inc");
session.click("inc");
assert_eq!(session.model().count, 2);
session.assert_text("count", "2");
```

## Status

Pre-1.0. The core works (built-in widgets, event system, themes,
multi-window, testing, accessibility) but the API is still evolving.
Pin to an exact version and read the
[CHANGELOG](../../CHANGELOG.md) when upgrading.

## License

MIT OR Apache-2.0
