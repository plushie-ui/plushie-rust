# Building for WASM

plushie compiles to WebAssembly via the `plushie-renderer-wasm` crate. The WASM
module runs the full iced renderer in the browser (or any WASM host)
and communicates with the host via JavaScript callbacks.

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## Quick build

```bash
wasm-pack build crates/plushie-renderer-wasm --target web
```

Output lands in `crates/plushie-renderer-wasm/pkg/`:

```
crates/plushie-renderer-wasm/pkg/
  plushie_renderer_wasm.js          # JS glue code (ESM)
  plushie_renderer_wasm.d.ts        # TypeScript declarations
  plushie_renderer_wasm_bg.wasm     # WASM binary
  plushie_renderer_wasm_bg.wasm.d.ts
  package.json
```

Other targets: `--target nodejs` for Node.js, `--target bundler`
for webpack/vite. The `web` target works without a bundler.

## JavaScript API

```typescript
import init, { PlushieApp } from './plushie_renderer_wasm.js';

await init();

const app = new PlushieApp(settingsJson, (event: string) => {
    const parsed = JSON.parse(event);
    console.log(parsed.type, parsed);
});

// Send protocol messages (Snapshot, Patch, Subscribe, etc.)
app.send_message(JSON.stringify({
    type: "snapshot",
    tree: { type: "window", id: "main", children: [...] }
}));
```

The constructor validates the protocol version, emits the hello
handshake, and starts the iced daemon in the background. Messages
sent via `send_message()` are processed on the next event loop tick.

## Custom builds with widgets

Widgets are Rust code compiled into the WASM binary. Create a
crate that depends on `plushie-renderer-wasm` and registers widgets:

```rust
use plushie_renderer_wasm::PlushieApp;
use plushie_core::app::PlushieAppBuilder;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn create_app(
    settings: &str,
    on_event: js_sys::Function,
) -> Result<PlushieApp, JsValue> {
    let mut builder = PlushieAppBuilder::new();
    builder.register(Box::new(MyCustomWidget));
    PlushieApp::with_widgets(settings, on_event, builder)
}
```

## Size optimization

The default `wasm-pack build` produces a ~10 MB WASM binary (~4.1 MB
gzipped). This includes the full iced renderer, text shaping
(cosmic-text), canvas, markdown, syntax highlighting, image/SVG
decoding, and accessibility.

### Profile settings

Shipping builds go through the `dist` profile defined in the
workspace `Cargo.toml`:

```toml
[profile.dist]
inherits = "release"
lto = true
codegen-units = 1
strip = true
opt-level = 3

[profile.dist.package.plushie-renderer-wasm]
opt-level = "z"
```

Native crates get `opt-level = 3` (speed); the WASM crate overrides
to `opt-level = "z"` (size). Invoke the profile explicitly:

```bash
cargo build --profile dist -p plushie-renderer-wasm --target wasm32-unknown-unknown
```

`dist` increases compile times (~3-5x) compared to the default
`release` profile. Keep `release` for local iteration; reach for
`dist` only for shipping artifacts and CI release builds.

### wasm-opt post-processing

wasm-pack runs wasm-opt automatically, but the bundled version may
not support bulk memory operations emitted by Rust 1.82+. If
wasm-pack fails at the optimization step, run wasm-opt manually:

```bash
# Build without wasm-opt
cargo build --target wasm32-unknown-unknown --release -p plushie-renderer-wasm

# Run wasm-opt manually, enabling only the features rustc actually emits
wasm-opt target/wasm32-unknown-unknown/release/plushie_renderer_wasm.wasm \
    -Oz \
    --enable-bulk-memory \
    --enable-bulk-memory-opt \
    --enable-mutable-globals \
    --enable-nontrapping-float-to-int \
    --enable-sign-ext \
    --enable-reference-types \
    --enable-multivalue \
    -o crates/plushie-renderer-wasm/pkg/plushie_renderer_wasm_bg.wasm
```

**Do not use `--all-features`.** It enables every wasm proposal (GC,
memory64, relaxed-simd, etc.) which rewrites the binary using type
encodings that browsers reject. Only enable the features that rustc's
`wasm32-unknown-unknown` target actually uses.

Install a recent wasm-opt via `npm install -g binaryen` or your
system package manager if the wasm-pack bundled version is too old.

### Size comparison

Measured with Rust 1.92, plushie-iced 0.8, wasm-opt from binaryen:

| | Default | Profile opts | + wasm-opt -Oz |
|---|---|---|---|
| Raw | 10.0 MB | 10.0 MB | 8.1 MB |
| Gzip | 4.1 MB | 3.6 MB | 3.5 MB |
| Brotli | -- | -- | 2.7 MB |

Most CDNs and browsers support Brotli, so 2.7 MB is the effective
transfer size for production deployments.

### What contributes to binary size

The largest contributors (approximate, based on feature analysis):

- **iced renderer** (wgpu shaders, layout, text) - unavoidable core
- **cosmic-text** (text shaping, fontdb) - unavoidable for text
- **markdown + highlighter** - pulldown-cmark, syntect, themes
- **image** - PNG/JPEG/etc. decoding
- **svg** - resvg/usvg vector rendering
- **canvas** - 2D drawing, hit testing, tessellation

Feature-gating `markdown`, `highlighter`, `image`, and `svg` in
plushie-core would let WASM builds exclude unused capabilities. This
is not yet implemented but would be the next meaningful size
reduction (estimated 20-30% for a minimal build).

## Known issues

**wasm-opt compatibility.** Rust 1.82+ emits `memory.copy` (bulk
memory operations) which older wasm-opt versions reject. Enable
`--enable-bulk-memory` (and the other flags listed above) or
upgrade binaryen. Do not use `--all-features` as it injects GC
and memory64 types that browsers reject. wasm-pack's bundled
wasm-opt may lag behind.

**Effects.** File dialogs, clipboard, and notifications are stubbed
as unsupported. Web API implementations (Clipboard API, File System
Access API, Notification API) can be added to `WebEffectHandler`.

**Fonts.** File path fonts in the Settings `fonts` array are not
supported on WASM (no filesystem). Use inline font data
(`{"data": "base64..."}` objects) or the `load_font` widget op
with base64-encoded bytes.

**Sessions.** WASM is single-session by architecture. Standard
`wasm32-unknown-unknown` has no threads, so the multiplexed
session dispatcher in the native renderer has no counterpart in
the browser. Host integrations that need isolation between test
fixtures must spawn separate WASM instances instead of relying on
the `session` routing field. The `--max-sessions` CLI flag is
native-only; the WASM entry point does not parse CLI flags.
