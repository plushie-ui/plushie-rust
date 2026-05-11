# Rust SDK panic semantics

Direction for making Rust app callback panics match the resilience
promise: an app exception reverts to the last good state, surfaces a
typed diagnostic, and lets the loop continue.

This is design direction, not current behavior. Today
`runtime/view_errors.rs` catches `A::update()` but cannot roll back
mutations already made through `&mut A::Model`; direct and wire mode
therefore keep a partially mutated model after an update panic.
`TestSession` is looser still: several paths call `A::update()` and
`prepare_tree()` directly, so app panics usually unwind the harness.

## Recommendation

Change the Rust SDK before 1.0 so app update is commit-after-success
rather than mutate-in-place:

```rust
fn update(model: &Self::Model, event: Event) -> (Self::Model, Command);
```

The runner keeps owning the last good model. For each event it calls
`A::update(&model, event)` under `catch_unwind`. If the callback
returns, the runner commits the returned model, executes the returned
command, renders the committed model, diffs the tree, and syncs
subscriptions. If the callback panics, the runner keeps the old model,
drops any command, keeps the last good tree, records a typed panic
diagnostic, and continues.

This shape gives rollback without adding `Clone`, `Serialize`, or an
app-authored checkpoint trait to `A::Model`. It also aligns Rust with
the functional direction already present in the sibling SDKs: update
computes the next model and command, and the runtime decides whether
that result is committed.

`view` keeps its current semantic shape. It reads the committed model
and returns the next tree. A view panic never changes the model. The
runner keeps drawing the last good tree, emits a typed diagnostic, and
injects the frozen-UI overlay after sustained callback failure.

Update panics should not be cleared just because the old model can
still render. Recovery is a full successful cycle: a later update
returns normally and the following view returns normally. This avoids
the current weak spot where an update panic increments the shared
counter and a successful render of the unchanged model can immediately
clear it.

## Typed diagnostics

Caught Rust app panics should surface through structured diagnostics,
not only logs. The canonical payload should remain
`plushie_core::Diagnostic`, with dedicated variants for app callback
panics.

The diagnostic should carry:

- Callback phase: update, view, subscribe, window config, renderer exit
  hook, async task, or stream task.
- Panic message extracted from the payload.
- Source location when available from the panic hook.
- Consecutive callback failure state.
- Rollback result: model reverted, tree retained, command dropped, or
  not applicable.

`ViewPanicked` and `UpdatePanicked` already exist, but they should be
made the actual runtime surface rather than log-only helper values.
If the payload grows beyond those variant names, prefer extending or
replacing the variants before 1.0 over adding a stringly side channel.

Direct and wire mode should feed the same diagnostic path. For
renderer-originated diagnostics this is already a typed wire message.
For host-side Rust SDK diagnostics, the SDK should expose the same
typed values locally: logs for operators, `TestSession` accessors for
tests, and any future app-facing diagnostic hook should receive the
typed enum rather than a rendered string.

Source locations are developer aids, not secrets under the current
trust model. Do not strip file, line, or column from app panic
diagnostics on information-disclosure grounds. The host is trusted in
the current model.

## Rejected alternatives

- Keep `fn update(&mut Model, Event) -> Command` and document partial
  mutation after panic. This contradicts the resilience promise and
  leaves Rust weaker than the rest of the host SDK family.
- Clone the model before every update. It preserves the current API,
  but it imposes `Clone` on every app model and makes the steady-state
  cost easy to miss.
- Add an app-provided checkpoint trait. It avoids the `Clone` bound,
  but every app now has to maintain a second rollback API. A broken
  checkpoint is worse than an explicit replacement-model update shape.
- Serialize the model for rollback. Many useful Rust models are not
  serde-shaped, and serialization would turn a runtime safety net into
  a model-schema constraint.
- Try to undo arbitrary side effects performed inside `update` before
  the panic. The framework can roll back its owned model commit and
  suppress returned commands. It cannot undo user code that wrote a
  file, mutated a global, or sent data elsewhere before panicking.
- Represent panic diagnostics as `SystemEvent` strings. That would
  make tests and automation depend on display text, when the codebase
  already has typed diagnostics and `DiagnosticKind`.

## Migration and API impact

This is a breaking Rust SDK API change and should happen before 1.0.
Existing apps that mutate in place will need to return a new model
instead.

Most small apps migrate mechanically:

```rust
fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(Click { id, .. }) if id == "inc" => next.count += 1,
        _ => {}
    }
    (next, Command::none())
}
```

Larger apps should choose their own cheap replacement strategy:
`Arc` for shared immutable data, persistent collections where useful,
or localized field cloning. The framework should not prescribe one
data structure.

`init`, `view`, `subscribe`, `settings`, `window_config`,
`handle_renderer_exit`, and `restart_policy` can keep their current
shapes. Hook callbacks that take `&mut Model` today should be audited:
if they can panic after partial mutation, they either need the same
commit-after-success treatment or must be documented as terminal or
best-effort paths.

The public lifecycle docs should be updated when the code changes.
Until then, they should not claim that Rust already rolls back
`update` mutations.

## Test implications

`TestSession` should drive the same guarded runtime path as direct and
wire mode. It should not call `A::update()` or `prepare_tree()` through
unguarded side paths when those calls represent app runtime behavior.

Strict test sessions should fail on unasserted panic diagnostics the
same way they fail on unasserted normalization diagnostics today.
Tests that intentionally exercise panic recovery can opt out with
`allow_diagnostics()` and assert on `typed_diagnostics()` or
`has_diagnostic(DiagnosticKind::UpdatePanicked)`.

Coverage should pin these behaviors:

- An update panic leaves the model equal to the last committed model.
- A command that would have been returned by the panicking update is
  not executed.
- The last good tree remains in place after an update or view panic.
- Repeated callback panics eventually inject the frozen-UI overlay.
- A later successful update plus successful view clears the callback
  failure state.
- Direct mode, wire mode, and `TestSession` expose the same typed
  diagnostic shape.
- Async and stream task panics continue to resolve as typed async
  errors and do not unwind executor workers or the test harness.

Use real renderer integration where renderer behavior is involved.
`TestSession` coverage is appropriate for pure Rust SDK rollback,
typed diagnostic accumulation, and command suppression because those
are host runtime semantics rather than renderer protocol behavior.
