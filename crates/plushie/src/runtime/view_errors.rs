//! Consecutive-callback-error tracking and frozen-UI overlay injection.
//!
//! When `A::view()` or `A::update()` panics repeatedly, the
//! renderer keeps drawing the last-good tree. Without intervention
//! the user sees a UI frozen at its last working state with no
//! feedback about why.
//!
//! This module mirrors the Elixir SDK's `Plushie.Runtime.ViewErrors`
//! safety net: every panic in `A::view()` or `A::update()`
//! increments a shared counter; at [`VIEW_ERROR_THRESHOLD`]
//! consecutive panics the runtime overlays a minimal error container
//! onto the tree so the user knows the UI is stale. The counter
//! resets and the overlay clears the next time a view runs to
//! completion.
//!
//! A panicking `update()` leaves `A::Model` in whatever state the
//! handler reached before it unwound; Rust's `catch_unwind` does not
//! roll back mutations made through `&mut`. The frozen-UI overlay is
//! the app-visible signal that the next event should be treated as
//! recovery, not continuation.
//!
//! This is a *production* safety net, not a dev-only banner. It
//! runs in both debug and release builds; the dev rebuild banner
//! is a separate feature that Rust does not currently implement
//! (see by-design.md).

use std::panic::{AssertUnwindSafe, catch_unwind};

// `run_guarded_view` and `run_guarded_update` rely on `catch_unwind`
// to stop a panic in the user's `A::view` or `A::update` from
// killing the process; the consecutive-panic counter and the
// frozen-UI overlay are the user-visible recovery surface. Building
// with `panic = "abort"` would silently make `catch_unwind` a no-op
// and turn every callback panic into an abort. Surface the conflict
// at compile time, next to the guards that explain the why.
#[cfg(panic = "abort")]
compile_error!(
    "plushie requires `panic = \"unwind\"` because catch_unwind in \
     run_guarded_view / run_guarded_update is load-bearing for the \
     SDK's view+update panic recovery and the frozen-UI overlay. \
     Building with `panic = \"abort\"` would silently make this a no-op."
);

use plushie_core::protocol::{PropMap, PropValue, Props, TreeNode};

use crate::App;
use crate::command::Command;
use crate::event::Event;
#[cfg(feature = "direct")]
use crate::runtime::prepare_tree;
#[cfg(feature = "wire")]
use crate::widget::WidgetRegistrar;
#[cfg(feature = "direct")]
use crate::widget::WidgetStateStore;

/// Number of consecutive `A::view()` or `A::update()` panics
/// before the frozen-UI overlay is injected. Matches the Elixir
/// SDK's threshold; shared across SDKs via the protocol
/// documentation.
pub const VIEW_ERROR_THRESHOLD: u32 = 5;

/// Prop marker used to detect and clear the injected overlay
/// (keyed on TreeNode::id). Not a stable protocol contract; purely
/// an internal sentinel.
const FROZEN_OVERLAY_ID: &str = "__plushie_frozen_ui_overlay__";

/// Runtime state tracking view-error recovery.
#[derive(Debug, Default)]
pub struct ViewErrors {
    /// Count of consecutive panics in `A::view()` without a
    /// successful render in between.
    pub consecutive: u32,
    /// Whether a frozen-UI overlay is currently injected into the
    /// last-good tree. Cleared on the first successful render.
    pub overlay_active: bool,
}

/// Outcome of a guarded `A::view()` call.
pub enum ViewOutcome {
    /// View returned normally. The [`ViewErrors`] counter has been
    /// reset; any active overlay has been cleared from `tree`.
    Ok(TreeNode, Vec<plushie_core::Diagnostic>),
    /// View panicked. `last_good` is the previous tree, with the
    /// frozen-UI overlay optionally injected when the consecutive
    /// count reaches [`VIEW_ERROR_THRESHOLD`].
    Panicked {
        last_good: TreeNode,
        /// Consecutive panic count after this failure. Callers can
        /// use this to emit richer diagnostics (count included in
        /// the log message already).
        #[allow(dead_code)]
        consecutive: u32,
        /// Extracted panic message, best-effort. Useful when the
        /// caller wants to surface the panic in a diagnostic event
        /// beyond the log entry this module emits.
        #[allow(dead_code)]
        message: String,
    },
}

/// Outcome of a guarded `A::update()` call.
pub enum UpdateOutcome {
    /// Update returned normally.
    Ok(Command),
    /// Update panicked. Model may be partially mutated (Rust's
    /// panic-unwind does not roll back mutations made via
    /// `&mut`). The consecutive counter is incremented; callers
    /// fall through to [`run_guarded_view`] / [`run_guarded_view_wire`]
    /// which surfaces the frozen-UI overlay at
    /// [`VIEW_ERROR_THRESHOLD`]. The returned [`Command`] is
    /// [`Command::None`] so the caller can treat a panic exactly
    /// like a successful update that produced no side effect.
    Panicked {
        cmd: Command,
        /// Consecutive panic count after this failure.
        #[allow(dead_code)]
        consecutive: u32,
        /// Best-effort panic message.
        #[allow(dead_code)]
        message: String,
    },
}

/// Call `A::view()` under `catch_unwind` and update `state`.
///
/// On success, resets the counter and clears any prior overlay
/// from the returned tree. On panic, increments the counter and
/// (at threshold) injects the frozen-UI overlay into `last_good`.
#[cfg(feature = "direct")]
pub fn run_guarded_view<A: App>(
    state: &mut ViewErrors,
    model: &A::Model,
    widget_store: &mut WidgetStateStore,
    memo_cache: &mut crate::runtime::MemoCache,
    widget_view_cache: &mut crate::runtime::WidgetViewCache,
    last_good: &TreeNode,
) -> ViewOutcome {
    let result = catch_unwind(AssertUnwindSafe(|| {
        prepare_tree::<A>(model, widget_store, memo_cache, widget_view_cache)
    }));
    match result {
        Ok((tree, warnings)) => {
            state.consecutive = 0;
            // The successful tree is canonical. Any overlay that
            // survived into `last_good` is ignored; we commit the
            // fresh tree.
            state.overlay_active = false;
            ViewOutcome::Ok(tree, warnings)
        }
        Err(payload) => {
            let message = panic_payload_message(&*payload);
            state.consecutive = state.consecutive.saturating_add(1);
            let diag = plushie_core::Diagnostic::ViewPanicked {
                consecutive: state.consecutive,
                message: message.clone(),
            };
            log::error!("{diag}");
            // Emit is log-only. The typed `Diagnostic` pipeline is
            // fed by `WalkCtx::warnings` on the normal successful
            // walk path, and a panicking walk cannot push into it.
            let tree = if state.consecutive >= VIEW_ERROR_THRESHOLD && !state.overlay_active {
                state.overlay_active = true;
                inject_overlay(last_good)
            } else {
                last_good.clone()
            };
            ViewOutcome::Panicked {
                last_good: tree,
                consecutive: state.consecutive,
                message,
            }
        }
    }
}

/// Call the wire-mode view path (no widget expansion) under
/// `catch_unwind`. Mirrors [`run_guarded_view`] but skips
/// [`prepare_tree`] because wire mode doesn't expand composite
/// widgets on the Rust side.
#[cfg(feature = "wire")]
pub fn run_guarded_view_wire<A: App>(
    state: &mut ViewErrors,
    model: &A::Model,
    last_good: &TreeNode,
) -> ViewOutcome {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut registrar = WidgetRegistrar::new();
        // view() returns a ViewList; collapse it to a single root
        // (empty container, single window, or synthetic multi-window
        // container) so wire-mode diff still sees a valid shape.
        let view = A::view(model, &mut registrar).into_tree_node();
        crate::runtime::normalize::normalize(&view)
    }));
    match result {
        Ok((tree, warnings)) => {
            state.consecutive = 0;
            state.overlay_active = false;
            ViewOutcome::Ok(tree, warnings)
        }
        Err(payload) => {
            let message = panic_payload_message(&*payload);
            state.consecutive = state.consecutive.saturating_add(1);
            let diag = plushie_core::Diagnostic::ViewPanicked {
                consecutive: state.consecutive,
                message: message.clone(),
            };
            log::error!("{diag}");
            let tree = if state.consecutive >= VIEW_ERROR_THRESHOLD && !state.overlay_active {
                state.overlay_active = true;
                inject_overlay(last_good)
            } else {
                last_good.clone()
            };
            ViewOutcome::Panicked {
                last_good: tree,
                consecutive: state.consecutive,
                message,
            }
        }
    }
}

/// Call `A::update()` under `catch_unwind` and update `state`.
///
/// On success, returns the [`Command`] the user produced. On panic,
/// increments the same consecutive-error counter that
/// [`run_guarded_view`] feeds so the frozen-UI overlay surfaces
/// whether the failures came from view, update, or a mix. The
/// returned `Command` is [`Command::None`] after a panic.
///
/// Note: Rust's `catch_unwind` does not roll back mutations made
/// to `&mut` bindings before the panic. A partial mutation of
/// `A::Model` is therefore observable in the next frame. The
/// frozen-UI overlay at the threshold is the app-visible signal
/// that recovery is needed; user code that mutates fields in
/// place before validating should validate first or use a
/// transactional wrapper.
pub fn run_guarded_update<A: App>(
    state: &mut ViewErrors,
    model: &mut A::Model,
    event: Event,
) -> UpdateOutcome {
    // AssertUnwindSafe: we do not guarantee model consistency after
    // a panic (see module docs). Frozen-UI overlay is the recovery
    // mechanism; a rolling clone would require `A::Model: Clone`
    // which is a larger API constraint than the safety net is worth.
    let result = catch_unwind(AssertUnwindSafe(|| A::update(model, event)));
    match result {
        Ok(cmd) => UpdateOutcome::Ok(cmd),
        Err(payload) => {
            let message = panic_payload_message(&*payload);
            state.consecutive = state.consecutive.saturating_add(1);
            let diag = plushie_core::Diagnostic::UpdatePanicked {
                consecutive: state.consecutive,
                message: message.clone(),
            };
            log::error!("{diag}");
            UpdateOutcome::Panicked {
                cmd: Command::None,
                consecutive: state.consecutive,
                message,
            }
        }
    }
}

/// Extract a best-effort string message from a panic payload.
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Clone the last-good tree and append a minimal frozen-UI overlay
/// to each top-level window's children.
fn inject_overlay(tree: &TreeNode) -> TreeNode {
    let overlay = build_overlay_node();
    let mut new_tree = tree.clone();
    attach_overlay(&mut new_tree, &overlay);
    new_tree
}

/// Construct the overlay node: a red container with a text child.
fn build_overlay_node() -> TreeNode {
    let mut container_props = PropMap::new();
    container_props.insert("background", PropValue::Str("#b91c1c".to_string()));
    container_props.insert("padding", PropValue::F64(12.0));

    let mut text_props = PropMap::new();
    text_props.insert(
        "value",
        PropValue::Str("UI is not updating. Check error logs for details.".to_string()),
    );
    text_props.insert("color", PropValue::Str("#ffffff".to_string()));

    let text_node = TreeNode {
        id: String::new(),
        type_name: "text".to_string(),
        props: Props::from(text_props),
        children: vec![],
    };

    TreeNode {
        id: FROZEN_OVERLAY_ID.to_string(),
        type_name: "container".to_string(),
        props: Props::from(container_props),
        children: vec![text_node],
    }
}

/// Append the overlay to any window nodes in the tree. If the root
/// is a window, attach directly; otherwise attach to each window
/// found among the root's children. If no windows are present we
/// attach to the root itself.
fn attach_overlay(tree: &mut TreeNode, overlay: &TreeNode) {
    if tree.type_name == "window" {
        tree.children.push(overlay.clone());
        return;
    }
    let mut attached = false;
    for child in &mut tree.children {
        if child.type_name == "window" {
            child.children.push(overlay.clone());
            attached = true;
        }
    }
    if !attached {
        tree.children.push(overlay.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_node(id: &str) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: "window".to_string(),
            props: Props::from(PropMap::new()),
            children: vec![],
        }
    }

    fn overlay_count(node: &TreeNode) -> usize {
        let mut n = if node.id == FROZEN_OVERLAY_ID { 1 } else { 0 };
        for child in &node.children {
            n += overlay_count(child);
        }
        n
    }

    #[test]
    fn inject_overlay_attaches_to_every_window() {
        // Root with three windows: the overlay must land inside every
        // one so the user sees the banner on any visible window.
        let tree = TreeNode {
            id: "root".to_string(),
            type_name: "container".to_string(),
            props: Props::from(PropMap::new()),
            children: vec![
                window_node("main"),
                window_node("secondary"),
                window_node("tertiary"),
            ],
        };

        let result = inject_overlay(&tree);

        assert_eq!(overlay_count(&result), 3, "one overlay per window");
        for child in &result.children {
            let overlay_children: Vec<&TreeNode> = child
                .children
                .iter()
                .filter(|c| c.id == FROZEN_OVERLAY_ID)
                .collect();
            assert_eq!(
                overlay_children.len(),
                1,
                "window {:?} should carry exactly one overlay",
                child.id
            );
        }
    }

    #[test]
    fn inject_overlay_falls_through_when_no_windows() {
        // Without any window children, the overlay lands on the root
        // so the frozen-UI banner still reaches a top-level node.
        let tree = TreeNode {
            id: "root".to_string(),
            type_name: "column".to_string(),
            props: Props::from(PropMap::new()),
            children: vec![],
        };

        let result = inject_overlay(&tree);

        assert_eq!(overlay_count(&result), 1);
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].id, FROZEN_OVERLAY_ID);
    }

    #[test]
    fn inject_overlay_handles_root_window() {
        // Single top-level window as the root itself: the overlay is
        // appended to its children rather than wrapping.
        let tree = window_node("only");

        let result = inject_overlay(&tree);

        assert_eq!(overlay_count(&result), 1);
        assert_eq!(result.type_name, "window");
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].id, FROZEN_OVERLAY_ID);
    }

    // --- Update guard ------------------------------------------------------
    //
    // The update guard catches panics from `A::update()` so the iced
    // task thread survives a bug in the user's handler. The tests below
    // drive a tiny App impl whose update() panics on command and verify
    // the outcome, counter behavior, and shared-counter fall-through to
    // the frozen-UI overlay.

    use crate::App;
    use crate::command::Command;
    use crate::event::{Event, WidgetEvent};
    use crate::widget::WidgetRegistrar;

    /// Test-only app: update() panics when the incoming event carries
    /// ID "boom"; any other event is a no-op. view() returns an empty
    /// container so normalize never fails.
    struct BoomApp;

    impl App for BoomApp {
        type Model = Self;

        fn init() -> (Self, Command) {
            (BoomApp, Command::None)
        }

        fn update(_model: &mut Self::Model, event: Event) -> Command {
            if let Event::Widget(WidgetEvent { scoped_id, .. }) = &event
                && scoped_id.id == "boom"
            {
                panic!("update boom");
            }
            Command::None
        }

        fn view(_model: &Self::Model, _widgets: &mut WidgetRegistrar) -> crate::ViewList {
            crate::View::empty().into()
        }
    }

    fn boom_event() -> Event {
        Event::Widget(WidgetEvent {
            event_type: plushie_core::EventType::Click,
            scoped_id: plushie_core::ScopedId::new("boom".to_string(), Vec::new(), None),
            value: serde_json::Value::Null,
        })
    }

    fn benign_event() -> Event {
        Event::Widget(WidgetEvent {
            event_type: plushie_core::EventType::Click,
            scoped_id: plushie_core::ScopedId::new("ok".to_string(), Vec::new(), None),
            value: serde_json::Value::Null,
        })
    }

    #[test]
    fn run_guarded_update_catches_panic() {
        // A panic in update() must not propagate out; the helper
        // returns a Panicked outcome with the panic message captured
        // so callers can keep driving the event loop.
        let mut state = ViewErrors::default();
        let mut model = BoomApp;
        match run_guarded_update::<BoomApp>(&mut state, &mut model, boom_event()) {
            UpdateOutcome::Panicked { message, .. } => {
                assert!(
                    message.contains("update boom"),
                    "expected panic message, got {message:?}"
                );
            }
            UpdateOutcome::Ok(_) => panic!("expected panic to be caught"),
        }
        assert_eq!(state.consecutive, 1);
    }

    #[test]
    fn run_guarded_update_passes_ok_through() {
        // Non-panicking update() must flow through unchanged and must
        // not touch the consecutive counter.
        let mut state = ViewErrors {
            consecutive: 7,
            ..ViewErrors::default()
        };
        let mut model = BoomApp;
        match run_guarded_update::<BoomApp>(&mut state, &mut model, benign_event()) {
            UpdateOutcome::Ok(_) => {}
            UpdateOutcome::Panicked { .. } => panic!("benign event should not panic"),
        }
        // The view guard is what resets the counter on success; update
        // success is neutral. This keeps the semantics aligned with
        // Elixir's ViewErrors.track_view_error / clear_view_errors:
        // only a successful render clears the count.
        assert_eq!(state.consecutive, 7);
    }

    #[test]
    fn update_panics_share_counter_with_view() {
        // Repeated update panics accumulate in the same counter the
        // view guard reads, so the frozen-UI overlay surfaces after
        // VIEW_ERROR_THRESHOLD total callback panics whether they
        // came from view(), update(), or a mix.
        let mut state = ViewErrors::default();
        let mut model = BoomApp;
        for _ in 0..VIEW_ERROR_THRESHOLD {
            let _ = run_guarded_update::<BoomApp>(&mut state, &mut model, boom_event());
        }
        assert_eq!(state.consecutive, VIEW_ERROR_THRESHOLD);
        assert!(
            !state.overlay_active,
            "update guard should not flip the overlay flag directly; \
             the view guard owns overlay injection"
        );
    }
}
