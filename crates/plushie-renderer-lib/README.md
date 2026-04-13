# plushie-renderer-lib

Shared renderer engine for [Plushie](https://github.com/plushie-ui/plushie-rust).
**Pre-1.0**

Contains the core application logic shared between the native renderer
binary and the WASM entry point. Handles incoming message processing,
iced update/view dispatch, event emission, subscription management,
window operations, and widget ops.

Platform differences are injected via the `EffectHandler` trait. The
native binary provides `NativeEffectHandler` (file dialogs, clipboard,
notifications); the WASM entry point provides `WebEffectHandler` (stubs).

Not intended for direct use by application developers. Depend on
`plushie` (Rust SDK) or `plushie-widget-sdk` (widget authors) instead.

## License

MIT OR Apache-2.0
