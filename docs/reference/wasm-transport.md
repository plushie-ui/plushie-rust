# WASM transport

`plushie-renderer-wasm` exposes the renderer to browser hosts
through `wasm-bindgen`. It uses the same protocol message types as
native wire mode, but the transport is JavaScript calls instead of
stdin/stdout or a socket.

The WASM transport is for browser-hosted renderers. It is not the
Rust app SDK compiled to `wasm32`; the host loop still lives in
JavaScript, another browser-capable SDK, or a remote native process
that bridges messages into the page.

## API shape

The generated JavaScript module exposes `init` and `PlushieApp`:

```javascript
import init, { PlushieApp } from "./plushie_renderer_wasm.js";

await init();

const app = new PlushieApp(settingsJson, (chunk) => {
  for (const line of chunk.split("\n")) {
    if (line.length === 0) continue;
    handleRendererMessage(JSON.parse(line));
  }
});

app.send_message(JSON.stringify({
  type: "snapshot",
  session: "",
  tree,
}));
```

`settingsJson` is the raw Settings object, not a tagged
`{"type":"settings"}` envelope. It must include
`protocol_version` matching the renderer's protocol version. The
constructor parses this settings object, validates the version, emits
`hello`, validates `required_widgets`, and starts the iced daemon.

Normal host-to-renderer messages use `send_message(json)`. These are
the same tagged incoming messages documented in
[wire protocol](wire-protocol.md), including `snapshot`, `patch`,
`subscribe`, `effect`, `query`, `reset`, and related request types.

## Callback output

The constructor's second argument is the output callback. The
renderer invokes it with a string containing JSONL protocol output.
Each message is a UTF-8 JSON object followed by `\n`.

The callback argument is a transport chunk, not a guaranteed
one-message delivery. A chunk can contain one message or several
newline-delimited messages, especially when buffered events are
flushed together. Hosts should split on `\n`, ignore the final empty
segment, and parse each non-empty line as a protocol message.

`hello` is emitted synchronously during construction after the output
sink is installed. A `required_widgets_missing` diagnostic can also be
emitted during construction. Hosts must install a callback that is
ready to buffer and route messages before calling `new PlushieApp`.

If the callback throws, the renderer sees a write error. The callback
is not a backpressure mechanism; hosts should keep it small and hand
off work to their own queue if message handling can be expensive.

## Codec behavior

The WASM transport always uses `Codec::Json`. There is no first-byte
codec detection, no MessagePack negotiation, and no length-prefixed
frame. The `hello` message advertises:

- `codec`: `"json"`
- `mode`: `"web"`
- `backend`: `"tiny-skia"`
- `transport`: `"wasm"`
- `widget_sets`: `["iced"]`

JSON output is encoded by the same renderer codec used by native JSON
mode, including JSON value sanitization. Binary fields are represented
with JSON-compatible values. For example, screenshot `rgba` bytes are
base64 strings in JSON output, and `load_font.payload.data` is a
base64 string in JSON input.

MessagePack-only behavior is not available in WASM. A browser bridge
to a remote native app should exchange text frames containing the JSON
messages that `send_message` accepts and the callback emits.

## Message size cap

WASM applies the same 64 MiB per-message cap as the native codec.

The constructor rejects `settingsJson` when the string exceeds the
cap. `send_message(json)` rejects an oversized message before it is
queued for the renderer. Both failures are returned to JavaScript as
`JsValue` errors.

Outgoing messages also pass through the JSON codec's size checks. If
the renderer cannot encode an outgoing message within the cap, that is
treated as an output error by the renderer.

## Parse and runtime errors

Constructor errors are synchronous JavaScript exceptions returned by
the `wasm-bindgen` wrapper. They include invalid settings JSON,
missing or mismatched `protocol_version`, oversized settings, and
shared-memory WASM builds.

`send_message(json)` only validates size and queue state before
returning. JSON syntax and protocol-shape errors are parsed later by
the renderer subscription. A parse failure becomes a warning diagnostic
with `wire_input_error` detail rather than a synchronous exception from
`send_message`.

After the renderer daemon stops because of a runtime error, later
`send_message` calls fail with a JavaScript error. The WASM transport
does not provide the native subprocess restart behavior; the browser
host owns page-level recovery.

## Required widgets

`required_widgets` in Settings has the same meaning as in native wire
mode. The WASM renderer validates the list against:

- built-in iced widget type names
- custom widget type names registered in the WASM build through
  `PlushieApp::with_widgets`

Missing names emit a warning diagnostic:

```json
{
  "type": "diagnostic",
  "session": "",
  "level": "warn",
  "diagnostic": {
    "kind": "required_widgets_missing",
    "missing": ["gauge"]
  }
}
```

The diagnostic is non-fatal. The renderer continues running so the
host can decide whether to show an error, fall back, or keep going.

Custom widgets cannot be added dynamically from JavaScript. They must
be compiled into a custom WASM renderer and registered before
construction.

## Effects and stubs

Browser platform effects are stubbed as unsupported in the stock WASM
renderer. File dialogs, clipboard access, and notifications do not use
browser APIs yet. An unstubbed `effect` request receives an
`effect_response` with an unsupported result.

Effect stubs still work and take precedence over the unsupported web
handler. Hosts can send `register_effect_stub` for a known effect kind
with a canned response, then later `unregister_effect_stub`. The
renderer emits the same `effect_stub_register_ack` and
`effect_stub_unregister_ack` response types as native transports.

Unknown effect kinds are rejected by the stub registry with an error
acknowledgement. Known stubs are stored in renderer state, just like
native wire mode.

## Differences from native transports

Native wire mode owns a byte stream to a renderer process or socket.
WASM owns an in-page renderer object and two JavaScript call paths:
`send_message` for host-to-renderer input and the callback for
renderer-to-host output.

Important differences:

- WASM is JSON-only. Native transports can start in JSON and switch to
  MessagePack after `hello`; WASM never switches.
- WASM has no stdin/stdout framing, no socket framing, and no
  4-byte MessagePack length prefix. The callback emits JSONL text
  chunks.
- The initial Settings payload is passed to the constructor as a raw
  JSON object. Normal messages sent after construction are tagged
  protocol envelopes.
- WASM runs in the browser's single-threaded `wasm32-unknown-unknown`
  model. Shared-memory WASM modules are rejected because the JavaScript
  callback output path is not thread-safe.
- Native crash isolation comes from a separate renderer process and a
  host restart policy. WASM has no subprocess boundary; the JavaScript
  host must recreate the renderer object or reload the page after a
  fatal renderer error.
- Native platform effects use native libraries. WASM reports platform
  effects as unsupported unless an effect stub is registered.
- Native deployment discovers or connects to a renderer binary. WASM
  deployment loads `plushie_renderer_wasm.js` and
  `plushie_renderer_wasm_bg.wasm` as static browser assets.

The trust-model posture is otherwise the same at the protocol level:
renderer-to-host output is still a closed set of typed events,
responses, and diagnostics. Browser transport integrity and
confidentiality are provided by the page's outer transport, usually
HTTPS or a WebSocket protected by the site.

## See also

- [Wire protocol](wire-protocol.md)
- [Configuration](configuration.md)
- [Direct vs wire](direct-vs-wire.md)
- [WASM deployment](../guides/17-wasm-deployment.md)
- [Trust model](../stewardship/trust-model.md)
