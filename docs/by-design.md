# plushie-rust by-design policies

A persistent record of decisions the project has made about choices
that look like bugs or omissions but are deliberate. Future reviews
and contributors should cross-reference this document before
recommending the same changes again.

This is not a list of review findings; it is a list of *decisions*.
Each section captures a class of recommendation we have rejected,
the reasoning, and the rough boundary at which the decision should
be revisited.

For load-bearing direction, see the stewardship docs under
`docs/stewardship/`. `triage.md` is the routing tool; this document
is the looser companion that captures policies that don't warrant a
full stewardship section but that we keep coming back to.

The same `--` rule from the rest of the repo applies here: avoid
double dashes in prose, use commas, periods, parentheses, or new
sentences instead.

---

## Pre-1.0: zero backcompat obligation

The project is pre-1.0 and not yet used by anyone outside the
maintainers. There is no migration story, no users to coordinate
with, no compat shims to preserve. The right design wins; the
rename happens.

What this rules out:

- Preserving old `Display` output, error messages, or wire field
  names "for migration."
- Adding `Other(...)` catch-alls or `Legacy` bridge variants to
  enums "to ease the transition."
- Maintaining deprecated APIs alongside their replacements.
- Keeping unused enum variants, fields, or modules "for future
  use." Pre-1.0 dead code is deleted; if a use case shows up
  later, the variant can be re-added with the right shape.

This applies to internal types, public types, and wire types. The
only carve-out is the `CHANGELOG.md`, which records what changed
and is the historical record.

Revisit at the 1.0 cut, after which the rules invert and breaking
changes need real cause.

---

## API hardening is a single 1.0 sweep, not piecemeal work

`#[non_exhaustive]`, `#[must_use]`, sealed traits, and similar
forward-compatibility annotations on public types happen in a
single planned sweep at the 1.0 cut. They do not happen
incrementally during pre-1.0 development.

Reviewers regularly flag specific public enums as missing
`#[non_exhaustive]` (`EffectRequest`, `SystemOp`, `EffectKind`,
`Selector`, `OutgoingMessage`, `IncomingMessage`, `CoalesceHint`,
`SelectionMode`, `Codec`, `Incoming`, `EffectRequestValidationError`,
`HandleResult`, etc.) or specific types as missing `#[must_use]`
(`Command`, `View`, `ViewList`, `Subscription`). These are correct
observations; the timing is wrong.

The reasoning is two-fold:

- Pre-1.0 has no semver obligations, so the annotations cost
  nothing to add today and buy nothing today either.
- The 1.0 sweep is itself a real design pass. Doing it
  piecemeal produces drift between which enums are guarded and
  which aren't, scatters the annotations across many commits,
  and makes the overall stability surface harder to audit at
  the 1.0 boundary. Coherent is better than incremental.

What still warrants action:

- Public types whose contract genuinely benefits from these
  annotations *for correctness reasons*, not for semver. For
  example, a sealed-trait pattern that prevents external manual
  implementation of a derive-only contract is a correctness
  concern (the manual impl can produce wire-incompatible
  output), not a semver concern, and is worth doing today.

Revisit at the 1.0 cut. Until then, decline `#[non_exhaustive]`
and `#[must_use]` requests with a pointer to this section.

---

## Renderer-to-host integrity is defended; host-to-renderer is broad by design

The trust model (see `docs/stewardship/trust-model.md`) makes the
renderer-to-host channel closed and typed: a fixed enumeration of
event variants and structured response types, no opaque-blob path,
no string-eval path. The host is structurally protected today
against a compromised or malicious renderer, and we make that
claim deliberately.

The host-to-renderer channel is broader by design. The host has
to be able to ask the renderer to do real things: load fonts and
images from paths, load SVGs, save screenshots, dispatch platform
effects, spawn child processes via `--exec`. A compromised host
can drive the full operation set against the user's machine.
Bounding this surface is the focus of the capability-manifest
roadmap; it is a stated direction, not currently scheduled work.

Findings framed against a "compromised host attacking the
machine" threat model are not currently in scope and should be
declined or deferred to the capability-manifest roadmap.

Specifically declined under this section:

- Restricting SVG, image, or font source paths to an allowlist.
  The host today is trusted with full filesystem access via the
  renderer; restricting paths against the host adversary is the
  capability-manifest's job, not a per-call hardening.
- Reading raw arbitrary file content via image or SVG widget
  source. Same boundary.
- The `--exec` flag passing its argument to `sh -c <command>`.
  This is documented design for trusted operators who need pipe
  chains and shell features. Apps that interpolate untrusted
  input into `--exec` are making their own choice.
- Forwarding `--exec` child stderr unsanitized to the renderer's
  own stderr.
- Inheriting the renderer's stdout to `--exec` children.

Revisit when the capability-manifest direction is taken up.

---

## TCP `--listen` security belongs with the outer transport

The wire protocol is byte-stream agnostic. Confidentiality,
integrity, and authentication-strength obligations are delegated
to the outer transport (SSH, mTLS, named pipe, OS pipe). The
session token at the wire boundary binds a host to a particular
renderer instance; it is not a confidentiality boundary.

Declined under this section:

- Warning when `--listen` binds to non-loopback. The user chose
  to bind there; documenting the recommendation that
  network-exposed listen mode should ride an SSH tunnel or TLS
  is enough.
- Defaulting to SHA-256 digest token transmission instead of
  plaintext on TCP. The plaintext-on-TCP concern is a confidentiality
  concern; it belongs to the outer transport.
- Per-message MAC, sequence numbers, or other integrity layer
  on the wire after authentication. Same.
- Constant-time length comparison hardening on token validation.
  The token length is a fixed 16 bytes (32 hex chars); the
  attacker already knows it.
- Rate limiting on failed token attempts. A 128-bit token gives
  the attacker 2^128 attempts as the brute-force ceiling;
  online lockout is browser-grade-isolation territory.
- Suppressing the protocol version from the rejection error
  message. Probing the listen socket for fingerprinting is not
  a meaningful attack against an already-network-exposed
  socket.
- Token printed to stderr in listen mode. Documented behavior;
  the operator chose to launch with `--listen` and read the
  token.

Revisit if the project takes on remote-host-as-bounded-trust as
a goal.

---

## Information-disclosure findings against the compromised-host threat are misframed

Per the trust model, the host is a user-trusted-component today.
"The host might log this and the logs leak" is not a threat the
project currently makes a claim against.

Declined:

- Stripping panic source-file/line/column from `session_error`
  events. The host is trusted; the panic location is a
  developer aid.
- Stripping or truncating debug-build payload dumps in codec
  decode error messages. Debug builds aren't a deployment
  shape; the dump is for the developer running the renderer
  with `cargo run`.
- Sanitizing protocol-version strings in handshake errors.
- Withholding renderer build metadata from a connected host.

Revisit when the remote-host-as-bounded-trust direction is
taken up.

---

## Adversary-axis DoS hardening is a non-goal

DoS-proofness "at any cost" is a stated non-goal. The renderer
has parser timeouts, frame caps, and per-resource caps, but a
malicious peer can still flood typed events at the protocol
rate. The host SDK has to handle that gracefully; the renderer
does not try to make it impossible.

Declined under this heading:

- Subprocess-isolating SVG parsing so timed-out parsers can be
  SIGKILLed. The 8-worker cap with parse-deadline is good
  enough; documented limitation.
- Watchdog-aborting the process when all SVG slots are stuck.
- Aggressively tight resource caps that break legitimate-but-edge
  use cases.

This is distinct from *correctness bounds*: hard caps that exist
so the renderer can stop cleanly when something upstream has
gone wrong are real and in scope (`MAX_TREE_DEPTH`,
`MAX_LOADED_FONTS`, `MAX_FONT_BYTES`, `MAX_IMAGES`, the wire
message cap). Inconsistent enforcement of an existing cap (one
code path enforces it, another bypasses) is a real bug; adding
new caps for adversary defense is not.

Revisit only if a real things-go-wrong path produces an
unbounded-growth or unbounded-CPU outcome that the existing caps
do not catch.

---

## WASM operates under a different threat model

The WASM build runs in a browser where the JS host controls the
renderer at the JS layer. The host-to-renderer trust boundary is
upstream of the WASM module; there is no separate token gate.

Declined:

- Adding token validation to the WASM entry point.
- Hardening the WASM `MSG_RX` against poisoned-mutex panics
  beyond what the native renderer does. The single-threaded
  nature of `wasm32-unknown-unknown` makes the poison surface
  near-zero in practice.

What is in scope on WASM: install the panic hook, report the
correct backend in the hello message, propagate inferred
settings, and surface structured errors when settings parsing
fails. These are resilience concerns, not threat-model
concerns.

Revisit if a server-side WASM deployment shape is ever taken on
(currently a stated non-goal).

---

## Refactoring without a forcing function is a non-goal

"Module size or file length alone is not a reason to refactor.
The trigger is a real change that the existing structure cannot
accommodate cleanly." (`docs/stewardship/goals-and-non-goals.md`)

Reviewers regularly flag large modules as candidates for splitting
(`ops.rs` at ~1774 lines, `wire.rs` at ~1990 lines, the
`plushie-widget-sdk` crate as having a "dual personality").
These are correct observations about size; they are not forcing
functions.

Declined:

- Splitting `ops.rs` into `ops/window.rs`, `ops/system.rs`,
  `ops/effects.rs`, etc. The file is large but cohesive; types
  and their wire methods live together.
- Splitting `wire.rs` into `runner/async_task.rs`,
  `runner/message_decode.rs`, `runner/wire_command.rs`. Same
  cohesion argument; the runner is a state machine and its
  pieces are entangled by design.
- Gating `plushie-core`'s `serde_json` and `base64` deps behind
  a `wire` cargo feature. `serde_json` is the workspace
  exchange currency; the feature would conditionally compile
  half the crate for no realistic consumer.
- Reworking the `View` type to be a thin alias of `TreeNode`.
  They are semantically distinct; `View` is the SDK-level type
  and may grow SDK-only fields, `TreeNode` is the protocol type.
- Promoting `runtime_internals` from `#[doc(hidden)]` to a
  feature-gated module. The current shape is the right shape
  pre-1.0.
- Replacing the `pub use plushie_core::protocol::*` glob in
  `plushie-widget-sdk::protocol` with an enumerated list. Same
  argument as the dual-personality finding.
- Centralizing the codec in a single location instead of having
  an `App.codec` field plus the writer-sink internal codec.
  They are kept in sync at the same code site; the duplication
  is at most two assignments.

What remains in scope under this heading is *behavioral
divergence between code paths that are supposed to do the same
thing*. If direct-mode clipboard returns an error where
wire-mode returns `{"text": ""}`, that is a real bug; fixing the
divergence does not require refactoring the surrounding
duplication. The fix is the convergence, not the dedup.

Revisit when a real change shows up that the existing structure
cannot accommodate cleanly. If a future feature spans both the
direct and wire effect handlers and the duplication actively
costs the implementation, that is the forcing function.

---

## Performance optimizations need a realistic N or a measured profile

`docs/stewardship/performance-bar.md` is the authority. Lightweight
by default is a goal: algorithmic consolidation, removing
redundant work, and choosing data structures appropriate to the
realistic workload are real work. Clever-for-speed at the cost of
intent, or big-O claims without realistic N, are declined absent
measurement.

What this rules out by default (unless paired with a profile):

- `SmallVec`, `CompactString`, `Arc<str>` interning for fields
  whose realistic worst case is two or three short strings per
  call.
- Replacing `BTreeMap`-style or `Vec<(K,V)>` prop maps with
  `HashMap` for theoretical lookup speed at typical sizes
  (5 to 15 entries).
- Hash-comparing PropMap before deep equality. The current
  shape is small-N friendly.
- Static-string interning for two-element accumulate fields on
  scroll events.
- `#[inline(always)]` on trivial methods that the compiler
  already inlines.
- Per-frame thread-local buffer reuse for `Vec<Element>`
  allocations whose realistic size is a few children.

What is in scope:

- Replacing per-write stdout flushing with buffered output.
  Clean code, large win, no readability cost.
- Adding a node-id index to the tree so per-event lookups are
  O(1) instead of O(n). Clean code, well-understood pattern,
  the realistic N is documented (a few hundred to ~1000
  nodes).
- Returning `&str` from window-id lookup instead of cloning a
  `String`. Cleaner code than the current shape.
- Replacing the image-registry LRU `Vec` with `LruCache` or
  `LinkedHashMap`. Both readable, both algorithmically better
  at the cap.
- Skipping the prop-map-to-JSON conversion in animation scan
  and validation passes when the map can be inspected
  directly.

Revisit any declined item when a profile shows it dominates a
realistic workload.

---

## iced's font system is global and process-lifetime

iced has no font-unload API. Fonts loaded into the global font
system stay there for the life of the process. The
`MAX_LOADED_FONTS` cap (256) and `MAX_FONT_BYTES` cap (16 MiB)
bound the worst case at ~4 GiB of pinned font bytes.

Declined:

- Reclaiming font memory across session resets.
- Tracking per-session font lifetimes for unload.
- Adding a custom font system that supports unload.

In scope:

- Inconsistent enforcement of the existing cap across code
  paths (file-path loads bypass the cap that inline loads
  enforce). Fixing the inconsistency is a real bug; the cap
  itself is not the problem.

Revisit when iced ships a font-unload API.

---

## Box::leak intern caches are bounded by their cap and that's the contract

`intern_dash_segments` and `intern_font_family` use `Box::leak`
to mint `'static` slices for iced types that demand them. The
caches are capped at 1024 entries.

The accepted design is that the *cache* is bounded; the contract
is "up to 1024 unique inputs leak over the process lifetime, no
more." Past the cap, the interner returns `None` and the caller
falls back to a non-leaking default (no dash for strokes,
`Family::SansSerif` for fonts). A one-time info diagnostic fires
on the first cap hit. Dash patterns are also bounded per-entry by
a 64-segment cap so a single oversized pattern cannot dominate
the cache budget.

Declined:

- Replacing the intern caches with an `Arc<str>` or arena-based
  approach as a refactoring goal in itself. The current shape
  is correct for its bounded contract.

Revisit if iced ever accepts non-`'static` slices for the
relevant types.

---

## Mocking the renderer is forbidden

`docs/stewardship/test-discipline.md` is the authority. Tests
that exercise wire-protocol behavior must drive the real
renderer (default backend in every SDK runs the actual
binary). A pure-language mock that diverges from the real
binary is worse than no test.

Declined:

- Adding mock-based tests for "speed" that bypass the real
  renderer.
- Replacing TestSession or run_wire integration patterns with
  in-process stubs for unit-level coverage of wire behavior.

In scope:

- Mocking acceptable only for forced renderer-crash simulation,
  malformed wire bytes that the codec rejects pre-typed, and
  test infrastructure that has no behavioral content of its
  own.

---

## `panic = "unwind"` is required

`catch_unwind` is the load-bearing primitive for widget panic
isolation, session isolation in multiplexed mode, and the SDK's
view/update guarding. With `panic = "abort"`, all three become
no-ops and the resilience model collapses.

The project ships with `panic = "unwind"` (the default for
debug, release, and `dist` profiles). Proposals to enable
`panic = "abort"` for binary-size or performance reasons are
declined on resilience grounds.

In scope: a compile-time assertion that fails the build under
`panic = "abort"`, so the requirement is structurally enforced
rather than convention. Worth adding when convenient, not
urgent.

---

## Cross-SDK shape changes route through plushie-sdk-parity

Wire format, SDK API surface (names, parameter shapes, types,
docstrings), and behavior names need to converge across all six
host SDKs (Rust, Elixir, Gleam, Python, Ruby, TypeScript).
Findings that propose a unilateral change to a shape that
appears in multiple SDKs are routed through the
`plushie-sdk-parity` workflow, not decided in plushie-rust.

Declined as unilateral changes:

- Renaming wire fields, message types, or event families
  without parity coordination.
- Reshaping `OutgoingMessage` / `IncomingMessage` variants
  without updating the parity audit.
- Adding new public SDK types, builder shapes, or trait names
  to the Rust SDK without parity routing.

This is procedural, not a class of recommendation we decline
outright. The work happens; it just routes through the right
workflow.

---

## `parking_lot` is the workspace default for non-poisoning mutexes

Production code uses `parking_lot::Mutex` and `parking_lot::RwLock`.
The state these guard (event sinks, font registries, take-once
init slots, image registries) survives a panic in another holder:
the data is structurally consistent, so poisoning would only force
boilerplate `unwrap_or_else(|e| e.into_inner())` recovery at every
call site. parking_lot has no poisoning by design, so the recovery
boilerplate disappears and behaviour is the same.

Reviewers occasionally flag this as "lost safety" relative to
`std::sync::Mutex`. The poison signal isn't load-bearing for the
data we keep behind these locks; it's a debugging hint we already
choose to ignore. Converging on parking_lot also removes a class
of inconsistency (some sites recovered, others did not).

Test code inside `#[cfg(test)]` may use `std::sync::Mutex` for
the standalone test-serialization pattern (a private static used
to gate a small set of tests that share mutable global state); the
defaults rule applies to production paths.

External locks owned by upstream crates (e.g. iced's font
`FONT_SYSTEM`) are not ours to switch and stay as the upstream
chose.

---

## Platform effect implementations live in `plushie-renderer-lib`

The native rfd/arboard/notify-rust effect implementations sit in
`plushie-renderer-lib::effects::native`, gated on
`cfg(not(target_arch = "wasm32"))`. The `plushie-renderer` binary
and direct mode in the `plushie` SDK both consume the single
`NativeEffectHandler` type from that module instead of each
maintaining their own copy.

Why renderer-lib and not a separate `plushie-effects-impl` crate:
the trait that bounds these implementations (`EffectHandler`)
already lives here, the wasm and native sides already split via
target-cfg in this crate, and the impls are not large enough to
justify a fifth public-facing crate. Both runners depend on
renderer-lib already.

Why a concrete shared type and not default trait method bodies on
`EffectHandler`: there is currently only one native implementation
shape (rfd + arboard + notify-rust). Default trait methods would
ship that single implementation as the default for every consumer
of the trait, including the wasm `WebEffectHandler`, which would
then need to override every method to opt out. A standalone
struct keeps the trait neutral and lets each target pick its own
handler.

The duplication this resolved had already produced a quiet
divergence (clipboard `ContentNotAvailable` was handled in one
copy and not the other) before the consolidation. Future native
effect work lands in one place.

Revisit if a second meaningfully different native handler shape
appears (e.g. a sandboxed/no-OS variant for testing) that doesn't
fit a single struct.

---

## Runner wire bootstrap and transport internals are intentionally narrow

The Rust SDK bridge starts in JSON mode, reads the renderer's
`hello` message, then switches to the codec advertised by that
hello. This bootstrap is the protocol contract: `hello` stays
JSON so a host can negotiate the later stream codec without
already knowing it.

`Bridge::send_load_font` has a separate MsgPack path on purpose.
The typed `OutgoingMessage::LoadFont` payload is a
`serde_json::Value`, which cannot express native MsgPack binary.
The helper writes the same logical envelope with a MsgPack binary
value for font bytes. That second serialization path is not a
refactor target by itself; tests should pin the wire shape.

The bridge reader uses bounded channels. When the consumer is
gone, the reader exits quietly because shutdown is already in
progress. When the channel is full, `SyncSender::send` applies
backpressure instead of dropping messages. A real deadlock or
stall repro is a resilience bug, but the bounded channel shape is
not itself a finding.

The owned Tokio runtime in wire mode is the default for apps that
do not provide one. `run_wire_with_runtime` is the escape hatch
for hosts that already own a runtime or need tighter control.
Lazy runtime creation can be reconsidered if a concrete design
keeps task, timer, and effect behavior clear; the existence of a
private default runtime is not by itself a bug.

Revisit any of these if the wire bootstrap changes, if font
messages stop needing native binary in MsgPack, or if a measured
runtime footprint issue shows the default runtime dominates normal
apps.

---

## Ephemeral concerns that are not bugs

A handful of patterns get flagged repeatedly as "fragile" or
"could be cleaner" but are deliberate and stable. Capturing
them here so reviewers can move past them.

- **Test infrastructure duplication (`RecordingSink`,
  `LineReceiver`, `StderrCapture`).** Three or four copies of a
  small test helper across crates is fine; consolidating into a
  shared dev-dep crate is structural work without a forcing
  function.
- **Selection state retains stale IDs after order changes.**
  The current `Selection` API places the order-pruning
  responsibility on the caller; documented contract.
- **`SubscriptionManager::sync` updates `self.active`
  unconditionally.** Documented design choice; runners apply
  ops synchronously.
- **`MemoCache` and `WidgetViewCache` do not verify node type
  on cache hit.** Type collision under the same scoped ID with
  the same deps hash requires a normalization-level diagnostic
  miss. The existing diagnostics catch the precondition.
- **`Drop` on `TransportGuard` blocks on `child.wait()`.**
  Acceptable; the typical case is sub-second teardown.
- **Small duplicated wire handshake blocks.** Protocol-version
  validation and codec negotiation appear in both connect and
  spawn paths. The duplication is local and readable. Extract
  only when a behavioral change needs to touch both paths.
- **Direct and wire window-sync ordering.** Wire mode must send
  window operations before patches that reference new windows.
  Direct mode applies in-process state and does not need to
  mirror the exact serialized ordering absent a failing behavior.

Revisit individually if a real failure mode against any of
these is observed.

---

## What this document is for

When a future review surfaces a finding that this document
already addresses, the right response is a one-line link, not
a re-litigation:

> "We don't add `#[non_exhaustive]` piecemeal; see by-design.md
> 'API hardening is a single 1.0 sweep'."

If the reasoning here turns out to be wrong, write a superseding
section. Don't silently flip the decision.
