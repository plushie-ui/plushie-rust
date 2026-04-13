# plushie-renderer-wasm

WebAssembly bindings for the [Plushie](https://github.com/plushie-ui/plushie-rust)
renderer. **Pre-1.0**

Runs the full Plushie renderer in the browser (or any WASM host) via
wasm-bindgen. Communication with the host happens through JavaScript
callbacks instead of stdin/stdout.

## Build

```bash
wasm-pack build crates/plushie-renderer-wasm --target web
```

See `docs/wasm-build.md` in the workspace root for size optimization
and build configuration details.

## License

MIT OR Apache-2.0
