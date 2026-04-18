//! Multi-window lifecycle synchronization.
//!
//! Ported from `plushie-elixir/lib/plushie/runtime/windows.ex`. The
//! Elixir runtime walks the tree after every update to detect added,
//! removed, and changed windows and explicitly sends
//! `open_window` / `close_window` / `update_window` ops to the
//! renderer. The Rust side now mirrors that.
//!
//! Why a separate pass rather than letting the tree diff carry window
//! props:
//!
//! - Window creation and destruction is a lifecycle event, not a tree
//!   mutation. Splitting it out keeps the renderer's per-window state
//!   (icon, level, scale factor) under a single protocol op rather
//!   than reconstructing it from prop diffs.
//! - Per-window prop updates (title, theme, size) land on a
//!   dedicated `update_window` op so the renderer doesn't have to
//!   treat a window-root `UpdateProps` as a special case.
//! - It cleanly separates "the tree structure changed" from "the
//!   window configuration changed" in the wire transcript.

use std::collections::{BTreeSet, HashMap};

use plushie_core::protocol::TreeNode;
use serde_json::{Map, Value};

/// Window setting keys that can be specified as node props on window
/// elements. Mirrors the Elixir `@window_prop_keys` list so the two
/// SDKs produce equivalent update payloads.
const WINDOW_PROP_KEYS: &[&str] = &[
    "title",
    "size",
    "width",
    "height",
    "position",
    "min_size",
    "max_size",
    "maximized",
    "fullscreen",
    "visible",
    "resizable",
    "closeable",
    "minimizable",
    "decorations",
    "transparent",
    "blur",
    "level",
    "exit_on_close_request",
    "scale_factor",
    "theme",
];

/// Side-effect operation produced by [`sync_windows`].
///
/// These are shaped to hand directly to a wire bridge (or the direct
/// runner's equivalent) as `open`/`close`/`update` window ops. Props
/// payloads are opaque JSON so the caller can forward them unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowSyncOp {
    /// A window newly appeared in the tree.
    Open {
        /// Window ID (matches the `id` field on the `window` node).
        window_id: String,
        /// Merged settings for the new window.
        settings: Value,
    },
    /// A window no longer appears in the tree.
    Close {
        /// Window ID of the removed window.
        window_id: String,
    },
    /// A surviving window's settings changed.
    Update {
        /// Window ID of the changed window.
        window_id: String,
        /// Full, new settings. Consumers apply this as a replacement.
        settings: Value,
    },
}

/// Tracks the set of windows currently reflected on the renderer side.
///
/// Each [`sync`](Self::sync) call walks the new tree, compares to the
/// previous state, and returns a list of ops to drive the renderer
/// back into agreement.
#[derive(Debug, Default)]
pub struct WindowSync {
    windows: BTreeSet<String>,
    last_props: HashMap<String, Value>,
}

impl WindowSync {
    /// Construct an empty tracker. No windows are considered open
    /// until [`sync`](Self::sync) observes them in a tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of currently tracked window IDs. Exposed so callers
    /// can populate auxiliary maps (e.g. `window_id -> iced::window::Id`).
    #[allow(dead_code)] // Not yet used externally; kept for future introspection.
    pub fn active(&self) -> impl Iterator<Item = &str> {
        self.windows.iter().map(String::as_str)
    }

    /// Diff the tree against the tracked state and return ops.
    ///
    /// Base settings (if any) are merged behind the per-window props
    /// so a window declared in the tree without an explicit `title`
    /// falls back to the host's `window_config` defaults.
    pub fn sync(&mut self, tree: &TreeNode, base_settings: &Value) -> Vec<WindowSyncOp> {
        let new_windows = detect_windows(tree);
        let mut ops = Vec::new();

        // Open: in new but not previously tracked.
        for window_id in new_windows.difference(&self.windows) {
            let per_window = extract_window_props(tree, window_id);
            let settings = merge_settings(base_settings, &per_window);
            self.last_props.insert(window_id.clone(), per_window);
            ops.push(WindowSyncOp::Open {
                window_id: window_id.clone(),
                settings,
            });
        }

        // Close: in previous but not new.
        for window_id in self.windows.difference(&new_windows) {
            self.last_props.remove(window_id);
            ops.push(WindowSyncOp::Close {
                window_id: window_id.clone(),
            });
        }

        // Update: still present, props changed.
        for window_id in self.windows.intersection(&new_windows) {
            let new_props = extract_window_props(tree, window_id);
            let old_props = self.last_props.get(window_id);
            if old_props != Some(&new_props) {
                let settings = merge_settings(base_settings, &new_props);
                self.last_props.insert(window_id.clone(), new_props);
                ops.push(WindowSyncOp::Update {
                    window_id: window_id.clone(),
                    settings,
                });
            }
        }

        self.windows = new_windows;
        ops
    }
}

/// Walk the tree and collect the set of window IDs.
///
/// Matches Elixir's `detect_windows`: recurses through any container
/// carrying children so window nodes nested under synthetic wrappers
/// (e.g. the row used in TestSession fixtures) are still found.
pub fn detect_windows(tree: &TreeNode) -> BTreeSet<String> {
    fn walk(node: &TreeNode, out: &mut BTreeSet<String>) {
        if node.type_name == "window" && !node.id.is_empty() {
            out.insert(node.id.clone());
        }
        for child in &node.children {
            walk(child, out);
        }
    }
    let mut out = BTreeSet::new();
    walk(tree, &mut out);
    out
}

/// Extract the subset of `window_id`'s props that belong to window
/// settings. Returns a JSON object with only the recognized keys so
/// drive-by unrelated props don't flood the update op.
pub fn extract_window_props(tree: &TreeNode, window_id: &str) -> Value {
    let Some(node) = find_window_node(tree, window_id) else {
        return Value::Object(Map::new());
    };
    let full = node.props.to_value();
    let mut out = Map::new();
    if let Some(map) = full.as_object() {
        for key in WINDOW_PROP_KEYS {
            if let Some(v) = map.get(*key) {
                out.insert((*key).to_string(), v.clone());
            }
        }
    }
    Value::Object(out)
}

/// Find the window node with the given ID anywhere in the tree.
fn find_window_node<'a>(node: &'a TreeNode, window_id: &str) -> Option<&'a TreeNode> {
    if node.type_name == "window" && node.id == window_id {
        return Some(node);
    }
    for child in &node.children {
        if let Some(n) = find_window_node(child, window_id) {
            return Some(n);
        }
    }
    None
}

/// Shallow merge: per-window props override same-keyed entries in the
/// base settings.
fn merge_settings(base: &Value, per_window: &Value) -> Value {
    let base_map = base.as_object();
    let pw_map = per_window.as_object();
    match (base_map, pw_map) {
        (Some(b), Some(p)) => {
            let mut merged = b.clone();
            for (k, v) in p {
                merged.insert(k.clone(), v.clone());
            }
            Value::Object(merged)
        }
        (Some(_), None) => base.clone(),
        (None, Some(_)) => per_window.clone(),
        (None, None) => Value::Object(Map::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie_core::protocol::{PropMap, Props};
    use serde_json::json;

    fn window(id: &str, props: Value, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: "window".to_string(),
            props: Props::from_json(props),
            children,
        }
    }

    fn container(id: &str, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: "container".to_string(),
            props: Props::from(PropMap::new()),
            children,
        }
    }

    #[test]
    fn detects_top_level_and_nested_windows() {
        let tree = container(
            "root",
            vec![
                window("main", json!({"title": "Main"}), vec![]),
                window("modal", json!({"title": "Modal"}), vec![]),
            ],
        );
        let set = detect_windows(&tree);
        assert!(set.contains("main"));
        assert!(set.contains("modal"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn extract_returns_only_recognized_keys() {
        let tree = window(
            "main",
            json!({"title": "Main", "size": [100, 200], "some_unknown": "ignored"}),
            vec![],
        );
        let props = extract_window_props(&tree, "main");
        assert_eq!(props["title"], json!("Main"));
        assert_eq!(props["size"], json!([100, 200]));
        assert!(props.get("some_unknown").is_none());
    }

    #[test]
    fn sync_emits_open_for_new_windows() {
        let mut sync = WindowSync::new();
        let tree = container(
            "root",
            vec![window("main", json!({"title": "Main"}), vec![])],
        );
        let ops = sync.sync(&tree, &Value::Object(Map::new()));
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            WindowSyncOp::Open { window_id, .. } => assert_eq!(window_id, "main"),
            other => panic!("expected Open, got {other:?}"),
        }
    }

    #[test]
    fn sync_emits_close_for_removed_windows() {
        let mut sync = WindowSync::new();
        let with = container(
            "root",
            vec![window("main", json!({"title": "Main"}), vec![])],
        );
        let _ = sync.sync(&with, &Value::Object(Map::new()));

        let empty = container("root", vec![]);
        let ops = sync.sync(&empty, &Value::Object(Map::new()));
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            WindowSyncOp::Close { window_id } => assert_eq!(window_id, "main"),
            other => panic!("expected Close, got {other:?}"),
        }
    }

    #[test]
    fn sync_emits_update_when_title_changes() {
        let mut sync = WindowSync::new();
        let a = container("root", vec![window("main", json!({"title": "A"}), vec![])]);
        let _ = sync.sync(&a, &Value::Object(Map::new()));

        let b = container("root", vec![window("main", json!({"title": "B"}), vec![])]);
        let ops = sync.sync(&b, &Value::Object(Map::new()));
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            WindowSyncOp::Update {
                window_id,
                settings,
            } => {
                assert_eq!(window_id, "main");
                assert_eq!(settings["title"], json!("B"));
            }
            other => panic!("expected Update, got {other:?}"),
        }
    }

    #[test]
    fn sync_is_quiet_when_nothing_changes() {
        let mut sync = WindowSync::new();
        let tree = container(
            "root",
            vec![window("main", json!({"title": "Main"}), vec![])],
        );
        let _ = sync.sync(&tree, &Value::Object(Map::new()));
        let ops = sync.sync(&tree, &Value::Object(Map::new()));
        assert!(ops.is_empty(), "second sync with unchanged tree: {ops:?}");
    }

    #[test]
    fn sync_handles_two_windows_with_independent_changes() {
        let mut sync = WindowSync::new();
        let v1 = container(
            "root",
            vec![
                window("main", json!({"title": "Main v1"}), vec![]),
                window("modal", json!({"title": "Modal"}), vec![]),
            ],
        );
        let _ = sync.sync(&v1, &Value::Object(Map::new()));

        let v2 = container(
            "root",
            vec![
                window("main", json!({"title": "Main v2"}), vec![]),
                window("modal", json!({"title": "Modal"}), vec![]),
            ],
        );
        let ops = sync.sync(&v2, &Value::Object(Map::new()));
        // Only `main` changed.
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            WindowSyncOp::Update {
                window_id,
                settings,
            } => {
                assert_eq!(window_id, "main");
                assert_eq!(settings["title"], json!("Main v2"));
            }
            other => panic!("expected Update for main, got {other:?}"),
        }
    }

    #[test]
    fn base_settings_merge_behind_per_window_props() {
        let mut sync = WindowSync::new();
        let tree = container(
            "root",
            vec![window("main", json!({"title": "Main"}), vec![])],
        );
        let base = json!({"theme": "dark", "title": "ignored"});
        let ops = sync.sync(&tree, &base);
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            WindowSyncOp::Open { settings, .. } => {
                assert_eq!(settings["title"], json!("Main"), "per-window wins");
                assert_eq!(settings["theme"], json!("dark"), "base fills in");
            }
            other => panic!("expected Open, got {other:?}"),
        }
    }

    #[test]
    fn open_close_first_window_and_open_second() {
        let mut sync = WindowSync::new();
        let a = container(
            "root",
            vec![window("main", json!({"title": "Main"}), vec![])],
        );
        let _ = sync.sync(&a, &Value::Object(Map::new()));
        let b = container(
            "root",
            vec![window("secondary", json!({"title": "Secondary"}), vec![])],
        );
        let ops = sync.sync(&b, &Value::Object(Map::new()));
        let mut closes = 0;
        let mut opens = 0;
        for op in &ops {
            match op {
                WindowSyncOp::Close { window_id } => {
                    assert_eq!(window_id, "main");
                    closes += 1;
                }
                WindowSyncOp::Open { window_id, .. } => {
                    assert_eq!(window_id, "secondary");
                    opens += 1;
                }
                other => panic!("unexpected op {other:?}"),
            }
        }
        assert_eq!((closes, opens), (1, 1));
    }
}
