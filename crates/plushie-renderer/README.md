# plushie-renderer

Native GUI renderer for [Plushie](https://github.com/plushie-ui/plushie-rust).
**Pre-1.0**

A standalone binary that receives UI tree diffs over stdin (MessagePack
or JSONL), renders them via [iced](https://github.com/iced-rs/iced),
and sends user interaction events back over stdout.

This is the renderer binary that all Plushie host SDKs (Elixir, Gleam,
Python, Ruby, TypeScript) spawn as a subprocess. It also serves as the
default renderer for the Rust SDK's wire mode.

## Modes

- **Windowed** (default) - full iced rendering with native windows
- **Headless** (`--headless`) - real rendering via tiny-skia, no
  display server needed. Supports screenshots.
- **Mock** (`--mock`) - protocol-only, no rendering. Fast testing.

All three modes support session multiplexing via `--max-sessions N`
for concurrent test execution.

## Usage

The renderer is typically spawned by a host SDK, not run directly.
For manual experimentation:

```bash
echo '{"type":"settings","settings":{}}' | cargo run -p plushie-renderer -- --json
```

## License

MIT OR Apache-2.0
