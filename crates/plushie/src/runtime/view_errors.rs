//! Consecutive-view-error tracking and frozen-UI overlay injection.
//!
//! When `A::view()` panics repeatedly, the renderer keeps drawing
//! the last-good tree. Without intervention the user sees a UI
//! frozen at its last working state with no feedback about why.
//!
//! This module mirrors the Elixir SDK's `Plushie.Runtime.ViewErrors`
//! safety net: every panic in `A::view()` increments a counter;
//! at [`VIEW_ERROR_THRESHOLD`] consecutive panics the runtime
//! overlays a minimal error container onto the tree so the user
//! knows the UI is stale. The counter resets and the overlay
//! clears the next time `A::view()` returns normally.
//!
//! This is a *production* safety net, not a dev-only banner. It
//! runs in both debug and release builds; the dev rebuild banner
//! is a separate feature that Rust does not currently implement
//! (see by-design.md).

use std::panic::{AssertUnwindSafe, catch_unwind};

use plushie_core::protocol::{PropMap, PropValue, Props, TreeNode};

use crate::App;
#[cfg(feature = "direct")]
use crate::runtime::prepare_tree;
#[cfg(feature = "wire")]
use crate::widget::WidgetRegistrar;
#[cfg(feature = "direct")]
use crate::widget::WidgetStateStore;

/// Number of consecutive `A::view()` panics before the frozen-UI
/// overlay is injected. Matches the Elixir SDK's threshold; shared
/// across SDKs via the protocol documentation.
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
    last_good: &TreeNode,
) -> ViewOutcome {
    let result = catch_unwind(AssertUnwindSafe(|| {
        prepare_tree::<A>(model, widget_store, memo_cache)
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
            let message = panic_payload_message(&payload);
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
        let view = A::view(model, &mut registrar);
        crate::runtime::normalize::normalize(&view)
    }));
    match result {
        Ok((tree, warnings)) => {
            state.consecutive = 0;
            state.overlay_active = false;
            ViewOutcome::Ok(tree, warnings)
        }
        Err(payload) => {
            let message = panic_payload_message(&payload);
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
}
