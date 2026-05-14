# cargo-plushie

Cargo subcommand for building and managing [Plushie](https://github.com/plushie-ui/plushie-rust)
renderer binaries. **Pre-1.0**

Host SDKs (plushie-elixir, plushie-gleam, plushie-python, plushie-ruby,
plushie-typescript) shell out to this tool to generate a renderer
workspace that includes the user's custom native widgets, then build
it with `cargo`. It also scaffolds new Plushie apps and custom widget
crates.

## Install

```
cargo install cargo-plushie
```

The tool becomes available as a cargo subcommand:

```
cargo plushie --help
```

## What it does

- `cargo plushie init` - scaffold a new Plushie app in the current
  directory
- `cargo plushie new-widget <name>` - scaffold a new native widget
  crate
- `cargo plushie build` - generate a renderer workspace that wires in
  every native widget declared under `[package.metadata.plushie]` and
  compile it with `cargo build`
- `cargo plushie build --wasm` - build the WebAssembly renderer via
  `wasm-pack`
- `cargo plushie doctor` - diagnose environment issues (toolchain,
  source path, native libs)
- `cargo plushie download` - download a prebuilt renderer binary for
  the target triple
- `cargo plushie default-icons --out DIR` - write Plushie's bundled
  default app icon PNGs to a directory
- `cargo plushie package` - build a standalone Rust launcher from a
  Plushie package manifest and payload archive

## Configuration

Key environment variables:

- `PLUSHIE_RUST_SOURCE_PATH` - absolute path to a plushie-rust
  checkout. When set, generated workspaces patch plushie-renderer and
  plushie-widget-sdk to the local tree, and `build --wasm` runs
  against that checkout.
- `PLUSHIE_BINARY_PATH` - absolute path to a prebuilt
  `plushie-renderer` binary, used by host SDKs that prefer to supply
  their own.

See [docs/build-tool.md](https://github.com/plushie-ui/plushie-rust/blob/main/docs/build-tool.md)
for the full command reference.

## Versioning

`cargo-plushie` ships at the plushie-rust workspace version. Host
SDKs pin to a specific `PLUSHIE_RUST_VERSION` and install the
matching `cargo-plushie`. See
[docs/versioning.md](https://github.com/plushie-ui/plushie-rust/blob/main/docs/versioning.md).

## License

MIT OR Apache-2.0
