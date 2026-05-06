# Test discipline

How tests are written, what they cost, and what they commit to.
The discipline below shows up in plushie-rust's own test suite,
in `plushie-widget-sdk` tests, and in parallel form across every
host SDK; it is one of the project's load-bearing conventions.

## The integration spine

Tests exercise the real renderer. The default test backend in
every SDK runs the actual `plushie-renderer` binary (or, in the
Rust SDK's direct mode, the actual iced-based runner in-process)
with the actual wire protocol, the actual codec, and the actual
Core engine. The only thing the default backend strips is the
GPU rendering step.

A test that passes against a pure-language mock and would fail
against the real renderer is worse than no test. It gives
confidence on the exact class of bugs the integration is meant
to catch: wire format drift between sender and parser, startup
handshake ordering, codec edge cases, lifecycle on restart, the
small protocol-level details that pure-language mocks have no
mechanism to diverge on.

This is not about coverage as a metric. It is about catching
the bugs that matter where they actually live, which is at
boundaries.

## Three test modes

The renderer offers three runtime modes; the test backends
follow them by name across every SDK:

- **mock**: microseconds to milliseconds. Protocol-only. Real
  binary, real wire protocol, real Core engine, no rendering.
  The default for most tests; fast enough that a full suite
  runs through the real binary without flinching.
- **headless**: tens to low hundreds of milliseconds. Real
  rendering via tiny-skia, no display server. Used when the
  test cares about pixels: screenshot golden files, visual
  verification, layout-affecting bugs.
- **windowed**: seconds. Full iced rendering with a real
  display (headless weston on Linux, native display
  elsewhere). Used when the test cares about the full window
  lifecycle, focus events, or platform-specific behavior.

The naming is a cross-SDK contract; the modes mean the same
thing in plushie-elixir, plushie-gleam, plushie-typescript,
and plushie-rust. New bindings inherit the naming.

## When mocking is acceptable

A pure-language mock that does not go through the renderer is
acceptable only for failure modes the binary cannot be made to
exhibit cleanly:

- Forced renderer crash simulation. The binary cannot be told
  "panic now" via the protocol.
- Malformed wire bytes the codec rejects before any typed
  delivery path runs.
- Test infrastructure that wraps the integration primitives
  themselves.

If a test can run against the binary, it does. The bar for
adding a non-binary mock is "what failure mode does this
expose that nothing else can," answered concretely.

## Direct vs wire dual coverage

The Rust SDK supports two runner modes: direct (in-process
iced) and wire (subprocess renderer). Most tests run against
direct because it is faster; tests of code paths where direct
and wire could meaningfully diverge run in both modes. The
discipline applies whenever a test exercises the wire codec,
the bridge lifecycle, the renderer-restart path, or platform
effects routed through the wire.

Both runner modes are real. Direct is not a mock; it is the
in-process equivalent of the same code paths.

## Tests as documentation

Tests should read as a story for the next person who opens the
file. A clear setup, an explicit action, and an assertion that
names what is being verified beats a clever expression that
folds the three together. The behavior-driven shape of the
test is the documentation; the test framework is incidental.

The corollary: tests are not allowed to be slow. If a test is
slow, the underlying code path is usually slow in production
too. Speed up the code; do not accept the slow test. Mock
mode exists to skip the GPU step, not to hide a slow code path
behind a faster harness.

## Failing test before fix

For a bug fix, write the failing test first when possible. A
test added alongside the fix that would have passed without
the fix proves nothing about the bug. The failing test is the
definition of done.

Exceptions: refactors with no behavior change (the existing
suite is the regression net), and new features where the test
and the implementation arrive together.

## Implications

- A feature has to be testable through the renderer. If a
  feature cannot be exercised through the integration spine,
  that is a design problem with the feature, not a problem
  with the test discipline.
- "Let's mock the renderer for speed" proposals are declined.
  Speed comes from mock mode in the real binary, which is
  already fast; the real cost of a pure-language mock is the
  bug class it hides, not the saved milliseconds.
- Coverage as a percentage is a non-goal (see
  `goals-and-non-goals.md`). Coverage of real surfaces is
  what matters; the integration spine is what produces it.
- Cross-SDK parity audits inherit this discipline: they
  verify actual behavior across SDKs, not asserted behavior,
  by reading source on each side rather than diffing
  documentation.
