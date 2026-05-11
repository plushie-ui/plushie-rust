//! In-tree dev-mode rebuilding overlay.
//!
//! Builds a slim status bar (plus optional detail drawer) that a
//! host-side watcher can inject into the view tree to surface
//! rebuild progress without pulling the user out of the app.
//!
//! The overlay uses the `__plushie_dev__/` ID prefix for every node
//! it produces. Widget IDs outside that namespace are left untouched,
//! and runtime event dispatch (future work) can recognise the prefix
//! to route overlay interactions internally rather than forwarding
//! them to `App::update`. Port of Elixir's
//! `Plushie.Dev.RebuildingOverlay`.
//!
//! The overlay operates on the wire-level [`TreeNode`] rather than
//! the SDK's authoring [`crate::View`] because injection happens at
//! the runtime boundary, after `View` has already been collapsed to
//! the canonical tree shape. The Elixir port had this at the View
//! layer, but in Rust the runtime always passes a `TreeNode`.

use plushie_core::protocol::TreeNode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// ID prefix reserved for overlay widgets. Must not collide with any
/// app-defined ID; the double-underscore convention matches Elixir's
/// `__plushie_dev__` marker.
pub const OVERLAY_PREFIX: &str = "__plushie_dev__";

/// Auto-dismiss delay applied to the `Success` status before the
/// overlay stops injecting itself into the tree.
pub const DISMISS_DELAY: Duration = Duration::from_millis(1500);

/// Current rebuild state the overlay renders. Variants mirror the
/// Elixir `Plushie.Dev.RebuildingOverlay` type and carry the same
/// semantics (auto-dismiss on success, sticky on failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// A rebuild is in progress.
    Rebuilding,
    /// The most recent rebuild succeeded. Auto-dismisses after
    /// [`DISMISS_DELAY`].
    Success,
    /// The most recent rebuild failed. Sticky until the next
    /// rebuild starts.
    Failed,
    /// The UI is frozen because a subsequent cycle keeps failing.
    /// Sticky, no dismiss button.
    Frozen,
}

impl Status {
    /// Human-readable label displayed in the status bar.
    pub fn message(self) -> &'static str {
        match self {
            Status::Rebuilding => "Rebuilding...",
            Status::Success => "Rebuild succeeded.",
            Status::Failed => "Rebuild failed.",
            Status::Frozen => "UI frozen: view/1 is failing repeatedly.",
        }
    }

    /// Compact status icon rendered before the message.
    pub fn icon(self) -> &'static str {
        match self {
            Status::Rebuilding => "...",
            Status::Success => "ok",
            Status::Failed | Status::Frozen => "!!",
        }
    }
}

/// Overlay snapshot: the data the watcher pushes to the UI side
/// after every build-state change.
#[derive(Debug, Clone)]
pub struct RebuildingOverlay {
    /// Current status variant.
    pub status: Status,
    /// Free-form detail text (cargo stdout + stderr) rendered in the
    /// drawer when expanded.
    pub detail: String,
    /// When true, the drawer is expanded and shows `detail`.
    pub expanded: bool,
    /// Timestamp the status last transitioned to `Success`, used to
    /// drive the auto-dismiss window.
    pub success_at: Option<Instant>,
}

impl Default for RebuildingOverlay {
    fn default() -> Self {
        Self {
            status: Status::Rebuilding,
            detail: String::new(),
            expanded: false,
            success_at: None,
        }
    }
}

impl RebuildingOverlay {
    /// Returns true when a `Success` overlay has been visible longer
    /// than [`DISMISS_DELAY`] and should stop injecting itself.
    pub fn should_dismiss(&self) -> bool {
        matches!(self.status, Status::Success)
            && self
                .success_at
                .map(|t| t.elapsed() >= DISMISS_DELAY)
                .unwrap_or(false)
    }
}

/// Thread-safe handle shared between the watcher (producer) and
/// the runtime tree walker (consumer).
///
/// The watcher calls [`DevOverlayHandle::set`] from its loop to push
/// a new snapshot; the runtime reads the snapshot via
/// [`DevOverlayHandle::snapshot`] when assembling each frame's view
/// tree.
#[derive(Debug, Clone, Default)]
pub struct DevOverlayHandle {
    inner: Arc<Mutex<Option<RebuildingOverlay>>>,
}

impl DevOverlayHandle {
    /// Create a handle with no active overlay.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the current overlay snapshot (`None` hides the overlay).
    pub fn set(&self, overlay: Option<RebuildingOverlay>) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *guard = overlay;
    }

    /// Read the current overlay snapshot, dismissing expired ones.
    pub fn snapshot(&self) -> Option<RebuildingOverlay> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(o) = guard.as_ref()
            && o.should_dismiss()
        {
            *guard = None;
            return None;
        }
        guard.clone()
    }
}

/// Returns true when the given widget ID belongs to the overlay.
/// Host runtimes can use this to intercept events before they reach
/// `App::update`.
pub fn is_overlay_id(id: &str) -> bool {
    id == OVERLAY_PREFIX || id.starts_with(&format!("{OVERLAY_PREFIX}/"))
}

/// Inject an overlay snapshot into a rendered tree, wrapping
/// each window's content with a stack whose top layer is the overlay
/// status bar.
///
/// When `overlay` is `None`, the tree is returned unchanged.
pub fn inject(tree: TreeNode, overlay: Option<&RebuildingOverlay>) -> TreeNode {
    let Some(overlay) = overlay else {
        return tree;
    };
    let overlay_node = build_overlay(overlay);
    wrap_windows(tree, &overlay_node)
}

// ---------------------------------------------------------------------------
// Internal tree builders
// ---------------------------------------------------------------------------

fn wrap_windows(mut node: TreeNode, overlay_node: &TreeNode) -> TreeNode {
    if node.type_name == "window" {
        // Prepend a stack that layers the overlay over the window's
        // existing child tree. Windows with no child yet get just the
        // overlay so the status bar still shows.
        let existing = std::mem::take(&mut node.children);
        let wrapped = stack_with_overlay(existing, overlay_node.clone());
        node.children = vec![wrapped];
        return node;
    }
    node.children = node
        .children
        .into_iter()
        .map(|c| wrap_windows(c, overlay_node))
        .collect();
    node
}

fn stack_with_overlay(window_children: Vec<TreeNode>, overlay_node: TreeNode) -> TreeNode {
    let mut children = window_children;
    children.push(overlay_node);
    TreeNode {
        id: format!("{OVERLAY_PREFIX}/stack"),
        type_name: "stack".to_string(),
        props: Default::default(),
        children,
    }
}

fn build_overlay(overlay: &RebuildingOverlay) -> TreeNode {
    let bar = build_bar(overlay);
    let mut column_children = vec![bar];
    if overlay.expanded {
        column_children.push(build_drawer(overlay));
    }
    let column = TreeNode {
        id: format!("{OVERLAY_PREFIX}/column"),
        type_name: "column".to_string(),
        props: Default::default(),
        children: column_children,
    };
    TreeNode {
        id: format!("{OVERLAY_PREFIX}/anchor"),
        type_name: "container".to_string(),
        props: Default::default(),
        children: vec![column],
    }
}

fn build_bar(overlay: &RebuildingOverlay) -> TreeNode {
    let icon_node = simple_text(
        &format!("{OVERLAY_PREFIX}/icon"),
        &format!("[{}]", overlay.status.icon()),
    );
    let status_node = simple_text(
        &format!("{OVERLAY_PREFIX}/status"),
        overlay.status.message(),
    );
    let mut row_children = vec![icon_node, status_node];
    if !matches!(overlay.status, Status::Frozen) {
        let toggle_label = if overlay.expanded { "^" } else { "v" };
        row_children.insert(
            0,
            simple_button(&format!("{OVERLAY_PREFIX}/toggle"), toggle_label),
        );
    }
    if matches!(overlay.status, Status::Failed | Status::Frozen) {
        row_children.push(simple_button(&format!("{OVERLAY_PREFIX}/dismiss"), "x"));
    }
    let row = TreeNode {
        id: format!("{OVERLAY_PREFIX}/bar_row"),
        type_name: "row".to_string(),
        props: Default::default(),
        children: row_children,
    };
    TreeNode {
        id: format!("{OVERLAY_PREFIX}/bar"),
        type_name: "container".to_string(),
        props: Default::default(),
        children: vec![row],
    }
}

fn build_drawer(overlay: &RebuildingOverlay) -> TreeNode {
    let content = if overlay.detail.is_empty() {
        "(waiting for output)".to_string()
    } else {
        overlay.detail.clone()
    };
    let text_node = simple_text(&format!("{OVERLAY_PREFIX}/output"), &content);
    let scrollable = TreeNode {
        id: format!("{OVERLAY_PREFIX}/scrollable"),
        type_name: "scrollable".to_string(),
        props: Default::default(),
        children: vec![text_node],
    };
    TreeNode {
        id: format!("{OVERLAY_PREFIX}/drawer"),
        type_name: "container".to_string(),
        props: Default::default(),
        children: vec![scrollable],
    }
}

fn simple_text(id: &str, content: &str) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: "text".to_string(),
        props: plushie_core::protocol::Props::from_json(serde_json::json!({ "content": content })),
        children: vec![],
    }
}

fn simple_button(id: &str, label: &str) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: "button".to_string(),
        props: plushie_core::protocol::Props::from_json(serde_json::json!({ "label": label })),
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlay(status: Status, detail: impl Into<String>) -> RebuildingOverlay {
        RebuildingOverlay {
            status,
            detail: detail.into(),
            expanded: matches!(status, Status::Failed | Status::Frozen),
            success_at: matches!(status, Status::Success).then(Instant::now),
        }
    }

    #[test]
    fn is_overlay_id_matches_prefix_and_sub_ids() {
        assert!(is_overlay_id("__plushie_dev__"));
        assert!(is_overlay_id("__plushie_dev__/bar"));
        assert!(is_overlay_id("__plushie_dev__/bar/toggle"));
        assert!(!is_overlay_id("__plushie_dev"));
        assert!(!is_overlay_id("app/button"));
    }

    #[test]
    fn status_message_and_icon_are_distinct() {
        // Sanity: each variant carries different display text.
        assert_ne!(Status::Rebuilding.message(), Status::Success.message());
        assert_ne!(Status::Failed.icon(), Status::Rebuilding.icon());
    }

    #[test]
    fn should_dismiss_only_for_expired_success() {
        let mut overlay = RebuildingOverlay {
            status: Status::Success,
            detail: String::new(),
            expanded: false,
            success_at: Some(Instant::now() - Duration::from_secs(5)),
        };
        assert!(overlay.should_dismiss());
        overlay.success_at = Some(Instant::now());
        assert!(!overlay.should_dismiss());
        overlay.status = Status::Failed;
        overlay.success_at = Some(Instant::now() - Duration::from_secs(5));
        assert!(!overlay.should_dismiss());
    }

    #[test]
    fn handle_publishes_and_dismisses() {
        let handle = DevOverlayHandle::new();
        assert!(handle.snapshot().is_none());
        handle.set(Some(overlay(Status::Rebuilding, "")));
        assert_eq!(handle.snapshot().unwrap().status, Status::Rebuilding);
        handle.set(Some(overlay(Status::Success, "done")));
        // Not yet expired.
        assert_eq!(handle.snapshot().unwrap().status, Status::Success);
    }

    #[test]
    fn inject_wraps_window_children_in_stack() {
        let mut window = TreeNode {
            id: "main".to_string(),
            type_name: "window".to_string(),
            props: Default::default(),
            children: vec![],
        };
        window.children.push(simple_text("hello", "hi"));
        let overlay = RebuildingOverlay {
            status: Status::Rebuilding,
            detail: String::new(),
            expanded: false,
            success_at: None,
        };
        let wrapped = inject(window, Some(&overlay));
        assert_eq!(wrapped.type_name, "window");
        assert_eq!(wrapped.children.len(), 1);
        let stack = &wrapped.children[0];
        assert_eq!(stack.type_name, "stack");
        assert_eq!(stack.id, format!("{OVERLAY_PREFIX}/stack"));
        // Original content + overlay node.
        assert_eq!(stack.children.len(), 2);
        assert_eq!(stack.children[0].id, "hello");
        assert_eq!(stack.children[1].id, format!("{OVERLAY_PREFIX}/anchor"));
    }

    #[test]
    fn inject_none_returns_tree_unchanged() {
        let window = TreeNode {
            id: "main".to_string(),
            type_name: "window".to_string(),
            props: Default::default(),
            children: vec![],
        };
        let wrapped = inject(window.clone(), None);
        assert_eq!(wrapped.id, window.id);
        assert!(wrapped.children.is_empty());
    }

    #[test]
    fn build_overlay_failed_includes_dismiss_button() {
        let overlay = RebuildingOverlay {
            status: Status::Failed,
            detail: "boom".to_string(),
            expanded: true,
            success_at: None,
        };
        let node = build_overlay(&overlay);
        // Walk and collect every ID for a simple presence check.
        let mut ids = Vec::new();
        collect_ids(&node, &mut ids);
        assert!(
            ids.iter()
                .any(|i| i == &format!("{OVERLAY_PREFIX}/dismiss")),
            "expected dismiss button, got ids: {ids:?}"
        );
        assert!(ids.iter().any(|i| i == &format!("{OVERLAY_PREFIX}/drawer")));
    }

    fn collect_ids(n: &TreeNode, out: &mut Vec<String>) {
        out.push(n.id.clone());
        for c in &n.children {
            collect_ids(c, out);
        }
    }
}
