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

## Animation target comparison is exact after parsing

Animation restart checks compare parsed target values exactly for the
stored representation. For colors, a target that differs by a hex
channel is a different requested color and should retarget the active
animation. Do not broaden this comparison to a display-oriented
epsilon such as one color channel step unless the restart semantics
themselves change.

Animation color values are serialized back to CSS-compatible hex for
the interpolated prop cache. Opaque colors omit alpha and translucent
colors include it. This mirrors normal CSS authoring and keeps the
cache easy for render code to consume; fixed-width color strings are
not a renderer invariant.

`TransitionState` is shared by timed and spring animations. Some fields
are meaningful only for one kind, for example spring velocity. Keep the
shape until a broader animation-state redesign has a concrete forcing
function; splitting it only to remove an inactive field adds matching
and conversion cost without improving behavior.

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
effects, spawn child processes through renderer-owned exec. A compromised host
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
- The renderer-owned exec path uses `--exec-bin` with repeated
  `--exec-arg`, launching the host with argv directly instead of
  passing a shell command string.
- Forwarding renderer-owned exec child stderr unsanitized to the renderer's
  own stderr.
- Inheriting the renderer's stdout to renderer-owned exec children.

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
- Encrypting or hiding the listen token digest on TCP. The renderer
  Settings contract uses `token_sha256`, but that digest is still the
  bearer credential for the connection. Confidentiality belongs to the
  outer transport.
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

## Widget SDK debug guards stay development-only

The widget SDK has debug assertions that verify `prepare` ran before
`render` and that the node did not change between those phases. These
checks intentionally clone node snapshots and panic in debug builds,
because they catch programming errors in the prepare/render split.

Release builds rely on the normal renderer path, which always prepares
the tree before rendering. Keeping the full snapshot check in release
would add per-frame cost to protect against a registry misuse pattern,
not user-facing bad input.

Revisit if a public API allows callers to bypass prepare in ordinary
renderer use, or if a production failure shows the invariant can be
violated without programmer error.

---

## Keyboard `command` is the platform shortcut modifier

`KeyModifiers.command` means "the platform command shortcut is active",
matching `plushie_core::key` combo parsing and the wire docs. On macOS
that usually tracks Command. On Linux and Windows it usually tracks
Control. This is intentionally distinct from `logo`, which reports the
platform logo key itself.

Do not redefine `command` to mean only the physical Command or logo key.
Hosts that need the physical modifier should read `logo`; hosts that
need shortcut semantics should read `command`.

Revisit only if the cross-SDK key model changes through the parity
workflow.

---

## Widget family collision diagnostics prefer deterministic order

`WidgetRegistry::family_collision_diagnostics` walks active widget
implementations in sorted type-name order before de-duplicating
implementations that own multiple type names. The extra sort keeps
diagnostic ownership stable across `HashMap` iteration order and across
process runs.

This path runs around settings/init time, not in the frame loop. Avoid
replacing it with insertion-order or unsorted iteration unless the
diagnostic contract changes.

Revisit if this path ever moves into a hot loop.

---

## Canvas scroll stays opt-in for element interaction

Interactive canvas elements implicitly enable press, release, and move
tracking because those events are fundamental to hover, pressed, drag,
and focus behavior. Scroll is different: capturing wheel or trackpad
events can block page or container scrolling, and many interactive
canvases do not need scroll input.

For that reason, interactive elements do not automatically enable
canvas scroll events. Authors opt in with the canvas-level scroll prop
or the broader interactive prop when scroll is part of the widget
contract.

Revisit only if canvas element metadata grows an explicit per-element
scroll capability.

---

## Canvas focus events use canvas and element scopes deliberately

Canvas widgets can emit focus events for the canvas widget itself and
for semantic elements inside the canvas. Element IDs are scoped under
the canvas ID, for example `canvas/slider/thumb`, and are intended to
be first-class event sources for canvas-local semantics.

Do not collapse canvas-level and element-level focus events into a
single event solely to reduce event volume. The canvas-level event
tracks iced widget focus; the element-level event tracks the active
descendant. Hosts that only care about one level can filter by ID.

Revisit through the wire parity workflow if all SDKs adopt a separate
active-descendant field or a distinct element-focus family.

---

## Canvas public query helpers are pure tree helpers

`canvas_hit_test`, `canvas_find_element_by_id`, and related public
helpers intentionally parse the supplied tree node directly. They are
usable in tests, headless interaction code, and SDK utilities without
requiring a live `CanvasEngine` instance or its prepare lifecycle.

The engine caches parsed interaction data for render-time use. The
public helpers favor portability and simple call sites over sharing
engine-private caches.

Revisit if a measured path repeatedly calls these helpers on large
canvases and the caller can naturally hold an engine reference.

---

## Canvas engine internals are not a query API

`CanvasEngine` exposes operations needed by widget authors composing
the engine: prepare, render, message handling, programmatic focus, and
pruning. Internal state such as pending focus and parsed interaction
lists stays private unless there is a concrete author workflow that
needs it.

Adding accessors just to complete symmetry with setters is declined.
Pre-1.0 still prefers the smaller surface until a real use case draws
the boundary.

Revisit when a custom widget needs to make a state decision that cannot
be made from its own model and cannot be expressed through the existing
engine operations.

---

## Text widget cache misses indicate lifecycle misuse

Markdown and text editor widgets keep prepared parser/content state in
factory-owned caches. Render-time cache misses should not happen during
normal renderer operation because the registry prepare pass runs before
render. The fallback text and warning are last-ditch developer signals,
not a recoverable host-facing state.

Do not add a separate wire diagnostic for these cache misses unless a
real renderer path can reach them without violating the prepare/render
contract. The right fix for such a path would be restoring the lifecycle
invariant, not teaching hosts to recover from it.

Revisit if a multi-session or direct-mode test demonstrates an ordinary
prepare/render ordering hole.

---

## Text editor key binding rules keep malformed modifiers non-matching

Malformed `modifiers` values in text editor key binding rules are logged
when the rule is parsed and represented internally as a rule that never
matches. This preserves the simple rule iteration path while ensuring a
bad modifier list cannot accidentally become an unmodified binding.

Revisit only if the key binding parser grows a typed rule model that can
carry parse errors explicitly.

---

## Canvas pointer coordinate sanitization preserves event shape

Canvas pointer coordinates are renderer-derived values. They pass
through a finite-value sanitizer before JSON construction so an
unexpected non-finite local calculation does not turn one pointer event
into a serialization failure or malformed wire object.

This is not the same as accepting non-finite host-provided wire data.
Host input still follows the strict codec and typed parser rules.

Revisit if non-finite renderer-derived coordinates show up in practice;
that would be a bug in the geometry path and should be fixed at the
source.

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

## Text widget state and cap boundaries are intentional

Text widgets have a few behaviors that look lossy or asymmetric but
are deliberate parts of the current renderer model.

Content caps for `text_input.value`, `text_editor.content`, and
`markdown.content` truncate before expensive shaping or parsing work.
The truncation is not log-only: `plushie-widget-sdk::diagnostics`
emits `Diagnostic::ContentLengthExceeded`, and renderer-lib installs
a hook that forwards those diagnostics as structured
`DiagnosticMessage` values over the normal event sink.

`text_editor.content` is an authoritative host prop. User edits update
the renderer-side content hash so ordinary host echoes of the same
text do not clobber cursor, selection, or undo state. A genuinely new
host `content` value replaces the editor content, including cursor,
selection, and undo state. That is the controlled-widget contract,
not an attempt to merge concurrent host and user edits.

Physical text motion aliases (`left`, `right`, `word_left`,
`word_right`, `home`, `end`) stay physical even in RTL text. Logical
motions (`backward`, `forward`, `word_backward`, `word_forward`,
etc.) route through `TextMotion` and apply direction-aware mapping.
Keep both paths unless the cross-SDK key-binding shape changes.

`text_editor.width` is numeric because iced's text editor currently
accepts pixel widths, unlike `text_input`, which accepts `Length`.
Do not paper over that with a local API shim unless the underlying
widget or the cross-SDK API shape changes.

Tuple-style internal message variants are not public API hardening
work. Rename or reshape them when a behavioral change needs it, but
do not churn internal enum spelling solely to make future additive
fields cheaper before 1.0.

Markdown and rich-text link click payloads carry the author-provided
link string in a typed `{ "link": ... }` event payload. The renderer
does not validate URL schemes or reject control characters there; the
host authored the content and receives the same string back as event
data.

Text input icons are parsed from the same tree-prop data model in every
mode. Direct mode is not a separate native-widget API surface for icon
construction; custom native composition belongs in a custom widget.

Text editor non-edit actions update iced/editor-local state and only
emit host `input` events when content changes. Copy, select, movement,
and similar actions are not host-visible events today.

Missing text content defaults to empty text. Text widgets are display
primitives, and an empty text node is a valid state rather than a
configuration error.

The `rich` alias remains registered as a compatibility alias for
`rich_text` unless the cross-SDK widget-name set is changed together.
Removing a local alias unilaterally creates parity drift for little
clarity gain.

Rich text span parsing happens per render from the raw `spans` array.
For normal rich text sizes this is acceptable and keeps the shape
local. Optimize only with a realistic high-span workload or a measured
profile.

Revisit these if the parity workflow changes the controlled text
contract, if iced's text editor gains `Length` width support, or if a
real profile shows rich span parsing dominating frame time.

---

## Documented visual-style defaults are part of the wire shape

Some visual descriptor decoders accept partial objects and fill in
documented defaults. `Padding` omits sides as `0.0`; `Border` omits
width and radius as zero values; `Shadow` omits color, offset, and
blur as black and zero values. These defaults are intentional builder
ergonomics shared with the wire shape, not malformed-input recovery.

This does not mean every decoder should be permissive. Dropping an
invalid element from a list, truncating an integer, or accepting an
unknown enum value as a different known value is still a protocol
correctness bug. The accepted pattern is only for fields whose absence
has a documented neutral value.

Revisit if a type needs to distinguish "absent" from "explicit zero"
for real behavior.

---

## Forgiving string conversions are ergonomic constructors

Several core types implement `From<&str>` as convenience constructors
for app and test code. These conversions may normalize input, preserve
unknown names as catch-all typed values, or choose a neutral default.
They are not the strict wire-boundary API.

Use the explicit parse method at boundaries when the type provides
one: `KeyPress::from_str`, `MouseButton::from_wire`,
`PointerKind::from_wire`, `InteractAction::from_wire`, and similar
methods return `Result` or `Option` so malformed wire input can be
rejected or diagnosed.

Declined under this section:

- Treating `KeyPress::from("Crtl+s")` as a wire parsing bug. The
  strict path is `"Crtl+s".parse::<KeyPress>()`.
- Treating `MouseButton::from("unknown")` or
  `PointerKind::from("unknown")` as the boundary contract. The
  strict `from_wire` methods exist for that contract.

Revisit only if a wire-boundary call site is using an ergonomic
constructor where it should use a strict parser.

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
- **Wire-mode integration tests are a spine, not a command matrix.**
  Wire and connect tests must use the real renderer when they make
  wire-behavior claims. That does not mean every command variant
  needs its own end-to-end subprocess test. Add real-renderer tests
  when changing a command path or when a failure needs coverage; keep
  broad command shape coverage in lower-level contract tests.
- **Canvas wire values decode into the declared Rust precision.**
  Canvas geometry is represented as `f32`, matching the renderer's
  drawing APIs. Decoders should reject non-finite values and values
  that overflow the declared type, but ordinary `f64` to `f32`
  precision narrowing is the type boundary, not a bug.
- **Canvas feature shape is intentionally current-scope only.**
  Linear gradients, translate/rotate/scale transforms, and current
  animation descriptors are the supported surface today. Radial
  gradients, skew transforms, animation hash keys, and richer
  constructor validation need a concrete renderer or API use case
  before expanding the model.
- **Default-valued animation and style fields may omit on encode.**
  Omitted `delay`, `auto_reverse`, and similar defaults decode to
  their semantic default. Encoders do not need to preserve whether a
  default was explicitly written by the host.
- **Canvas enum hashes use discriminants for process-local caches.**
  The affected enums are fieldless protocol enums with stable variant
  order in source. Their hash values are not persisted or wire-visible;
  they only invalidate in-process canvas caches.

Revisit individually if a real failure mode against any of
these is observed.

---

## Layout widget shorthand precedence

`container.center(true)` is shorthand for setting both child-alignment
axes to center. Explicit `align_x` and `align_y` props remain more
specific and may override the shorthand on their axes. This lets hosts
combine "center by default" with one-axis alignment without adding
extra conflict rules to the prop model.

Revisit only if container alignment becomes ordered rather than
declarative in the host APIs.

---

## Responsive fills by default

The `responsive` widget defaults width and height to `Fill` because
its job is to report available rendered size to the host. Defaulting
to `Shrink` would make it behave more like passive containers, but it
would also commonly report the child size instead of the space a
responsive branch needs to react to.

Use explicit `width` and `height` props when a responsive wrapper
should measure a constrained box.

---

## Animated prop helpers include static fallback

`prop_animated_f32` and the companion animated helpers first consult
the interpolated animation cache, then fall back to static tree props.
Widgets should not duplicate a second `f32::extract` fallback for the
same prop. Object-valued animation descriptors intentionally return
`None` until interpolation supplies a concrete value, so the widget's
normal default applies on that frame.

This means a container `max_width` or `max_height` set as a plain
number is not animation-only. It is read by `prop_animated_f32`.

---

## Interactive widget boundaries stay narrow

Several interactive widget behaviors are deliberately handled at the
widget boundary instead of by broad protocol reshaping.

Image and SVG `alt`, `description`, and `decorative` props are passed
to iced's native image widgets. The `infer_a11y` hook only adds an
override for cases iced cannot infer from those native methods, such
as hiding decorative images from the accessible tree. Do not wrap
image and SVG nodes with duplicate `A11yOverride` labels unless iced
stops exposing the native semantics.

Stateful widget cache misses in `combo_box`, text widgets, and similar
factories indicate a prepare/render lifecycle bug, not normal user
input. A visible fallback plus a renderer log is intentional because
it makes lifecycle corruption obvious during development. Add
structured diagnostics only if real hosts need to recover from this
state.

Pointer area event family names (`press`, `release`, `right_press`,
`middle_press`, `drag`, and related families) are part of the
cross-SDK event shape. Do not rename them unilaterally to a single
generic event with a button payload. Route that through the
plushie-sdk-parity workflow if the event model changes.

Style preset fallback uses the widget's native default style when a
preset name is unknown. Installing no style closure is the default
for iced widgets such as sliders, pick lists, combo boxes, radio
buttons, and togglers. Unknown names should warn, then keep that
default rather than inventing a separate error style.

Progress bar accessibility is inferred from the tree props, while the
rendered visual value may temporarily come from animation state. That
split keeps a11y inference stateless and avoids threading animation
caches through every widget hook. Revisit if animated progress values
need live accessibility announcements.

Widget-sdk unit tests are not the place for malformed wire byte
coverage. Malformed bytes belong in codec and renderer tests, and
wire behavior claims must drive the real renderer. Add direct and
wire dual coverage when changing a path that can meaningfully diverge,
not as a per-widget coverage metric.

Table column parsing and QR module construction happen during render
for ordinary widget sizes. Keep that local unless a realistic workload
or profile shows the cost matters.

---

## Test and app query lanes are separate

Top-level `tree_hash`, `screenshot`, `query`, and `interact` wire
messages are test and automation protocol. They are used by
TestSession implementations, replay scripts, and renderer inspection
paths that need direct request/response control over a live renderer.

Normal application APIs should use the command, window-query,
system-query, and widget-op lanes. For example, `Command::tree_hash`,
`Command::find_focused`, and `Command::screenshot` produce tagged
system events through the app's ordinary event stream instead of
exposing raw top-level request messages.

Do not add app-facing constructors for raw `OutgoingMessage::TreeHash`
or `OutgoingMessage::Screenshot`. If a host SDK needs better test
helpers, add them under that SDK's testing or automation surface while
keeping the raw messages out of the normal app command API.

---

## Bad typed values clamp or reject with diagnostics

Renderer and SDK boundaries distinguish malformed structure from
coercible numeric values.

Malformed structure is rejected. Unknown enum names, wrong object
shapes, missing required fields, invalid variant tags, and impossible
nested forms should fail at the boundary with a clear error or
diagnostic instead of being guessed.

Coercible numeric or bounded values should clamp to the nearest
reasonable value and emit a warning diagnostic. Negative padding,
border width, radius, heading level outside 1..=6, oversized pixel
coordinates, and reversed numeric ranges should not crash an app when
a sane local repair exists. They also must not be repaired silently:
silent clamp, silent drop, and silent default are bugs because tests
and host tooling cannot detect the bad input.

Rust typed builders may normalize dynamic numeric inputs instead of
panicking. The renderer must still defend independently at the wire
boundary and forward diagnostics to the host. Panics remain reserved
for framework invariants that surrounding code proves cannot be
violated by user-facing data.

---

## Canonical visual wire forms stay singular

Some SDK APIs expose ergonomic aliases such as `offset_x` / `offset_y`
for a shadow, and some internal Rust types can represent richer values
than every SDK helper exposes today. Those are not separate wire
contracts.

The shadow wire shape is `{"color", "offset": [x, y],
"blur_radius"}`. The renderer should not accept `offset_x` /
`offset_y` as an alternate shadow wire form.

Canvas path commands use the tagged array command family. For
`rounded_rect`, the canonical shape is `["rounded_rect", x, y, w, h,
radius]`, where `radius` is the normal radius value. Do not keep object
and array command forms as permanent peers unless a future cross-SDK
wire design explicitly adds value beyond spelling.

Color wire values are lowercase long hex strings: `#rrggbb` or
`#rrggbbaa`. SDK helpers may accept short hex or named colors, but
they must normalize before transport.

---

## Status events are internal unless opted in

Renderer status events are used by host runtimes to track widget focus
and derive higher-level focus / blur events. They are not ordinary app
events by default because they are high-churn implementation status
signals.

Do not disable the internal status lane to implement an app-facing
policy. A raw status opt-in should be a separate app-facing lane or
prop so focus tracking remains available to runtimes that need it.

---

## Animation descriptors are render-time props

`Animatable<T>` is an SDK encode-side convenience. Full animation
descriptor decode happens in the renderer's animation manager, where
the current value, prop type, active animation state, and interpolation
cache are available.

Do not add a general `Animatable<T>::wire_decode` round-trip contract
unless that contextual requirement changes. Malformed descriptors
should produce diagnostics and be ignored rather than silently failing
or pretending to decode as an ordinary static prop.

---

## What this document is for

When a future review surfaces a finding that this document
already addresses, the right response is a one-line link, not
a re-litigation:

> "We don't add `#[non_exhaustive]` piecemeal; see by-design.md
> 'API hardening is a single 1.0 sweep'."

If the reasoning here turns out to be wrong, write a superseding
section. Don't silently flip the decision.
