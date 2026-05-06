# CLI flags

`plushie::cli::run` is a zero-config wrapper around `plushie::run`
that adds a small set of reserved `--plushie-*` flags for mode
selection, socket attach, automation, and tree inspection. Apps
that already own their CLI can ignore the module and dispatch to
the underlying primitives (`plushie::run`,
`plushie::automation::cli`) directly.

```rust
fn main() -> plushie::Result {
    plushie::cli::run::<MyApp>()
}
```

The parser only consumes flags with the `--plushie-` prefix.
Anything else passes through untouched, so a bespoke argument
parser layered on top still sees its own arguments.

## Reserved flags

| Flag | Argument | Effect |
|---|---|---|
| `--plushie-help` | (none) | Print the reserved-flag summary and exit. |
| `--plushie-mode` | `direct` or `wire` | Force the runner selection. Honoured by `plushie::run` directly. |
| `--plushie-socket` | path or `host:port` | Attach to a listen-mode renderer over the given socket. Honoured by `plushie::run`. |
| `--plushie-token` | string | Token presented during the socket handshake. Used with `--plushie-socket`. |
| `--plushie-script` | path | Run a `.plushie` automation script through the headless harness. |
| `--plushie-replay` | path | Run a `.plushie` script against the windowed renderer so the user can watch. |
| `--plushie-inspect` | (none) | Print a pretty-JSON snapshot of the initial view tree and exit. |

Both `--flag value` and `--flag=value` forms parse identically. An
unrecognised `--plushie-*` flag is a hard error pointing at
`--plushie-help`; this surfaces typos rather than silently
ignoring them.

## Pass-through flags

`--plushie-mode`, `--plushie-socket`, and `--plushie-token` are
parsed by `plushie::run` itself. The CLI module recognises them so
the strict-unknown rule does not reject them, but it does not
re-parse the values; control flow falls through to `plushie::run`
which handles the actual dispatch.

## Automation

`--plushie-script` and `--plushie-replay` route through
`plushie::automation::cli::script` and
`plushie::automation::cli::replay`. Both accept a `.plushie` file
path; see [Testing](testing.md) for the script grammar.

The difference is the backend:

- `--plushie-script` runs against an in-process `TestSession`
  (mock or headless, depending on the file's `backend:` header).
  Fast and headless. Useful in CI.
- `--plushie-replay` forces the windowed backend regardless of
  the header, spawning the real renderer. Useful for visual
  inspection during development.

## Inspect

`--plushie-inspect` builds a `TestSession`, runs `init`, and
prints the resulting view tree as pretty-printed JSON. No
renderer process is spawned. Useful for sanity-checking the
initial tree shape from a shell pipeline.

## Building a custom CLI

The reserved set is small by design; the easy path is opinionated
and meant to disappear when the app's own CLI takes over. To wire
the same behaviour into a bespoke parser, dispatch directly:

| Subcommand | Underlying call |
|---|---|
| `--plushie-script <path>` | `plushie::automation::cli::script::<A>(&path)` |
| `--plushie-replay <path>` | `plushie::automation::cli::replay::<A>(&path)` |
| `--plushie-inspect` | `plushie::automation::cli::inspect::<A>()` then `println!` |
| (no flag) | `plushie::run::<A>()` |

`plushie::cli::run` itself is the canonical reference for the
wiring: it is a thin match expression on top of those primitives.
