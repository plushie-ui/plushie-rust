# Capability and manifest system

A direction under consideration for the trust model: a vanilla
`plushie-renderer` binary paired with an arbitrary host could
eventually be a bounded security boundary on the host-to-renderer
side too, anchored in a manifest of declared capabilities.

Today the renderer-to-host side is already structurally bounded
by the closed shape of the wire protocol (see `trust-model.md`).
The host-to-renderer side is not. This roadmap item is about
bounding that side.

This is a direction under consideration, not currently scheduled
work. The section below captures the rough shape it could take,
the threat model it could target, and observations from the
current codebase that may be relevant when the work is taken up.
Notes that connect to this direction (from any source: a
refactor, a design discussion, an observation surfaced during
review, an exploration) get appended to "Observations" rather
than tracked as standalone work.

## Shape it could take

- Capability-based, not sandbox-only. Each capability the host can
  exercise (filesystem read on path X, clipboard, OS
  notifications, network endpoint Y, spawning subprocesses)
  could be named and explicitly declared.
- Defense in depth. Process-level privilege drop as early as
  possible (filesystem allowlist, network allowlist, syscall
  restrictions, namespace isolation where supported) and an
  in-process enforcement layer that refuses operations outside
  the declared set from the start. The two layers should cover
  different failure modes: process-level for defense against
  memory-corruption-style escape, in-process for defense against
  logic bugs in trusted code.
- Manifest-anchored. Initial use case: the manifest ships with or
  is compiled into the renderer (Rust apps that bundle the
  renderer, or vendors that publish a renderer plus their app
  together). Worth considering later: the host declares a
  manifest the renderer presents to the user before connecting.

## Threat model it could target

A vendor ships a stock `plushie-renderer` plus their own host app
to end users. The host is later compromised. The compromise
should be bounded.

A compromised host could push misleading UI (today and after the
work), but should not be able to:

- RCE the user's machine through the renderer.
- Read or write filesystem paths outside the manifest.
- Reach network endpoints outside the manifest.
- Spawn arbitrary subprocesses through the renderer's effect or
  transport surface.

The user could audit the manifest and reason about the worst case
on the host-to-renderer side.

The renderer-to-host side is already covered by the protocol's
closed shape and does not depend on this roadmap item.

## Open design questions

- Manifest format: declarative file shipped alongside the binary?
  Embedded? Both?
- Capability granularity: per-path, per-glob, per-effect-kind?
- How should a capability denial surface on the wire so the host
  can degrade gracefully rather than crash?
- Process-level enforcement varies by platform (Linux Landlock /
  seccomp, macOS sandbox-exec / app entitlements, Windows
  AppContainer / job objects, WASM origin permissions). How much
  of that should be abstracted versus exposed as
  platform-conditional knobs?
- How does the WASM target map onto this? Origin-based browser
  permissions cover some of the same ground; the manifest concept
  may collapse onto that layer rather than duplicating it.
- What is the upgrade story when the manifest format changes?

## Observations from current code that may be relevant here

Append entries as relevant observations surface from any source.
Each entry: brief description, why it could matter, current
treatment.

- Effect dispatch (clipboard, file dialogs, OS notifications)
  flows through a single `EffectHandler` trait in
  `plushie-renderer-lib`. That is a natural shape for capability
  gating; gating logic could plug in there rather than at each
  call site. Worth preserving as a coherent boundary regardless
  of whether this roadmap item lands.
- File path inputs (SVG, image, font, screenshot save) currently
  resolve any absolute or relative path. A manifest could
  restrict this to declared roots. Aggressively restricting it
  today would break legitimate app-bundled assets, so the broad
  acceptance is a known direction debt rather than a bug.
- Intern caches in widget-sdk (`LineDash`, font family) use
  `Box::leak` past their cap. Under a future memory capability
  these could become bounded LRU. The unbounded leak is itself a
  correctness concern, see `trust-model.md`; that part is not
  blocked on this roadmap item.
- Renderer-owned child process startup inherits the filtered parent
  process environment and forwards the child's stdout/stderr unchanged.
  A capability system should formalize what the child is allowed to
  inherit. Today renderer-parent exec is operator-trusted surface
  (developer tooling, hot-reload connect). The Rust SDK's
  `Bridge::spawn` already uses an env whitelist via
  `runner/env.rs`; the renderer uses `renderer_env.rs` with
  `--exec-env` for configured passthrough. Convergence on the whitelist
  pattern is the natural pre-capability-system step and would
  not preclude a richer capability mechanism later.
- The renderer-to-host event surface is fixed by the wire
  protocol and not extensible from the renderer side. That is
  what makes the host-protection claim in `trust-model.md`
  structural rather than aspirational. Worth preserving
  deliberately as the protocol evolves; any extension that
  loosens this should be flagged here.
- `--listen` TCP mode binds to user-specified addresses without
  a loopback default. Under a capability system this could be
  declared transport scope. Today it is documented as
  trusted-network-only and depends on the outer transport for
  confidentiality and integrity (see `trust-model.md`).
