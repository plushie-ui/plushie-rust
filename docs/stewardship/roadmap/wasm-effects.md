# WASM browser effects

The stock WASM renderer currently returns `unsupported` for every
platform effect unless the host has registered an effect stub. That is
the correct baseline while browser-backed effects are unsettled. This
roadmap item defines which browser effects are worth implementing first
and the behavior they should expose when the work is taken up.

This is design direction only. It does not change the current
`WebEffectHandler` behavior.

## Current shape

The effect boundary is already centralized:

- Host SDKs send `RendererOp::Effect` with a known `EffectRequest`
  kind and JSON payload.
- `plushie-renderer-engine` validates known kinds, checks the effect
  stub registry first, then dispatches to the renderer's
  `EffectHandler`.
- Native and direct Rust paths use `NativeEffectHandler`, which handles
  file dialogs, clipboard operations, and notifications.
- WASM uses `WebEffectHandler`, which reports all effects as
  unsupported today.

This should stay the boundary. Browser-backed effects belong in
`WebEffectHandler`, not in transport glue or host SDK special cases.
Registered stubs should continue to take precedence over any browser
implementation so tests and host-owned mocks remain deterministic.

## First effects to implement

### Plain-text clipboard

Implement first:

- `clipboard_read`
- `clipboard_write`
- `clipboard_clear`

These map cleanly to the browser Clipboard API and preserve the native
result shapes:

- read success: `ok` with `{"text": "..."}`
- write success: `ok` with `null`
- clear success: `ok` with `null`

An empty clipboard should read as an empty string when the browser can
distinguish that state. Permission denial, blocked user activation, and
browser policy failures should return `error` with a short platform
message. Missing Clipboard API support, insecure contexts, and
non-browser runtimes should return `unsupported`.

### HTML clipboard

Implement after plain text:

- `clipboard_read_html`
- `clipboard_write_html`

These are valuable for editor-style apps, but browser support is less
uniform than plain text and often depends on `ClipboardItem` support.
When implemented, preserve the native result shapes:

- read success: `ok` with `{"html": "..."}`
- write success: `ok` with `null`

If the browser exposes only plain-text clipboard APIs, HTML operations
should return `unsupported`, not silently degrade to text. If the API is
present but rejects the specific operation because of permissions,
policy, activation, or data type restrictions, return `error`.

`clipboard_read_primary` and `clipboard_write_primary` should remain
unsupported in WASM. Browser platforms do not expose the X11 or Wayland
primary selection model.

### Notifications

Implement after clipboard:

- `notification`

The browser Notification API has a useful mapping for simple title and
body notifications. The native `icon` option can map to the browser
`icon` field when present. `timeout`, `urgency`, and `sound` should be
ignored unless a browser API can honor them without surprising users.
Successful display should return `ok` with `null`.

Permission behavior should be explicit:

- If the API is absent, the page is not in a secure context, or the
  browser policy blocks notifications entirely, return `unsupported`.
- If the user grants permission and the notification is created, return
  `ok`.
- If the user dismisses the permission prompt without a final grant or
  denial, return `cancelled`.
- If the user denies permission, or permission was previously denied,
  return `error` with a permission message.

Browsers increasingly restrict notification prompts that are not tied to
a user gesture. A prompt rejection for that reason is an `error`, not an
unsupported effect, because the API exists but the call was not allowed.

## Effects not worth implementing first

Do not implement the existing file and directory effect kinds as
browser-backed effects until the protocol has a browser file story:

- `file_open`
- `file_open_multiple`
- `file_save`
- `directory_select`
- `directory_select_multiple`

The current result contract returns native filesystem paths. Browsers
do not reliably expose those paths. File inputs return `File` objects,
object URLs, or file handles scoped to the page. The File System Access
API can provide handles in some browsers, but those handles are not the
same thing as a stable path string, are permission scoped, and are not
portable across browsers.

Returning fake paths, blob URLs in a `path` field, or display names as
paths would create a cross-SDK trap. A later design can add browser file
effect kinds or a browser-specific result shape if the SDK family needs
in-page file contents or handles. Until then, stock WASM should keep
these effects unsupported and let hosts use effect stubs or host-owned
JavaScript bridges when they need browser file input.

## Async model

Browser-backed effects should be treated as asynchronous even when the
native equivalent is synchronous. Clipboard and notification APIs are
promise-based or permission-gated in browsers, and they must not block
the browser event loop.

`WebEffectHandler::is_async` should return true for browser-backed
kinds. Unsupported kinds may remain synchronous so they can return an
immediate `unsupported` response.

The existing SDK effect tracker remains responsible for timeouts,
one-effect-per-tag replacement, shutdown, and orphaned late responses.
The browser handler should not invent a second cancellation protocol.

## Permission semantics

Use the existing statuses consistently:

- `unsupported`: the backend cannot perform this effect kind in this
  environment. Examples: missing browser API, insecure context for APIs
  that require one, unsupported primary selection, and current
  path-returning file effects.
- `cancelled`: the user dismissed a chooser or permission prompt without
  granting or denying the request. This is a normal user outcome.
- `error`: the browser tried or could have tried the effect but rejected
  it. Examples: permission denied, policy blocked, missing user
  activation, malformed payload, and API promise rejection.
- `ok`: the browser completed the effect and the response payload matches
  the existing typed result contract.

Do not preflight browser permissions as a separate renderer capability
query unless a later design adds a typed query. Prompting is part of the
effect. Passive permission reads are advisory and can race with browser
policy changes.

## Cancellation and timeouts

WASM should preserve the current app-facing lifecycle:

- User dismissal maps to `cancelled`.
- SDK timeouts map to `Timeout`.
- Renderer teardown maps to `Shutdown` for tracked effects.
- A browser promise that resolves after the SDK timed out becomes an
  orphaned response through the existing tracker path.

The renderer should not attempt to abort browser permission prompts.
Most web APIs do not expose a reliable cancellation primitive for those
prompts. If a later effect uses an API with `AbortController`, abortion
can be an internal cleanup optimization, but it must not change the
observable status contract.

## Test direction

When browser-backed effects are implemented, cover them with
browser-run WASM tests rather than native mocks. Unit tests can cover
payload conversion and unsupported fallback, but permission, activation,
and API availability behavior need a browser harness.

Effect stubs should continue to be tested at the renderer protocol
level. Stubs are the deterministic path for app tests and should not
depend on the browser's permission state.

## Observations

- `docs/reference/wasm-transport.md` documents the current behavior:
  stock WASM reports platform effects as unsupported unless a stub is
  registered.
- `crates/plushie-renderer-wasm/src/effects.rs` already names Clipboard
  API and File System Access API as possible future web implementations,
  but the path-returning file result shape makes file effects a poor
  first target.
- The native handler normalizes empty plain-text clipboard reads to
  `{"text": ""}`. The web handler should preserve that when browser
  APIs make the distinction available.
- The existing `EffectResponse` statuses are sufficient for browser
  permission behavior. A new status would make every host SDK parse path
  more complex without adding needed semantics.
