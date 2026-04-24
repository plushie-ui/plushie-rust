# Themes and styling

Plushie's visual styling works at three layers: **themes** set the
overall palette, **style maps** override individual widget
appearance, and **type modules** (`Color`, `Border`, `Shadow`,
`Gradient`, `Font`) provide the building blocks. All of these
types live in `plushie_core::types` and are re-exported from
`plushie::types`. The renderer applies the resolved styling the
same way in both direct and wire modes, so the builders and wire
shapes described here apply identically.

## Color

`Color` is a single hex string wrapper: constructors normalize
every input to a canonical lowercase `#rrggbb` or `#rrggbbaa`
form, and `as_hex()` returns the stored representation. The
strict wire decoder accepts only those canonical forms; the host
`Color::hex(..)` constructor additionally expands `#rgb` and
`#rgba` shorthand.

### Constructors

```rust
use plushie::types::Color;

let primary = Color::hex("#3b82f6");
let short = Color::hex("#f0f");             // expands to #ff00ff
let translucent = Color::hex("#3b82f680");  // #rrggbbaa
let rgb = Color::rgb(0.23, 0.51, 0.96);
let rgba = Color::rgba(1.0, 0.0, 0.0, 0.5);
```

| Function | Signature | Notes |
|---|---|---|
| `Color::hex` | `(&str) -> Color` | Accepts 3, 4, 6, or 8 hex digits with or without `#`. Panics on invalid input |
| `Color::try_hex` | `(&str) -> Option<Color>` | Fallible variant of `hex` |
| `Color::rgb` | `(f32, f32, f32) -> Color` | Channels in 0.0-1.0, clamped |
| `Color::rgba` | `(f32, f32, f32, f32) -> Color` | Channels and alpha in 0.0-1.0, clamped |
| `Color::as_hex` | `(&self) -> &str` | Canonical lowercase hex |

`Color` also implements `From<&str>` and `From<String>`, so most
setters that take `impl Into<Color>` accept a hex literal
directly: `.color("#3b82f6")`.

### Named colors

All CSS Color Module Level 4 named colors ship as zero-argument
constructors alongside `Color::transparent()`. Method names match
the lowercase CSS identifier:

```rust
use plushie::types::Color;

let accent = Color::cornflowerblue();
let badge = Color::rebeccapurple();
let surface = Color::whitesmoke();
let nothing = Color::transparent();
```

Both spelling variants are available where CSS defines them
(`Color::darkgray()` and `Color::darkgrey()` resolve to the same
hex value). The constructors are thin wrappers over `Color::hex`,
so they compose with any setter that accepts `Color`.

## Theme

Every window has a theme that drives the palette for its widget
subtree. Themes are selected at three scopes: application-wide
via `App::settings().theme`, per-window via
`window(..).theme(..)` or `WindowConfig::theme`, and per-subtree
via the `themer` widget.

```rust
use plushie::prelude::*;
use plushie::types::Theme;

fn view(_model: &Model, _widgets: &mut WidgetRegistrar) -> ViewList {
    window("main")
        .title("App")
        .theme("dark")
        .child(column().padding(16).children([
            text("Hello").into(),
        ]))
        .into()
}
```

`Theme` implements `From<&str>`, so setters accept a bare string
name; an empty theme leaves the window on the renderer's default.

### Built-in themes

`Theme::builtin_names()` returns the canonical list. Pass any of
these names as a `Theme::Named(..)` (or, equivalently, a `&str`)
to `window.theme(..)` or `Settings.theme`:

| Family | Names |
|---|---|
| Default | `light`, `dark` |
| Dracula | `dracula` |
| Nord | `nord` |
| Solarized | `solarized_light`, `solarized_dark` |
| Gruvbox | `gruvbox_light`, `gruvbox_dark` |
| Catppuccin | `catppuccin_latte`, `catppuccin_frappe`, `catppuccin_macchiato`, `catppuccin_mocha` |
| Tokyo Night | `tokyo_night`, `tokyo_night_storm`, `tokyo_night_light` |
| Kanagawa | `kanagawa_wave`, `kanagawa_dragon`, `kanagawa_lotus` |
| Moonfly / Nightfly | `moonfly`, `nightfly` |
| Oxocarbon | `oxocarbon` |
| Ferra | `ferra` |

The wire value `"system"` (which also round-trips through
`Theme::System` and `Theme::from("system")`) follows the
operating system's light/dark preference.

### Per-window theme overrides

Windows inherit the application theme from `App::settings()` and
can override it independently:

```rust
use plushie::prelude::*;
use plushie::types::Theme;

window("prefs")
    .title("Preferences")
    .theme(Theme::Named("solarized_light".into()))
    .child(column().children([ /* ... */ ]));
```

The `Theme` value is wire-encoded at `.theme(..)` call time and
stored on the window node's props. See
[Windows and layout](windows-and-layout.md) for the rest of the
per-window configuration surface.

### Custom themes

`Theme::custom(name)` builds a `CustomTheme` that starts from a
base built-in theme and overrides individual palette slots. The
builder methods delegate to a shared `color(key, hex)` helper, so
every slot is reachable:

```rust
use plushie::types::Theme;

let brand = Theme::custom("brand")
    .base("dark")
    .background("#1a1a2e")
    .text("#e0e0e8")
    .primary("#3b82f6")
    .danger("#ef4444")
    .primary_strong("#1d4ed8")
    .background_weakest("#0f0f1a");
```

| Seed slot | Purpose |
|---|---|
| `background` | Window background |
| `text` | Default text color |
| `primary` | Primary accent (buttons, focus rings) |
| `success` | Success indicators |
| `warning` | Warning indicators |
| `danger` | Error and destructive actions |

Hex values flow through `Color::hex`, so short forms expand and
both cases work. The base theme name defaults to `"dark"` when
`.base(..)` is not called; the renderer fills in any palette slot
the custom theme does not set.

### Shade overrides

Each semantic family exposes three base shades and matching text
variants for fine-grained control:

| Family | Shade keys | Text keys |
|---|---|---|
| Primary | `primary_base`, `primary_weak`, `primary_strong` | `primary_base_text`, `primary_weak_text`, `primary_strong_text` |
| Secondary | `secondary_base`, `secondary_weak`, `secondary_strong` | `secondary_base_text`, `secondary_weak_text`, `secondary_strong_text` |
| Success | `success_base`, `success_weak`, `success_strong` | `success_base_text`, `success_weak_text`, `success_strong_text` |
| Warning | `warning_base`, `warning_weak`, `warning_strong` | `warning_base_text`, `warning_weak_text`, `warning_strong_text` |
| Danger | `danger_base`, `danger_weak`, `danger_strong` | `danger_base_text`, `danger_weak_text`, `danger_strong_text` |

The background family has an extended ramp:

- Shades: `background_base`, `background_weakest`,
  `background_weaker`, `background_weak`, `background_neutral`,
  `background_strong`, `background_stronger`,
  `background_strongest`.
- Text pair for each shade: `background_base_text`,
  `background_weakest_text`, `background_weaker_text`,
  `background_weak_text`, `background_neutral_text`,
  `background_strong_text`, `background_stronger_text`,
  `background_strongest_text`.

Each shade has a dedicated builder method on `Theme` with the
matching name. For keys not covered by a named method (e.g.
forward-compatible additions), `Theme::color(key, hex)` sets an
arbitrary slot.

### Subtree theming with `themer`

The `themer` widget changes the theme for a subtree without
affecting sibling content:

```rust
use plushie::prelude::*;

themer("sidebar")
    .theme("dark")
    .child(column().padding(12).children([ /* ... */ ]));
```

`themer` takes a single child and forwards everything else to the
renderer's theme resolution pipeline.

## Style

`Style` controls the appearance of a single widget. It has two
variants: a named preset and a fully custom `StyleMap`.

```rust
pub enum Style {
    Preset(String),
    Custom(Box<StyleMap>),
}
```

`Style` implements `From<&str>` and `From<StyleMap>`, so widget
builders that accept `impl Into<Style>` take a preset name, a
constructor result, or a built-up `StyleMap` interchangeably.

### Named presets

`Style` exposes constructors for the presets the renderer
recognizes. Availability varies by widget: a `button` honors
`primary`, `secondary`, `success`, `danger`, `warning`, and
`text`; a `container` honors `rounded_box`, `bordered_box`, and
`transparent`; and most widgets accept `default_style()`, `dark`,
or `weak` where they make sense.

| Constructor | Preset name on the wire |
|---|---|
| `Style::primary()` | `"primary"` |
| `Style::secondary()` | `"secondary"` |
| `Style::success()` | `"success"` |
| `Style::danger()` | `"danger"` |
| `Style::warning()` | `"warning"` |
| `Style::text()` | `"text"` |
| `Style::default_style()` | `"default"` |
| `Style::dark()` | `"dark"` |
| `Style::weak()` | `"weak"` |
| `Style::rounded_box()` | `"rounded_box"` |
| `Style::bordered_box()` | `"bordered_box"` |
| `Style::transparent()` | `"transparent"` |
| `Style::custom()` | returns an empty `StyleMap` |

```rust
use plushie::prelude::*;
use plushie::types::Style;

button("save", "Save").style(Style::primary());
button("cancel", "Cancel").style("text");
```

### StyleMap

`StyleMap` is the fully custom shape. Every field is `Option`,
and the setters either take the typed value directly or a
closure that builds a `StatusOverride` for per-state variants.
Wrap a built `StyleMap` in `Style::Custom(Box::new(map))` or
rely on `From<StyleMap> for Style`:

```rust
use plushie::prelude::*;
use plushie::types::{Border, Color, Shadow, StyleMap};

let pill = StyleMap::new()
    .base("primary")
    .background(Color::hex("#3b82f6"))
    .text_color(Color::white())
    .border(Border::new().color(Color::hex("#2563eb")).width(1.0).radius(999.0))
    .shadow(Shadow::new().color(Color::hex("#0000001a")).blur_radius(4.0))
    .hovered(|s| s.background(Color::hex("#2563eb")))
    .pressed(|s| s.background(Color::hex("#1d4ed8")))
    .disabled(|s| s.background(Color::hex("#9ca3af")).text_color(Color::hex("#6b7280")))
    .focused(|s| s.border(Border::new().color(Color::hex("#3b82f6")).width(2.0)));

button("save", "Save").style(pill);
```

| Method | Signature | Purpose |
|---|---|---|
| `StyleMap::new` | `() -> StyleMap` | Empty style map |
| `base` | `(&str) -> Self` | Extend a preset by name |
| `background` | `(impl Into<Background>) -> Self` | Solid color or gradient background |
| `text_color` | `(impl Into<Color>) -> Self` | Text color |
| `border` | `(Border) -> Self` | Border descriptor |
| `shadow` | `(Shadow) -> Self` | Shadow descriptor |
| `hovered` | `(FnOnce(StatusOverride) -> StatusOverride) -> Self` | Hover state override |
| `pressed` | `(FnOnce(StatusOverride) -> StatusOverride) -> Self` | Pressed state override |
| `disabled` | `(FnOnce(StatusOverride) -> StatusOverride) -> Self` | Disabled state override |
| `focused` | `(FnOnce(StatusOverride) -> StatusOverride) -> Self` | Focused state override |

The `hovered` / `pressed` / `disabled` / `focused` closures build
a `StatusOverride`, a flat record limited to `background`,
`text_color`, `border`, and `shadow`. Only the fields the closure
touches participate in the override: the rest inherit from the
base.

### Per-widget style and background setters

Most widgets accept a `Style` through `.style(..)`:

```rust
button("save", "Save").style(Style::primary());
text_input("email", &model.email).style("default");
container("card").style(Style::rounded_box());
```

`container` additionally exposes direct `.background(..)`,
`.border(..)`, and `.shadow(..)` setters for the common case of a
one-off visual. Widgets that render text (`text`, `rich_text`,
`markdown`, canvas text) take `.color(..)` for the foreground,
and `text` accepts a `.background(..)`.

See the "Styling setters" column in
[Built-in widgets](built-in-widgets.md) for the full list.

## Background

`Background` is the discriminated union accepted by every
`.background(..)` setter:

```rust
pub enum Background {
    Color(Color),
    Gradient(Gradient),
}
```

`Background` implements `From<Color>`, `From<Gradient>`,
`From<&str>`, and `From<String>`, so a setter signed as
`impl Into<Background>` accepts a `Color`, a hex literal, or a
`Gradient` directly:

```rust
use plushie::prelude::*;
use plushie::types::{Color, Gradient};

container("card").background(Color::hex("#0f172a"));
container("hero").background(
    Gradient::linear_from_angle(
        135.0,
        vec![(0.0, Color::hex("#667eea")), (1.0, Color::hex("#764ba2"))],
    ),
);
```

## Gradient

`Gradient` is a linear gradient with explicit start/end points
and a sequence of color stops. The wire shape uses unit-square
coordinates and a `"linear"` discriminator:

```json
{"type": "linear", "start": [0.0, 0.0], "end": [1.0, 0.0], "stops": [[0.0, "#rrggbb"], ...]}
```

Two constructors cover the common cases:

```rust
use plushie::types::{Color, Gradient, GradientStop};

// Explicit endpoints.
let g1 = Gradient::linear(
    (0.0, 0.0),
    (1.0, 1.0),
    vec![(0.0, Color::hex("#3b82f6")), (1.0, Color::hex("#1d4ed8"))],
);

// Angle-based (0 deg east, 90 deg south).
let g2 = Gradient::linear_from_angle(
    90.0,
    vec![(0.0, Color::white()), (1.0, Color::hex("#f5f5f5"))],
);
```

`GradientStop::new(offset, color)` exists for callers that prefer
a typed constructor over the tuple form; the `Vec<(f32, Color)>`
shape passed to `linear` and `linear_from_angle` is flattened
into stops internally.

## Border

`Border` describes a widget border: optional color, width, and a
corner radius that is either uniform or per-corner.

```rust
use plushie::types::{Border, Color, Radius};

let b = Border::new()
    .color(Color::hex("#e5e7eb"))
    .width(1.0)
    .radius(8.0);

let split = Border::new()
    .color(Color::hex("#d4d4d8"))
    .width(1.0)
    .radius_corners(8.0, 8.0, 0.0, 0.0); // rounded top, square bottom
```

| Method | Signature | Purpose |
|---|---|---|
| `Border::new` | `() -> Border` | Defaults: no color, zero width, zero radius |
| `color` | `(impl Into<Color>) -> Self` | Border color |
| `width` | `(f32) -> Self` | Width in logical pixels |
| `radius` | `(f32) -> Self` | Uniform corner radius |
| `radius_corners` | `(f32, f32, f32, f32) -> Self` | Per-corner radius: top-left, top-right, bottom-right, bottom-left |

The underlying `Radius` enum (`Radius::Uniform(f32)` and
`Radius::PerCorner { .. }`) is exposed publicly for callers that
need to store a radius separately from a `Border`. Negative
widths or radii panic at `wire_encode`.

## Shadow

`Shadow` is a drop shadow: color, pixel offset, and blur radius.

```rust
use plushie::types::{Color, Shadow};

let s = Shadow::new()
    .color(Color::hex("#0000001a"))
    .offset(0.0, 4.0)
    .blur_radius(8.0);
```

| Method | Signature | Purpose |
|---|---|---|
| `Shadow::new` | `() -> Shadow` | Defaults: opaque black, zero offset, zero blur |
| `color` | `(impl Into<Color>) -> Self` | Shadow color |
| `offset` | `(f32, f32) -> Self` | Horizontal and vertical offset in logical pixels |
| `blur_radius` | `(f32) -> Self` | Blur radius in logical pixels (0.0 = sharp) |

On the wire, `offset` is encoded as a two-element array
(`[x, y]`); the decoder also accepts `offset_x` / `offset_y`
scalar fields for backward compatibility.

## Font

`Font` pairs a family name with optional weight, style, and
stretch metadata. Widgets that render text accept it through
`.font(..)`:

```rust
use plushie::prelude::*;
use plushie::types::{Font, FontStretch, FontStyle, FontWeight};

text("Hello")
    .font(
        Font::new()
            .family("Fira Code")
            .weight(FontWeight::Bold)
            .style(FontStyle::Italic)
            .stretch(FontStretch::Condensed),
    );

text("Code sample").font(Font::monospace());
```

The wire encoder collapses the common cases: a family with no
modifiers serializes as a plain string, and the special strings
`"default"` and `"monospace"` round-trip directly. Anything with
weight, style, or stretch set encodes as an object with the
set fields.

### FontWeight

Variants map to the standard CSS numeric weights: `Thin` (100),
`ExtraLight` (200), `Light` (300), `Normal` (400), `Medium`
(500), `SemiBold` (600), `Bold` (700), `ExtraBold` (800), `Black`
(900).

### FontStyle

`FontStyle::Normal`, `FontStyle::Italic`, `FontStyle::Oblique`.

### FontStretch

From narrowest to widest: `UltraCondensed`, `ExtraCondensed`,
`Condensed`, `SemiCondensed`, `Normal`, `SemiExpanded`,
`Expanded`, `ExtraExpanded`, `UltraExpanded`.

## Application and window settings

`Settings` (returned from `App::settings`) carries the
application-wide theme alongside default fonts, text size, and
event rate:

```rust
use plushie_core::settings::Settings;
use plushie::types::Theme;

Settings {
    default_font: Some("Inter".into()),
    default_text_size: Some(14.0),
    theme: Some(Theme::Named("dark".into())),
    fonts: vec!["./fonts/Inter.ttf".into()],
    ..Settings::default()
}
```

`WindowConfig::theme` overrides the application theme for a
specific window at startup. Runtime changes happen by re-rendering
with a different `window(..).theme(..)`; the renderer applies the
new theme on the next diff.

## Theming custom widgets

Custom widgets receive the resolved theme and the decoded `Style`
on the `RenderCtx` passed to `PlushieWidget::render`. Widgets can
read the active theme's palette to pick colors that match the
host application, and can branch on `Style::Preset` versus
`Style::Custom` to decide how much of their appearance to derive
from the `StyleMap`. See
[Custom widgets](custom-widgets.md) for the render context API
and styling helpers.

## See also

- [Built-in widgets](built-in-widgets.md) for the widgets that
  accept `.style`, `.background`, `.border`, `.shadow`, `.color`,
  and `.font`.
- [Windows and layout](windows-and-layout.md) for the
  `window.theme(..)` setter, `WindowConfig`, and per-window
  overrides.
- [Canvas](canvas.md) for fill and stroke colors, gradients, and
  fonts in canvas shapes.
- [Animation](animation.md) for `Animatable<Color>` and animated
  backgrounds.
- [Accessibility](accessibility.md) for screen-reader hints that
  travel alongside visual styling.
