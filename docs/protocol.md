# Wire Protocol

The renderer communicates with the host process over stdin (incoming
messages) and stdout (outgoing events). All log output goes to stderr.

Protocol version: **1**

## Encoding

Two wire formats are supported. Both carry the same message structures.

### JSON

One JSON object per line (JSONL). No length prefix.

    {"session":"","type":"settings","settings":{"default_text_size":14}}\n
    {"session":"","type":"snapshot","tree":{"id":"root","type":"column",...}}\n

### MessagePack

Each message is a 4-byte big-endian length prefix followed by a
MessagePack payload.

    [4 bytes: payload length as u32 BE][msgpack payload bytes]

### Choosing a format

JSON works for most use cases. MessagePack is better when sending
binary data (images, pixel buffers) or when serialization overhead
matters at high update rates.

### Format detection

The renderer auto-detects the format from the first byte of stdin:

- `0x7B` (`{`) - JSON
- Anything else - MessagePack

Override with `--json` or `--msgpack` CLI flags.

### Limits

Maximum message size: **64 MiB**. Messages exceeding this are rejected.

---

## Sessions

Every wire message carries a `session` field (string) identifying the
logical session it belongs to. In single-session mode (the default),
all messages use the same session value. In multiplexed mode
(`--max-sessions N` with N > 1), multiple sessions run concurrently
in separate threads, each with fully isolated state.

The renderer echoes the `session` value from each incoming message
back on the corresponding outgoing message(s). This is routing
metadata, not message content; the renderer does not interpret the
session value beyond using it for dispatch.

```json
{"session": "test_42", "type": "snapshot", "tree": {...}}
{"session": "test_42", "type": "query", "id": "q1", ...}
```

**Session lifecycle in multiplexed mode:**

- A session is created implicitly when the first message with a new
  session value arrives.
- A `Reset` message tears down the session (thread exits, all state
  freed). The session value can be reused; a new session is created
  on the next message.
- The `--max-sessions` flag limits concurrent sessions. Messages for
  new sessions beyond the limit are dropped with a log error.

**Single-session mode:** When `--max-sessions` is 1 (or omitted),
the renderer runs one session on the main thread with no threading
overhead. The session field is still present on all messages.

### Session lifecycle events

In multiplexed mode, the renderer emits lifecycle events to inform the
host when sessions fail or complete teardown.

**session_error**: Emitted when a session thread panics or when
`max_sessions` is exceeded.

```json
{"type": "event", "session": "s1", "family": "session_error", "id": "", "value": {"error": "session thread panicked: ..."}}
```

- Emitted to stdout so the host knows a session is no longer functional.
- The `session` field identifies which session failed.

**session_closed**: Emitted after a Reset completes and the session
thread exits.

```json
{"type": "event", "session": "s1", "family": "session_closed", "id": "", "value": {"reason": "reset"}}
```

- Confirms the old session has fully torn down.
- The host should wait for this before recycling the session ID.
- Delivery is best-effort: if the renderer's stdout writer is already
  gone by the time the session thread tries to emit `session_closed`,
  the event is dropped. Hosts that wait for `session_closed` before
  reusing the session ID should apply a timeout and fall back on a
  plain `session_error` observation.

**session_error code field**: `session_error` events include a
stable `code` field in the payload so hosts can match by code rather
than error text. Current codes:

| Code | Meaning |
|------|---------|
| `max_sessions_reached` | `--max-sessions` limit reached |
| `session_reset_in_progress` | New message arrived while the session is closing from Reset; wait for `session_closed` before reusing the ID |
| `session_panic` | Session thread panicked; `error` carries the panic payload |
| `session_backpressure_overflow` | Per-session queue + pending buffer both full; session ejected |
| `session_channel_closed` | Session thread exited unexpectedly |
| `writer_dead` | Renderer's stdout writer thread exited; affects every active session |
| `font_cap_exceeded` | Process-wide `load_font` cap reached; this session tried to load another font |
| `renderer_panic` | Renderer process panicked (caught by the process-wide panic hook); `error` carries the panic payload and `location` carries `file:line:col` |

### Multiplex test-harness notes

Multi-session mode is primarily a test-harness feature, so a few
process-level resources are shared across sessions:

- **Fonts.** iced's font system is a process-global. Every
  `load_font` goes into the same backing store, so session A's
  fonts are visible to session B's render. Tests that care about
  isolation should load all required fonts at shared-setup time
  before any concurrent session starts. The cap
  (`load_font`-per-process) is global; per-session attribution is
  reported via `font_cap_exceeded`.
- **Clipboard and notifications.** OS-level singletons. Tests
  should stub these via `register_effect_stub` to avoid both
  cross-session contamination and the session-thread stall that
  native clipboard APIs can cause.

---

## Startup sequence

1. Host spawns the renderer and writes a **Settings** message to stdin.
2. Renderer peeks at the first byte of stdin to detect the wire format.
3. Renderer writes a **hello** message to stdout.
4. Renderer reads and validates Settings (protocol version, token).
5. Normal message exchange begins.

The hello message confirms the renderer is ready and reports its
protocol version:

```json
{
  "type": "hello",
  "session": "",
  "protocol_version": 1,
  "protocol": 1,
  "codec": "json",
  "version": "0.6.1",
  "name": "plushie-renderer",
  "mode": "headless",
  "backend": "tiny-skia",
  "transport": "stdio",
  "native_widgets": ["charts", "editor"],
  "widget_sets": ["iced"],
  "widgets": ["button", "charts", "column", "editor", "text"]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `protocol_version` | number | Canonical protocol version (currently 1), encoded as an integer JSON number that must fit in `u32` |
| `protocol` | number | Legacy alias for `protocol_version`, emitted temporarily for host SDK compatibility |
| `codec` | string | Wire codec detected before hello: `"json"` or `"msgpack"`. This confirms the active codec; it is not negotiated in hello. |
| `version` | string | Renderer build version |
| `name` | string | Renderer name (always `"plushie-renderer"`) |
| `mode` | string | Execution mode: `"windowed"`, `"headless"`, or `"mock"` |
| `backend` | string | Rendering backend: `"wgpu"`, `"tiny-skia"`, or `"mock"` |
| `transport` | string | Transport backend: `"stdio"`, `"exec"`, `"listen"`, or `"wasm"` |
| `native_widgets` | array | Type names handled by registered native (Rust-backed) widgets, sorted alphabetically |
| `widget_sets` | array | Names of the widget-set groups that registered the built-in widgets (e.g. `"iced"`). Lets hosts detect the widget-provider groups compiled into the renderer without enumerating every widget type. |
| `widgets` | array | All compiled widget type names (built-in + native), sorted alphabetically |

All fields shown are emitted by the renderer. The host should check that `protocol_version` matches the version it expects.
The `mode` field tells the SDK what capabilities are available (e.g.
headless mode supports `interact_step` round-trips and real
screenshots; mock mode returns stubs). The `session` field on
`hello` is always empty (it is a process-level message, not
scoped to any session).

---

## Subprocess environment whitelist

Every host SDK that spawns `plushie-renderer` as a child process must
isolate the child from the host application's environment. The host's
environment typically contains application secrets (API keys, database
URLs, auth tokens) that the renderer has no business seeing; an
undiscovered CVE in an image or font parser should not become a
secret-exfiltration vector.

The canonical whitelist below is enforced by every host SDK (Elixir,
Gleam, Python, Ruby, TypeScript, Rust). Non-matching parent variables
are actively unset in the child process.

### Exact entries

Display, rendering, library-loading, locale, accessibility, font, and
renderer-diagnostic variables:

```
DISPLAY                      NO_AT_BRIDGE
WAYLAND_DISPLAY              WGPU_BACKEND
WAYLAND_SOCKET               RUST_LOG
WINIT_UNIX_BACKEND           RUST_BACKTRACE
XDG_RUNTIME_DIR              HOME
XDG_DATA_DIRS                USER
XDG_DATA_HOME                PATH
LD_LIBRARY_PATH              LANG
DYLD_LIBRARY_PATH            LANGUAGE
DYLD_FALLBACK_LIBRARY_PATH   DBUS_SESSION_BUS_ADDRESS
GTK_MODULES
```

### Prefix entries

Any variable whose name starts with one of these prefixes is
forwarded:

```
LC_           GALLIUM_
MESA_         AT_SPI_
LIBGL_        FONTCONFIG_
__GLX_        PLUSHIE_
VK_
```

`PLUSHIE_` is a plushie-reserved catch-all for renderer-side controls
such as diagnostics and test snapshot updates. Adding a new plushie
control with the `PLUSHIE_` prefix propagates automatically without
updating per-SDK whitelists. No legitimate secret should use this
prefix.

### Rationale

The whitelist is a belt-and-braces mitigation: host applications
routinely hold secrets in their environment, the renderer doesn't need
them, and the OS-level parent-env inheritance model defaults to leak.
Explicitly unsetting non-whitelisted variables (rather than merely not
forwarding them) keeps behaviour deterministic across spawn APIs that
differ in whether `env` is additive or replacing.

---

## Common value types

**Colors** are canonical hex strings: `"#rrggbb"` (6-char) or
`"#rrggbbaa"` (8-char with alpha). Short forms (`#rgb`, `#rgba`)
are not accepted; the host must normalize before sending.

**Lengths** are numbers (pixels), `"fill"`, `"shrink"`, or
`{"fill_portion": n}`.

**Padding** is either a number (uniform) or a four-key object
`{"top": n, "right": n, "bottom": n, "left": n}`. Array forms are
not accepted.

**Angles** are numbers in degrees (rotations, arc sweeps, etc.).

**Themes** are either a string (`"system"`, `"dark"`, `"light"`,
`"dracula"`, etc.) or a JSON object for custom themes:
```json
{
  "name": "my-theme",
  "base": "dark",
  "background": "#1a1a2e",
  "text": "#e0e0e0",
  "primary": "#0f3460",
  "primary_strong": "#1a5276",
  "background_weakest": "#0d0d1a",
  "cursor_color": "#ffffff",
  "scrollbar_color": "#1f2937",
  "scroller_color": "#6b7280"
}
```
The `base` field selects a built-in theme to extend. Seed colors
(background, text, primary, success, warning, danger) set the
foundation. Shade keys provide fine-grained control over the
extended palette (5 color families x 3 shades + 8 background
levels, each with optional `_text` variant, 52 keys total).
Chrome color tokens (`cursor_color`, `scrollbar_color`,
`scroller_color`) are carried outside iced's palette. Scrollbar
tokens become defaults for scrollable widgets, with widget props
taking precedence. `cursor_color` applies to focused text entry
widgets through iced's focused text style.

---

## Tree nodes

A UI tree is a nested structure of nodes. Every node has four fields:

```json
{
  "id": "main#form/email",
  "type": "widget-type",
  "props": {},
  "children": []
}
```

| Field      | Type     | Description |
|------------|----------|-------------|
| `id`       | string   | Unique identifier for this node (see ID format below) |
| `type`     | string   | Widget type (e.g. `"text"`, `"button"`, `"column"`) |
| `props`    | object   | Widget-specific properties |
| `children` | array    | Child TreeNode objects |

**Node ID format.** IDs use the format `window#scope/path/id`. The
`#` separator divides the window name from the widget path within
that window. For example, `"main#form/email"` refers to the widget
`email` inside the `form` scope in the `main` window. Window nodes
themselves use bare IDs without the `#` prefix (e.g. `"main"`).

Window nodes (`"type": "window"`) are special: they map to native
windows. Place them at the top level of the tree (root or direct
children of root).

---

## Incoming messages (host -> renderer)

All messages are JSON objects with a `"type"` field that determines
the message kind and a `"session"` field identifying the session.
Field names use `snake_case`.

### Decoding policy

**Unknown fields.** Unknown fields in any incoming message are
silently ignored during decoding so the protocol can evolve without
breaking older hosts. The renderer surfaces each unknown field as
a non-fatal `diagnostic` event (code `unknown_field`) carrying the
message type and field path, so host-side typos and stale field
names are still visible during development.

**Duplicate JSON keys.** JSON parsers used on the renderer side
follow last-wins semantics for duplicate keys. Hosts should avoid
emitting duplicates; if they appear, the last occurrence in document
order takes effect. MessagePack maps with duplicate keys follow the
same rule via the underlying parser.

See also the `diagnostic` event under "Outgoing messages" for the
full list of protocol-level diagnostic codes.

### Settings

Sent as the first message. Configures the renderer.

```json
{
  "type": "settings",
  "session": "s1",
  "settings": {
    "protocol_version": 1,
    "default_text_size": 14.0,
    "default_font": { "family": "monospace" },
    "antialiasing": false,
    "vsync": true,
    "fonts": ["/path/to/font.ttf"],
    "scale_factor": 1.0,
    "validate_props": false,
    "widget_config": {}
  }
}
```

All fields inside `settings` are optional.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `protocol_version` | number | required | Protocol version as an integer JSON number that must match the renderer and fit in `u32` |
| `default_text_size` | number | 16.0 | Default text size for all text widgets |
| `default_font` | object | system | Default font descriptor (`{"family": "..."}`) |
| `antialiasing` | bool | false | Enable anti-aliasing (startup only) |
| `vsync` | bool | true | Enable VSync (startup only) |
| `fonts` | array | [] | Paths to font files to load (startup only) |
| `scale_factor` | number | 1.0 | Global scale factor (startup only) |
| `validate_props` | bool | false | Enable prop validation warnings in release builds |
| `widget_config` | object | {} | Configuration passed to custom widgets |
| `default_event_rate` | number | -- | Default max events per second for all coalescable events. Omit for unlimited. |

**Startup-only fields** (ignored if sent after the first Settings):
`antialiasing`, `vsync`, `fonts`, `scale_factor`, `validate_props`.

**Runtime fields** (can be updated by sending Settings again):
`default_text_size`, `default_font`, `widget_config`,
`default_event_rate`.

**Log level.** Renderer log verbosity is controlled via the `RUST_LOG`
environment variable on the renderer process, not via a Settings field.
The renderer's built-in default level is `warn`. Examples:
`RUST_LOG=plushie_renderer=debug`, `RUST_LOG=plushie_widget_sdk=trace`.

### Snapshot

Replace the entire tree. The simplest way to update the UI, no
diffing required on the host side.

```json
{
  "type": "snapshot",
  "session": "s1",
  "tree": {
    "id": "root",
    "type": "window",
    "props": { "title": "My App" },
    "children": [...]
  }
}
```

The renderer replaces the current tree, reconciles windows (opens new
ones, closes removed ones), and re-renders.

**Duplicate node IDs.** When a snapshot contains duplicate node IDs,
the renderer accepts the tree but emits an error event:

```json
{"type": "event", "session": "", "family": "error", "id": "duplicate_node_ids", "value": {"error": "snapshot contains duplicate node IDs", "duplicates": ["btn1 (button)", "btn1 (text)"]}}
```

The host should treat this as a bug in the tree construction. Duplicate
IDs cause undefined behavior in widget caching and event routing.

### Patch

Incrementally update the existing tree. More efficient than Snapshot
for large trees with small changes.

```json
{
  "type": "patch",
  "session": "s1",
  "ops": [...]
}
```

Each operation in `ops` is an object with an `op` field and a `path`
field. The path is an array of child indices from the root to the
target node.

#### replace_node

Replace the node at the given path.

```json
{
  "op": "replace_node",
  "path": [0, 2],
  "node": { "id": "new", "type": "text", "props": {}, "children": [] }
}
```

An empty path replaces the root.

#### update_props

Merge properties into the node at the given path. Setting a value to
`null` removes that key.

```json
{
  "op": "update_props",
  "path": [0],
  "props": { "label": "updated", "old_key": null }
}
```

#### insert_child

Insert a child node at the given index under the parent at path.

```json
{
  "op": "insert_child",
  "path": [0],
  "index": 2,
  "node": { "id": "new-child", "type": "text", "props": {}, "children": [] }
}
```

If `index` exceeds the number of children, the node is appended.

#### remove_child

Remove the child at the given index under the parent at path.

```json
{
  "op": "remove_child",
  "path": [0],
  "index": 2
}
```

#### Error handling

Operations are applied sequentially. If one fails (missing fields,
out-of-bounds path), it is skipped with a warning and subsequent
operations still apply.

#### Ordering guarantee

The host emits patch operations in a safe application order:
removals (descending child index), then updates (with indices adjusted
for prior removals), then inserts (ascending child index). The
renderer MUST apply operations in the order they appear in the `ops`
array. Reordering operations will produce incorrect results because
update paths reference the tree state after removals but before inserts.

### Subscribe

Subscribe to a category of events. The `tag` is included in events of
this kind so the host can route them.

```json
{
  "type": "subscribe",
  "session": "s1",
  "kind": "on_key_press",
  "tag": "my_key_handler"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `kind` | string | Yes | Event category to subscribe to (see table below) |
| `tag` | string | Yes | Tag included in events of this kind for routing |
| `window_id` | string | No | Scope delivery to events associated with one window. Omit for all windows. If the subscribed event kind has no window association, a scoped subscription does not match. |
| `max_rate` | integer | No | Maximum events per second. Omit for unlimited. Zero means "subscribe but never emit." |

When `max_rate` is set, the renderer delivers at most that many events
per second for this subscription kind. Between deliveries, it coalesces
buffered events (latest value wins, or delta accumulation for scroll
events). Re-subscribing with a different `max_rate` updates the limit
in place.

When `window_id` is set, the renderer only delivers events whose source
window matches that ID. This applies to both direct window subscriptions
such as `on_window_event` and window-originated input subscriptions such
as `on_key_press` or `on_pointer_move`.

**Available subscription kinds:**

| Kind | Events delivered |
|------|-----------------|
| `on_key_press` | Key press with key, modifiers, text |
| `on_key_release` | Key release |
| `on_modifiers_changed` | Modifier key state change |
| `on_pointer_move` | Cursor moved, entered, left |
| `on_pointer_button` | Pointer button pressed/released |
| `on_pointer_scroll` | Scroll wheel |
| `on_pointer_touch` | Finger press/move/lift/lost |
| `on_ime` | Input method events (open, preedit, commit, close) |
| `on_window_event` | All window lifecycle events |
| `on_window_open` | Window opened |
| `on_window_close` | Window close requested |
| `on_window_resize` | Window resized |
| `on_window_move` | Window moved |
| `on_window_focus` | Window gained focus |
| `on_window_unfocus` | Window lost focus |
| `on_file_drop` | File hovered/dropped on window |
| `on_animation_frame` | Per-frame timestamp (for animations) |
| `on_theme_change` | System theme changed (light/dark) |
| `on_event` | Catch-all: all keyboard, mouse, touch, and IME events |

`on_event` is a convenience that subscribes to everything at once. If
both `on_event` and a specific subscription (e.g. `on_key_press`) are
registered, events are delivered once, not twice.

### Unsubscribe

Remove a subscription.

```json
{
  "type": "unsubscribe",
  "session": "s1",
  "kind": "on_key_press"
}
```

### Command

Send a command to a widget by ID. This is the unified format for
all widget-targeted operations: focus, scroll, text cursor, pane
grid, and custom widget commands.

```json
{
  "type": "command",
  "session": "s1",
  "id": "input-1",
  "family": "focus",
  "value": null
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Target widget ID (scoped path, e.g. `"form/email"`) |
| `family` | string | Yes | Operation name (see table below) |
| `value` | any | No | Operation-specific data. Defaults to `null` if omitted. |

**Built-in command families:**

| Family | Value | Description |
|--------|-------|-------------|
| `focus` | null | Focus a widget by ID |
| `focus_next` | null | Focus next focusable widget |
| `focus_previous` | null | Focus previous focusable widget |
| `scroll_to` | `{x, y}` | Scroll to absolute offset |
| `scroll_by` | `{x, y}` | Scroll by relative amount |
| `snap_to` | `{x, y}` | Snap scrollable to relative position (0.0-1.0) |
| `snap_to_end` | null | Snap scrollable to end |
| `select_all` | null | Select all text |
| `select_range` | `{start, end}` | Select text range |
| `move_cursor_to` | `{position}` | Move cursor to position |
| `move_cursor_to_front` | null | Move cursor to start |
| `move_cursor_to_end` | null | Move cursor to end |
| `pane_split` | `{pane, axis, new_pane_id}` | Split a pane |
| `pane_close` | `{pane}` | Close a pane |
| `pane_swap` | `{a, b}` | Swap two panes |
| `pane_maximize` | `{pane}` | Maximize a pane |
| `pane_restore` | null | Restore maximized pane |

Custom widgets receive commands through the same mechanism. The
`family` string and `value` shape are defined by the widget's
command spec (see `command_specs()` on the PlushieWidget trait).

### Commands

Send multiple widget commands in a single message.

```json
{
  "type": "commands",
  "session": "s1",
  "commands": [
    { "id": "chart-1", "family": "append_data", "value": {"values": [1.0, 2.5]} },
    { "id": "chart-2", "family": "reset", "value": null }
  ]
}
```

Each item has the same `id`, `family`, `value` structure as a
single Command message.

### WidgetOp

Perform a global operation not targeted at a specific widget, or
query renderer state.

```json
{
  "type": "widget_op",
  "session": "s1",
  "op": "announce",
  "payload": { "text": "Item saved", "politeness": "polite" }
}
```

**Operations:**

| Op | Payload | Description |
|----|---------|-------------|
| `announce` | `text`, optional `politeness` (`"polite"` or `"assertive"`, default `"assertive"`) | Screen reader announcement (no visible widget needed) |
| `focus_next_within` | `scope` (widget ID) | Move focus to the next focusable widget within the subtree rooted at `scope` |
| `focus_previous_within` | `scope` (widget ID) | Move focus to the previous focusable widget within the subtree rooted at `scope` |
| `exit` | -- | Exit the renderer |
| `tree_hash` | `tag` (optional) | Compute SHA-256 hash of current tree; response via `op_query_response` |
| `find_focused` | `tag` (optional) | Find the currently focused widget; response via `op_query_response` |
| `load_font` | `family` (string), `data` (base64 TTF/OTF) | Load a font at runtime. `family` is the name the host uses to reference the font in `default_font` and widget font props. |
| `list_images` | `tag` (optional) | List all image handle names; response via `op_query_response` |
| `clear_images` | -- | Remove all in-memory image handles |

Window lifecycle (open / close / update) is driven by `window` tree
nodes and the `window_op` message type; see the WindowOp section below.

Widget op query responses (`tree_hash`, `find_focused`, `list_images`)
use the `op_query_response` outgoing message type.

### WindowOp

Manage windows directly (outside of tree-driven sync).

Uses the unified `_op` envelope: op-specific data lives under
`payload`; the `window_id` addressing field stays flat beside `op`.

```json
{
  "type": "window_op",
  "session": "s1",
  "op": "open",
  "window_id": "win-1",
  "payload": { "width": 800, "height": 600, "title": "New Window" }
}
```

**Operations:**

| Op | Description |
|----|-------------|
| `open` | Open a new window |
| `close` | Close a window |
| `update` | Update window properties |
| `resize` | Resize (width, height) |
| `move` | Move (x, y) |
| `maximize` | Maximize (maximized: bool) |
| `minimize` | Minimize (minimized: bool) |
| `set_mode` | Set mode (windowed, fullscreen, hidden) |
| `toggle_maximize` | Toggle maximized state |
| `toggle_decorations` | Toggle window decorations |
| `gain_focus` | Bring window to front |
| `set_level` | Set window level (normal, always_on_top, always_on_bottom) |
| `drag` | Begin window drag |
| `drag_resize` | Begin window resize drag (direction) |
| `request_attention` | Flash taskbar (urgency: informational, critical) |
| `show_system_menu` | Show system menu (Windows only) |
| `set_resizable` | Set resizable (bool) |
| `set_min_size` | Set minimum size (width, height) |
| `set_max_size` | Set maximum size (width, height) |
| `mouse_passthrough` | Enable/disable click-through (enabled: bool) |
| `set_resize_increments` | Set resize step size (width, height) |

**Query operations** (response sent as `effect_response`):

| Op | Response fields |
|----|-----------------|
| `get_size` | width, height |
| `get_position` | x, y |
| `get_mode` | mode |
| `get_scale_factor` | scale_factor |
| `is_maximized` | maximized |
| `is_minimized` | minimized |
| `screenshot` | width, height, bytes_len, rgba |
| `raw_id` | raw_id, platform |
| `monitor_size` | width, height (logical pixels) |
| `set_icon` | icon_data (base64 RGBA), width, height |

These accept an optional `request_id` field in the payload, echoed
back in the response for correlation.

### SystemOp

Run a system-level operation that is not tied to a specific window.

Uses the unified `_op` envelope: op-specific data lives under `payload`.

```json
{
  "type": "system_op",
  "session": "s1",
  "op": "allow_automatic_tabbing",
  "payload": { "enabled": true }
}
```

**Operations:**

| Op | Description |
|----|-------------|
| `allow_automatic_tabbing` | macOS automatic tab grouping (enabled: bool) |

### SystemQuery

Query system-level state that is not tied to a specific window.

Uses the unified `_op` envelope: query-specific data lives under `payload`.

```json
{
  "type": "system_query",
  "session": "s1",
  "op": "get_system_theme",
  "payload": { "tag": "theme-check" }
}
```

**Operations** (response sent as `op_query_response`):

| Op | Response kind | Response data |
|----|---------------|---------------|
| `get_system_theme` | `system_theme` | `"light"` or `"dark"` |
| `get_system_info` | `system_info` | CPU, memory, GPU info object |

### Effect

Request a platform effect (file dialog, clipboard, notification).

```json
{
  "type": "effect",
  "session": "s1",
  "id": "req-1",
  "kind": "file_open",
  "payload": {
    "title": "Open File",
    "directory": "/home/user",
    "filters": [["Text (*.txt)", "*.txt"], ["All Files", "*"]]
  }
}
```

**Effect kinds:**

| Kind | Payload | Response |
|------|---------|----------|
| `file_open` | title, directory, filters | path |
| `file_open_multiple` | title, directory, filters | paths (array) |
| `file_save` | title, directory, filters, default_name | path |
| `directory_select` | title, directory | path |
| `directory_select_multiple` | title, directory | paths (array) |
| `clipboard_read` | -- | text |
| `clipboard_write` | text | -- |
| `clipboard_read_html` | -- | html |
| `clipboard_write_html` | html, alt_text (optional) | -- |
| `clipboard_clear` | -- | -- |
| `clipboard_read_primary` | -- | text (Linux only; `unsupported` elsewhere) |
| `clipboard_write_primary` | text | -- (Linux only; `unsupported` elsewhere) |
| `notification` | title, body, icon, timeout, urgency, sound | -- |

**Notification options.** The `notification` effect accepts optional fields
beyond `title` and `body`:

| Field | Type | Description |
|-------|------|-------------|
| `icon` | string | Icon name (freedesktop icon spec, e.g. `"dialog-information"`) |
| `timeout` | number | Timeout in milliseconds |
| `urgency` | string | `"low"`, `"normal"` (default), or `"critical"` |
| `sound` | string | Sound theme name (e.g. `"message-new-instant"`) |

**Security notes for effects.**

- **File dialog results (`file_open`, `file_open_multiple`, `file_save`,
  `directory_select`, `directory_select_multiple`):** the returned path
  reflects the user's selection at the moment the dialog resolved. File
  metadata can change between the dialog returning and the host acting
  on the path (TOCTOU: time-of-check to time-of-use). Hosts that read,
  write, or execute based on a returned path should re-verify the file
  state (existence, type, permissions, symlink target) after opening
  rather than trusting the path alone.
- **`clipboard_write_html`:** the HTML string is written to the OS
  clipboard verbatim. No sanitisation is applied. Apps that accept
  user-supplied HTML and paste it to the clipboard must sanitise before
  calling this effect; the receiving application decides how to render
  the payload and may execute embedded scripts or load external
  resources depending on the target. `alt_text` is stored as the
  plain-text fallback for clipboards that refuse HTML.
- **`notification`:** `title`, `body`, `icon`, and `sound` are forwarded
  as-is to the platform notification daemon (DBus org.freedesktop on
  Linux, Windows Toast, macOS User Notifications). Specific daemons
  interpret the fields differently; older freedesktop daemons historically
  interpret markup in `body`, icon strings starting with `/` are treated
  as file paths, and sound names are resolved against the active
  sound theme. Hosts that surface untrusted strings in these fields
  should sanitise before calling this effect.

### ImageOp

Manage in-memory image handles for use by image widgets.

Uses the unified `_op` envelope: op-specific data (including the image
`handle`, `data`, `pixels`, `width`, `height`) lives under `payload`.

```json
{
  "type": "image_op",
  "session": "s1",
  "op": "create_image",
  "payload": {
    "handle": "sprite-1",
    "data": "<base64-encoded PNG/JPEG bytes>"
  }
}
```

Or with raw RGBA pixels:

```json
{
  "type": "image_op",
  "session": "s1",
  "op": "create_image",
  "payload": {
    "handle": "sprite-1",
    "pixels": "<base64-encoded RGBA bytes>",
    "width": 64,
    "height": 64
  }
}
```

| Op | Description |
|----|-------------|
| `create_image` | Create or replace an image handle |
| `update_image` | Same as create_image |
| `delete_image` | Remove an image handle |

In MessagePack mode, `data` and `pixels` can be sent as raw binary
(no base64 encoding needed).

### Query

Inspect the tree or find widgets by selector.

```json
{
  "type": "query",
  "session": "s1",
  "id": "q1",
  "target": "find",
  "selector": {"by": "id", "value": "btn1"}
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Request ID for response correlation |
| `target` | string | `"find"` (find widget) or `"tree"` (full tree) |
| `selector` | object | Selector for find queries (see below) |

**Selector format:**

| by | Description |
|----|-------------|
| `id` | Find by node ID |
| `text` | Find by text content |
| `role` | Find by a11y role |
| `label` | Find by a11y label |
| `focused` | Find the focused widget (no value field needed) |

When `by` is `"id"`, the selector value can include the `window#`
prefix to limit the lookup to one window (e.g. `"main#btn1"`).

**Selector search semantics:**

Selectors search the tree depth-first (max depth 256). The first
matching node is returned.

| by | Matches against |
|----|----------------|
| `id` | Exact match on node `id` field |
| `text` | Node `props.content`, `props.label`, `props.value`, or `props.placeholder` |
| `role` | Node `props.a11y.role`, or falls back to `type` field (e.g. type `"button"` matches role `"button"`) |
| `label` | Node `props.a11y.label`, or falls back to `props.label` or `props.content` |
| `focused` | Node with `props.focused == true` or `props.a11y.focused == true` |

**When not found:** Query returns `data: null`. Interact returns
empty events list.

Response: `query_response` with `id`, `target`, `data`.

**Example: full tree query**

```json
{
  "type": "query",
  "session": "s1",
  "id": "q2",
  "target": "tree",
  "selector": {}
}
```

Returns the entire tree as `data`, or `null` if no tree has been
sent via Snapshot.

### Interact

Simulate user interactions (click, type, etc.). Available in both
all modes (gui, headless, mock) for programmatic inspection and
interaction.

```json
{
  "type": "interact",
  "session": "s1",
  "id": "i1",
  "action": "click",
  "selector": {"by": "id", "value": "submit_btn"},
  "payload": {}
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Request ID for response correlation |
| `action` | string | Interaction type (see action table below) |
| `selector` | object | Target widget selector (see Query for format). Required for widget-specific actions, optional for global actions like `press`/`release`/`move_to`/`scroll`. |
| `payload` | object | Action-specific parameters (see payload table below) |

**Actions and their iced event mappings:**

| Action | Iced events injected | Typical widget response |
|--------|---------------------|------------------------|
| `click` | CursorMoved, ButtonPressed, ButtonReleased | Click |
| `toggle` | CursorMoved, ButtonPressed, ButtonReleased | Toggle |
| `select` | CursorMoved, ButtonPressed, ButtonReleased | Select |
| `type_text` | KeyPressed + KeyReleased per character | Input per char |
| `type_key` | KeyPressed, KeyReleased | Depends on widget |
| `press` | KeyPressed | Depends on widget |
| `release` | KeyReleased | Depends on widget |
| `submit` | KeyPressed(Enter), KeyReleased(Enter) | Submit |
| `scroll` | WheelScrolled | Scroll |
| `move_to` | CursorMoved | -- |
| `slide` | synthetic only | Slide |
| `paste` | synthetic only | Paste |
| `sort` | synthetic only | Sort |
| `canvas_press` | CursorMoved, ButtonPressed | Canvas press (also triggers element events) |
| `canvas_release` | CursorMoved, ButtonReleased | Canvas release (also triggers element events) |
| `canvas_move` | CursorMoved | Canvas move (also triggers element enter/leave) |
| `click_element` | CursorMoved, ButtonPressed, ButtonReleased | Click at coordinates within canvas |
| `focus` | KeyPressed (Tab) | Tab into canvas |
| `pane_focus_cycle` | synthetic only | Pane focus cycle |

Actions marked **synthetic only** have no iced event equivalent
(e.g. slider requires a precise mouse drag, paste has no iced
input event). The renderer produces synthetic OutgoingEvents
directly without widget processing.

**Action payloads:**

| Action | Payload fields | Description |
|--------|---------------|-------------|
| `click` | (none) | |
| `toggle` | `value` (bool) | Toggle value. Defaults to `false` if omitted. SDKs should compute the inverse of the current widget state (e.g. read `is_checked` from tree props, invert it). In headless mode, the real widget value is captured from iced regardless of this field. |
| `select` | `value` (string) | Value to select from a pick_list, combo_box, or radio group. |
| `type_text` | `text` (string, required) | Text to type into the widget |
| `type_key` | `key` (string, required) | Key to press and release (see Key format below) |
| `press` | `key` (string, required) | Key to press (key down only) |
| `release` | `key` (string, required) | Key to release (key up only) |
| `submit` | `value` (string) | Submit value. Defaults to `""` if omitted. The renderer does not read from the tree; SDKs should read the widget's current `props.value` and provide it. |
| `scroll` | `delta_x` (number), `delta_y` (number) | Scroll deltas in lines |
| `move_to` | `x` (number), `y` (number) | Cursor position in logical pixels |
| `slide` | `value` (number, required) | Slider value |
| `paste` | `text` (string, required) | Text to paste |
| `sort` | `column` (string, required) | Column key to sort by |
| `canvas_press` | `x` (number), `y` (number) | Canvas coordinates |
| `canvas_release` | `x` (number), `y` (number) | Canvas coordinates |
| `canvas_move` | `x` (number), `y` (number) | Canvas coordinates |
| `click_element` | `x` (number), `y` (number) | Element center in canvas coordinates |

| `pane_focus_cycle` | (none) | |

**Key format:**

Keys for `press`, `release`, and `type_key` actions can be specified
in two formats:

Combined format (modifiers joined with `+`):
```json
{"key": "ctrl+shift+s"}
```

Explicit modifiers:
```json
{"key": "a", "modifiers": {"ctrl": true, "shift": false, "alt": false, "logo": false}}
```

Modifier aliases: `command` maps to `ctrl`, `super` and `meta` map
to `logo`.

Named keys (case-insensitive, aliases separated by `/`):

`Enter`/`Return`, `Tab`, `Space`, `Escape`/`Esc`,
`Backspace`, `Delete`/`Del`, `ArrowUp`/`Up`,
`ArrowDown`/`Down`, `ArrowLeft`/`Left`, `ArrowRight`/`Right`,
`Home`, `End`, `PageUp`/`Page_Up`, `PageDown`/`Page_Down`,
`F1` through `F12`.

Single characters (e.g. `"a"`, `"1"`, `"/"`) are sent as character
key events. Multi-character strings that don't match a named key
are sent as-is (the renderer does not reject them).

In **windowed mode**, all actions produce synthetic events regardless:
the interact protocol is a scripting convenience, not a substitute
for real user input via iced subscriptions.

#### Headless mode: iterative interact with round-trips

In `--headless` mode, the renderer injects iced events one at a
time. When an event produces widget Messages, the renderer emits
an `interact_step` and waits for the host to process the events
and send back a Snapshot with the updated tree before continuing.
This matches production behaviour where each event triggers a full
host round-trip.

```
Host -> Renderer:  interact(type_key, ...)
Renderer -> Host:  interact_step(events: [key_press])
Host -> Renderer:  snapshot(updated_tree)
Renderer -> Host:  interact_step(events: [key_release])
Host -> Renderer:  snapshot(updated_tree)
Renderer -> Host:  interact_response(events: [])
```

The final `interact_response` carries an empty events list when
steps were used (events were already delivered via steps). For
actions with no iced events (synthetic-only), no steps are emitted
and all events are in the final response.

#### Mock mode: synthetic events

In `--mock` mode, there is no iced renderer. All events are
synthetic, constructed from the action name and selector without
widget processing. No `interact_step` messages are emitted. All
events are in the final `interact_response`.

### TreeHash

Compute a SHA-256 hash of the renderer's current tree (serialized
as JSON). Used for structural regression testing.

```json
{
  "type": "tree_hash",
  "session": "s1",
  "id": "th1",
  "name": "after_click"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Request ID for response correlation |
| `name` | string | Label for this hash capture |

Response: `tree_hash_response`.

**Scope.** The hash covers tree structure and props only (the
serialized JSON representation of the node tree). Mutable widget
state (editor content, scroll position, slider value, canvas
cache state, etc.) is not included. Two trees with identical
structure and props will produce the same hash even if their
runtime widget state differs.

### Screenshot

Capture rendered pixels. In headless mode, renders the tree via
tiny-skia and returns RGBA pixel data. In mock mode, returns an
empty stub.

```json
{
  "type": "screenshot",
  "session": "s1",
  "id": "sc1",
  "name": "homepage",
  "width": 1024,
  "height": 768
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Request ID |
| `name` | string | Label for this screenshot |
| `width` | number | Viewport width in pixels (optional, default 1024) |
| `height` | number | Viewport height in pixels (optional, default 768) |

Response: `screenshot_response`.

**Cross-platform determinism.** Screenshot hashes may differ across
platforms due to font rendering and hinting. For deterministic
screenshot comparison in CI, pin fonts via the `fonts` array in
Settings and avoid relying on system fonts. The headless backend
(tiny-skia) produces consistent output across Linux environments
when the same fonts are loaded.

### Reset

Reset all session state: tree, caches, images, theme, widgets.
In multiplexed mode, the session thread is torn down and the session
ID can be reused.

```json
{
  "type": "reset",
  "session": "s1",
  "id": "r1"
}
```

Response: `reset_response`.

**Multiplexed mode ordering constraint.** In multiplexed mode, the host
must wait for the `reset_response` before sending new messages to a
recycled session ID. Without this, the host may receive stale responses
from the old session interleaved with responses from the new one, because
the old session thread may still be draining its channel when the reader
thread creates a replacement.

### AdvanceFrame

Advance the animation clock by one frame in headless/mock mode. If
`on_animation_frame` is subscribed, emits an `animation_frame` event
with the given timestamp. Used for deterministic animation testing.
Windowed mode is driven by iced frame ticks, so the renderer ignores
`AdvanceFrame` there and logs a warning.

```json
{
  "type": "advance_frame",
  "session": "s1",
  "timestamp": 16000
}
```

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | number | Frame timestamp, passed through to the `animation_frame` event as-is. By convention, milliseconds (matching the windowed mode's `Instant::as_millis()` output). |

### RegisterEffectStub

Register a canned response for an effect kind. While registered, the
renderer returns the stub response immediately instead of performing
the real effect. Used by test frameworks to avoid OS dialogs.

```json
{
  "type": "register_effect_stub",
  "session": "s1",
  "kind": "file_open",
  "response": {"path": "/tmp/test.txt"}
}
```

| Field | Type | Description |
|-------|------|-------------|
| `kind` | string | Built-in effect kind to stub (e.g. `"file_open"`, `"file_save"`, `"clipboard_read"`). Unknown kinds are rejected with an error acknowledgement. |
| `response` | object | Canned response payload. Shape must match the real effect response. |

Response: `effect_stub_register_ack`.

### UnregisterEffectStub

Remove a previously registered effect stub, restoring real behavior.

```json
{
  "type": "unregister_effect_stub",
  "session": "s1",
  "kind": "file_open"
}
```

Response: `effect_stub_unregister_ack`.

---

## Outgoing messages (renderer -> host)

### Request-response reference

Every request message produces exactly one response. The `id` and
`session` fields are echoed back for correlation.

| Request | Response type | Notes |
|---------|--------------|-------|
| Query | `query_response` | |
| Interact | `interact_response` | May be followed by `interact_step` messages in headless mode |
| TreeHash | `tree_hash_response` | |
| Screenshot | `screenshot_response` | |
| Reset | `reset_response` | |
| Effect | `effect_response` | |
| RegisterEffectStub | `effect_stub_register_ack` | |
| UnregisterEffectStub | `effect_stub_unregister_ack` | |
| WidgetOp (query ops) | `op_query_response` | tree_hash, find_focused, list_images |
| WindowOp (query ops) | `effect_response` | get_size, get_position, get_mode, etc. |
| SystemQuery | `op_query_response` | get_system_theme, get_system_info |

Messages without responses: Settings, Snapshot, Patch,
Subscribe, Unsubscribe, Command, Commands, WidgetOp
(non-query), WindowOp (non-query), SystemOp, ImageOp,
AdvanceFrame.

### diagnostic

Structured diagnostic emitted by the renderer when something
unexpected happens (invalid settings, font family not found, content
truncated by an internal cap, widget panic, and so on). Hosts can
surface these to end users, log them, or feed them into a test
assertion. Each diagnostic is also mirrored to the renderer's log
channel so existing log consumers still see the message.

```json
{
  "type": "diagnostic",
  "session": "s1",
  "level": "warn",
  "diagnostic": {
    "kind": "font_family_not_found",
    "family": "Inter"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Always `"diagnostic"` |
| `session` | string | Session the diagnostic is attributable to (may be empty for process-level sites) |
| `level` | string | Severity: `"info"`, `"warn"`, `"error"` |
| `diagnostic` | object | Typed payload. `kind` identifies the variant; remaining fields are variant-specific. See [`plushie_core::Diagnostic`](../crates/plushie-core/src/diagnostic.rs) for the full shape per variant. |

### event

User interaction or subscription event.

```json
{
  "type": "event",
  "session": "s1",
  "family": "click",
  "id": "main#toolbar/btn-1"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Always `"event"` |
| `session` | string | Session that produced this event |
| `family` | string | Event kind (see tables below) |
| `id` | string | Node ID that produced the event (includes `window#` prefix for widget events) |
| `value` | any | Event value (optional, carries all event-specific data) |
| `tag` | string | Subscription tag (optional, for subscription events) |
| `modifiers` | object | Keyboard modifiers (optional) |
| `captured` | bool | Whether a widget consumed this event (optional, subscription events only) |

Fields that are null or absent are omitted from the serialized output.

The window that produced a widget event is encoded in the node ID
via the `window#` prefix (e.g. `"main#btn-1"`). Subscription events
use bare IDs without the window prefix.

**Event capture status.** All keyboard, mouse, touch, and IME subscription
events include an optional `captured` boolean. When `true`, an iced widget
already consumed the event (e.g. a TextEditor captured a Tab key press).
When `false` or absent, no widget handled the event. Widget-level events
(click, input, submit, etc.) never carry this field.

#### Widget events

Produced by widget interactions. The `id` field is the full wire ID
(including the `window#` prefix).

| Family | Fields | Description |
|--------|--------|-------------|
| `click` | id | Button or clickable clicked |
| `input` | id, value (string) | Text input changed |
| `submit` | id, value (string) | Text input submitted (Enter) |
| `toggle` | id, value (bool) | Checkbox or toggler changed |
| `slide` | id, value (f64) | Slider moved |
| `slide_release` | id, value (f64) | Slider released |
| `select` | id, value (string) | Pick list or radio selected |
| `paste` | id, value (string) | Text pasted into input |
| `option_hovered` | id, value (string) | Combo box option hovered |
| `resize` | id, value: {width, height} | Sensor widget resized. Coalescable (Replace). |
| `scrolled` | id, value: {absolute_x, absolute_y, relative_x, relative_y, bounds_width, bounds_height, content_width, content_height} | Scrollable viewport changed. NOT a pointer event. Coalescable (Replace). |
| `sort` | id, value: {column} | Table column sort clicked |
| `key_binding` | id, value | TextEditor key binding rule matched |
| `open` | id | PickList or ComboBox menu opened |
| `close` | id | PickList or ComboBox menu closed |

Renderer-side errors also use the normal `event` envelope:

| Family | Fields | Description |
|--------|--------|-------------|
| `error` | id, value | Renderer or protocol error |

Current renderer error payloads include:

| `value.kind` | Other fields | Description |
|-------------|--------------|-------------|
| `command` | `reason`, `id`, `family`, `message`, `widget_type` (optional) | Command failed. `reason` is currently `"unknown_node"`, `"poisoned"`, or `"panic"`. |

#### Pointer events

All pointer interactions (from `pointer_area`, canvas, and touch
input) use a unified set of event families. The same families are
emitted regardless of widget type; the `pointer` field in the
value distinguishes the input device.

| Family | Value fields | Coalescable | Description |
|--------|-------------|-------------|-------------|
| `press` | `{x, y, button, pointer, finger?, modifiers}` | No | Pointer button down with coordinates |
| `release` | `{x, y, button, pointer, finger?, modifiers}` | No | Pointer button up |
| `move` | `{x, y, pointer, finger?, modifiers}` | Replace | Pointer movement |
| `scroll` | `{x, y, delta_x, delta_y, pointer, modifiers}` | Accumulate (delta_x, delta_y) | Wheel/trackpad scroll input |
| `enter` | (none) | No | Pointer entered widget area |
| `exit` | (none) | No | Pointer left widget area |
| `click` | (none) | No | Semantic activation (high-level, no coordinates) |
| `double_click` | `{x, y, pointer, modifiers}` | No | Double click |
| `resize` | `{width, height}` | Replace | Widget resized (from sensor) |

**Pointer type.** The `pointer` field identifies the input device:

| Value | Description |
|-------|-------------|
| `"mouse"` | Mouse or trackpad |
| `"touch"` | Touchscreen finger |
| `"pen"` | Stylus or pen tablet |

**Finger field.** Touch events include a `finger` field (u64) that
identifies the finger. This field is absent for mouse and pen input.

**Button field.** The `button` field in `press` and `release` events
identifies the button: `"left"`, `"right"`, `"middle"`, `"back"`,
or `"forward"`.

**Modifiers.** The `modifiers` object has the shape:
`{shift, ctrl, alt, logo, command}` (all booleans).

These families are emitted by:
- `pointer_area` widget - mouse/touch/pen interactions on an
  invisible overlay area.
- `canvas` widget - pointer interactions on the canvas surface.
  The `id` field is the canvas node ID.
- Canvas interactive elements - pointer interactions on canvas
  elements. The `id` field is the element's scoped wire ID.

#### Scrolled (viewport state)

The `scrolled` event is distinct from `scroll` (raw wheel input).
It is emitted by the `scrollable` widget when the viewport position
changes, reporting where the viewport ended up:

| Field | Type | Description |
|-------|------|-------------|
| `absolute_x` | f32 | Absolute horizontal scroll offset |
| `absolute_y` | f32 | Absolute vertical scroll offset |
| `relative_x` | f32 | Relative horizontal offset (0.0 to 1.0) |
| `relative_y` | f32 | Relative vertical offset (0.0 to 1.0) |
| `bounds_width` | f32 | Viewport width |
| `bounds_height` | f32 | Viewport height |
| `content_width` | f32 | Total content width |
| `content_height` | f32 | Total content height |

Coalescable: Replace (only latest viewport state matters).

Pane grid events:

| Family | Fields | Description |
|--------|--------|-------------|
| `pane_resized` | id, value: {split, ratio} | Pane divider moved |
| `pane_dragged` | id, value: {action, pane, target, region, edge} | Pane dragged (action: picked/dropped/canceled) |
| `pane_clicked` | id, value: {pane} | Pane clicked |
| `pane_focus_cycle` | id, value: {pane} | Pane focus cycled (F6/Shift+F6) |

#### Subscription events

Produced by registered subscriptions. The `tag` field contains the
tag from the subscription registration.

**Keyboard:**

| Family | Fields |
|--------|--------|
| `key_press` | tag, value: {key, modified_key, physical_key, location, text, repeat}, modifiers |
| `key_release` | tag, value: {key, modified_key, physical_key, location}, modifiers |
| `modifiers_changed` | tag, modifiers: {shift, ctrl, alt, logo, command} |

**Mouse:**

| Family | Fields |
|--------|--------|
| `cursor_moved` | tag, value: {x, y} |
| `cursor_entered` | tag |
| `cursor_left` | tag |
| `button_pressed` | tag, value (button name) |
| `button_released` | tag, value (button name) |
| `wheel_scrolled` | tag, value: {delta_x, delta_y, unit} |

**Touch:**

| Family | Fields |
|--------|--------|
| `finger_pressed` | tag, value: {id, x, y} |
| `finger_moved` | tag, value: {id, x, y} |
| `finger_lifted` | tag, value: {id, x, y} |
| `finger_lost` | tag, value: {id, x, y} |

**Subscription events vs widget pointer events.** Subscription
pointer events (`cursor_moved`, `button_pressed`, `finger_pressed`,
etc.) use iced-native family names and data shapes. Widget pointer
events (`press`, `release`, `move`, `scroll`, etc.) use the unified
pointer model documented above. Both coexist on the wire; the SDK
is responsible for merging them into a consistent event model on the
host side. For example, a `button_pressed` subscription event and a
`press` widget event represent the same physical action at different
abstraction levels.

**IME (input method):**

| Family | Fields |
|--------|--------|
| `ime_opened` | tag |
| `ime_preedit` | tag, value: {text, cursor} |
| `ime_commit` | tag, value: {text} |
| `ime_closed` | tag |

**Window lifecycle:**

| Family | Fields |
|--------|--------|
| `window_opened` | tag, value: {window_id, position: {x, y}, width, height, scale_factor} |
| `window_closed` | tag, value: {window_id} |
| `window_close_requested` | tag, value: {window_id} |
| `window_moved` | tag, value: {window_id, x, y} |
| `window_resized` | tag, value: {window_id, width, height} |
| `window_focused` | tag, value: {window_id} |
| `window_unfocused` | tag, value: {window_id} |
| `window_rescaled` | tag, value: {window_id, scale_factor} |
| `file_hovered` | tag, value: {window_id, path} |
| `file_dropped` | tag, value: {window_id, path} |
| `files_hovered_left` | tag, value: {window_id} |

**Other:**

| Family | Fields |
|--------|--------|
| `animation_frame` | tag, value: {timestamp} |
| `theme_changed` | tag, value (light/dark) |
| `all_windows_closed` | (none; emitted when last window closes) |

**`animation_frame` timing.** In windowed mode, timestamps are
monotonic milliseconds since the first animation frame after startup
(or since the last Reset; the epoch resets so animations restart
from zero). In headless/mock mode, `AdvanceFrame` passes timestamps
through directly and the renderer does not apply any epoch offset.

### effect_response

Response to an Effect.

```json
{
  "type": "effect_response",
  "session": "s1",
  "id": "req-1",
  "status": "ok",
  "result": { "path": "/home/user/file.txt" }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session that produced this response |
| `id` | string | Matches the request id |
| `status` | string | `"ok"`, `"cancelled"`, `"error"`, or `"unsupported"` |
| `result` | any | Result data (when status is ok) |
| `error` | string | Error message (when status is error) |

The `"cancelled"` status is returned when the user dismisses a dialog
without selecting (e.g. clicks Cancel on a file picker). It carries no
`result` or `error` field. Clients should treat it as a normal outcome,
not as a failure.

The `"unsupported"` status is returned when the renderer recognizes the
effect kind but cannot perform it in the current mode or backend.

Window query operations (get_size, get_position, etc.) also use this
format, with the `id` set to the window_id.

### query_response

Response to a Query message.

```json
{
  "type": "query_response",
  "session": "s1",
  "id": "q1",
  "target": "find",
  "data": {"id": "btn1", "type": "button", "props": {}, "children": []}
}
```

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session |
| `id` | string | Matches query request id |
| `target` | string | Echoes the query target |
| `data` | any | Query result (node object for find, full tree for tree, null if not found) |

### op_query_response

Response to widget op queries (`tree_hash`, `find_focused`,
`list_images`, `system_theme`, `system_info`).

```json
{
  "type": "op_query_response",
  "session": "s1",
  "kind": "find_focused",
  "tag": "focus_check",
  "data": {"focused": "input1"}
}
```

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session |
| `kind` | string | Query kind (tree_hash, find_focused, list_images, system_theme, system_info) |
| `tag` | string | Tag from the widget op request |
| `data` | object | Query-specific result |

**Data shapes by query kind:**

| kind | data | Description |
|------|------|-------------|
| `tree_hash` | `{"hash": "sha256hex"}` | SHA-256 hash of the tree |
| `find_focused` | `{"focused": "widget_id"}` | ID of focused widget, or null |
| `list_images` | `{"handles": ["name1", ...]}` | All registered image handle names |
| `system_theme` | `"light"` or `"dark"` | Current OS theme preference |
| `system_info` | `{"cpu_brand": "...", "cpu_cores": N, "memory_total": N, "memory_used": N, "graphics_backend": "...", "graphics_adapter": "...", ...}` | System hardware info |

### interact_step

Emitted during headless iterative interact when an injected iced
event produces widget Messages.

```json
{
  "type": "interact_step",
  "session": "s1",
  "id": "i1",
  "events": [{"type": "event", "session": "s1", "family": "click", "id": "btn1"}]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Always `"interact_step"` |
| `session` | string | Session that produced this step |
| `id` | string | Matches the interact request id |
| `events` | array | OutgoingEvent objects captured from this iced event |

The host must process the events (update model, re-render tree)
and send a Snapshot or Patch back to the renderer before the next
event is injected.

### interact_response

Final response to an Interact message, signalling the interaction
is complete.

```json
{
  "type": "interact_response",
  "session": "s1",
  "id": "i1",
  "events": []
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Always `"interact_response"` |
| `session` | string | Session that produced this response |
| `id` | string | Matches the interact request id |
| `events` | array | Empty when steps were used; contains all events for synthetic/mock actions |

In headless mode with steps, the events list is empty (all events
were delivered via prior `interact_step` messages). In mock mode
or for synthetic-only actions, no steps are emitted and all events
are in this final response.

### tree_hash_response

Response to a TreeHash message.

```json
{
  "type": "tree_hash_response",
  "session": "s1",
  "id": "th1",
  "name": "after_click",
  "hash": "a1b2c3..."
}
```

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session |
| `id` | string | Matches request id |
| `name` | string | Echoes the capture name |
| `hash` | string | SHA-256 hex hash of the tree serialized as JSON |

### screenshot_response

Response to a Screenshot message. The structured fields below are always
present. The optional `rgba` payload is encoded as base64 in JSON mode
and as native binary in MessagePack mode. In mock mode, `rgba` is
omitted and the renderer returns the existing empty placeholders.

```json
{
  "type": "screenshot_response",
  "session": "s1",
  "id": "sc1",
  "name": "homepage",
  "hash": "d4e5f6...",
  "width": 1024,
  "height": 768,
  "rgba": "<binary pixel data>"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session |
| `id` | string | Matches request id |
| `name` | string | Echoes the capture name |
| `hash` | string | SHA-256 hex hash of RGBA data (empty string in mock mode) |
| `width` | number | Rendered width in pixels (0 in mock mode) |
| `height` | number | Rendered height in pixels (0 in mock mode) |
| `rgba` | binary | RGBA pixel data (base64 in JSON, native binary in MessagePack). Omitted when no pixel buffer is returned, including mock mode. |

Maximum screenshot dimension: 16384 pixels (width and height are
clamped to this limit).

### reset_response

Response to a Reset message.

```json
{
  "type": "reset_response",
  "session": "s1",
  "id": "r1",
  "status": "ok"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session |
| `id` | string | Matches request id |
| `status` | string | Always `"ok"` |

### effect_stub_register_ack

Acknowledgment that an effect stub registration was accepted or
rejected.

```json
{
  "type": "effect_stub_register_ack",
  "session": "s1",
  "kind": "file_open",
  "status": "registered"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Always `"effect_stub_register_ack"` |
| `session` | string | Session |
| `kind` | string | Effect kind that was registered |
| `status` | string | `"registered"` when accepted, `"error"` when rejected |

### effect_stub_unregister_ack

Acknowledgment that an effect stub removal was accepted or rejected.

```json
{
  "type": "effect_stub_unregister_ack",
  "session": "s1",
  "kind": "file_open",
  "status": "unregistered"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Always `"effect_stub_unregister_ack"` |
| `session` | string | Session |
| `kind` | string | Effect kind that was removed |
| `status` | string | `"unregistered"` when accepted, `"error"` when rejected |

---

## Execution modes

The renderer runs in one of three modes, selected by CLI flags.
Behaviour differences that affect SDK implementations:

### Windowed mode (default, `"mode": "windowed"`)

Full iced rendering with real windows. Production mode.

- All messages work as documented.
- Interact always produces synthetic events (not captured from iced).
- Subscriptions emit real events from the window system.
- Effects (file dialogs, clipboard) execute natively.

### Headless mode (`--headless`, `"mode": "headless"`)

Real rendering via tiny-skia. No display server required.

- Interact injects real iced events and captures widget output.
  May emit `interact_step` messages requiring snapshot round-trips.
- Screenshot returns real RGBA pixel data.
- Effects always return `"cancelled"` status (no platform dialogs).
- Subscriptions work (events emitted on registration match).
- Window operations are no-ops (no real windows).

**Announce events.** In headless and mock modes, `announce` widget ops
emit a synthetic event instead of dispatching to the platform
accessibility layer (which does not exist without a display server):

```json
{"type": "event", "session": "", "family": "announce", "id": "", "value": {"text": "Item saved successfully", "politeness": "polite"}}
```

This allows host test suites to verify that announcements are triggered
correctly. In windowed mode, announces go directly to the platform
screen reader API and do NOT produce a wire event.

### Mock mode (`--mock`, `"mode": "mock"`)

No rendering. Protocol-only. Fastest mode for testing.

- Interact always produces synthetic events. No `interact_step`.
- Screenshot returns an empty stub (hash `""`, no rgba).
- Effects always return `"cancelled"` status.
- Subscriptions register/unregister but no events are emitted.
- Window operations and widget operations (focus, scroll) are no-ops.

---

## Transport modes

The `transport` field in the hello message reports how the renderer
is connected to the host.

### stdio (default)

The renderer reads from stdin and writes to stdout. The host spawns
plushie as a subprocess and communicates over the pipe.

### exec (`--exec <command>`)

The renderer spawns a command via the system shell (`sh -c` on Unix,
`cmd /c` on Windows) and uses its stdin/stdout as the protocol
channel. The child's stderr is forwarded to plushie's stderr with a
`[remote]` prefix.

This enables remote rendering scenarios (e.g. `--exec "ssh host plushie"`)
where the host process runs on a different machine. All modes
(windowed, headless, mock) work with `--exec`.

---

## Binary data

Fields that carry binary data (`pixels`, `data` in ImageOp;
`rgba` in screenshot responses) are encoded differently depending on
the wire format:

- **JSON**: Base64-encoded string (standard alphabet, no padding required)
- **MessagePack**: Native binary type (no encoding needed)

The renderer accepts both formats transparently via a custom
deserializer.

---

## Float handling

All floating-point values in outgoing events are sanitized before
serialization. NaN and infinity are replaced with `0.0`. This prevents
JSON serialization errors and ensures all values are valid numbers.

---

## Limits

| Limit | Value | Applies to |
|-------|-------|-----------|
| MAX_MESSAGE_SIZE | 64 MiB | Wire message size (both codecs) |
| MAX_RMPV_DEPTH | 128 | MessagePack nesting depth |
| MAX_TREE_DEPTH | 256 | Widget tree recursion depth for rendering, caching, and window search |
| MAX_FONT_BYTES | 16 MiB | Decoded font data from `load_font` widget op |
| MAX_LOADED_FONTS | 256 | Runtime font loads per process lifetime |
| MAX_IMAGES | 4096 | Image handles in the registry |
| MAX_TOTAL_IMAGE_BYTES | 1 GiB | Aggregate image data in the registry |
| MAX_DIMENSION | 16384 | Single image dimension (width or height) |
| MAX_PIXEL_BYTES | 256 MiB | Single RGBA image buffer |
| MAX_SCREENSHOT_DIMENSION | 16384 | Screenshot width or height |
| MAX_MARKDOWN_CONTENT | 1 MiB | Markdown widget content (truncated with warning) |
| MAX_TEXT_EDITOR_CONTENT | 10 MiB | Text editor initial content (truncated with warning) |
| MAX_FONT_FAMILY_LEN | 256 chars | Font family name length |
| MAX_FONT_FAMILY_CACHE | 1024 entries | Interned font family name cache |
| MAX_DASH_CACHE | 1024 entries | Interned canvas dash pattern cache |
| MAX_WINDOW_DIM | 16384 | Window width or height |

Limits protect against OOM and excessive computation from untrusted or
buggy input. Most limits return errors to the host; content limits
(markdown, text editor) truncate with warnings logged to stderr.

Image and font limits are per-process; resetting a session does not
reset them. This prevents a misbehaving session from exhausting
process-wide resources that would affect other sessions in multiplexed
mode.

---

## Error handling

The renderer is resilient to malformed input. Errors are logged to
stderr but do not crash the process.

- **Decode errors** (malformed JSON, invalid msgpack): Message is
  skipped. No response is sent. The renderer continues reading.
- **Unknown message type**: Deserialization fails. Message skipped.
- **Missing required fields**: Deserialization fails. Message skipped.
- **Unknown widget op or window op**: Logged as warning. No response.
- **Unknown interact action**: Empty events in response.
- **Selector finds nothing**: Query returns `data: null`. Interact
  returns empty events.
- **Broken stdout pipe**: Renderer exits cleanly.
- **Protocol version mismatch**: SDK stops the bridge and shuts down
  the runtime.

---

## Message pipelining

The host can send multiple messages without waiting for responses.
The renderer processes messages sequentially within each session,
so responses arrive in the order requests were sent.

Fire-and-forget messages (Settings, Snapshot, Patch, Subscribe,
Unsubscribe, WidgetOp, WindowOp, ImageOp, WidgetCommand,
WidgetCommands, AdvanceFrame) can be sent freely at any time.

Request messages (Query, Interact, TreeHash, Screenshot, Reset,
Effect) can also be pipelined; the renderer queues them and
responds in order.

**Exception: interact steps.** During an Interact in headless mode,
the renderer may emit `interact_step` messages. When this happens,
the host **must** send a Snapshot or Patch back before the renderer
will continue to the next iced event. Do not send other messages
to the same session between an `interact_step` and the
corresponding Snapshot; the renderer is blocked waiting for the
tree update.

---

## Accessibility props

Any tree node can carry an `a11y` object in its `props` to control
accessibility behaviour. All fields are optional.

```json
{
  "a11y": {
    "role": "button",
    "label": "Submit form",
    "description": "Sends the form to the server",
    "hidden": false,
    "expanded": true,
    "required": false,
    "level": 2,
    "live": "polite",
    "busy": false,
    "invalid": false,
    "modal": false,
    "read_only": false,
    "mnemonic": "S",
    "toggled": null,
    "selected": null,
    "value": "current value",
    "orientation": "horizontal",
    "labelled_by": "label-node-id",
    "described_by": "desc-node-id",
    "error_message": "error-node-id",
    "disabled": false,
    "position_in_set": 3,
    "size_of_set": 10,
    "has_popup": "menu"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `role` | string | Accessible role (e.g. `"button"`, `"text_input"`, `"image"`) |
| `label` | string | Primary accessible label |
| `description` | string | Extended description |
| `hidden` | bool | Hide from assistive technology |
| `expanded` | bool | Expanded/collapsed state |
| `required` | bool | Required field indicator |
| `level` | number | Heading level (1-6) |
| `live` | string | Live region: `"polite"` or `"assertive"`. Omit (or send `null` in a patch) to clear the live-region attribute. |
| `busy` | bool | Suppresses AT announcements until cleared (maps to `aria-busy`). Omit to use widget auto-detection (e.g. sliders set busy during drag). Set explicitly to override. |
| `invalid` | bool | Validation failed |
| `modal` | bool | Modal container |
| `read_only` | bool | Read-only field |
| `mnemonic` | string | Keyboard mnemonic (single character) |
| `toggled` | bool | Toggle state |
| `selected` | bool | Selection state |
| `value` | string | Text value announced by AT |
| `orientation` | string | `"horizontal"` or `"vertical"` |
| `labelled_by` | string | Node ID of the labelling element |
| `described_by` | string | Node ID of the describing element |
| `error_message` | string | Node ID of the error message element |
| `disabled` | bool | Widget is disabled (not interactive) |
| `position_in_set` | number | Position in set (1-based, for list items, radios, tabs) |
| `size_of_set` | number | Total items in set (paired with `position_in_set`) |
| `has_popup` | string | Popup type: `"listbox"`, `"menu"`, `"dialog"`, `"tree"`, `"grid"` |

**Auto-inference:** Image and SVG widgets with an `alt` prop auto-populate
`label` from the alt text. Text input and text editor widgets auto-populate
`description` from their `placeholder` prop. Explicit `a11y` values always
take priority.

### Precedence

Host SDKs, the tree normalizer, and the renderer each contribute defaults
and overrides. Fields resolve highest-priority-first:

1. **Author explicit overrides.** Any `a11y.<field>` set by app code on
   a widget builder wins for that field. Other fields are unaffected:
   setting `a11y.label` does not clear an inferred `description`.
2. **Host SDK builder defaults.** Builders like `tooltip(...)`,
   `text_input(...).placeholder(...)`, `pick_list(...)`, etc. author
   common a11y fields directly on the tree (`role`, `described_by`,
   `description`). These are visible to test harnesses.
3. **Normalizer auto-population.** The tree normalizer fills in
   `a11y.role` from the widget type when unset and wires implicit
   radio groups from the shared `group` prop.
4. **widget-sdk fallback.** The widget SDK's `infer_a11y` provides a
   safety net for custom widgets not using the host builder defaults
   (placeholder -> description, alt -> label).
5. **iced native widget defaults.** At the bottom, the fork's iced
   widgets contribute baseline attributes (e.g. a button's `role`).

All levels compose through `A11y::merge` semantics so a widget-level
default never clobbers an explicit override and vice versa.

### Widget-specific accessibility props

Some widgets accept top-level props (outside the `a11y` object) that feed
into accessibility output.

| Widget | Prop | Type | Description |
|--------|------|------|-------------|
| `button` | `mnemonic` | string | Alt-key mnemonic. First character is used. Explicit `a11y.mnemonic` takes priority. |
| `button` | `access_key` | string | Alias for `mnemonic` |
| `checkbox` | `mnemonic` | string | Alt-key mnemonic. First character is used. Explicit `a11y.mnemonic` takes priority. |
| `checkbox` | `access_key` | string | Alias for `mnemonic` |
| `radio` | `mnemonic` | string | Alt-key mnemonic. First character is used. Explicit `a11y.mnemonic` takes priority. |
| `radio` | `access_key` | string | Alias for `mnemonic` |
| `image` | `alt` | string | Accessible label (auto-populates `a11y.label`) |
| `image` | `description` | string | Extended accessible description |
| `image` | `decorative` | bool | When true, hides the image from assistive technology. Use for purely visual images that don't convey information. |
| `svg` | `alt` | string | Accessible label (auto-populates `a11y.label`) |
| `svg` | `description` | string | Extended accessible description |
| `svg` | `decorative` | bool | When true, hides the SVG from assistive technology. Use for purely visual images that don't convey information. |
| `slider` | `label` | string | Accessible label (e.g. `"Volume"`). Without this, screen readers announce the value without context. |
| `slider` | `keyboard_step` | number | Overrides the base step used for keyboard movement and pointer snapping. `shift_step` still controls Shift-modified adjustment. |
| `vertical_slider` | `label` | string | Accessible label (e.g. `"Volume"`). Without this, screen readers announce the value without context. |
| `vertical_slider` | `keyboard_step` | number | Overrides the base step used for keyboard movement and pointer snapping. `shift_step` still controls Shift-modified adjustment. |
| `progress_bar` | `label` | string | Accessible label (e.g. `"Upload progress"`). |
| `qr_code` | `alt` | string | Accessible label for the QR code |
| `qr_code` | `description` | string | Extended description |
| `canvas` | `alt` | string | Accessible label for the canvas |
| `canvas` | `description` | string | Extended description |

---

## Extended styling props

Beyond the standard `style` prop (which accepts a preset name or a
StyleMap object), several widgets support additional colour and sizing
props.

| Widget | Prop | Type | Description |
|--------|------|------|-------------|
| `text` | `align_x` | string | Text alignment: `"default"`, `"left"`, `"center"`, `"right"`, `"start"`, `"end"`, `"justified"` |
| `text` | `text_direction` | string | Text direction for logical alignment: `"auto"`, `"ltr"`, `"rtl"` |
| `text` | `ellipsis` | string | Text overflow: `"none"`, `"start"`, `"middle"`, `"end"` |
| `rich_text` | `wrapping` | string | Text wrapping mode |
| `rich_text` | `ellipsis` | string | Text overflow: `"none"`, `"start"`, `"middle"`, `"end"` |
| `text_input` | `align_x` | string | Physical text alignment: `"left"`, `"center"`, `"right"` |
| `text_input` | `placeholder_color` | hex color | Placeholder text colour |
| `text_input` | `selection_color` | hex color | Text selection highlight |
| `text_input` | `ime_purpose` | string | IME hint: `"normal"`, `"secure"`, `"terminal"` |
| `text_input` | `text_direction` | string | Direction hint for placeholder and value text: `"auto"`, `"ltr"`, `"rtl"` |
| `text_editor` | `text_direction` | string | Text direction for logical key-binding motions: `"auto"`, `"ltr"`, `"rtl"` |
| `text_editor` | `placeholder_color` | hex color | Placeholder text colour |
| `text_editor` | `selection_color` | hex color | Text selection highlight |
| `text_editor` | `ime_purpose` | string | IME hint: `"normal"`, `"secure"`, `"terminal"` |
| `slider` | `rail_color` | hex color | Track rail colour |
| `slider` | `rail_width` | number | Track rail thickness |
| `vertical_slider` | `rail_color` | hex color | Track rail colour |
| `vertical_slider` | `rail_width` | number | Track rail thickness |
| `scrollable` | `scrollbar_color` | hex color | Scrollbar track colour |
| `scrollable` | `scroller_color` | hex color | Scroller handle colour |
| `pick_list` | `ellipsis` | string | Text overflow for selected value |
| `pick_list` | `menu_style` | object | StyleMap overrides for the dropdown menu |
| `combo_box` | `ellipsis` | string | Text overflow for selected value |
| `combo_box` | `menu_style` | object | StyleMap overrides for the dropdown menu |
| `combo_box` | `shaping` | string | Text shaping (`"basic"` or `"advanced"`) |
| `grid` | `fluid` | number | Max cell width for fluid auto-wrapping columns |
| `table` | `header_text_size` | number | Header row text size |
| `table` | `row_text_size` | number | Body row text size |
| `pane_grid` | `divider_color` | hex color | Pane divider colour |
| `pane_grid` | `divider_width` | number | Pane divider thickness |
| `markdown` | `link_color` | hex color | Hyperlink colour |
| `markdown` | `code_theme` | string | Syntax highlighting theme for code blocks |

### Text alignment and direction

Layout props that use horizontal alignment remain physical:
`"left"`, `"center"`, and `"right"`. Text widgets use the separate
text alignment vocabulary because text can be direction-aware and can be
justified.

`text.align_x` accepts:

| Value | Meaning |
|-------|---------|
| `"default"` | Renderer default. Left-to-right text aligns left and right-to-left text aligns right. |
| `"left"` | Physical left |
| `"center"` | Center |
| `"right"` | Physical right |
| `"start"` | Logical start, resolved using `text_direction` |
| `"end"` | Logical end, resolved using `text_direction` |
| `"justified"` | Justified text |

`text_direction` accepts `"auto"`, `"ltr"`, and `"rtl"`. For
`text.align_x`, `"start"` with `"auto"` uses the renderer default.
`"end"` with `"auto"` falls back to physical right because iced cannot
represent direction-aware end alignment.

For `text_input`, `text_direction` is a directionality hint for the
placeholder and value. When `align_x` is absent, `"rtl"` maps to
physical right alignment, while `"ltr"` and `"auto"` map to physical
left alignment. An explicit `align_x` value always wins.

### Text editor motion names

Text editor `key_bindings` can use logical motion names in `move` and
`select` binding objects:

```json
{"move": "backward"}
```

The canonical motion names are `"backward"`, `"forward"`, `"up"`,
`"down"`, `"word_backward"`, `"word_forward"`, `"line_start"`,
`"line_end"`, `"page_up"`, `"page_down"`, `"document_start"`, and
`"document_end"`.

With `text_direction: "rtl"`, `"backward"` maps to a physical right
move and `"forward"` maps to a physical left move. Word motions follow
the same rule. With `"auto"`, logical backward and forward use
left-to-right behavior.

The old physical motion strings remain valid aliases in key bindings:
`"left"`, `"right"`, `"word_left"`, `"word_right"`, `"home"`, and
`"end"`. These aliases keep their physical behavior even when
`text_direction` is `"rtl"`.

**Table rows.** Tables use children-based rows. The `table` node's
children are `table_row` nodes, each containing `table_cell`
children. Both `table_row` and `table_cell` are wire types.

**StyleMap `base` field.** A StyleMap object can include a `"base"` field
naming a preset to extend. The style starts from the preset's defaults,
then remaining fields override individual properties:

```json
{
  "style": {
    "base": "secondary",
    "background": "#ff0000"
  }
}
```

---

## Event throttling

The renderer supports host-controlled rate limiting of high-frequency
events. Without configuration, all events are delivered at full speed
(backward compatible). The host opts in by setting rates at one or
more levels.

### Rate hierarchy

For a given event, the effective rate is determined by (highest
priority first):

1. Per-widget `event_rate` prop
2. Per-subscription `max_rate` field on Subscribe
3. Global `default_event_rate` from Settings
4. No limit (full speed) if none of the above are set

### Coalescable events

Only high-frequency events are eligible for rate limiting. The
renderer classifies events into two coalescing strategies:

**Replace (latest value wins):** `cursor_moved`, `finger_moved`,
`modifiers_changed`, `animation_frame`, `theme_changed`, `slide`,
`move`, `resize`, `scrolled`, `pane_resized`.

**Accumulate (deltas sum):** `wheel_scrolled`, `scroll` (pointer).

**Never coalesced:** `click`, `input`, `submit`, `toggle`, `select`,
`paste`, `key_press`, `key_release`, `button_pressed`,
`button_released`, `cursor_entered`, `cursor_left`, `slide_release`,
and all window lifecycle events. These are always delivered
immediately regardless of rate settings.

### Widget `event_rate` prop

Any widget node can include an `event_rate` prop (integer, events per
second) to rate-limit its coalescable events:

```json
{
  "id": "volume",
  "type": "slider",
  "props": { "range": [0, 100], "value": 50, "event_rate": 30 }
}
```

Different widgets of the same type can have different rates. The prop
is accepted on all widget types (it is a universal prop like `a11y`).

### Ordering guarantees

Non-coalescable events flush the coalesce buffer before emitting.
This preserves ordering: the host always sees the latest coalesced
state (e.g. mouse position) before the discrete event (e.g. click)
that follows.

Incoming stdin messages also flush the buffer, providing adaptive
throughput matching: when the host sends a message, it gets all
pending coalesced events immediately.

### Custom widget events

Custom widget events participate in rate limiting and coalescing
when they carry a `CoalesceHint`. Widget authors set hints on
outgoing events via `.with_coalesce(CoalesceHint::Replace)` or
`.with_coalesce(CoalesceHint::Accumulate(vec!["field_x".into(), "field_y".into()]))`.
`Replace` uses the standard latest-value-wins strategy.
`Accumulate(fields)` sums the named data fields across coalesced
events; other fields keep the latest values. Events returned without
a hint are never coalesced; they are delivered immediately
regardless of any `event_rate` setting.

### Headless and mock modes

Rate limiting configuration is accepted and stored but not applied
in headless or mock modes. Events in those modes are driven by the
scripting protocol at the host's pace, so rate limits would make
test timing unpredictable.

---

## Performance considerations

The renderer is designed for moderate UI complexity (hundreds to low
thousands of nodes). Practical considerations for large trees:

- **Snapshot vs Patch.** For trees with many nodes, prefer Patch for
  incremental updates rather than replacing the entire tree with
  Snapshot on every change. Snapshot triggers a full tree walk for
  widget prepare, cache invalidation, and window reconciliation.
- **Tree depth.** Rendering and tree search are bounded to 256 levels
  of nesting. Deeply nested trees also increase layout cost.
- **Canvas caching.** Canvas widgets cache per-layer tessellation.
  Avoid unnecessary shape changes; only layers whose content hash
  changes are re-tessellated.
- **Screenshot size.** Large screenshots allocate proportional RGBA
  buffers (width * height * 4 bytes). The maximum dimension is
  16384 px per axis.

---

## Overlay widget

The `overlay` widget positions a popup element relative to an anchor
sibling. It takes exactly two children: the first is the anchor
(rendered normally in the layout), the second is the overlay content
(rendered as an iced overlay above all other content).

```json
{
  "id": "dropdown-1",
  "type": "overlay",
  "props": {
    "position": "below",
    "gap": 4,
    "offset_x": 0,
    "offset_y": 0,
    "flip": true,
    "align": "start"
  },
  "children": [
    {"id": "anchor", "type": "button", "props": {"label": "Open"}, "children": []},
    {"id": "menu", "type": "column", "props": {}, "children": [...]}
  ]
}
```

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `position` | string | `"below"` | Overlay placement: `"below"`, `"above"`, `"left"`, `"right"` |
| `gap` | number | 0 | Pixel gap between anchor and overlay content |
| `offset_x` | number | 0 | Horizontal pixel offset (applied after positioning) |
| `offset_y` | number | 0 | Vertical pixel offset (applied after positioning) |
| `flip` | bool | false | Auto-flip position when content overflows the viewport in the primary direction. Flips Below<->Above or Left<->Right. The flip only occurs if the opposite side has enough space; otherwise the original position is kept and viewport clamping applies. |
| `align` | string | `"center"` | Cross-axis alignment: `"start"`, `"center"`, `"end"`. For Below/Above, controls horizontal alignment (start = left-aligned with anchor edge, center = centered on anchor, end = right-aligned with anchor edge). For Left/Right, controls vertical alignment. |

**Focus and accessibility.** Both children participate in focus cycling
(Tab/Shift+Tab) and the accessibility tree. Setting `a11y.modal = true`
on the overlay node signals a modal popup, but focus trapping is the
host SDK's responsibility; plushie does not intercept focus navigation
at the iced level.

---

## Interactive canvas elements

Groups within canvas layers can be made interactive by adding an `id`
field. An interactive group (called an "element" in the event
vocabulary) responds to pointer events, keyboard navigation, drag
gestures, tooltips, and participates in the accessibility tree.

Only groups can be interactive. Leaf shapes (rect, circle, line, path,
text, image, svg) are never interactive on their own; wrap them in a
group to add interaction.

### Terminology

- **Shape**: a leaf drawing primitive (rect, circle, line, path, text,
  image, svg). Pure visual, no interactivity.
- **Group**: the only container type. Carries transforms, clips, and
  optionally interactivity (when it has an `id` field).
- **Element**: an interactive group (one with an `id`). Uses the
  element's scoped wire ID for events, commands (`focus`), and
  test actions.

### Group wire format

A group with an `id` field is an interactive element. All interactive
properties live at the group's top level (no nested `interactive`
sub-object).

```json
{
  "type": "group",
  "children": [...],

  "transforms": [
    {"type": "translate", "x": 50, "y": 30},
    {"type": "rotate", "angle": 45},
    {"type": "scale", "x": 2.0, "y": 2.0}
  ],
  "clip": {"x": 0, "y": 0, "w": 200, "h": 200},

  "id": "star-0",
  "on_click": true,
  "on_hover": true,
  "cursor": "pointer",
  "tooltip": "1 star",
  "hit_rect": {"x": 0, "y": 0, "w": 40, "h": 40},

  "hover_style": {"fill": "#ddd"},
  "pressed_style": {"fill": "#bbb"},
  "focus_style": {"stroke": "#3b82f6", "stroke_width": 2},
  "show_focus_ring": true,

  "draggable": false,
  "drag_axis": "both",
  "drag_bounds": {"min_x": 0, "max_x": 100, "min_y": 0, "max_y": 100},

  "a11y": {"role": "radio", "label": "1 star", "selected": true,
           "position_in_set": 1, "size_of_set": 5},

  "focusable": false
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | `"group"` | Yes | Must be `"group"` |
| `children` | array | Yes | Child shapes drawn in local coordinates |
| `transforms` | array | No | Ordered list of transforms: `translate`, `rotate`, `scale` |
| `clip` | object | No | Clip rectangle `{x, y, w, h}` in local coordinates |
| `id` | string | No* | Unique ID. Presence makes the group interactive. |
| `on_click` | bool | No | Emit `click` events on the element |
| `on_hover` | bool | No | Emit `enter` / `exit` events on the element |
| `cursor` | string | No | Cursor on hover (`pointer`, `grab`, `crosshair`, `move`, `text`) |
| `tooltip` | string | No | Tooltip text on hover |
| `hit_rect` | object | No | Explicit hit region `{x, y, w, h}` in local coords |
| `hover_style` | object | No | Style overrides while hovered |
| `pressed_style` | object | No | Style overrides while pressed |
| `focus_style` | object | No | Style overrides while keyboard-focused |
| `show_focus_ring` | bool | No | Show default focus ring (default: `true`) |
| `draggable` | bool | No | Enable drag interaction |
| `drag_axis` | string | No | `"both"` (default), `"x"`, `"y"` |
| `drag_bounds` | object | No | `{min_x, max_x, min_y, max_y}` |
| `a11y` | object | No | Accessibility overrides (see Accessibility below) |
| `focusable` | bool | No | Make this group a Tab stop for two-level navigation |

\* Groups without `id` are non-interactive (pure structural containers
for transforms, clips, and visual grouping).

**Transforms** are applied in array order. Each entry has a `type` and
type-specific fields:

| Transform | Fields | Description |
|-----------|--------|-------------|
| `translate` | `x`, `y` | Translate by (x, y) |
| `rotate` | `angle` | Rotate by angle in degrees |
| `scale` | `x`, `y` or `factor` | Non-uniform or uniform scale |

Groups without transforms or clips are pure nesting containers with
no rendering overhead.

### Canvas widget props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `role` | string | `"group"` if interactive elements exist, `"image"` otherwise | Accessible role for the canvas widget (for example `"group"`, `"toolbar"`, `"radio_group"`) |
| `arrow_mode` | string | `"wrap"` | Arrow key behavior: `"wrap"`, `"clamp"`, `"linear"`, `"none"` |
| `alt` | string | - | Accessible label |
| `description` | string | - | Extended accessible description |

### Hit testing

Hit regions are computed in the group's **local** coordinate space.
The renderer accumulates a 2D affine transform matrix from all
ancestor groups and uses the inverse matrix to transform cursor
positions from canvas space to local space for testing.

| Children | Hit test method |
|----------|----------------|
| `rect` | Point-in-rect |
| `circle` | Distance from center <= radius |
| `line` | Distance to line segment (min 2px half-width) |
| `path` | Bounding box of command coordinates |
| `text` | Estimated bounds from position + content |
| `group` | Union bounding box of children |

Groups are tested in reverse draw order (topmost first). Explicit
`hit_rect` overrides automatic inference. `hit_rect` is in local
coordinates.

Clip regions from ancestor groups are intersected and tested in
canvas space before the hit region test. Clicks outside the
accumulated clip are ignored.

A 0.5px epsilon is applied to boundary comparisons for floating-point
precision at transformed element boundaries.

### Events emitted

Canvas element events are regular `Message::Event` messages. The
wire `id` field is the element's scoped wire ID, which the SDK's
scoped ID system splits into `id`, `scope`, and window. Canvas
elements look like regular widgets inside a container from the
SDK's perspective.

| Family | ID | Value | Coalescable | Description |
|--------|-----|-------|-------------|-------------|
| `enter` | scoped element ID | `x`, `y` | No | Pointer entered hit region |
| `exit` | scoped element ID | - | No | Pointer left hit region |
| `click` | scoped element ID | `x`, `y`, `button` | No | Activated (click or keyboard). `button`: `"left"`, `"right"`, `"keyboard"` |
| `key_press` | scoped element ID | `key`, `modifiers` | No | Navigation key on focused element when `arrow_mode` is `"none"`. Keys: arrows, Home, End, PageUp, PageDown. `modifiers`: `{shift, ctrl, alt, logo, command}` |
| `drag` | scoped element ID | `x`, `y`, `delta_x`, `delta_y` | Replace | Drag movement |
| `drag_end` | scoped element ID | `x`, `y` | No | Drag released |
| `focused` | scoped element ID | - | No | Element gained keyboard focus |
| `blurred` | scoped element ID | - | No | Element lost keyboard focus |
| `focused` | canvas ID | - | No | Canvas widget gained iced-level focus |
| `blurred` | canvas ID | - | No | Canvas widget lost iced-level focus |
| `focused` | scoped group ID | - | No | Focusable group entered |
| `blurred` | scoped group ID | - | No | Focusable group exited |
| `diagnostic` | canvas ID | `level`, `element_id`, `code`, `message` | Deduplicate | Validation warning |

**Event ordering guarantees:**

- Click on element: `focused` (canvas, if new) -> `blurred` (old element) -> `focused` (new element) -> `click`
- Tab to next: `blurred` (old element) -> `focused` (new element)
- Tab out: `blurred` (element) -> `blurred` (canvas)
- Tab in: `focused` (canvas) -> `focused` (element)

Raw canvas pointer events (`press`, `release`, `move`, `scroll`)
fire on the canvas node alongside element-level events.

### Keyboard navigation

A canvas with interactive elements is a single Tab stop. Once focused,
internal keyboard navigation uses the roving tabindex pattern.

| Key | Action |
|-----|--------|
| Tab | Enter canvas (first element) / advance to next top-level entry / exit canvas |
| Shift+Tab | Enter canvas (last element) / move to previous top-level entry / exit canvas |
| Arrow Down/Right | Next element within current scope (respects `arrow_mode`) |
| Arrow Up/Left | Previous element within current scope |
| Home | First element in scope |
| End | Last element in scope |
| Page Down/Up | Jump by a small page step within scope |
| Enter / Space | Activate focused element (`click` with `button: "keyboard"`) |
| Escape | Exit focusable group / clear focus / unfocus canvas (three levels) |

**Arrow mode** (`arrow_mode` canvas prop):

| Mode | Boundary behavior |
|------|-------------------|
| `wrap` (default) | Wraps last->first and first->last. Always captures. |
| `clamp` | Stops at first/last. Captures. |
| `linear` | Stops at first/last. Captures. |
| `none` | Navigation keys (arrows, Home, End, PageUp, PageDown) emit `key_press` to the host instead of navigating elements. Tab-only navigation for focus. Use for custom value adjustment on focused elements. |

**Focusable groups** (`focusable: true` on a group): Tab moves between
top-level entries (standalone elements + focusable groups). When Tab
lands on a focusable group, it auto-enters and focuses the first (or
last for Shift+Tab) child. Arrows navigate within the group's children.
Escape exits the group. Canvases without focusable groups use flat-list
navigation (backward compatible).

Click-to-focus: clicking an interactive element grants the canvas
iced-level focus and sets internal focus to the clicked element.
Clicking empty canvas area clears internal focus.

### Widget operations

| Op | Payload | Description |
|----|---------|-------------|
| `focus` | - | Programmatically focus the canvas element named by the widget op's scoped target (for example `canvas/element` or `main#canvas/element`). The canvas stores the request as pending focus and applies it on the next render so the focus ring and active-descendant state reflect the new element. |

Canvas-level focus moves (`focus_next`, `focus_previous`, and the
`*_within` variants) are application-wide widget ops dispatched
outside the canvas; see the application focus and keyboard sections
for their payloads.

### Test interact actions

| Action | Payload | Description |
|--------|---------|-------------|
| `click_element` | `x`, `y` | Synthesize click at coordinates |
| `focus_element` | - | Synthesize Tab to enter canvas |

### Style overrides

`hover_style`, `pressed_style`, and `focus_style` on the group are
merged into each child shape's JSON during draw. Any shape property
can be overridden (`fill`, `stroke`, `stroke_width`, `opacity`, etc.).

Priority (highest wins): `pressed_style` > `hover_style` > `focus_style`.

Children can also declare their own per-child overrides at the top
level of the child shape JSON; these take precedence over the group's
style.

Layers with an active style override bypass the geometry cache.

### Accessibility

The canvas widget's accessible role defaults to `Group` when
interactive elements exist, `Image` otherwise. Override with the
`role` prop.

Interactive elements with `a11y` appear as child nodes in the
accessibility tree. `active_descendant` on the canvas node
dynamically tracks the focused element.

Focusable groups create parent-child relationships in the a11y tree
via `traverse()` blocks.

The renderer emits validation diagnostics (as log warnings) for:
- Interactive element without `a11y` metadata
- `switch` role without `toggled`, `radio` without `selected`,
  `check_box` without `toggled`
- Multiple elements without `position_in_set`/`size_of_set`

### Focus ring

The focus ring adapts to the element's hit region geometry:
- **Rect**: rounded rectangle
- **Circle**: circle
- **Line**: capsule (stadium shape) oriented along the line

The ring respects the element's full accumulated transform (translate,
rotate, scale). Suppressed when `show_focus_ring: false` is set (use
`focus_style` for custom focus indicators instead).

### Tooltips

When the cursor hovers over an element with a `tooltip` field, the
text is drawn as an overlay within the canvas.

### Known limitations

- Keyboard-driven drag not yet supported (drag operations have no
  keyboard equivalent).
- Nested focusable groups (focusable inside focusable) are not fully
  supported; the inner group appears as an arrow-navigable child but
  cannot be "drilled into" via keyboard.
- Role-specific keyboard patterns (slider value adjustment, tree
  expand/collapse) are not implemented.
- The overlay widget auto-flips when `flip: true` is set, but only
  along the primary axis.
