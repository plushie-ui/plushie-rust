# Configuration

Rust plushie apps have no `plushie.toml`. Every runtime setting
lives in the app itself: the `App::settings`, `App::window_config`,
and `App::restart_policy` trait methods produce plain values from
the `plushie::settings` module. A small set of environment
variables and two Cargo features cover the deployment knobs that
make sense outside the app binary.

The configuration types are re-exported from `plushie::settings`
and defined in `plushie_core::settings` (see `crates/plushie-core/src/settings.rs`).

## Settings

`App::settings() -> Settings` is called once during the initial
handshake with the renderer. Returning `Settings::default()` (the
default impl) hands the renderer only the fields it cannot default
on its own. Every field is `Option<T>` or a `Vec<T>` / `HashMap`;
fields you do not set fall back to the renderer's built-in default.

```rust
use plushie::settings::Settings;
use plushie::types::Theme;

impl plushie::App for MyApp {
    // ...
    fn settings() -> Settings {
        Settings {
            default_text_size: Some(16.0),
            theme: Some(Theme::Named("catppuccin_mocha".into())),
            fonts: vec!["assets/inter.ttf".into()],
            default_event_rate: Some(60),
            ..Settings::default()
        }
    }
}
```

| Field | Type | Default | Description |
|---|---|---|---|
| `default_font` | `Option<String>` | renderer default | Default font family name (e.g. `"monospace"`) |
| `default_text_size` | `Option<f32>` | renderer default | Default text size in logical pixels |
| `antialiasing` | `Option<bool>` | renderer default | Enable multi-sample anti-aliasing |
| `vsync` | `Option<bool>` | renderer default | Sync frame presentation with display refresh |
| `scale_factor` | `Option<f32>` | `1.0` | Multiplier on top of OS DPI scaling |
| `theme` | `Option<Theme>` | renderer default | App-wide theme; see [themes](themes-and-styling.md) |
| `fonts` | `Vec<String>` | `[]` | Paths to font files loaded at startup |
| `default_event_rate` | `Option<u32>` | unlimited | Events per second for coalescable event types |
| `widget_config` | `HashMap<String, Value>` | `{}` | Per-namespace config forwarded to native widgets |
| `required_widgets` | `Vec<String>` | `[]` | Native widget type names the app requires |

`required_widgets` is validated during the Settings handshake:
missing names produce a `required_widgets_missing` diagnostic. It
is non-fatal; the app decides how to react via the diagnostic
stream.

`widget_config` values reach a `PlushieWidget` implementation
through the widget SDK's init context, keyed on the widget
crate's `namespace()`. See [custom widgets](custom-widgets.md)
for the authoring side.

## WindowConfig

`App::window_config(&model) -> WindowConfig` is called once after
`init`. Fields here set app-wide defaults for every `window(...)`
call in `view`. Per-window setters on the `window` builder
override the corresponding `WindowConfig` field at the call site.

```rust
use plushie::settings::WindowConfig;

fn window_config(_model: &Self::Model) -> WindowConfig {
    WindowConfig {
        width: Some(1024.0),
        height: Some(768.0),
        resizable: Some(true),
        exit_on_close_request: Some(true),
        ..WindowConfig::default()
    }
}
```

| Field | Type | Description |
|---|---|---|
| `title` | `Option<String>` | Title bar text |
| `width` | `Option<f32>` | Initial width in logical pixels |
| `height` | `Option<f32>` | Initial height in logical pixels |
| `position` | `Option<(f32, f32)>` | Initial (x, y) in logical pixels |
| `min_size` | `Option<(f32, f32)>` | Minimum (w, h) |
| `max_size` | `Option<(f32, f32)>` | Maximum (w, h) |
| `maximized` | `Option<bool>` | Start maximized |
| `fullscreen` | `Option<bool>` | Start in fullscreen |
| `visible` | `Option<bool>` | Visible at launch |
| `resizable` | `Option<bool>` | User may resize |
| `decorations` | `Option<bool>` | Show native title bar and borders |
| `transparent` | `Option<bool>` | Transparent background |
| `closeable` | `Option<bool>` | Show close button |
| `minimizable` | `Option<bool>` | Allow minimize |
| `blur` | `Option<bool>` | Background blur (platform-dependent) |
| `level` | `Option<WindowLevel>` | Stacking level |
| `exit_on_close_request` | `Option<bool>` | Close request exits the app |
| `event_rate` | `Option<u32>` | Max events/sec for this window |
| `a11y` | `Option<Value>` | Accessibility annotations |
| `theme` | `Option<Theme>` | Per-window theme override |
| `scale_factor` | `Option<f32>` | Per-window DPI override |

### Per-window overrides

A `window("id")` builder exposes setters that mirror the
`WindowConfig` fields. Anything set on the builder overrides the
app-level default for that specific window:

```rust
use plushie::prelude::*;
use plushie::types::Theme;

window("detail")
    .title("Detail")
    .size(640.0, 480.0)
    .theme(Theme::Named("light".into()))
    .resizable(true)
```

See [built-in widgets](built-in-widgets.md) for the full setter
surface on the window builder.

## Restart policy

Wire mode only. `App::restart_policy() -> RestartPolicy` controls
auto-restart of the renderer subprocess on unexpected exit. The
default is five consecutive restart attempts, exponential backoff
from a 100 ms base, and a 30-second wire heartbeat.

| Field | Type | Default | Description |
|---|---|---|---|
| `max_restarts` | `u32` | `5` | Consecutive restart attempts before giving up |
| `restart_delay` | `Duration` | `100 ms` | Base delay; actual delay is `restart_delay * 2.pow(count)` |
| `heartbeat_interval` | `Option<Duration>` | `Some(30 s)` | Watchdog; `None` disables heartbeats |

Set `max_restarts: 0` to disable auto-restart entirely: the first
crash delivers [`ExitReason::Crash`] to
`App::handle_renderer_exit` and `plushie::run` returns with
`Error::RendererExit`. See
[app lifecycle](app-lifecycle.md) for the full exit hook contract.

## Environment variables

The SDK reads these at startup. Spelling and precedence rules are
fixed: the same names work across every host SDK.

| Variable | Consumer | Purpose |
|---|---|---|
| `PLUSHIE_BINARY_PATH` | SDK (wire) | Explicit renderer binary. Highest-precedence discovery step; a missing file fails fast |
| `PLUSHIE_MODE` | SDK | `wire` forces wire mode; `direct` defers to the feature default. Same effect as `--plushie-mode` |
| `PLUSHIE_SOCKET` | SDK | Connect to an existing renderer over a Unix or TCP socket instead of spawning |
| `PLUSHIE_TOKEN` | SDK | Auth token sent with the Settings handshake when connecting to a listen-mode renderer |
| `PLUSHIE_RUST_SOURCE_PATH` | `cargo plushie` | Path to a local plushie-rust checkout. Required for native-widget source builds; optional for WASM (falls back to crates.io) |
| `PLUSHIE_UPDATE_SNAPSHOTS` | Test harness | When set to `1`, overwrite tree-hash golden files instead of comparing |
| `RUST_LOG` | Renderer | `env_logger` filter forwarded to the renderer subprocess |
| `RUST_BACKTRACE` | Renderer | Standard Rust backtrace switch |
| `WGPU_BACKEND` | Renderer | Force a specific wgpu backend |

The `PLUSHIE_*` prefix is reserved for plushie internals. The wire
runner spawns the renderer with a filtered environment: only the
variables above, the platform display and locale set (`DISPLAY`,
`WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `PATH`, `LC_*`, etc.), and
anything starting with a plushie-reserved prefix (`PLUSHIE_`,
`MESA_`, `VK_`, `FONTCONFIG_`, ...) pass through. Everything else
is stripped before the child process starts. See
`crates/plushie/src/runner/env.rs` for the exact allowlist.

### Discovery precedence

`plushie::run` resolves the runner in this order:

1. `PLUSHIE_SOCKET` (or `--plushie-socket`) non-empty: wire-connect.
2. `PLUSHIE_BINARY_PATH` set: wire-spawn with that explicit binary.
3. `PLUSHIE_MODE=wire` (or `--plushie-mode wire`): wire-spawn via
   the discovery chain.
4. Feature default: `direct` if compiled in, otherwise `wire` via
   discovery.

Wire discovery steps, when triggered, are documented in
[direct vs wire](direct-vs-wire.md).

## Cargo feature flags

The `plushie` crate exposes two rendering features and a dev-mode
helper. Pick one of the rendering features; both can be enabled,
in which case direct mode wins at `run` time unless an env or CLI
flag forces wire.

| Feature | Default | Notes |
|---|---|---|
| `direct` | yes | In-process iced daemon. Native only |
| `wire` | no | MessagePack subprocess renderer. Works without iced |
| `dev` | no | File-watch and dev-overlay helpers (pulls `notify`, `cargo_metadata`) |

```toml
[dependencies]
plushie = { version = "0.7.0", default-features = false, features = ["wire"] }
```

Pre-1.0, pin the exact patch version. `plushie`, `plushie-core`,
`plushie-renderer-lib`, and the `plushie-renderer` binary release
as a single workspace; mismatched versions produce a handshake
warning. For the full feature matrix and the direct-vs-wire
trade-offs, see [direct vs wire](direct-vs-wire.md).

## cargo-plushie configuration

`cargo-plushie` reads its inputs from the app crate's `Cargo.toml`
under `[package.metadata.plushie]` rather than from a dedicated
config file:

```toml
[package.metadata.plushie]
source_path = "../plushie-rust"
native_widgets = ["my-gauge", "my-chart"]
```

| Key | Purpose |
|---|---|
| `source_path` | Path to a local plushie-rust checkout. Required for native-widget source builds; optional for WASM (falls back to crates.io). Overridden by `PLUSHIE_RUST_SOURCE_PATH` |
| `native_widgets` | Allowlist of native widget crate names to bundle into the custom renderer. Omit to auto-discover via dep-graph metadata |

Native widget crates declare their own
`[package.metadata.plushie.widget]` table (`type_name`,
`constructor`, `namespace`); `cargo plushie new-widget` scaffolds
the correct shape. See [CLI commands](cli-commands.md) for the
build, download, run, and doctor subcommands that consume this
metadata.

## Host-SDK configuration vs in-crate configuration

Plushie ships host SDKs in multiple languages. Each host's
configuration story is shaped by what's idiomatic in that
language:

- **Rust**: code-first. `Settings`, `WindowConfig`, and
  `RestartPolicy` are plain structs returned from trait methods.
  No external config file. Feature flags and `Cargo.toml`
  metadata cover the out-of-code knobs.
- **Elixir**: `config :plushie, ...` in `config/*.exs` and
  runtime options on `Plushie.start_link/2`.
- **Gleam**: a `[plushie]` table in `gleam.toml` plus
  `StartOpts` at runtime.

The wire protocol is identical across hosts, so a renderer
built against one host works with any other. The only things
that vary are the surface the app author writes against and the
place configuration physically lives.

## See also

- [Direct vs wire](direct-vs-wire.md)
- [App lifecycle](app-lifecycle.md)
- [Themes and styling](themes-and-styling.md)
- [CLI commands](cli-commands.md)
