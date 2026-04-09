# plushie

Desktop GUI framework for Rust. Build native apps with the Elm architecture.

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

    fn view(model: &Self) -> View {
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

## Features

- **Elm architecture**: init/update/view with typed events and commands
- **38 built-in widgets** with theming and style presets
- **Canvas engine** with interactive elements, hit testing, focus management
- **Two rendering modes**:
  - **Direct** (default): in-process rendering, no subprocess
  - **Wire**: separate renderer binary via stdin/stdout protocol
- **Same API for both modes**: write once, choose at runtime
- **Multi-language**: same widget system powers Elixir, TypeScript, Python, Ruby, and Gleam SDKs

## Installation

```toml
[dependencies]
plushie = "0.6"
```

Direct mode (default) embeds the renderer. For wire-only (lighter build):

```toml
[dependencies]
plushie = { version = "0.6", default-features = false, features = ["wire"] }
```

## Testing

Use `TestSession` for headless testing without a display:

```rust
use plushie::test::TestSession;

let mut session = TestSession::<Counter>::start();
session.click("inc");
session.click("inc");
assert_eq!(session.model().count, 2);
session.assert_text("display", "2");
```

## Crate ecosystem

| Crate | Audience | Role |
|---|---|---|
| `plushie` | App developers | Rust SDK (this crate) |
| `plushie-widget-sdk` | Widget authors | PlushieWidget trait, CanvasEngine |
| `plushie-renderer-lib` | Renderer builders | Shared renderer logic |
| `plushie-renderer` | All SDKs | Default renderer binary |

## License

MIT OR Apache-2.0
