//! Retained UI tree.
//!
//! [`Tree`] holds the current root [`TreeNode`] and supports full
//! replacement via [`snapshot`](Tree::snapshot) and incremental updates
//! via [`apply_patch`](Tree::apply_patch). Renderer modes read the tree
//! to render widgets, answer queries, and keep host-owned window state
//! synchronized; the host mutates it by sending Snapshot and Patch
//! messages.

use std::collections::{HashMap, HashSet};

use plushie_core::protocol::{PatchOp, TreeNode};
use plushie_widget_sdk::shared_state::MAX_TREE_DEPTH;

/// Retained tree store. Holds the current root node (if any) and supports
/// full replacement (snapshot) and incremental patch application.
///
/// Maintains an internal `id_index` mapping each node ID to the
/// child-index path that addresses it from the root. The index is
/// rebuilt on every snapshot and after every `apply_patch` call so
/// `find_by_id` is O(path-depth) instead of O(tree-size). Empty IDs
/// (legal for unaddressable nodes) are not indexed; under duplicate
/// IDs the first depth-first occurrence wins, matching the recursive
/// scan it replaces.
#[derive(Debug, Default)]
pub struct Tree {
    root: Option<TreeNode>,
    id_index: HashMap<String, Vec<usize>>,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the entire tree with a new root (snapshot).
    ///
    /// The tree is always accepted (the renderer needs UI even if the tree
    /// has problems). Returns `Ok(())` if all node IDs are unique, or
    /// `Err` with a list of `"id (type_name)"` strings for each duplicate.
    /// The caller should emit a protocol error so the host can fix the bug.
    pub fn snapshot(&mut self, root: TreeNode) -> Result<(), Vec<String>> {
        self.root = Some(root);
        self.rebuild_id_index();
        // Validate after setting: the tree is accepted regardless, but
        // duplicates are reported as errors.
        if let Some(root) = self.root.as_ref() {
            validate_unique_ids(root)
        } else {
            Ok(())
        }
    }

    /// Return a reference to the current root, if any.
    pub fn root(&self) -> Option<&TreeNode> {
        self.root.as_ref()
    }

    /// Return a mutable reference to the current root, if any. Used
    /// by transforms that drive the shared [`tree_walk`] walker, which
    /// takes `&mut TreeNode` even for read-only passes.
    ///
    /// Do not change node IDs or structural children through this
    /// reference. Structural mutation invalidates the internal id index;
    /// use [`snapshot`](Self::snapshot) or [`apply_patch`](Self::apply_patch)
    /// so the index is rebuilt.
    ///
    /// [`tree_walk`]: plushie_core::tree_walk
    pub fn root_mut(&mut self) -> Option<&mut TreeNode> {
        self.root.as_mut()
    }

    /// Find a window node by its window ID, searching the entire tree recursively.
    pub fn find_window(&self, window_id: &str) -> Option<&TreeNode> {
        let root = self.root.as_ref()?;
        find_window_recursive(root, window_id, 0)
    }

    /// Collect the IDs of all window nodes in the tree (recursive search).
    pub fn window_ids(&self) -> Vec<String> {
        let Some(root) = self.root.as_ref() else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        collect_window_ids_recursive(root, &mut ids, 0);
        ids
    }

    /// Find a node by ID. Goes through the `id_index` for O(depth)
    /// lookup instead of a full depth-first scan.
    pub fn find_by_id(&self, node_id: &str) -> Option<&TreeNode> {
        let root = self.root.as_ref()?;
        let path = self.id_index.get(node_id)?;
        navigate(root, path).ok()
    }

    /// Returns the type name of the node with the given ID, if found.
    pub fn find_by_type(&self, node_id: &str) -> Option<&str> {
        self.find_by_id(node_id).map(|n| n.type_name.as_str())
    }

    fn rebuild_id_index(&mut self) {
        self.id_index.clear();
        if let Some(root) = self.root.as_ref() {
            let mut path = Vec::new();
            collect_id_index(root, &mut path, &mut self.id_index, 0);
        }
    }

    /// Validate protocol ordering for a sequence of patch operations.
    ///
    /// The wire protocol orders structural ops so indices stay meaningful:
    /// removes first, updates/replacements next, inserts last. Removes and
    /// inserts are ordered only within the same parent path.
    pub fn validate_patch_order(ops: &[PatchOp]) -> Result<(), String> {
        validate_patch_order(ops)
    }

    /// Apply a sequence of patch operations to the tree.
    ///
    /// The caller is responsible for validating protocol ordering before
    /// applying renderer-owned patch sequences. Operations are applied
    /// sequentially. If one operation fails, it is skipped with a warning
    /// and subsequent operations are still applied. This preserves existing
    /// best-effort handling for malformed or stale individual ops.
    /// Applies patch operations and returns any removed nodes that had an
    /// "exit" prop (for exit animation ghost promotion).
    pub fn apply_patch(&mut self, ops: Vec<PatchOp>) -> Vec<(String, usize, TreeNode)> {
        log::debug!("applying patch: {} ops", ops.len());
        let mut exit_nodes = Vec::new();
        for op in ops {
            if op.path.len() > MAX_TREE_DEPTH {
                log::warn!(
                    "failed to apply patch op {:?}: path depth {} exceeds max {}",
                    op.op,
                    op.path.len(),
                    MAX_TREE_DEPTH
                );
                continue;
            }
            // Check for exit nodes before removal
            if op.op == "remove_child"
                && let Some(root) = self.root.as_ref()
                && let Ok(parent) = navigate(root, &op.path)
            {
                let index = op
                    .rest
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(u64::MAX) as usize;
                if index < parent.children.len() {
                    let child = &parent.children[index];
                    if child.props.get("exit").is_some() {
                        exit_nodes.push((parent.id.clone(), index, child.clone()));
                    }
                }
            }
            if let Err(e) = self.apply_op(&op) {
                if matches!(e, PatchApplyError::NoTree) {
                    log::debug!("failed to apply patch op {:?}: {}", op.op, e);
                } else {
                    log::warn!("failed to apply patch op {:?}: {}", op.op, e);
                }
            }
        }
        // Index paths can shift under insert_child/remove_child and
        // get rewritten under replace_node. Rebuilding once per
        // apply_patch keeps incremental complexity off the inner
        // loop and matches the snapshot path's index ownership.
        self.rebuild_id_index();
        exit_nodes
    }

    fn apply_op(&mut self, op: &PatchOp) -> Result<(), PatchApplyError> {
        let root = self.root.as_mut().ok_or(PatchApplyError::NoTree)?;

        match op.op.as_str() {
            "replace_node" => {
                let node = op.rest.get("node").ok_or_else(|| {
                    PatchApplyError::invalid("replace_node: missing 'node' field")
                })?;
                let new_node: TreeNode = serde_json::from_value(node.clone()).map_err(|e| {
                    PatchApplyError::invalid(format!("replace_node: invalid node: {e}"))
                })?;

                if op.path.is_empty() {
                    // Replace root
                    *root = new_node;
                } else {
                    let parent = navigate_mut(root, &op.path[..op.path.len() - 1])
                        .map_err(PatchApplyError::invalid)?;
                    let idx = *op.path.last().unwrap();
                    if idx < parent.children.len() {
                        parent.children[idx] = new_node;
                    } else {
                        return Err(PatchApplyError::invalid(format!(
                            "replace_node: index {idx} out of bounds"
                        )));
                    }
                }
                Ok(())
            }
            "update_props" => {
                let target = navigate_mut(root, &op.path).map_err(PatchApplyError::invalid)?;
                let props = op.rest.get("props").ok_or_else(|| {
                    PatchApplyError::invalid("update_props: missing 'props' field")
                })?;

                let patch_map = props.as_object().ok_or_else(|| {
                    PatchApplyError::invalid(format!(
                        "update_props: patch props is not an object: {props}"
                    ))
                })?;
                let target_map = target.props.as_prop_map_mut();
                for (k, v) in patch_map {
                    if v.is_null() {
                        target_map.remove(k);
                    } else {
                        target_map.insert(
                            k.clone(),
                            plushie_core::protocol::PropValue::from(v.clone()),
                        );
                    }
                }
                Ok(())
            }
            "insert_child" => {
                let parent = navigate_mut(root, &op.path).map_err(PatchApplyError::invalid)?;
                let index = op
                    .rest
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        PatchApplyError::invalid("insert_child: missing or invalid 'index'")
                    })? as usize;
                let node = op.rest.get("node").ok_or_else(|| {
                    PatchApplyError::invalid("insert_child: missing 'node' field")
                })?;
                let new_node: TreeNode = serde_json::from_value(node.clone()).map_err(|e| {
                    PatchApplyError::invalid(format!("insert_child: invalid node: {e}"))
                })?;

                if index <= parent.children.len() {
                    parent.children.insert(index, new_node);
                } else {
                    log::warn!(
                        "insert_child: index {index} is beyond children length {}, appending instead",
                        parent.children.len()
                    );
                    parent.children.push(new_node);
                }
                Ok(())
            }
            "remove_child" => {
                let parent = navigate_mut(root, &op.path).map_err(PatchApplyError::invalid)?;
                let index = op
                    .rest
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        PatchApplyError::invalid("remove_child: missing or invalid 'index'")
                    })? as usize;

                if index < parent.children.len() {
                    parent.children.remove(index);
                    Ok(())
                } else {
                    Err(PatchApplyError::invalid(format!(
                        "remove_child: index {index} out of bounds (len={})",
                        parent.children.len()
                    )))
                }
            }
            other => {
                plushie_core::diagnostics::error(plushie_core::Diagnostic::UnknownPatchOp {
                    op: other.to_string(),
                    payload: patch_op_payload(op),
                });
                Ok(())
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PatchApplyError {
    NoTree,
    Invalid(String),
}

impl PatchApplyError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }
}

impl std::fmt::Display for PatchApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTree => f.write_str("no tree to patch"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

fn patch_op_payload(op: &PatchOp) -> serde_json::Value {
    serde_json::to_value(op).unwrap_or_else(|_| {
        serde_json::json!({
            "op": op.op,
            "path": op.path,
            "rest": op.rest
        })
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PatchPhase {
    Remove,
    Middle,
    Insert,
}

enum PatchOrderOp<'a> {
    Remove { path: &'a [usize], index: usize },
    Middle { parent_path: &'a [usize] },
    Insert { path: &'a [usize], index: usize },
}

#[derive(Debug)]
struct ParentPatchOrder {
    phase: PatchPhase,
    last_remove: Option<usize>,
    last_insert: Option<usize>,
}

impl Default for ParentPatchOrder {
    fn default() -> Self {
        Self {
            phase: PatchPhase::Remove,
            last_remove: None,
            last_insert: None,
        }
    }
}

fn validate_patch_order(ops: &[PatchOp]) -> Result<(), String> {
    let mut parent_orders: HashMap<Vec<usize>, ParentPatchOrder> = HashMap::new();

    for (op_index, op) in ops.iter().enumerate() {
        let Some(order_op) = classify_patch_order_op(op) else {
            continue;
        };

        match order_op {
            PatchOrderOp::Remove { path, index } => {
                let order = parent_orders.entry(path.to_vec()).or_default();
                if order.phase > PatchPhase::Remove {
                    return Err(format!(
                        "patch op at index {op_index} is remove_child, but removes must appear before update_props, replace_node, and insert_child"
                    ));
                }
                if let Some(previous) = order.last_remove
                    && index >= previous
                {
                    return Err(format!(
                        "patch op at index {op_index} removes child {index} from parent path {path:?}, but remove_child ops for the same parent must strictly decrease child indices"
                    ));
                }
                order.last_remove = Some(index);
            }
            PatchOrderOp::Middle { parent_path } => {
                let order = parent_orders.entry(parent_path.to_vec()).or_default();
                if order.phase == PatchPhase::Insert {
                    return Err(format!(
                        "patch op at index {op_index} is update_props or replace_node, but updates and replacements must appear before insert_child"
                    ));
                }
                order.phase = PatchPhase::Middle;
            }
            PatchOrderOp::Insert { path, index } => {
                let order = parent_orders.entry(path.to_vec()).or_default();
                order.phase = PatchPhase::Insert;
                if let Some(previous) = order.last_insert
                    && index < previous
                {
                    return Err(format!(
                        "patch op at index {op_index} inserts child {index} into parent path {path:?}, but insert_child ops for the same parent must not decrease child indices"
                    ));
                }
                order.last_insert = Some(index);
            }
        }
    }

    Ok(())
}

fn classify_patch_order_op(op: &PatchOp) -> Option<PatchOrderOp<'_>> {
    match op.op.as_str() {
        "remove_child" => op
            .rest
            .get("index")
            .and_then(|value| value.as_u64())
            .map(|index| PatchOrderOp::Remove {
                path: &op.path,
                index: index as usize,
            }),
        "update_props" if op.rest.get("props").is_some_and(|props| props.is_object()) => op
            .path
            .split_last()
            .map(|(_, parent_path)| PatchOrderOp::Middle { parent_path }),
        "replace_node"
            if op
                .rest
                .get("node")
                .is_some_and(|node| serde_json::from_value::<TreeNode>(node.clone()).is_ok()) =>
        {
            op.path
                .split_last()
                .map(|(_, parent_path)| PatchOrderOp::Middle { parent_path })
        }
        "insert_child" => op
            .rest
            .get("index")
            .and_then(|value| value.as_u64())
            .and_then(|index| {
                op.rest.get("node").and_then(|node| {
                    serde_json::from_value::<TreeNode>(node.clone())
                        .is_ok()
                        .then_some(PatchOrderOp::Insert {
                            path: &op.path,
                            index: index as usize,
                        })
                })
            }),
        _ => None,
    }
}

/// Walk the tree depth-first and record the path to each non-empty
/// node ID. Mirrors the original `find_by_id` semantics: when an ID
/// appears more than once (which the validator already flags as a
/// host bug), only the first depth-first occurrence is indexed.
fn collect_id_index(
    node: &TreeNode,
    path: &mut Vec<usize>,
    out: &mut HashMap<String, Vec<usize>>,
    depth: usize,
) {
    if depth > MAX_TREE_DEPTH {
        return;
    }
    if !node.id.is_empty() {
        out.entry(node.id.clone()).or_insert_with(|| path.clone());
    }
    for (idx, child) in node.children.iter().enumerate() {
        path.push(idx);
        collect_id_index(child, path, out, depth + 1);
        path.pop();
    }
}

fn find_window_recursive<'a>(
    node: &'a TreeNode,
    window_id: &str,
    depth: usize,
) -> Option<&'a TreeNode> {
    if depth > MAX_TREE_DEPTH {
        plushie_core::diagnostics::warn(plushie_core::Diagnostic::TreeDepthExceeded {
            id: node.id.clone(),
            max_depth: MAX_TREE_DEPTH,
        });
        return None;
    }
    if node.type_name == "window" && node.id == window_id {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_window_recursive(child, window_id, depth + 1) {
            return Some(found);
        }
    }
    None
}

fn collect_window_ids_recursive(node: &TreeNode, ids: &mut Vec<String>, depth: usize) {
    if depth > MAX_TREE_DEPTH {
        plushie_core::diagnostics::warn(plushie_core::Diagnostic::TreeDepthExceeded {
            id: node.id.clone(),
            max_depth: MAX_TREE_DEPTH,
        });
        return;
    }
    if node.type_name == "window" {
        ids.push(node.id.clone());
    }
    for child in &node.children {
        collect_window_ids_recursive(child, ids, depth + 1);
    }
}

/// Maximum number of duplicate IDs collected per validation pass
/// before short-circuiting.
///
/// A pathological tree (legitimate bug or hostile host) can legally
/// contain millions of nodes under the 64 MiB wire cap. Reporting
/// every duplicate burns CPU and memory walking the rest of the
/// tree; one `too_many_duplicate_ids` diagnostic is more useful than
/// a list that no one will read.
const MAX_DUPLICATE_IDS: usize = 100;

/// Walk the tree and check that all node IDs are unique.
///
/// Returns `Ok(())` if no duplicates are found, or `Err` with a list of
/// `"id (type_name)"` strings for each ID that appears more than once.
/// Empty IDs are skipped (some internal nodes may not have meaningful IDs).
///
/// The traversal short-circuits after [`MAX_DUPLICATE_IDS`] duplicates
/// have been collected; a single summary entry (`too_many_duplicates`)
/// is appended so the caller knows the list is capped.
fn validate_unique_ids(root: &TreeNode) -> Result<(), Vec<String>> {
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    let mut summary_emitted = false;
    collect_duplicate_ids(root, &mut seen, &mut duplicates, &mut summary_emitted, 0);
    if duplicates.is_empty() {
        Ok(())
    } else {
        Err(duplicates)
    }
}

fn collect_duplicate_ids(
    node: &TreeNode,
    seen: &mut HashSet<String>,
    duplicates: &mut Vec<String>,
    summary_emitted: &mut bool,
    depth: usize,
) {
    if depth > MAX_TREE_DEPTH {
        return;
    }
    if duplicates.len() >= MAX_DUPLICATE_IDS {
        if !*summary_emitted {
            let diag = plushie_core::Diagnostic::TooManyDuplicates {
                limit: MAX_DUPLICATE_IDS,
            };
            duplicates.push(diag.to_string());
            *summary_emitted = true;
        }
        return;
    }
    if !node.id.is_empty() && !seen.insert(node.id.clone()) {
        duplicates.push(format!("{} ({})", node.id, node.type_name));
    }
    for child in &node.children {
        collect_duplicate_ids(child, seen, duplicates, summary_emitted, depth + 1);
    }
}

/// Navigate to a node at the given path of child indices.
fn navigate<'a>(root: &'a TreeNode, path: &[usize]) -> Result<&'a TreeNode, String> {
    let mut current = root;
    for &idx in path {
        if idx < current.children.len() {
            current = &current.children[idx];
        } else {
            return Err(format!(
                "path navigation: index {idx} out of bounds (len={})",
                current.children.len()
            ));
        }
    }
    Ok(current)
}

fn navigate_mut<'a>(root: &'a mut TreeNode, path: &[usize]) -> Result<&'a mut TreeNode, String> {
    let mut current = root;
    for &idx in path {
        if idx < current.children.len() {
            current = &mut current.children[idx];
        } else {
            return Err(format!(
                "path navigation: index {idx} out of bounds (len={})",
                current.children.len()
            ));
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie_core::protocol::PatchOp;
    use plushie_widget_sdk::testing::{node, node_with_children, node_with_props};
    use serde_json::json;

    fn make_patch_op(op: &str, path: Vec<usize>, rest: serde_json::Value) -> PatchOp {
        // Deserialize from JSON to get proper PatchOp with flattened rest
        let mut obj = serde_json::Map::new();
        obj.insert("op".to_string(), json!(op));
        obj.insert("path".to_string(), json!(path));
        if let Some(map) = rest.as_object() {
            for (k, v) in map {
                obj.insert(k.clone(), v.clone());
            }
        }
        serde_json::from_value(serde_json::Value::Object(obj)).unwrap()
    }

    fn text_node_json(id: &str) -> serde_json::Value {
        json!({"id": id, "type": "text", "props": {}, "children": []})
    }

    // -----------------------------------------------------------------------
    // Tree basics
    // -----------------------------------------------------------------------

    #[test]
    fn new_tree_is_empty() {
        let tree = Tree::new();
        assert!(tree.root().is_none());
    }

    #[test]
    fn default_tree_is_empty() {
        let tree = Tree::default();
        assert!(tree.root().is_none());
    }

    #[test]
    fn snapshot_sets_root() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        assert!(tree.root().is_some());
        assert_eq!(tree.root().unwrap().id, "root");
        assert_eq!(tree.root().unwrap().type_name, "column");
    }

    #[test]
    fn snapshot_replaces_previous_root() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("first", "column"));
        let _ = tree.snapshot(node("second", "row"));
        assert_eq!(tree.root().unwrap().id, "second");
        assert_eq!(tree.root().unwrap().type_name, "row");
    }

    #[test]
    fn snapshot_preserves_children() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node("a", "text"), node("b", "button")],
        );
        let _ = tree.snapshot(root);
        assert_eq!(tree.root().unwrap().children.len(), 2);
        assert_eq!(tree.root().unwrap().children[0].id, "a");
        assert_eq!(tree.root().unwrap().children[1].id, "b");
    }

    // -----------------------------------------------------------------------
    // find_window
    // -----------------------------------------------------------------------

    #[test]
    fn find_window_at_root() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("main", "window"));
        let found = tree.find_window("main");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "main");
        assert_eq!(found.unwrap().type_name, "window");
    }

    #[test]
    fn find_window_root_wrong_id() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("main", "window"));
        assert!(tree.find_window("other").is_none());
    }

    #[test]
    fn find_window_in_children() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node("win1", "window"), node("win2", "window")],
        );
        let _ = tree.snapshot(root);
        assert!(tree.find_window("win1").is_some());
        assert!(tree.find_window("win2").is_some());
        assert_eq!(tree.find_window("win1").unwrap().id, "win1");
    }

    #[test]
    fn find_window_not_found() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        assert!(tree.find_window("nope").is_none());
    }

    #[test]
    fn find_window_on_empty_tree() {
        let tree = Tree::new();
        assert!(tree.find_window("anything").is_none());
    }

    #[test]
    fn find_window_ignores_non_window_children() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![
                node("btn", "button"),
                node("win", "window"),
                node("txt", "text"),
            ],
        );
        let _ = tree.snapshot(root);
        assert!(tree.find_window("btn").is_none());
        assert!(tree.find_window("txt").is_none());
        assert!(tree.find_window("win").is_some());
    }

    #[test]
    fn find_window_searches_grandchildren() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node_with_children(
                "inner",
                "row",
                vec![node("deep_win", "window")],
            )],
        );
        let _ = tree.snapshot(root);
        let found = tree.find_window("deep_win");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "deep_win");
    }

    #[test]
    fn find_window_deeply_nested() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node_with_children(
                "l1",
                "row",
                vec![node_with_children(
                    "l2",
                    "column",
                    vec![node_with_children(
                        "l3",
                        "row",
                        vec![node("buried_win", "window")],
                    )],
                )],
            )],
        );
        let _ = tree.snapshot(root);
        let found = tree.find_window("buried_win");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "buried_win");
    }

    #[test]
    fn window_ids_finds_nested_windows() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![
                node("w1", "window"),
                node_with_children("inner", "row", vec![node("w2", "window")]),
            ],
        );
        let _ = tree.snapshot(root);
        let ids = tree.window_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"w1".to_string()));
        assert!(ids.contains(&"w2".to_string()));
    }

    // -----------------------------------------------------------------------
    // window_ids
    // -----------------------------------------------------------------------

    #[test]
    fn window_ids_when_root_is_window() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("main", "window"));
        let ids = tree.window_ids();
        assert_eq!(ids, vec!["main".to_string()]);
    }

    #[test]
    fn window_ids_collects_child_windows() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![
                node("w1", "window"),
                node("w2", "window"),
                node("w3", "window"),
            ],
        );
        let _ = tree.snapshot(root);
        let ids = tree.window_ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"w1".to_string()));
        assert!(ids.contains(&"w2".to_string()));
        assert!(ids.contains(&"w3".to_string()));
    }

    #[test]
    fn window_ids_skips_non_windows() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![
                node("w1", "window"),
                node("btn", "button"),
                node("w2", "window"),
            ],
        );
        let _ = tree.snapshot(root);
        let ids = tree.window_ids();
        assert_eq!(ids.len(), 2);
        assert!(!ids.contains(&"btn".to_string()));
    }

    #[test]
    fn window_ids_empty_when_no_windows() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        assert!(tree.window_ids().is_empty());
    }

    #[test]
    fn window_ids_empty_on_empty_tree() {
        let tree = Tree::new();
        assert!(tree.window_ids().is_empty());
    }

    // -----------------------------------------------------------------------
    // apply_patch: replace_node
    // -----------------------------------------------------------------------

    #[test]
    fn patch_replace_root() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("old", "column"));
        let op = make_patch_op(
            "replace_node",
            vec![],
            json!({
                "node": {"id": "new", "type": "row", "props": {}, "children": []}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().id, "new");
        assert_eq!(tree.root().unwrap().type_name, "row");
    }

    #[test]
    fn patch_replace_child() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node("a", "text"), node("b", "button")],
        );
        let _ = tree.snapshot(root);
        let op = make_patch_op(
            "replace_node",
            vec![1],
            json!({
                "node": {"id": "c", "type": "text", "props": {"content": "replaced"}, "children": []}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().children[1].id, "c");
        assert_eq!(
            tree.root().unwrap().children[1].props.to_value()["content"],
            "replaced"
        );
    }

    #[test]
    fn patch_replace_nested_child() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node_with_children(
                "row",
                "row",
                vec![node("inner", "text")],
            )],
        );
        let _ = tree.snapshot(root);
        let op = make_patch_op(
            "replace_node",
            vec![0, 0],
            json!({
                "node": {"id": "replaced", "type": "button", "props": {}, "children": []}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().children[0].children[0].id, "replaced");
        assert_eq!(
            tree.root().unwrap().children[0].children[0].type_name,
            "button"
        );
    }

    #[test]
    fn patch_replace_out_of_bounds_does_not_panic() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        let op = make_patch_op(
            "replace_node",
            vec![5],
            json!({
                "node": {"id": "x", "type": "text", "props": {}, "children": []}
            }),
        );
        // Should report the malformed op but not panic.
        tree.apply_patch(vec![op]);
        // Root is unchanged
        assert_eq!(tree.root().unwrap().id, "root");
    }

    // -----------------------------------------------------------------------
    // apply_patch: update_props
    // -----------------------------------------------------------------------

    #[test]
    fn patch_update_props_on_root() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node_with_props("root", "column", json!({"spacing": 5})));
        let op = make_patch_op(
            "update_props",
            vec![],
            json!({
                "props": {"spacing": 10, "padding": 20}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().props.to_value()["spacing"], 10);
        assert_eq!(tree.root().unwrap().props.to_value()["padding"], 20);
    }

    #[test]
    fn patch_update_props_removes_null_keys() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node_with_props(
            "root",
            "text",
            json!({"content": "hi", "size": 14}),
        ));
        let op = make_patch_op(
            "update_props",
            vec![],
            json!({
                "props": {"size": null}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().props.to_value()["content"], "hi");
        assert!(tree.root().unwrap().props.get("size").is_none());
    }

    #[test]
    fn patch_update_props_on_child() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node_with_props("txt", "text", json!({"content": "old"}))],
        );
        let _ = tree.snapshot(root);
        let op = make_patch_op(
            "update_props",
            vec![0],
            json!({
                "props": {"content": "new"}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(
            tree.root().unwrap().children[0].props.to_value()["content"],
            "new"
        );
    }

    #[test]
    fn patch_overdeep_path_is_skipped_before_navigation() {
        let overdeep_path = vec![0; MAX_TREE_DEPTH + 1];
        let mut root = node_with_props("target", "text", json!({"content": "old"}));
        for i in 0..overdeep_path.len() {
            root = node_with_children(&format!("n{i}"), "column", vec![root]);
        }

        let mut tree = Tree::new();
        let _ = tree.snapshot(root);
        let overdeep = make_patch_op(
            "update_props",
            overdeep_path.clone(),
            json!({
                "props": {"content": "new"}
            }),
        );
        let valid = make_patch_op(
            "update_props",
            vec![],
            json!({
                "props": {"checked": true}
            }),
        );

        tree.apply_patch(vec![overdeep, valid]);

        let target = navigate(tree.root().unwrap(), &overdeep_path).unwrap();
        assert_eq!(target.props.to_value()["content"], "old");
        assert_eq!(tree.root().unwrap().props.to_value()["checked"], true);
    }

    #[test]
    fn patch_update_props_non_object_target_props_does_not_panic() {
        let mut tree = Tree::new();
        // A non-object props value collapses to an empty map on construction;
        // the merge then proceeds normally, inserting the patch keys.
        let _ = tree.snapshot(node_with_props("root", "text", json!("not an object")));
        let op = make_patch_op(
            "update_props",
            vec![],
            json!({
                "props": {"content": "new"}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().props.get_str("content"), Some("new"));
    }

    #[test]
    fn patch_update_props_non_object_patch_props_does_not_panic() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node_with_props("root", "text", json!({"content": "hi"})));
        // Patch props is a string, not an object
        let op = make_patch_op(
            "update_props",
            vec![],
            json!({
                "props": "not an object"
            }),
        );
        tree.apply_patch(vec![op]);
        // Props unchanged: the merge was skipped
        assert_eq!(tree.root().unwrap().props.to_value()["content"], "hi");
    }

    #[test]
    fn patch_update_props_non_object_patch_props_is_reported_as_apply_error() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node_with_props("root", "text", json!({"content": "hi"})));
        let op = make_patch_op("update_props", vec![], json!({"props": false}));

        let error = tree.apply_op(&op).unwrap_err();

        assert!(
            matches!(error, PatchApplyError::Invalid(message) if message.contains(
                "patch props is not an object"
            ))
        );
        assert_eq!(tree.root().unwrap().props.to_value()["content"], "hi");
    }

    // -----------------------------------------------------------------------
    // apply_patch: insert_child
    // -----------------------------------------------------------------------

    #[test]
    fn patch_insert_child_at_beginning() {
        let mut tree = Tree::new();
        let root = node_with_children("root", "column", vec![node("a", "text")]);
        let _ = tree.snapshot(root);
        let op = make_patch_op(
            "insert_child",
            vec![],
            json!({
                "index": 0,
                "node": {"id": "b", "type": "button", "props": {}, "children": []}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().children.len(), 2);
        assert_eq!(tree.root().unwrap().children[0].id, "b");
        assert_eq!(tree.root().unwrap().children[1].id, "a");
    }

    #[test]
    fn patch_insert_child_at_end() {
        let mut tree = Tree::new();
        let root = node_with_children("root", "column", vec![node("a", "text")]);
        let _ = tree.snapshot(root);
        let op = make_patch_op(
            "insert_child",
            vec![],
            json!({
                "index": 1,
                "node": {"id": "b", "type": "button", "props": {}, "children": []}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().children.len(), 2);
        assert_eq!(tree.root().unwrap().children[1].id, "b");
    }

    #[test]
    fn patch_insert_child_beyond_length_appends() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        let op = make_patch_op(
            "insert_child",
            vec![],
            json!({
                "index": 99,
                "node": {"id": "x", "type": "text", "props": {}, "children": []}
            }),
        );
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().children.len(), 1);
        assert_eq!(tree.root().unwrap().children[0].id, "x");
    }

    #[test]
    fn patch_insert_child_into_nested_parent() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node_with_children(
                "row",
                "row",
                vec![node("existing", "text")],
            )],
        );
        let _ = tree.snapshot(root);
        let op = make_patch_op(
            "insert_child",
            vec![0],
            json!({
                "index": 0,
                "node": {"id": "new", "type": "button", "props": {}, "children": []}
            }),
        );
        tree.apply_patch(vec![op]);
        let row = &tree.root().unwrap().children[0];
        assert_eq!(row.children.len(), 2);
        assert_eq!(row.children[0].id, "new");
        assert_eq!(row.children[1].id, "existing");
    }

    // -----------------------------------------------------------------------
    // apply_patch: remove_child
    // -----------------------------------------------------------------------

    #[test]
    fn patch_remove_child() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node("a", "text"), node("b", "button"), node("c", "text")],
        );
        let _ = tree.snapshot(root);
        let op = make_patch_op("remove_child", vec![], json!({"index": 1}));
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().children.len(), 2);
        assert_eq!(tree.root().unwrap().children[0].id, "a");
        assert_eq!(tree.root().unwrap().children[1].id, "c");
    }

    #[test]
    fn patch_remove_child_first() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node("a", "text"), node("b", "button")],
        );
        let _ = tree.snapshot(root);
        let op = make_patch_op("remove_child", vec![], json!({"index": 0}));
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().children.len(), 1);
        assert_eq!(tree.root().unwrap().children[0].id, "b");
    }

    #[test]
    fn patch_remove_child_last() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node("a", "text"), node("b", "button")],
        );
        let _ = tree.snapshot(root);
        let op = make_patch_op("remove_child", vec![], json!({"index": 1}));
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().children.len(), 1);
        assert_eq!(tree.root().unwrap().children[0].id, "a");
    }

    #[test]
    fn patch_remove_child_out_of_bounds_does_not_panic() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        let op = make_patch_op("remove_child", vec![], json!({"index": 0}));
        // Should report the malformed op but not panic.
        tree.apply_patch(vec![op]);
        assert!(tree.root().unwrap().children.is_empty());
    }

    // -----------------------------------------------------------------------
    // apply_patch: unknown op
    // -----------------------------------------------------------------------

    #[test]
    fn patch_unknown_op_does_not_panic() {
        let mut tree = Tree::new();
        let mut root = node("root", "column");
        root.children.push(node("existing", "text"));
        let _ = tree.snapshot(root);
        let unknown = make_patch_op("frobnicate", vec![], json!({}));
        let valid = make_patch_op(
            "insert_child",
            vec![],
            json!({
                "index": 1,
                "node": node("child", "text")
            }),
        );

        tree.apply_patch(vec![unknown, valid]);

        assert_eq!(tree.root().unwrap().id, "root");
        assert_eq!(tree.root().unwrap().children.len(), 2);
        assert_eq!(tree.root().unwrap().children[0].id, "existing");
        assert_eq!(tree.root().unwrap().children[1].id, "child");
    }

    #[test]
    fn unknown_patch_payload_preserves_flattened_fields() {
        let unknown = make_patch_op(
            "frobnicate",
            vec![1, 2],
            json!({
                "index": 3,
                "extra": {"answer": 42}
            }),
        );

        let payload = patch_op_payload(&unknown);

        assert_eq!(payload["op"], "frobnicate");
        assert_eq!(payload["path"], json!([1, 2]));
        assert_eq!(payload["index"], 3);
        assert_eq!(payload["extra"], json!({"answer": 42}));
    }

    // -----------------------------------------------------------------------
    // apply_patch: multiple ops in sequence
    // -----------------------------------------------------------------------

    #[test]
    fn patch_multiple_ops_applied_in_order() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));

        let ops = vec![
            make_patch_op(
                "insert_child",
                vec![],
                json!({
                    "index": 0,
                    "node": {"id": "a", "type": "text", "props": {}, "children": []}
                }),
            ),
            make_patch_op(
                "insert_child",
                vec![],
                json!({
                    "index": 1,
                    "node": {"id": "b", "type": "text", "props": {}, "children": []}
                }),
            ),
            make_patch_op(
                "insert_child",
                vec![],
                json!({
                    "index": 1,
                    "node": {"id": "c", "type": "text", "props": {}, "children": []}
                }),
            ),
        ];
        tree.apply_patch(ops);
        let children = &tree.root().unwrap().children;
        assert_eq!(children.len(), 3);
        assert_eq!(children[0].id, "a");
        assert_eq!(children[1].id, "c");
        assert_eq!(children[2].id, "b");
    }

    // -----------------------------------------------------------------------
    // apply_patch on empty tree
    // -----------------------------------------------------------------------

    #[test]
    fn patch_on_empty_tree_does_not_panic() {
        let mut tree = Tree::new();
        let op = make_patch_op(
            "replace_node",
            vec![],
            json!({
                "node": {"id": "x", "type": "text", "props": {}, "children": []}
            }),
        );
        tree.apply_patch(vec![op]);
        // Still empty: the op should fail gracefully
        assert!(tree.root().is_none());
    }

    // -----------------------------------------------------------------------
    // navigate_mut edge cases (tested indirectly through patch ops)
    // -----------------------------------------------------------------------

    #[test]
    fn patch_deep_path_navigation() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node_with_children(
                "r0",
                "row",
                vec![node_with_children(
                    "r0c0",
                    "column",
                    vec![node("deep", "text")],
                )],
            )],
        );
        let _ = tree.snapshot(root);
        let op = make_patch_op(
            "update_props",
            vec![0, 0, 0],
            json!({
                "props": {"content": "updated deep"}
            }),
        );
        tree.apply_patch(vec![op]);
        let deep = &tree.root().unwrap().children[0].children[0].children[0];
        assert_eq!(deep.props.to_value()["content"], "updated deep");
    }

    #[test]
    fn patch_invalid_path_does_not_panic() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        let op = make_patch_op(
            "update_props",
            vec![0, 1, 2],
            json!({
                "props": {"x": 1}
            }),
        );
        tree.apply_patch(vec![op]);
        // Root unchanged
        assert_eq!(tree.root().unwrap().id, "root");
    }

    // -----------------------------------------------------------------------
    // Malformed patch operations (error paths)
    // -----------------------------------------------------------------------

    #[test]
    fn patch_replace_node_missing_node_field_does_not_panic() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        // replace_node without the required "node" field
        let op = make_patch_op("replace_node", vec![], json!({}));
        tree.apply_patch(vec![op]);
        // Tree should be unchanged
        assert_eq!(tree.root().unwrap().id, "root");
    }

    #[test]
    fn patch_replace_node_invalid_node_json_does_not_panic() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        // "node" is present but not a valid TreeNode (missing required fields)
        let op = make_patch_op("replace_node", vec![], json!({"node": {"garbage": true}}));
        tree.apply_patch(vec![op]);
        assert_eq!(tree.root().unwrap().id, "root");
    }

    #[test]
    fn patch_update_props_missing_props_field_does_not_panic() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node_with_props("root", "text", json!({"content": "hi"})));
        let op = make_patch_op("update_props", vec![], json!({}));
        tree.apply_patch(vec![op]);
        // Props unchanged: the missing "props" field is handled gracefully
        assert_eq!(tree.root().unwrap().props.to_value()["content"], "hi");
    }

    #[test]
    fn patch_insert_child_missing_index_does_not_panic() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        let op = make_patch_op(
            "insert_child",
            vec![],
            json!({
                "node": {"id": "x", "type": "text", "props": {}, "children": []}
            }),
        );
        tree.apply_patch(vec![op]);
        // No child inserted because index is missing
        assert!(tree.root().unwrap().children.is_empty());
    }

    #[test]
    fn patch_insert_child_missing_node_does_not_panic() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        let op = make_patch_op("insert_child", vec![], json!({"index": 0}));
        tree.apply_patch(vec![op]);
        assert!(tree.root().unwrap().children.is_empty());
    }

    #[test]
    fn patch_remove_child_missing_index_does_not_panic() {
        let mut tree = Tree::new();
        let root = node_with_children("root", "column", vec![node("a", "text")]);
        let _ = tree.snapshot(root);
        let op = make_patch_op("remove_child", vec![], json!({}));
        tree.apply_patch(vec![op]);
        // Child should still be present: the op failed gracefully
        assert_eq!(tree.root().unwrap().children.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Multi-op patch tests
    // -----------------------------------------------------------------------

    #[test]
    fn patch_multi_op_mixed_types() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![
                node_with_props("a", "text", json!({"content": "hello"})),
                node("b", "button"),
            ],
        );
        let _ = tree.snapshot(root);

        let ops = vec![
            // Insert a third child at index 2
            make_patch_op(
                "insert_child",
                vec![],
                json!({
                    "index": 2,
                    "node": {"id": "c", "type": "text", "props": {"content": "new"}, "children": []}
                }),
            ),
            // Remove the first child (index 0 = "a")
            make_patch_op("remove_child", vec![], json!({"index": 0})),
            // Update props on current index 0 (was "b", now shifted to front)
            make_patch_op(
                "update_props",
                vec![0],
                json!({"props": {"label": "updated"}}),
            ),
        ];
        tree.apply_patch(ops);

        let children = &tree.root().unwrap().children;
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].id, "b");
        assert_eq!(children[0].props.to_value()["label"], "updated");
        assert_eq!(children[1].id, "c");
    }

    #[test]
    fn patch_remove_shifts_indices() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![
                node("first", "text"),
                node("second", "button"),
                node("third", "text"),
            ],
        );
        let _ = tree.snapshot(root);

        let ops = vec![
            // Remove child at index 0 ("first")
            make_patch_op("remove_child", vec![], json!({"index": 0})),
            // Replace node at index 0, which is now "second" after removal
            make_patch_op(
                "replace_node",
                vec![0],
                json!({
                    "node": {"id": "replaced", "type": "row", "props": {}, "children": []}
                }),
            ),
        ];
        tree.apply_patch(ops);

        let children = &tree.root().unwrap().children;
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].id, "replaced");
        assert_eq!(children[0].type_name, "row");
        assert_eq!(children[1].id, "third");
    }

    #[test]
    fn patch_bad_middle_op_continues() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![
                node_with_props("a", "text", json!({"content": "original"})),
                node("b", "button"),
            ],
        );
        let _ = tree.snapshot(root);

        let ops = vec![
            // First op: update props on child "a" (valid)
            make_patch_op(
                "update_props",
                vec![0],
                json!({"props": {"content": "changed"}}),
            ),
            // Second op: invalid path (out of bounds)
            make_patch_op(
                "update_props",
                vec![99, 0],
                json!({"props": {"content": "nope"}}),
            ),
            // Third op: update props on child "b" (valid)
            make_patch_op(
                "update_props",
                vec![1],
                json!({"props": {"label": "click me"}}),
            ),
        ];
        tree.apply_patch(ops);

        let children = &tree.root().unwrap().children;
        assert_eq!(children[0].props.to_value()["content"], "changed");
        assert_eq!(children[1].props.to_value()["label"], "click me");
    }

    // -----------------------------------------------------------------------
    // validate_patch_order
    // -----------------------------------------------------------------------

    #[test]
    fn validate_patch_order_accepts_empty_ops() {
        assert!(Tree::validate_patch_order(&[]).is_ok());
    }

    #[test]
    fn validate_patch_order_accepts_single_remove() {
        let ops = vec![make_patch_op("remove_child", vec![], json!({"index": 0}))];

        assert!(Tree::validate_patch_order(&ops).is_ok());
    }

    #[test]
    fn validate_patch_order_accepts_remove_middle_insert_boundary() {
        let ops = vec![
            make_patch_op("remove_child", vec![], json!({"index": 1})),
            make_patch_op(
                "update_props",
                vec![0],
                json!({"props": {"content": "changed"}}),
            ),
            make_patch_op(
                "insert_child",
                vec![],
                json!({"index": 0, "node": text_node_json("new")}),
            ),
        ];

        assert!(Tree::validate_patch_order(&ops).is_ok());
    }

    #[test]
    fn validate_patch_order_rejects_duplicate_remove_index() {
        let ops = vec![
            make_patch_op("remove_child", vec![], json!({"index": 1})),
            make_patch_op("remove_child", vec![], json!({"index": 1})),
        ];

        let error = Tree::validate_patch_order(&ops).unwrap_err();

        assert!(error.contains("strictly decrease child indices"));
    }

    // -----------------------------------------------------------------------
    // validate_unique_ids / snapshot duplicate detection
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_unique_ids_returns_ok() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node("a", "text"), node("b", "button")],
        );
        assert!(tree.snapshot(root).is_ok());
    }

    #[test]
    fn snapshot_duplicate_ids_returns_err() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node("dupe", "text"), node("dupe", "button")],
        );
        let result = tree.snapshot(root);
        assert!(result.is_err());
        let dupes = result.unwrap_err();
        assert_eq!(dupes.len(), 1);
        assert!(dupes[0].contains("dupe"));
    }

    #[test]
    fn snapshot_duplicate_ids_still_accepts_tree() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node("dupe", "text"), node("dupe", "button")],
        );
        let _ = tree.snapshot(root);
        // Tree was still accepted despite duplicates
        assert!(tree.root().is_some());
        assert_eq!(tree.root().unwrap().children.len(), 2);
    }

    #[test]
    fn snapshot_multiple_duplicate_ids() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![
                node("a", "text"),
                node("a", "text"),
                node("b", "button"),
                node("b", "button"),
            ],
        );
        let result = tree.snapshot(root);
        assert!(result.is_err());
        let dupes = result.unwrap_err();
        assert_eq!(dupes.len(), 2);
    }

    #[test]
    fn snapshot_empty_ids_are_ignored() {
        let mut tree = Tree::new();
        let root = node_with_children("root", "column", vec![node("", "text"), node("", "text")]);
        // Empty IDs should not be flagged as duplicates
        assert!(tree.snapshot(root).is_ok());
    }

    #[test]
    fn duplicate_collection_short_circuits_past_cap() {
        // Build a flat tree with MAX_DUPLICATE_IDS + 50 duplicated IDs.
        // The caller must still see an error, but the list is capped
        // and a summary entry tells downstream code the list was cut
        // short.
        let over = super::MAX_DUPLICATE_IDS + 50;
        let mut children = Vec::with_capacity(over * 2);
        for _ in 0..over {
            children.push(node("shared", "text"));
            children.push(node("shared", "text"));
        }
        let root = node_with_children("root", "column", children);
        let mut tree = Tree::new();
        let dupes = tree.snapshot(root).unwrap_err();

        // List should be capped at MAX_DUPLICATE_IDS duplicate entries
        // plus the one summary entry.
        assert_eq!(dupes.len(), super::MAX_DUPLICATE_IDS + 1);
        assert!(
            dupes
                .last()
                .is_some_and(|s| s.contains("too_many_duplicates")),
            "expected summary entry, got {:?}",
            dupes.last()
        );
    }

    #[test]
    fn find_window_returns_none_beyond_max_depth() {
        // Build a chain deeper than MAX_TREE_DEPTH (256).
        let mut deepest = node("deep_win", "window");
        for i in 0..MAX_TREE_DEPTH + 10 {
            deepest = node_with_children(&format!("n{i}"), "column", vec![deepest]);
        }
        let mut tree = Tree::new();
        let _ = tree.snapshot(deepest);

        // The window at depth > 256 should not be found.
        assert!(
            tree.find_window("deep_win").is_none(),
            "window beyond MAX_TREE_DEPTH should not be reachable"
        );
    }

    #[test]
    fn window_ids_skips_windows_beyond_max_depth() {
        let mut deepest = node("deep_win", "window");
        for i in 0..MAX_TREE_DEPTH + 10 {
            deepest = node_with_children(&format!("n{i}"), "column", vec![deepest]);
        }
        let mut tree = Tree::new();
        let _ = tree.snapshot(deepest);

        let ids = tree.window_ids();
        assert!(
            !ids.contains(&"deep_win".to_string()),
            "window beyond MAX_TREE_DEPTH should not appear in window_ids"
        );
    }

    // -----------------------------------------------------------------------
    // id_index: find_by_id across snapshot + patch
    // -----------------------------------------------------------------------

    #[test]
    fn find_by_id_finds_root() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        let found = tree.find_by_id("root").unwrap();
        assert_eq!(found.id, "root");
    }

    #[test]
    fn find_by_id_finds_nested_descendant() {
        let mut tree = Tree::new();
        let root = node_with_children(
            "root",
            "column",
            vec![node_with_children("row", "row", vec![node("deep", "text")])],
        );
        let _ = tree.snapshot(root);
        let found = tree.find_by_id("deep").unwrap();
        assert_eq!(found.id, "deep");
        assert_eq!(found.type_name, "text");
    }

    #[test]
    fn find_by_id_returns_none_for_missing() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        assert!(tree.find_by_id("ghost").is_none());
    }

    #[test]
    fn find_by_id_skips_empty_ids() {
        let mut tree = Tree::new();
        let root = node_with_children("root", "column", vec![node("", "text"), node("a", "text")]);
        let _ = tree.snapshot(root);
        // Empty ID is not indexed, but a real one is reachable.
        assert!(tree.find_by_id("").is_none());
        assert!(tree.find_by_id("a").is_some());
    }

    #[test]
    fn find_by_id_first_match_wins_under_duplicates() {
        let mut tree = Tree::new();
        // Two nodes share an ID: the first one (depth-first) is the
        // visible one through the index. The validator will report
        // the duplicate, but the lookup must remain stable.
        let root = node_with_children(
            "root",
            "column",
            vec![
                node_with_props("dupe", "text", json!({"content": "first"})),
                node_with_props("dupe", "button", json!({"content": "second"})),
            ],
        );
        let _ = tree.snapshot(root);
        let found = tree.find_by_id("dupe").unwrap();
        assert_eq!(found.type_name, "text");
        assert_eq!(found.props.to_value()["content"], "first");
    }

    #[test]
    fn id_index_reflects_inserts_and_removes() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        assert!(tree.find_by_id("root").is_some());

        // Insert "a" at index 0; insert_child shifts no existing
        // index because parent is empty.
        tree.apply_patch(vec![make_patch_op(
            "insert_child",
            vec![],
            json!({
                "index": 0,
                "node": {"id": "a", "type": "text", "props": {}, "children": []}
            }),
        )]);
        assert_eq!(tree.find_by_id("a").unwrap().id, "a");

        // Insert "b" at index 0, pushing "a" to index 1. The index
        // entry for "a" must follow the shift.
        tree.apply_patch(vec![make_patch_op(
            "insert_child",
            vec![],
            json!({
                "index": 0,
                "node": {"id": "b", "type": "text", "props": {}, "children": []}
            }),
        )]);
        assert_eq!(tree.find_by_id("a").unwrap().id, "a");
        assert_eq!(tree.find_by_id("b").unwrap().id, "b");

        // Update a prop on "a" (now at child index 1) and confirm
        // the index lookup still reaches the right node.
        tree.apply_patch(vec![make_patch_op(
            "update_props",
            vec![1],
            json!({"props": {"content": "updated"}}),
        )]);
        assert_eq!(
            tree.find_by_id("a").unwrap().props.to_value()["content"],
            "updated"
        );

        // Remove "b" at index 0; "a" shifts back to index 0. The
        // index for "b" disappears; "a" still resolves.
        tree.apply_patch(vec![make_patch_op(
            "remove_child",
            vec![],
            json!({"index": 0}),
        )]);
        assert!(tree.find_by_id("b").is_none());
        let a = tree.find_by_id("a").unwrap();
        assert_eq!(a.id, "a");
        assert_eq!(a.props.to_value()["content"], "updated");

        // Replace "a" with "c"; the old ID drops from the index,
        // the new one appears.
        tree.apply_patch(vec![make_patch_op(
            "replace_node",
            vec![0],
            json!({
                "node": {"id": "c", "type": "button", "props": {}, "children": []}
            }),
        )]);
        assert!(tree.find_by_id("a").is_none());
        assert_eq!(tree.find_by_id("c").unwrap().type_name, "button");
    }

    #[test]
    fn id_index_indexes_descendants_added_via_replace() {
        let mut tree = Tree::new();
        let _ = tree.snapshot(node("root", "column"));
        // Replace root with a richer subtree; every nested ID
        // should be reachable through find_by_id afterwards.
        tree.apply_patch(vec![make_patch_op(
            "replace_node",
            vec![],
            json!({
                "node": {
                    "id": "new_root",
                    "type": "column",
                    "props": {},
                    "children": [
                        {"id": "kid_a", "type": "text", "props": {}, "children": []},
                        {
                            "id": "kid_b",
                            "type": "row",
                            "props": {},
                            "children": [
                                {"id": "grand", "type": "text", "props": {}, "children": []}
                            ]
                        }
                    ]
                }
            }),
        )]);
        assert!(tree.find_by_id("root").is_none());
        assert_eq!(tree.find_by_id("new_root").unwrap().type_name, "column");
        assert_eq!(tree.find_by_id("kid_a").unwrap().type_name, "text");
        assert_eq!(tree.find_by_id("kid_b").unwrap().type_name, "row");
        assert_eq!(tree.find_by_id("grand").unwrap().type_name, "text");
    }
}
