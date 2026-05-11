# Wire protocol

The wire protocol is the message format that carries every
SDK-renderer exchange when Plushie runs in wire mode. The
canonical message types live in `plushie_core::protocol`; the
SDK-side framing, encoding, and handshake live in
`plushie::runner::wire` and `plushie::runner::bridge`. Direct
mode never touches this protocol (see
[Direct vs wire](direct-vs-wire.md)), but the same message types
and family strings are shared with every host SDK that drives the
renderer.

The protocol is language-agnostic. Every Plushie SDK (Rust,
Gleam, Elixir) produces the same wire bytes for the same
semantic operation.

## Transport

A wire-mode bridge owns a duplex byte stream to the renderer.
Two stream shapes are supported and pick the same codec
negotiation dance:

- **Subprocess** (`plushie::run`, `plushie::run_with_renderer`,
  `plushie::run_spawn`): stdin/stdout to the renderer binary.
- **Socket** (`plushie::run_connect`, `PLUSHIE_SOCKET`): a
  pre-connected Unix domain socket or TCP stream.

Both pass through `plushie::runner::bridge::Bridge`, which
carries a `Codec` (JSON or MessagePack) and an incoming
`BufReader`. The SDK opens the stream in `Codec::Json` and
switches to `Codec::MsgPack` if the renderer's `hello` message
advertises it.

### Framing

```rust
pub enum Codec {
    Json,
    MsgPack,
}
```

- `Codec::Json` frames each message as one UTF-8 JSON object
  terminated by a single `\n`. Messages must not contain embedded
  newlines. The renderer auto-detects JSON when stdin's first
  byte is `0x7B` (`{`). The first byte must be `{` literally:
  leading whitespace, BOM, or a stray newline before the opening
  brace routes the stream to MessagePack and the handshake fails.
  Senders must write the JSON object as the very first bytes on
  the channel.
- `Codec::MsgPack` frames each message as a 4-byte big-endian
  unsigned length prefix followed by that many MessagePack bytes.
  Non-`{` first bytes route to MessagePack. Payload size is
  capped at 64 MiB; larger frames fail decoding with an I/O
  error.

The SDK and renderer share the per-message size cap via
`plushie_core::codec_safety`. MessagePack depth is additionally
bounded to protect the rmpv parser from stack exhaustion.

## Message envelope

Every message is a tagged object keyed by `"type"`. The SDK
emits `plushie_core::OutgoingMessage`; the renderer emits
`plushie_core::protocol::IncomingMessage` plus response and
event types. Both enums use serde's `#[serde(tag = "type",
rename_all = "snake_case")]`, so the discriminator is always
lowercase snake case.

Every message carries a `session` field (string). Single-session
mode sends the empty string `""`. Test session pools use a
per-session ID so the renderer can isolate tree, subscription,
and effect state per session.

```json
{"type": "patch", "session": "", "ops": [...]}
```

## Outbound messages (SDK -> renderer)

The SDK builds wire messages through
`plushie::runner::bridge::Bridge` helpers. Every helper returns
a `crate::Result`; encode failures bubble as
`Error::WireEncode`, I/O failures as `Error::Io`.

| Type | Fields | Purpose |
|---|---|---|
| `settings` | `settings` | App settings and protocol version, sent at handshake and restart |
| `snapshot` | `tree` | Full UI tree, replaces renderer state |
| `patch` | `ops` | Incremental tree update: array of `PatchOp` |
| `subscribe` | `kind`, `tag`, `max_rate`, `window_id` | Activate an event source |
| `unsubscribe` | `kind`, `tag` | Remove a subscription |
| `widget_op` | `op`, `payload` | Non-targeted op (`focus_next`, `blur`, `announce`) |
| `command` | `id`, `family`, `value` | Widget-targeted command (focus, scroll, native widget op) |
| `commands` | `commands` | Atomic batch of widget commands |
| `window_op` | `op`, `window_id`, `payload` | Per-window lifecycle op (`open`, `update`, `close`, `resize`) |
| `system_op` | `op`, `payload` | System-wide op not tied to a window |
| `system_query` | `op`, `payload` | System-wide query (e.g. current theme) |
| `image_op` | `op`, `payload` | In-memory image lifecycle (`create_from_bytes`, `update`, `delete`, `list`, `clear`) |
| `effect` | `id`, `kind`, `payload` | Platform effect request (file dialog, clipboard, notification) |
| `interact` | `id`, `action`, `selector`, `payload` | Synthetic interaction for automation |
| `query` | `id`, `target`, `selector` | Read renderer state (tree, find widget) |
| `reset` | `id` | Tear down a session's renderer state |
| `advance_frame` | `timestamp` | Manual frame step for headless / test mode |
| `register_effect_stub` | `kind`, `response` | Return a canned response for an effect kind |
| `unregister_effect_stub` | `kind` | Remove a previously registered stub |

### Settings payload

The SDK builds the Settings payload in `build_settings::<A>()`
from `App::settings()`. The only required key is
`protocol_version`; every other field is omitted when the
matching `Settings` option is `None`.

| Key | Type | Source |
|---|---|---|
| `protocol_version` | `u32` | `plushie_core::protocol::PROTOCOL_VERSION` |
| `default_font` | object `{ "family": string }` | `settings.default_font` |
| `default_text_size` | number | `settings.default_text_size` |
| `antialiasing` | bool | `settings.antialiasing` |
| `vsync` | bool | `settings.vsync` |
| `scale_factor` | number | `settings.scale_factor` |
| `default_event_rate` | number | `settings.default_event_rate` |
| `fonts` | array | `settings.fonts` |
| `widget_config` | object | `settings.widget_config` |
| `required_widgets` | array of string | `settings.required_widgets` |
| `theme` | object | `settings.theme` (wire-encoded) |

The renderer wraps this object inside `{"type": "settings",
"session": "...", "settings": {...}}`; the keys above live
under the inner `"settings"` field.

## Inbound messages (renderer -> SDK)

The renderer emits three top-level families plus several
response types. The SDK deserialises all of them through
`plushie_core::protocol` types.

| Type | Struct | Purpose |
|---|---|---|
| `hello` | `serde_json::Value` (handshake only) | Protocol/version advertisement |
| `event` | `OutgoingEvent` | Widget event, subscription event, or widget author event |
| `diagnostic` | `DiagnosticMessage` | Structured renderer-side diagnostic |
| `effect_response` | `EffectResponse` | Reply to an `effect` request |
| `effect_stub_register_ack` | `EffectStubAck` (`status: "registered"`) | Reply to `register_effect_stub` |
| `effect_stub_unregister_ack` | `EffectStubAck` (`status: "unregistered"`) | Reply to `unregister_effect_stub` |
| `query_response` | `QueryResponse` | Reply to a `query` |
| `interact_response` | `InteractResponse` | Final reply to an `interact` (includes intermediate events) |
| `tree_hash_response` | `TreeHashResponse` | Reply to a `tree_hash` query |
| `screenshot_response` | `ScreenshotResponse` | Reply to a `screenshot` request (structured fields; RGBA carried in a codec-specific binary companion) |
| `reset_response` | `ResetResponse` (`status: "ok"`) | Reply to a `reset` |

`OutgoingEvent` is the only variant delivered asynchronously to
`App::update`. Everything else is a request-response artifact.

### Event envelope

```rust
pub struct OutgoingEvent {
    pub message_type: &'static str, // "event"
    pub session: String,
    pub family: String,
    pub id: String,
    pub value: Option<Value>,
    pub tag: Option<String>,
    pub modifiers: Option<KeyModifiers>,
    pub captured: Option<bool>,
}
```

- `family` identifies the event kind (`"click"`, `"key_press"`,
  `"window_opened"`). See `EventType::from_family` for the
  canonical mapping.
- `id` is the source widget ID for widget events, empty for
  subscription events.
- `tag` identifies which subscription requested the event; set
  on subscription events, absent on widget events.
- `modifiers` carries keyboard modifier state on keyboard,
  pointer, and IME events.
- `captured` indicates whether an iced widget consumed the
  event before the subscription listener saw it. Present on
  keyboard, pointer, touch, and IME events.

## Event families

Widget interaction families are declared in
`plushie_core::event_type` via the `event_types!` macro, so the
enum, `EventType::from_family`, and `EventType::as_family` stay
in lock-step. Subscription-only families such as window, IME,
theme, animation, and diagnostic events are emitted by
`OutgoingEvent` constructors and are listed separately from the
widget interaction enum.

### Widget interactions

| Family | Trigger |
|---|---|
| `click` | Pointer click on a focusable widget |
| `double_click` | Rapid pointer press sequence |
| `input` | Text input changed |
| `submit` | Input submitted (Enter) |
| `paste` | Paste into a text input |
| `toggle` | Boolean widget flipped |
| `select` | Selection chosen from a list |
| `slide` | Slider value changed while dragging |
| `slide_release` | Slider drag released |
| `sort` | Column sort changed |
| `focused` | Widget gained keyboard focus |
| `blurred` | Widget lost keyboard focus |
| `drag` | Drag gesture in progress |
| `drag_end` | Drag gesture ended |
| `open` | Overlay or disclosure expanded |
| `close` | Overlay or disclosure collapsed |
| `option_hovered` | Dropdown option hovered |
| `link_click` | Link in rich text or markdown |
| `status` | Arbitrary status update |
| `key_binding` | Keyboard binding fired on a widget |
| `transition_complete` | Declarative animation reached its end |

### Pointer and resize

| Family | Trigger |
|---|---|
| `press` | Pointer pressed (mouse, touch, or pen) |
| `release` | Pointer released |
| `move` | Pointer moved (coalescable) |
| `scroll` | Pointer scroll gesture (accumulable) |
| `scrolled` | Scrollable widget position changed (coalescable) |
| `enter` | Pointer entered a hit region |
| `exit` | Pointer exited a hit region |
| `resize` | Sensor widget resized (coalescable) |

Pointer events carry a `pointer` tag (`"mouse"`, `"touch"`,
`"pen"`), optional `finger` ID for touch, and pointer
`button` strings (`"left"`, `"right"`, `"middle"`, `"back"`,
`"forward"`).

### PaneGrid

`pane_resized`, `pane_dragged`, `pane_clicked`,
`pane_focus_cycle`.

### Keyboard and IME

| Family | Trigger |
|---|---|
| `key_press` | Keyboard key pressed |
| `key_release` | Keyboard key released |
| `modifiers_changed` | Modifier state changed (subscription) |
| `ime_opened` / `ime_closed` | IME composition boundary |
| `ime_preedit` | IME preedit with optional cursor range |
| `ime_commit` | IME committed text |

`KeyModifiers` serialises as a flat object with boolean
`shift`, `ctrl`, `alt`, `logo`, `command` fields; missing
fields default to `false`. Key names and combos are parsed
through `plushie_core::key`, which accepts both snake-case
(`"left_arrow"`) and concatenated (`"leftarrow"`) forms, plus
`"Ctrl+s"`-style combo strings in scripted input.

### Window lifecycle

`window_opened`, `window_closed`, `window_close_requested`,
`window_moved`, `window_resized`, `window_focused`,
`window_unfocused`, `window_rescaled`, `file_hovered`,
`file_dropped`, `files_hovered_left`.

`window_opened` carries `window_id`, optional top-level `x` and
`y`, `width`, `height`, and `scale_factor`. Other window events
carry `window_id` plus the changed fields.

### Animation and theme

`animation_frame` (coalescable; `value.timestamp` in
milliseconds) and `theme_changed` (coalescable; `value` is the
mode string).

### Diagnostics

`diagnostic` events carry `level`, optional `element_id`, a
machine-readable `code`, and a human `message`. They reach the
SDK both as `Event::System` on widget-owned canvases and as
standalone `DiagnosticMessage` values via
`plushie_core::protocol::DiagnosticMessage` (typed `level` plus a
structured `Diagnostic` enum under the `diagnostic` field).

## PropValue

`plushie_core::protocol::PropValue` is the typed representation
used by every node in the tree. The enum mirrors JSON's type
system without serde allocation overhead:

```rust
pub enum PropValue {
    Null,
    Bool(bool),
    F64(f64),
    I64(i64),
    U64(u64),
    Str(String),
    Array(Vec<PropValue>),
    Object(PropMap),
}
```

Numeric accessors (`as_f64`, `as_i64`, `as_u64`) perform exact
range checks: fractional floats never narrow to integer, lossy
integer-to-float conversions return `None`, non-finite floats
return `None`. This keeps wire inputs from silently corrupting
typed widget props.

`PropMap` is an ordered `Vec<(String, PropValue)>`. Serialised
JSON uses alphabetical key order because `serde_json` is
compiled without the `preserve_order` feature; this is an
invariant the workspace tests for.

### Null-as-absent

The protocol has no op for "set this prop to a JSON null." The
`update_props` op uses `null` to mean "remove this key," so
`PropMap` and `Props` compare null-valued and absent entries as
equal. Round-tripping a tree through diff and patch is lossless.

### Tree nodes

```rust
pub struct TreeNode {
    pub id: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub props: Props,
    pub children: Vec<TreeNode>,
}
```

`id` is a scoped identifier assigned by the SDK, unique within
the tree. `type_name` selects the renderer-side widget; wire
representation uses the JSON key `"type"`. `props` and
`children` default to empty so wire input may omit them.

### Canonical hash

`TreeNode::canonical_hash` and
`protocol::canonical_tree_hash(Option<&TreeNode>)` produce a
SHA-256 hex digest over recursively key-sorted JSON. Two trees
that differ only in object key insertion order hash identically.
An empty tree hashes to the empty string so every SDK shares one
empty-tree policy.

## Patch operations

A `patch` message carries `ops: Vec<PatchOp>`. Each op is a
flattened object keyed by `op` and `path`:

```rust
pub struct PatchOp {
    pub op: String,
    pub path: Vec<usize>,
    #[serde(flatten)]
    pub rest: Value,
}
```

| Op | Additional fields | Semantics |
|---|---|---|
| `replace_node` | `node` | Replace the node at `path` with a new subtree |
| `update_props` | `props` | Merge `props` into the node at `path`; `null` values remove keys |
| `insert_child` | `index`, `node` | Insert `node` as a child of `path` at `index` |
| `remove_child` | `index` | Remove the child at `index` under `path` |

`path` is a sequence of child indices from the root. The empty
path targets the root itself. Child reordering is expressed as a
parent replace; there is no dedicated `move_child` op. Identical
trees produce an empty op list, which the SDK suppresses
entirely.

## Handshake

The startup sequence is fixed and runs once per SDK connection
(and again after every successful restart in wire mode):

1. SDK sends `settings` with `protocol_version` set to
   `plushie_core::protocol::PROTOCOL_VERSION`. The renderer
   auto-detects JSON vs MessagePack from the first byte unless a
   CLI override is set.
2. Renderer replies with a `hello` message.
3. SDK sends the initial `snapshot`.
4. Normal message exchange begins.

`hello` carries:

| Key | Type | Meaning |
|---|---|---|
| `type` | string | Always `"hello"` |
| `protocol_version` | number | Renderer's protocol version |
| `protocol` | number | Legacy alias for `protocol_version` |
| `codec` | string | Negotiated codec name |
| `version` | string | Renderer crate version |
| `mode` | string | `"windowed"` or `"headless"` |
| `backend` | string | Rendering backend (e.g. `"wgpu"`, `"tiny_skia"`) |
| `transport` | string | Active transport (`"stdio"`, `"socket"`, etc.) |
| `widgets` | array of string | All compiled widget type names, sorted |
| `native_widgets` | array of string | Native widget type names, sorted |

The SDK accepts either `protocol_version` or `protocol` for
backwards compatibility. A version skew between the SDK's pinned
`PROTOCOL_VERSION` and the renderer's advertised value is logged
and surfaced through `plushie::RENDERER_VERSION` diagnostics;
the connection still proceeds.

## Error handling

The SDK surfaces wire failures through the `plushie::Error`
family:

- `Error::InvalidSettings` when a required setting is missing
  before a connection can be opened.
- `Error::WireEncode` when serialisation of an outbound message
  fails (non-finite floats in user data, etc.).
- `Error::Io` when the transport read or write itself fails.
- `Error::BinaryNotFound` when subprocess discovery finds no
  renderer.

On the renderer side, parse failures for inbound messages
produce a structured `diagnostic` event rather than a
disconnect. The SDK deserialises it as a
`DiagnosticMessage` and delivers it through `Event::System`
unless the widget owning the canvas has opted in to local
handling.

Renderer crashes (subprocess exit, socket disconnect) are
reported through `App::handle_renderer_exit`. The host keeps the
app model intact, resolves pending effects with
`EffectResult::RendererRestarted`, and re-runs the handshake
from step 1 on restart. See [Direct vs wire](direct-vs-wire.md)
for the full restart contract.

## See also

- [Direct vs wire](direct-vs-wire.md)
- [Events](events.md)
- [Commands](commands.md)
- [Configuration](configuration.md)
- [Versioning](versioning.md)
