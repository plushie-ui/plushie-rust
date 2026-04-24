# Styling

Layout decides where things sit. Styling decides how they look. Plushie
has a layered styling system: themes set the overall palette, per-widget
styles override individual elements, and type modules like
`plushie::types::Border`, `plushie::types::Shadow`, and
`plushie::types::Gradient` handle the details.

This chapter walks the parts you reach for most often. The full palette,
every shade override key, and every field on `StyleMap` live in the
[themes and styling reference](../reference/themes-and-styling.md).

## Themes

Every window has a theme that drives the palette for its widget subtree.
`window(..).theme(..)` takes anything that implements `Into<Theme>`, and
`Theme` itself implements `From<&str>`, so a bare name is the short
path:

```rust
use plushie::prelude::*;

fn view(_model: &Counter, _widgets: &mut WidgetRegistrar) -> ViewList {
    window("main")
        .title("Counter")
        .theme("dark")
        .child(column().padding(16).children([
            text("Hello").into(),
        ]))
        .into()
}
```

Plushie ships with a spread of built-in themes. Popular options:
`"light"`, `"dark"`, `"nord"`, `"dracula"`, `"catppuccin_mocha"`,
`"tokyo_night"`, `"gruvbox_dark"`, `"solarized_dark"`, `"kanagawa_wave"`.
The full list lives on `Theme::builtin_names()`, and any of them can be
passed as a bare string.

Pass `"system"` to follow the operating system's light/dark preference:

```rust
window("main").title("Counter").theme("system")
```

If you prefer the typed form, `Theme::Named("dark".into())`,
`Theme::System`, and `Theme::from("dark")` are all equivalent to the
string path. `"dark".into()` works wherever a `Theme` is expected.

Try a handful on the counter's window to see the whole UI adapt:
buttons, text inputs, sliders, and scrollbars all respond.

## Application-wide default

`App::settings` returns a `Settings` value with an optional `theme`
field. Setting it there makes every window inherit the theme unless the
window overrides it:

```rust
use plushie::prelude::*;
use plushie_core::settings::Settings;
use plushie::types::Theme;

impl App for Counter {
    type Model = Self;

    fn settings() -> Settings {
        Settings {
            theme: Some(Theme::Named("dark".into())),
            default_font: Some("Inter".into()),
            default_text_size: Some(14.0),
            ..Settings::default()
        }
    }

    // init, update, view ...
}
```

Per-window overrides win over the application setting, which wins over
the renderer's default.

## Subtree theming

The `themer` widget switches the theme for a single subtree without
touching its siblings:

```rust
use plushie::prelude::*;

window("main")
    .title("App")
    .theme("light")
    .child(column().children([
        text("Light text").into(),
        themer("sidebar")
            .theme("dark")
            .child(
                container()
                    .id("sidebar-body")
                    .padding(12)
                    .child(text("Dark section")),
            )
            .into(),
    ]))
```

This is the clean way to drop a dark sidebar into a light app, give a
preview pane a different palette, or scope a brand theme to one section.
No prop threading needed: `themer` owns the theme context for everything
underneath.

## Custom themes

`Theme::custom(name)` builds a `CustomTheme` that starts from a base
built-in theme and overrides individual palette slots. Every builder
method is a thin wrapper over the same internal `color(key, hex)`
helper, so every slot in the generated palette is reachable:

```rust
use plushie::types::Theme;

let brand = Theme::custom("Brand")
    .base("dark")
    .background("#1a1a2e")
    .text("#e0e0e8")
    .primary("#3b82f6")
    .danger("#ef4444");

window("main").theme(brand)
```

For fine-grained control, each semantic family exposes three shades and
matching text variants: `primary_base`, `primary_weak`, `primary_strong`,
`primary_base_text`, and so on, with parallel families for `secondary`,
`success`, `warning`, and `danger`. The background family has a longer
ramp (`background_weakest` through `background_strongest`). See the
[styling reference](../reference/themes-and-styling.md) for the full
list of shade keys.

When `.base(..)` is omitted, the custom theme seeds from `"dark"`. Any
slot you do not set falls through to the base theme.

## Color constructors

`Color` is a thin wrapper around a canonical hex string. There are
three ways to build one, and most setters accept any of them through
`impl Into<Color>`:

```rust
use plushie::types::Color;

let by_hex = Color::hex("#3b82f6");
let by_short = Color::hex("#f0f");            // expands to #ff00ff
let translucent = Color::hex("#3b82f680");    // #rrggbbaa
let by_rgb = Color::rgb(0.23, 0.51, 0.96);
let by_rgba = Color::rgba(1.0, 0.0, 0.0, 0.5);
let named = Color::cornflowerblue();
```

`Color::hex` panics on invalid input; `Color::try_hex` is the
fallible variant. Channels passed to `rgb` / `rgba` are clamped to
`0.0..=1.0`. Every CSS named color ships as a zero-argument constructor
(`Color::white()`, `Color::rebeccapurple()`, `Color::transparent()`),
and both British and American spellings resolve to the same value
(`Color::darkgray() == Color::darkgrey()`).

Because `Color: From<&str>`, setters that take `impl Into<Color>`
accept hex literals directly:

```rust
container().id("card").color("#3b82f6")
```

## Backgrounds

`Background` is a union of `Color` and `Gradient`. The
`.background(..)` setter takes anything convertible to `Background`,
which includes `Color`, `Gradient`, and string hex literals:

```rust
use plushie::prelude::*;
use plushie::types::{Color, Gradient};

container()
    .id("card")
    .background(Color::hex("#0f172a"))
    .padding(16)
    .child(text("Flat background"));

container()
    .id("hero")
    .background(
        Gradient::linear_from_angle(
            135.0,
            vec![
                (0.0, Color::hex("#667eea")),
                (1.0, Color::hex("#764ba2")),
            ],
        ),
    )
    .padding(24)
    .child(text("Gradient background"));
```

`Gradient::linear((x0, y0), (x1, y1), stops)` takes explicit unit-square
endpoints; `Gradient::linear_from_angle(deg, stops)` takes an angle
instead (0 degrees east, 90 degrees south). Stops are
`(offset, Color)` tuples where `offset` runs from `0.0` to `1.0`.

## Borders

`Border` describes a widget's outline: color, width, and corner radius.

```rust
use plushie::types::{Border, Color};

let card = Border::new()
    .color(Color::hex("#e5e7eb"))
    .width(1.0)
    .radius(8.0);

let tab = Border::new()
    .color("#d4d4d8")
    .width(1.0)
    .radius_corners(8.0, 8.0, 0.0, 0.0); // rounded top, square bottom
```

`radius_corners` takes the four corners in this order: top-left,
top-right, bottom-right, bottom-left. Passing a single value to
`.radius(..)` sets all four uniformly.

`container` exposes a direct `.border(..)` setter for the common case:

```rust
container()
    .id("panel")
    .border(Border::new().color("#333").width(1.0).radius(6.0))
    .padding(12)
    .child(text("Bordered panel"))
```

## Shadows

`Shadow` is a drop shadow: color, offset, and blur radius.

```rust
use plushie::types::{Color, Shadow};

let soft = Shadow::new()
    .color(Color::hex("#0000001a"))
    .offset(0.0, 4.0)
    .blur_radius(8.0);

container()
    .id("card")
    .background(Color::white())
    .border(Border::new().color("#e5e7eb").width(1.0).radius(8.0))
    .shadow(soft)
    .padding(16)
    .child(text("Elevated card"))
```

Offsets are logical pixels: positive `y` pushes the shadow down.
`blur_radius(0.0)` produces a sharp offset shadow.

## Named style presets

`Style` drives the appearance of a single widget. It is an enum with two
variants: a named preset and a fully custom `StyleMap`. Widgets that
accept styling take `impl Into<Style>`, and `Style: From<&str>`, so the
preset-by-name path is short:

```rust
use plushie::prelude::*;
use plushie::types::Style;

button("save", "Save").style(Style::primary());
button("cancel", "Cancel").style("text");
button("delete", "Delete").style("danger");
container().id("card").style(Style::rounded_box());
```

Which presets each widget honors varies. A `button` honors `primary`,
`secondary`, `success`, `danger`, `warning`, and `text`. A `container`
honors `rounded_box`, `bordered_box`, and `transparent`. Most widgets
also accept `default_style`, `dark`, or `weak` where they make sense.
See the reference page for the authoritative table.

## Per-widget styling with StyleMap

`StyleMap` is the fully custom path: every field is optional, and the
setters build it up one step at a time. Hand the result to `.style(..)`
and `From<StyleMap> for Style` wraps it in `Style::Custom` for you:

```rust
use plushie::prelude::*;
use plushie::types::{Border, Color, Shadow, StyleMap};

let save_style = StyleMap::new()
    .background(Color::hex("#3b82f6"))
    .text_color(Color::white())
    .border(Border::new().color("#2563eb").width(1.0).radius(6.0))
    .shadow(Shadow::new().color("#0000001a").blur_radius(4.0))
    .hovered(|s| s.background(Color::hex("#2563eb")))
    .pressed(|s| s.background(Color::hex("#1d4ed8")))
    .disabled(|s| {
        s.background(Color::hex("#9ca3af"))
            .text_color(Color::hex("#6b7280"))
    });

button("save", "Save").style(save_style)
```

The base fields are `background`, `text_color`, `border`, and `shadow`.
The state overrides are `hovered`, `pressed`, `disabled`, and `focused`,
each taking a closure that receives a `StatusOverride` with the same
four fields. Only the fields a closure actually sets participate in the
override; everything else inherits from the base.

To extend a named preset instead of starting from nothing, seed with
`.base(..)`:

```rust
let accent = StyleMap::new()
    .base("primary")
    .hovered(|s| s.background(Color::hex("#2563eb")));

button("save", "Save").style(accent)
```

## Fonts

`Font` pairs a family name with optional weight, style, and stretch
metadata. Widgets that render text accept it through `.font(..)`:

```rust
use plushie::prelude::*;
use plushie::types::{Font, FontStyle, FontWeight};

text("Plushie Pad")
    .font(
        Font::new()
            .family("Inter")
            .weight(FontWeight::Bold)
            .style(FontStyle::Italic),
    );

text("fn main() {}").font(Font::monospace());
```

Two shorthand forms exist for common cases:

- `Font::new()` with no modifiers is the system default proportional
  font.
- `Font::monospace()` selects the system monospace font.

`FontWeight` covers the CSS numeric weights from `Thin` (100) through
`Black` (900), with `Normal` (400) as the default. `FontStyle` is
`Normal`, `Italic`, or `Oblique`. `FontStretch` runs from
`UltraCondensed` to `UltraExpanded`.

Application-wide font defaults live on `Settings`:

```rust
fn settings() -> Settings {
    Settings {
        default_font: Some("Inter".into()),
        default_text_size: Some(14.0),
        fonts: vec!["./fonts/Inter.ttf".into()],
        ..Settings::default()
    }
}
```

Files listed in `fonts` are loaded at startup and become available by
family name to any widget's `.font(..)` setter.

## Bringing it together

Pull the pieces together on the counter from
[chapter 3](03-your-first-app.md): dark theme, a primary save button, a
bordered card wrapping the content, a subtle shadow, and a danger
variant on the reset button.

```rust
use plushie::prelude::*;
use plushie::types::{Border, Color, Shadow, Style};

fn view(model: &Counter, _widgets: &mut WidgetRegistrar) -> ViewList {
    let card_border = Border::new().color("#2a2a3e").width(1.0).radius(8.0);
    let card_shadow = Shadow::new()
        .color(Color::hex("#0000004d"))
        .offset(0.0, 2.0)
        .blur_radius(6.0);

    window("main")
        .title("Counter")
        .theme("dark")
        .child(
            container()
                .id("card")
                .background("#1a1a2e")
                .border(card_border)
                .shadow(card_shadow)
                .padding(24)
                .child(
                    column()
                        .spacing(12.0)
                        .child(
                            text(&format!("Count: {}", model.count))
                                .color("#e0e0e8"),
                        )
                        .child(
                            row().spacing(8.0).children([
                                button("inc", "+").style(Style::primary()),
                                button("dec", "-").style("secondary"),
                                button("reset", "Reset").style(Style::danger()),
                            ]),
                        ),
                ),
        )
        .into()
}
```

A dark theme, a framed card, and three distinct button roles. Small
moves, large visual payoff. The same pattern scales to a todo list: a
`container` per row, a `StyleMap` for the row's hover and focus states,
and a consistent palette driven by one theme at the window level.

## Try it

- Swap the theme: try `"nord"`, `"catppuccin_mocha"`,
  `"tokyo_night_storm"`. Watch every widget follow.
- Wrap the counter in a `themer("preview").theme("light")` and keep the
  rest of the window dark. Scope a distinct palette to part of the UI.
- Build a `StyleMap` for the `+` button with `.hovered`, `.pressed`, and
  `.focused` overrides. See how each state responds.
- Build a custom theme seeded from `"dark"` with `.primary("#88c0d0")`
  and use it on the window.
- Add a `Gradient::linear_from_angle(90.0, ..)` background to a
  container and layer a shadow on top.

## What's next

With the palette locked in, the next step is movement. Chapter 9 covers
animation and transitions: timed color fades, eased layout changes, and
physics-driven springs that make the app feel alive.

---

Next: [Animation](09-animation.md)
