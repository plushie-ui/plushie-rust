// Public API used by the wire runner; tests exercise it unconditionally.
#![allow(dead_code)]

//! Tree diffing: produce minimal patch operations between two trees.
//!
//! Walks old and new TreeNode trees simultaneously, emitting replace,
//! update, insert, and remove operations. Children are matched by ID
//! and diffed using a three-path strategy:
//!
//! - **Fast**: identical ID sequences. Diff each pair recursively.
//! - **Medium**: no reordering among common IDs. Pure inserts and
//!   removes, no moves.
//! - **Slow**: reordering detected. Uses longest increasing
//!   subsequence (LIS) to minimize remove+insert operations.
//!
//! PatchOp output values are `serde_json::Value` for wire
//! serialization, but the diff algorithm itself works on typed
//! `TreeNode` structs.

use std::collections::{HashMap, HashSet};

use plushie_core::protocol::TreeNode;
use serde_json::Value;

/// A single patch operation produced by diffing two trees.
///
/// # Application order invariant
///
/// The sequence returned by [`diff_tree`] is ordered so that a naive
/// in-order apply converges to the target tree:
///
/// 1. `RemoveChild` ops within the same parent come in **descending**
///    `index` order, so earlier removes don't shift the indices of
///    later ones.
/// 2. `UpdateProps` ops fire next. Their `path` is relative to the
///    tree shape *after* the removes have been applied.
/// 3. `InsertChild` ops come last, in **ascending** `index` order so
///    each insert's index is the final position in the new tree.
///
/// A consumer that applies ops out of this order (e.g. reorders by
/// depth or batches inserts before removes) will produce a different
/// tree. Cross-SDK renderers implement this contract; the `apply_patch`
/// helper in this module is the canonical reference.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "op")]
pub enum PatchOp {
    /// Replace an entire subtree at the given path.
    #[serde(rename = "replace_node")]
    ReplaceNode {
        /// Child-index path from the root to the node being replaced.
        path: Vec<usize>,
        /// Serialized replacement subtree.
        node: Value,
    },
    /// Update specific props on a node at the given path.
    #[serde(rename = "update_props")]
    UpdateProps {
        /// Child-index path from the root to the node being updated.
        path: Vec<usize>,
        /// Object of prop keys and their new values.
        props: Value,
    },
    /// Insert a child at the given index.
    #[serde(rename = "insert_child")]
    InsertChild {
        /// Child-index path to the parent.
        path: Vec<usize>,
        /// Zero-based index where the new child is inserted.
        index: usize,
        /// Serialized child subtree.
        node: Value,
    },
    /// Remove a child at the given index.
    #[serde(rename = "remove_child")]
    RemoveChild {
        /// Child-index path to the parent.
        path: Vec<usize>,
        /// Zero-based index of the child to remove.
        index: usize,
    },
}

fn node_to_value(node: &TreeNode) -> Value {
    serde_json::to_value(node).expect("TreeNode serialization cannot fail")
}

/// Diff two TreeNode trees and return a list of patch operations.
///
/// Both trees must be fully normalized (same scope rules, same a11y
/// rewrites). Feeding one normalized and one un-normalized tree will
/// land the whole structure in the `ReplaceNode` slow path even when
/// the underlying authored shape was unchanged, because IDs won't
/// line up. Callers that keep a long-lived `current_tree` should
/// rerun [`crate::runtime::normalize::normalize`] whenever a
/// normalization rule changes.
pub fn diff_tree(old: &TreeNode, new: &TreeNode) -> Vec<PatchOp> {
    if old.id != new.id {
        return vec![PatchOp::ReplaceNode {
            path: vec![],
            node: node_to_value(new),
        }];
    }
    diff_node(old, new, &[])
}

/// Apply a patch sequence to a tree in place.
///
/// The inverse of [`diff_tree`]: after
/// `apply_patch(&mut t, &diff_tree(&t_prev, &t_new))`, `t` equals
/// `t_new`. Used by the tree-diff property test to verify that the
/// diff algorithm and the renderer's apply path stay consistent.
///
/// This compatibility wrapper stops at the first invalid op and
/// leaves earlier ops applied. Use [`try_apply_patch`] when invalid
/// patch details should be reported to the caller.
pub fn apply_patch(tree: &mut TreeNode, ops: &[PatchOp]) {
    let _ = try_apply_patch(tree, ops);
}

/// Try to apply a patch sequence to a tree in place.
///
/// # Errors
///
/// Returns an error when an operation points to a node path that does
/// not exist in the current tree shape, or when a patch node cannot be
/// decoded. Operations before the failing op remain applied.
pub fn try_apply_patch(tree: &mut TreeNode, ops: &[PatchOp]) -> Result<(), String> {
    for op in ops {
        apply_one(tree, op)?;
    }
    Ok(())
}

fn apply_one(tree: &mut TreeNode, op: &PatchOp) -> Result<(), String> {
    match op {
        PatchOp::ReplaceNode { path, node } => {
            let new_node: TreeNode = serde_json::from_value(node.clone())
                .map_err(|e| format!("replace_node: invalid node: {e}"))?;
            if path.is_empty() {
                *tree = new_node;
            } else {
                let target = navigate_mut(tree, path)?;
                *target = new_node;
            }
        }
        PatchOp::UpdateProps { path, props } => {
            let target = navigate_mut(tree, path)?;
            // Merge-update: keys in `props` overwrite; keys absent
            // from the patch are preserved. Null values delete the
            // key, matching diff_props's removal encoding.
            let mut target_value = target.props.to_value();
            if let (Some(target_map), Some(patch_map)) =
                (target_value.as_object_mut(), props.as_object())
            {
                for (k, v) in patch_map {
                    if v.is_null() {
                        target_map.remove(k);
                    } else {
                        target_map.insert(k.clone(), v.clone());
                    }
                }
                target.props = plushie_core::protocol::Props::from_json(target_value);
            } else {
                // Fall back to whole-props replacement when either side
                // isn't an object.
                target.props = plushie_core::protocol::Props::from_json(props.clone());
            }
        }
        PatchOp::InsertChild { path, index, node } => {
            let new_node: TreeNode = serde_json::from_value(node.clone())
                .map_err(|e| format!("insert_child: invalid node: {e}"))?;
            let target = navigate_mut(tree, path)?;
            let idx = (*index).min(target.children.len());
            target.children.insert(idx, new_node);
        }
        PatchOp::RemoveChild { path, index } => {
            let target = navigate_mut(tree, path)?;
            if *index < target.children.len() {
                target.children.remove(*index);
            } else {
                return Err(format!(
                    "remove_child: index {} out of bounds for node {:?} with {} children",
                    index,
                    target.id,
                    target.children.len()
                ));
            }
        }
    }
    Ok(())
}

fn navigate_mut<'a>(tree: &'a mut TreeNode, path: &[usize]) -> Result<&'a mut TreeNode, String> {
    let mut cursor = tree;
    for (depth, &i) in path.iter().enumerate() {
        let child_count = cursor.children.len();
        cursor = cursor.children.get_mut(i).ok_or_else(|| {
            format!(
                "invalid patch path {:?}: index {} at depth {} is out of bounds for node {:?} with {} children",
                path, i, depth, cursor.id, child_count
            )
        })?;
    }
    Ok(cursor)
}

/// Recursively diff two nodes at the given path.
fn diff_node(old: &TreeNode, new: &TreeNode, path: &[usize]) -> Vec<PatchOp> {
    if old == new {
        return vec![];
    }

    if old.type_name != new.type_name {
        return vec![PatchOp::ReplaceNode {
            path: path.to_vec(),
            node: node_to_value(new),
        }];
    }

    // Wire-canonical prop equality (numeric variants compare equal,
    // null entries treated as absent) short-circuits before the JSON
    // round-trip below. Without this, `42 == 42.0` paths produce a
    // spurious `update_props` op every render after a numeric
    // representation change (e.g. animation interpolation).
    let prop_ops = if old.props == new.props {
        Vec::new()
    } else {
        let old_props_val = old.props.to_value();
        let new_props_val = new.props.to_value();
        diff_props(&old_props_val, &new_props_val, path)
    };
    let child_ops = diff_children(&old.children, &new.children, path);

    let mut ops = prop_ops;
    ops.extend(child_ops);
    ops
}

/// Diff two props objects. Returns at most one UpdateProps op.
///
/// Null-valued entries in either map are treated as absent, matching
/// the wire protocol contract (`update_props` uses `null` to signal
/// key removal). This keeps diff/apply self-consistent: the patch
/// format cannot represent "add an explicit null-valued key," so the
/// diff never produces one.
fn diff_props(old_props: &Value, new_props: &Value, path: &[usize]) -> Vec<PatchOp> {
    if old_props == new_props {
        return vec![];
    }

    let old_map = old_props.as_object();
    let new_map = new_props.as_object();

    let (old_map, new_map) = match (old_map, new_map) {
        (Some(o), Some(n)) => (o, n),
        _ => {
            // One or both aren't objects; if they differ, replace.
            if old_props != new_props {
                return vec![PatchOp::UpdateProps {
                    path: path.to_vec(),
                    props: new_props.clone(),
                }];
            }
            return vec![];
        }
    };

    let mut changed = serde_json::Map::new();

    // Find added and changed keys. A null value on the new side is
    // equivalent to "key absent" (wire protocol contract), so we emit
    // a null only when we need to clear a previously-set value.
    for (k, new_v) in new_map {
        match (old_map.get(k), new_v.is_null()) {
            (Some(old_v), _) if old_v == new_v => {}
            (Some(old_v), false) => {
                // Check if both are ID-keyed lists that are semantically equal.
                if !id_keyed_list_equal(old_v, new_v) {
                    changed.insert(k.clone(), new_v.clone());
                }
            }
            (Some(old_v), true) => {
                // new is null, old is not. Emit null to clear the key.
                // (Skip if old was also null: caught by the equal arm above
                // when both are null; otherwise fall through to emit null.)
                if !old_v.is_null() {
                    changed.insert(k.clone(), Value::Null);
                }
            }
            (None, false) => {
                changed.insert(k.clone(), new_v.clone());
            }
            (None, true) => {
                // new is null, old is absent: both encode "no value." Skip.
            }
        }
    }

    // Find removed keys (in old but not in new) and set to null. Skip
    // old entries that were already null (nothing to clear).
    for (k, old_v) in old_map {
        if !new_map.contains_key(k) && !old_v.is_null() {
            changed.insert(k.clone(), Value::Null);
        }
    }

    if changed.is_empty() {
        vec![]
    } else {
        vec![PatchOp::UpdateProps {
            path: path.to_vec(),
            props: Value::Object(changed),
        }]
    }
}

/// Check if two values are lists of ID-bearing objects with identical
/// content. Catches cases where structurally equivalent lists fail `==`
/// due to map key ordering or float re-encoding.
fn id_keyed_list_equal(old: &Value, new: &Value) -> bool {
    let (old_arr, new_arr) = match (old.as_array(), new.as_array()) {
        (Some(o), Some(n)) => (o, n),
        _ => return false,
    };

    if old_arr.len() != new_arr.len() {
        return false;
    }
    if old_arr.is_empty() {
        return true;
    }

    // All elements must have "id" fields.
    let all_have_ids = old_arr
        .iter()
        .chain(new_arr.iter())
        .all(|v| v.get("id").is_some());
    if !all_have_ids {
        return false;
    }

    // Build lookup from old, check new matches.
    let old_by_id: HashMap<&Value, &Value> = old_arr
        .iter()
        .filter_map(|v| v.get("id").map(|id| (id, v)))
        .collect();

    new_arr.iter().all(|v| {
        v.get("id")
            .and_then(|id| old_by_id.get(id))
            .is_some_and(|old_v| *old_v == v)
    })
}

/// Diff two children arrays using the three-path strategy.
fn diff_children(
    old_children: &[TreeNode],
    new_children: &[TreeNode],
    path: &[usize],
) -> Vec<PatchOp> {
    let old_ids: Vec<&str> = old_children.iter().map(|c| c.id.as_str()).collect();
    let new_ids: Vec<&str> = new_children.iter().map(|c| c.id.as_str()).collect();

    // Build index maps for O(1) lookup.
    let old_by_id: HashMap<&str, (usize, &TreeNode)> = old_children
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), (i, c)))
        .collect();

    // Fast path: identical ID sequences.
    if old_ids == new_ids {
        return diff_children_same_order(old_children, new_children, path);
    }

    // Common IDs in their respective orders.
    let new_id_set: HashSet<&str> = new_ids.iter().copied().collect();
    let old_id_set: HashSet<&str> = old_ids.iter().copied().collect();

    let common_old: Vec<&str> = old_ids
        .iter()
        .filter(|id| new_id_set.contains(*id))
        .copied()
        .collect();
    let common_new: Vec<&str> = new_ids
        .iter()
        .filter(|id| old_id_set.contains(*id))
        .copied()
        .collect();

    let old_only: HashSet<&str> = old_ids
        .iter()
        .filter(|id| !new_id_set.contains(*id))
        .copied()
        .collect();

    if common_old == common_new {
        // Medium path: no reordering among common IDs.
        diff_children_no_reorder(&old_by_id, new_children, &old_only, path)
    } else {
        // Slow path: reordering detected, use LIS.
        diff_children_reorder(&old_by_id, new_children, &common_new, &old_only, path)
    }
}

/// Fast path: same ID order, diff each pair recursively.
fn diff_children_same_order(
    old_children: &[TreeNode],
    new_children: &[TreeNode],
    path: &[usize],
) -> Vec<PatchOp> {
    old_children
        .iter()
        .zip(new_children.iter())
        .enumerate()
        .flat_map(|(idx, (old_child, new_child))| {
            let mut child_path = path.to_vec();
            child_path.push(idx);
            diff_node(old_child, new_child, &child_path)
        })
        .collect()
}

/// Medium path: common IDs maintain relative order. Pure inserts and
/// removes, no moves needed.
fn diff_children_no_reorder(
    old_by_id: &HashMap<&str, (usize, &TreeNode)>,
    new_children: &[TreeNode],
    old_only: &HashSet<&str>,
    path: &[usize],
) -> Vec<PatchOp> {
    // Collect and sort old indices for removal.
    let mut removed_indices: Vec<usize> = old_only.iter().map(|id| old_by_id[id].0).collect();
    removed_indices.sort_unstable();

    // Remove ops in reverse index order.
    let remove_ops: Vec<PatchOp> = removed_indices
        .iter()
        .rev()
        .map(|&idx| PatchOp::RemoveChild {
            path: path.to_vec(),
            index: idx,
        })
        .collect();

    // Walk new children for updates and inserts.
    let mut update_ops = Vec::new();
    let mut insert_ops = Vec::new();

    for (idx, child) in new_children.iter().enumerate() {
        match old_by_id.get(child.id.as_str()) {
            Some(&(old_idx, old_child)) => {
                let adjusted = index_after_removals(old_idx, &removed_indices);
                let mut child_path = path.to_vec();
                child_path.push(adjusted);
                update_ops.extend(diff_node(old_child, child, &child_path));
            }
            None => {
                insert_ops.push(PatchOp::InsertChild {
                    path: path.to_vec(),
                    index: idx,
                    node: node_to_value(child),
                });
            }
        }
    }

    let mut ops = remove_ops;
    ops.extend(update_ops);
    ops.extend(insert_ops);
    ops
}

/// Slow path: reordering detected. Use LIS to find the largest subset
/// of common elements that maintain relative order. Elements in the LIS
/// stay in place; elements not in the LIS are removed and re-inserted.
fn diff_children_reorder(
    old_by_id: &HashMap<&str, (usize, &TreeNode)>,
    new_children: &[TreeNode],
    common_new: &[&str],
    old_only: &HashSet<&str>,
    path: &[usize],
) -> Vec<PatchOp> {
    // For common IDs in new order, get their old indices.
    let old_indices_of_common: Vec<usize> = common_new.iter().map(|id| old_by_id[id].0).collect();

    // Find LIS positions (indices into common_new).
    let lis_positions = longest_increasing_subsequence(&old_indices_of_common);
    let lis_set: HashSet<usize> = lis_positions.into_iter().collect();

    // IDs that stay in place (in the LIS).
    let lis_ids: HashSet<&str> = common_new
        .iter()
        .enumerate()
        .filter(|(i, _)| lis_set.contains(i))
        .map(|(_, id)| *id)
        .collect();

    // IDs that need to move: common but not in LIS.
    let moved_ids: HashSet<&str> = common_new
        .iter()
        .filter(|id| !lis_ids.contains(*id))
        .copied()
        .collect();

    // All indices to remove: old-only IDs + moved IDs.
    let all_remove_ids: HashSet<&str> = old_only.union(&moved_ids).copied().collect();
    let mut removed_indices: Vec<usize> = all_remove_ids.iter().map(|id| old_by_id[id].0).collect();
    removed_indices.sort_unstable();

    // Remove ops in reverse index order.
    let remove_ops: Vec<PatchOp> = removed_indices
        .iter()
        .rev()
        .map(|&idx| PatchOp::RemoveChild {
            path: path.to_vec(),
            index: idx,
        })
        .collect();

    // Update ops for LIS elements (they survive removals, need adjusted indices).
    let mut update_ops = Vec::new();
    for id in &lis_ids {
        let &(old_idx, old_child) = &old_by_id[id];
        let new_child = new_children.iter().find(|c| c.id == *id).unwrap();
        let adjusted = index_after_removals(old_idx, &removed_indices);
        let mut child_path = path.to_vec();
        child_path.push(adjusted);
        update_ops.extend(diff_node(old_child, new_child, &child_path));
    }

    // Insert ops: new-only IDs and moved IDs, at their new positions.
    let insert_ops: Vec<PatchOp> = new_children
        .iter()
        .enumerate()
        .filter(|(_, child)| {
            let cid = child.id.as_str();
            !old_by_id.contains_key(cid) || moved_ids.contains(cid)
        })
        .map(|(idx, child)| PatchOp::InsertChild {
            path: path.to_vec(),
            index: idx,
            node: node_to_value(child),
        })
        .collect();

    let mut ops = remove_ops;
    ops.extend(update_ops);
    ops.extend(insert_ops);
    ops
}

/// Adjust an old index for removals using binary search.
/// Returns the index the element would have after all lower-indexed
/// removals have been applied.
fn index_after_removals(old_idx: usize, sorted_removed: &[usize]) -> usize {
    let count = sorted_removed.partition_point(|&r| r < old_idx);
    old_idx - count
}

/// Longest Increasing Subsequence using patience sorting.
///
/// Returns the indices (positions) in the input slice that form the LIS.
/// O(n log n) time, O(n) space.
fn longest_increasing_subsequence(arr: &[usize]) -> Vec<usize> {
    if arr.is_empty() {
        return vec![];
    }

    let n = arr.len();
    // tails[i] = smallest tail value for increasing subsequence of length i+1
    let mut tails = vec![0usize; n];
    // idxs[i] = position in the original array for tails[i]
    let mut idxs = vec![0usize; n];
    // preds[pos] = predecessor position for backtracking (None for first element)
    let mut preds = vec![None::<usize>; n];
    let mut len = 0usize;

    for (pos, &val) in arr.iter().enumerate() {
        // Binary search for the insertion point in tails[0..len].
        let insert_pos = tails[..len].partition_point(|&t| t < val);

        if insert_pos > 0 {
            preds[pos] = Some(idxs[insert_pos - 1]);
        }

        tails[insert_pos] = val;
        idxs[insert_pos] = pos;

        if insert_pos >= len {
            len = insert_pos + 1;
        }
    }

    // Reconstruct the LIS by following predecessors backward.
    if len == 0 {
        return vec![];
    }
    let mut result = vec![0usize; len];
    let mut k = idxs[len - 1];
    for i in (0..len).rev() {
        result[i] = k;
        if let Some(pred) = preds[k] {
            k = pred;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: &str, type_name: &str, props: Value, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: type_name.to_string(),
            props: plushie_core::protocol::Props::from_json(props),
            children,
        }
    }

    fn simple_node(id: &str, type_name: &str, children: Vec<TreeNode>) -> TreeNode {
        node(id, type_name, json!({}), children)
    }

    #[test]
    fn identical_trees_produce_no_ops() {
        let tree = simple_node(
            "root",
            "column",
            vec![
                simple_node("a", "text", vec![]),
                simple_node("b", "button", vec![]),
            ],
        );
        let ops = diff_tree(&tree, &tree);
        assert!(ops.is_empty());
    }

    #[test]
    fn different_root_id_produces_replace() {
        let old = simple_node("root1", "column", vec![]);
        let new = simple_node("root2", "column", vec![]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            PatchOp::ReplaceNode {
                path: vec![],
                node: node_to_value(&new),
            }
        );
    }

    #[test]
    fn different_root_type_produces_replace() {
        let old = simple_node("root", "column", vec![]);
        let new = simple_node("root", "row", vec![]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            PatchOp::ReplaceNode {
                path: vec![],
                node: node_to_value(&new),
            }
        );
    }

    #[test]
    fn changed_prop_produces_update() {
        let old = node("root", "text", json!({"content": "hello"}), vec![]);
        let new = node("root", "text", json!({"content": "world"}), vec![]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            PatchOp::UpdateProps {
                path: vec![],
                props: json!({"content": "world"}),
            }
        );
    }

    #[test]
    fn added_prop_produces_update() {
        let old = node("root", "text", json!({"content": "hello"}), vec![]);
        let new = node(
            "root",
            "text",
            json!({"content": "hello", "size": 18}),
            vec![],
        );
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            PatchOp::UpdateProps {
                path: vec![],
                props: json!({"size": 18}),
            }
        );
    }

    #[test]
    fn removed_prop_produces_update_with_null() {
        let old = node(
            "root",
            "text",
            json!({"content": "hello", "size": 18}),
            vec![],
        );
        let new = node("root", "text", json!({"content": "hello"}), vec![]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            PatchOp::UpdateProps {
                path: vec![],
                props: json!({"size": null}),
            }
        );
    }

    #[test]
    fn added_child_produces_insert() {
        let old = simple_node("root", "column", vec![simple_node("a", "text", vec![])]);
        let new_child = simple_node("b", "button", vec![]);
        let new = simple_node(
            "root",
            "column",
            vec![simple_node("a", "text", vec![]), new_child.clone()],
        );
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            PatchOp::InsertChild {
                path: vec![],
                index: 1,
                node: node_to_value(&new_child),
            }
        );
    }

    #[test]
    fn removed_child_produces_remove() {
        let old = simple_node(
            "root",
            "column",
            vec![
                simple_node("a", "text", vec![]),
                simple_node("b", "button", vec![]),
            ],
        );
        let new = simple_node("root", "column", vec![simple_node("a", "text", vec![])]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            PatchOp::RemoveChild {
                path: vec![],
                index: 1,
            }
        );
    }

    #[test]
    fn swap_two_children_among_unchanged_siblings_round_trips() {
        // The LIS reorder path is the riskiest algorithm in this
        // module; the simplest adversarial case is swapping two
        // siblings in place while everything else stays put. This
        // asserts the diff + apply_patch pair still converges.
        let old = simple_node(
            "root",
            "column",
            vec![
                simple_node("a", "text", vec![]),
                simple_node("b", "text", vec![]),
                simple_node("c", "text", vec![]),
                simple_node("d", "text", vec![]),
                simple_node("e", "text", vec![]),
            ],
        );
        // Swap b and d.
        let new = simple_node(
            "root",
            "column",
            vec![
                simple_node("a", "text", vec![]),
                simple_node("d", "text", vec![]),
                simple_node("c", "text", vec![]),
                simple_node("b", "text", vec![]),
                simple_node("e", "text", vec![]),
            ],
        );
        let ops = diff_tree(&old, &new);
        let mut copy = old.clone();
        try_apply_patch(&mut copy, &ops).unwrap();
        assert_eq!(copy, new, "diff+apply must converge on a swap");
    }

    #[test]
    fn reordered_children_produce_remove_and_insert() {
        let old = simple_node(
            "root",
            "column",
            vec![
                simple_node("a", "text", vec![]),
                simple_node("b", "text", vec![]),
                simple_node("c", "text", vec![]),
            ],
        );
        let new = simple_node(
            "root",
            "column",
            vec![
                simple_node("c", "text", vec![]),
                simple_node("b", "text", vec![]),
                simple_node("a", "text", vec![]),
            ],
        );
        let ops = diff_tree(&old, &new);

        let has_removes = ops
            .iter()
            .any(|op| matches!(op, PatchOp::RemoveChild { .. }));
        let has_inserts = ops
            .iter()
            .any(|op| matches!(op, PatchOp::InsertChild { .. }));
        assert!(has_removes, "reorder should produce remove ops");
        assert!(has_inserts, "reorder should produce insert ops");
    }

    #[test]
    fn nested_prop_change_at_depth() {
        let old = simple_node(
            "root",
            "column",
            vec![simple_node(
                "child",
                "row",
                vec![node("deep", "text", json!({"content": "old"}), vec![])],
            )],
        );
        let new = simple_node(
            "root",
            "column",
            vec![simple_node(
                "child",
                "row",
                vec![node("deep", "text", json!({"content": "new"}), vec![])],
            )],
        );
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            PatchOp::UpdateProps {
                path: vec![0, 0],
                props: json!({"content": "new"}),
            }
        );
    }

    #[test]
    fn lis_algorithm_correctness() {
        let arr = vec![3, 1, 4, 1, 5, 9, 2, 6];
        let lis = longest_increasing_subsequence(&arr);
        assert_eq!(lis.len(), 4);

        let values: Vec<usize> = lis.iter().map(|&i| arr[i]).collect();
        for w in values.windows(2) {
            assert!(w[0] < w[1], "LIS must be strictly increasing: {:?}", values);
        }
    }

    #[test]
    fn lis_empty_input() {
        assert!(longest_increasing_subsequence(&[]).is_empty());
    }

    #[test]
    fn lis_single_element() {
        let lis = longest_increasing_subsequence(&[42]);
        assert_eq!(lis, vec![0]);
    }

    #[test]
    fn lis_already_sorted() {
        let arr = vec![1, 2, 3, 4, 5];
        let lis = longest_increasing_subsequence(&arr);
        assert_eq!(lis, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn lis_reverse_sorted() {
        let arr = vec![5, 4, 3, 2, 1];
        let lis = longest_increasing_subsequence(&arr);
        assert_eq!(lis.len(), 1);
    }

    #[test]
    fn type_change_at_child_produces_replace() {
        let old = simple_node("root", "column", vec![simple_node("a", "text", vec![])]);
        let new = simple_node("root", "column", vec![simple_node("a", "button", vec![])]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            PatchOp::ReplaceNode {
                path: vec![0],
                node: node_to_value(&simple_node("a", "button", vec![])),
            }
        );
    }

    #[test]
    fn multiple_children_removed() {
        let old = simple_node(
            "root",
            "column",
            vec![
                simple_node("a", "text", vec![]),
                simple_node("b", "text", vec![]),
                simple_node("c", "text", vec![]),
            ],
        );
        let new = simple_node("root", "column", vec![simple_node("b", "text", vec![])]);
        let ops = diff_tree(&old, &new);

        let remove_ops: Vec<&PatchOp> = ops
            .iter()
            .filter(|op| matches!(op, PatchOp::RemoveChild { .. }))
            .collect();
        assert_eq!(remove_ops.len(), 2);

        if let (PatchOp::RemoveChild { index: i1, .. }, PatchOp::RemoveChild { index: i2, .. }) =
            (&remove_ops[0], &remove_ops[1])
        {
            assert!(i1 > i2, "removes should be in reverse index order");
        }
    }

    #[test]
    fn multiple_children_inserted() {
        let old = simple_node("root", "column", vec![simple_node("a", "text", vec![])]);
        let b = simple_node("b", "text", vec![]);
        let c = simple_node("c", "text", vec![]);
        let new = simple_node(
            "root",
            "column",
            vec![simple_node("a", "text", vec![]), b.clone(), c.clone()],
        );
        let ops = diff_tree(&old, &new);

        let insert_ops: Vec<&PatchOp> = ops
            .iter()
            .filter(|op| matches!(op, PatchOp::InsertChild { .. }))
            .collect();
        assert_eq!(insert_ops.len(), 2);
    }

    #[test]
    fn combined_prop_changes() {
        let old = node(
            "root",
            "text",
            json!({"content": "hello", "size": 14, "color": "red"}),
            vec![],
        );
        let new = node(
            "root",
            "text",
            json!({"content": "world", "size": 14, "bold": true}),
            vec![],
        );
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        if let PatchOp::UpdateProps { props, .. } = &ops[0] {
            let p = props.as_object().unwrap();
            assert_eq!(p.get("content"), Some(&json!("world")));
            assert_eq!(p.get("bold"), Some(&json!(true)));
            assert_eq!(p.get("color"), Some(&Value::Null));
            assert!(
                !p.contains_key("size"),
                "unchanged prop should not be in patch"
            );
        } else {
            panic!("expected UpdateProps");
        }
    }

    #[test]
    fn medium_path_insert_and_remove() {
        let old = simple_node(
            "root",
            "column",
            vec![
                simple_node("a", "text", vec![]),
                simple_node("b", "text", vec![]),
                simple_node("c", "text", vec![]),
            ],
        );
        let d = simple_node("d", "text", vec![]);
        let new = simple_node(
            "root",
            "column",
            vec![
                simple_node("a", "text", vec![]),
                simple_node("c", "text", vec![]),
                d.clone(),
            ],
        );
        let ops = diff_tree(&old, &new);

        let removes: Vec<_> = ops
            .iter()
            .filter(|op| matches!(op, PatchOp::RemoveChild { .. }))
            .collect();
        let inserts: Vec<_> = ops
            .iter()
            .filter(|op| matches!(op, PatchOp::InsertChild { .. }))
            .collect();
        assert_eq!(removes.len(), 1);
        assert_eq!(inserts.len(), 1);

        if let PatchOp::RemoveChild { index, .. } = removes[0] {
            assert_eq!(*index, 1);
        }
        if let PatchOp::InsertChild { index, node, .. } = inserts[0] {
            assert_eq!(*index, 2);
            assert_eq!(*node, node_to_value(&d));
        }
    }

    #[test]
    fn serialization_format() {
        let op = PatchOp::UpdateProps {
            path: vec![0, 1],
            props: json!({"size": 18}),
        };
        let serialized = serde_json::to_value(&op).unwrap();
        assert_eq!(serialized["op"], "update_props");
        assert_eq!(serialized["path"], json!([0, 1]));
        assert_eq!(serialized["props"], json!({"size": 18}));

        let op = PatchOp::InsertChild {
            path: vec![],
            index: 2,
            node: json!({}),
        };
        let serialized = serde_json::to_value(&op).unwrap();
        assert_eq!(serialized["op"], "insert_child");
        assert_eq!(serialized["index"], 2);
    }

    #[test]
    fn try_apply_patch_invalid_path_returns_error_without_panic() {
        let mut tree = simple_node("root", "column", vec![simple_node("a", "text", vec![])]);
        let original = tree.clone();
        let ops = [PatchOp::UpdateProps {
            path: vec![1, 0],
            props: json!({"content": "nope"}),
        }];

        let result = try_apply_patch(&mut tree, &ops);
        assert!(result.is_err());
        let message = result.unwrap_err();
        assert!(message.contains("invalid patch path"));
        assert!(message.contains("path [1, 0]"));
        assert!(message.contains("index 1"));
        assert_eq!(tree, original);
    }

    #[test]
    fn apply_patch_invalid_path_does_not_panic() {
        let mut tree = simple_node("root", "column", vec![simple_node("a", "text", vec![])]);
        let original = tree.clone();
        let ops = [PatchOp::UpdateProps {
            path: vec![1, 0],
            props: json!({"content": "nope"}),
        }];

        apply_patch(&mut tree, &ops);

        assert_eq!(tree, original);
    }

    #[test]
    fn try_apply_patch_invalid_replace_node_payload_returns_error() {
        let mut tree = simple_node("root", "column", vec![simple_node("a", "text", vec![])]);
        let original = tree.clone();
        let ops = [PatchOp::ReplaceNode {
            path: vec![],
            node: json!({"id": "broken"}),
        }];

        let result = try_apply_patch(&mut tree, &ops);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("replace_node: invalid node"));
        assert_eq!(tree, original);
    }

    #[test]
    fn try_apply_patch_invalid_insert_child_payload_returns_error() {
        let mut tree = simple_node("root", "column", vec![]);
        let original = tree.clone();
        let ops = [PatchOp::InsertChild {
            path: vec![],
            index: 0,
            node: json!({"id": "broken"}),
        }];

        let result = try_apply_patch(&mut tree, &ops);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("insert_child: invalid node"));
        assert_eq!(tree, original);
    }

    #[test]
    fn try_apply_patch_remove_child_out_of_bounds_returns_error() {
        let mut tree = simple_node("root", "column", vec![simple_node("a", "text", vec![])]);
        let original = tree.clone();
        let ops = [PatchOp::RemoveChild {
            path: vec![],
            index: 3,
        }];

        let result = try_apply_patch(&mut tree, &ops);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("remove_child: index 3 out of bounds")
        );
        assert_eq!(tree, original);
    }

    #[test]
    fn diff_apply_round_trips_when_new_has_null_valued_prop() {
        // {"a": null} is wire-equivalent to absent per the protocol.
        // Round-trip must hold in both directions even if one side
        // carries an explicit null-valued entry.
        let empty = node("root", "text", json!({}), vec![]);
        let with_null = node("root", "text", json!({"a": null}), vec![]);

        let ops = diff_tree(&empty, &with_null);
        let mut copy = empty.clone();
        try_apply_patch(&mut copy, &ops).unwrap();
        assert_eq!(copy, with_null);

        let ops = diff_tree(&with_null, &empty);
        let mut copy = with_null.clone();
        try_apply_patch(&mut copy, &ops).unwrap();
        assert_eq!(copy, empty);
    }

    #[test]
    fn diff_apply_round_trips_when_changing_non_null_to_null() {
        let with_value = node("root", "text", json!({"a": 5}), vec![]);
        let with_null = node("root", "text", json!({"a": null}), vec![]);

        let ops = diff_tree(&with_value, &with_null);
        let mut copy = with_value.clone();
        try_apply_patch(&mut copy, &ops).unwrap();
        assert_eq!(copy, with_null);

        let ops = diff_tree(&with_null, &with_value);
        let mut copy = with_null.clone();
        try_apply_patch(&mut copy, &ops).unwrap();
        assert_eq!(copy, with_value);
    }

    #[test]
    fn numeric_variant_equal_props_produce_no_patch() {
        // Wire-canonical numeric equality: 42 (integer) and 42.0
        // (float) collapse to the same JSON shape, so the diff must
        // not emit a spurious update_props when only the variant
        // differs. This covers the animation-interpolation case
        // (authored I64 vs interpolated F64) and any path that
        // re-serialises numbers through serde.
        let mut int_props = plushie_core::protocol::PropMap::new();
        int_props.insert("size", 18i64);
        let mut float_props = plushie_core::protocol::PropMap::new();
        float_props.insert("size", 18.0f64);

        let old = TreeNode {
            id: "root".to_string(),
            type_name: "text".to_string(),
            props: plushie_core::protocol::Props::from(int_props),
            children: vec![],
        };
        let new = TreeNode {
            id: "root".to_string(),
            type_name: "text".to_string(),
            props: plushie_core::protocol::Props::from(float_props),
            children: vec![],
        };

        let ops = diff_tree(&old, &new);
        assert!(
            ops.is_empty(),
            "numeric-equal props must not produce a patch, got {ops:?}"
        );
    }

    #[test]
    fn id_keyed_list_props_are_compared_semantically() {
        let shapes = json!([
            {"id": "s1", "type": "rect", "x": 0},
            {"id": "s2", "type": "circle", "r": 10},
        ]);
        let old = node("c", "canvas", json!({"shapes": shapes.clone()}), vec![]);
        let new = node("c", "canvas", json!({"shapes": shapes}), vec![]);
        let ops = diff_tree(&old, &new);
        assert!(ops.is_empty());
    }

    #[test]
    fn id_keyed_list_content_change_produces_patch() {
        // When a shape's content changes, the diff must still produce
        // an update_props: the semantic-equality path must not mask
        // real changes.
        let old_shapes = json!([
            {"id": "s1", "type": "rect", "x": 0},
            {"id": "s2", "type": "circle", "r": 10},
        ]);
        let new_shapes = json!([
            {"id": "s1", "type": "rect", "x": 5},
            {"id": "s2", "type": "circle", "r": 10},
        ]);
        let old = node("c", "canvas", json!({"shapes": old_shapes}), vec![]);
        let new = node("c", "canvas", json!({"shapes": new_shapes}), vec![]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], PatchOp::UpdateProps { .. }));
    }

    #[test]
    fn id_keyed_list_added_element_produces_patch() {
        let old_shapes = json!([{"id": "s1", "type": "rect"}]);
        let new_shapes = json!([
            {"id": "s1", "type": "rect"},
            {"id": "s2", "type": "circle"},
        ]);
        let old = node("c", "canvas", json!({"shapes": old_shapes}), vec![]);
        let new = node("c", "canvas", json!({"shapes": new_shapes}), vec![]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], PatchOp::UpdateProps { .. }));
    }

    #[test]
    fn id_keyed_list_removed_element_produces_patch() {
        let old_shapes = json!([
            {"id": "s1", "type": "rect"},
            {"id": "s2", "type": "circle"},
        ]);
        let new_shapes = json!([{"id": "s1", "type": "rect"}]);
        let old = node("c", "canvas", json!({"shapes": old_shapes}), vec![]);
        let new = node("c", "canvas", json!({"shapes": new_shapes}), vec![]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], PatchOp::UpdateProps { .. }));
    }

    #[test]
    fn id_keyed_list_with_different_key_ordering_still_equal() {
        // Two shapes with the same IDs and values, but produced via
        // different serde_json::Map key insertion order. This is the
        // realistic miss case `==` rarely catches (JSON object keys
        // are already sorted in Rust's serde_json::Map, but this
        // pattern still exercises the path).
        let old_shapes = json!([
            {"id": "s1", "x": 0, "type": "rect"},
            {"id": "s2", "r": 10, "type": "circle"},
        ]);
        let new_shapes = json!([
            {"type": "rect", "id": "s1", "x": 0},
            {"type": "circle", "id": "s2", "r": 10},
        ]);
        let old = node("c", "canvas", json!({"shapes": old_shapes}), vec![]);
        let new = node("c", "canvas", json!({"shapes": new_shapes}), vec![]);
        let ops = diff_tree(&old, &new);
        assert!(
            ops.is_empty(),
            "shapes with identical content should compare equal regardless of key order"
        );
    }

    #[test]
    fn id_keyed_list_reordered_same_content_still_equal() {
        // Same id set with identical content, but the elements appear
        // in a different order. Matches the Elixir/Ruby/Gleam contract
        // (id-keyed comparison is set-based, not positional) and is
        // what lets hosts resort canvas shape lists without forcing a
        // prop update.
        let old_shapes = json!([
            {"id": "s1", "type": "rect", "x": 0},
            {"id": "s2", "type": "circle", "r": 10},
        ]);
        let new_shapes = json!([
            {"id": "s2", "type": "circle", "r": 10},
            {"id": "s1", "type": "rect", "x": 0},
        ]);
        let old = node("c", "canvas", json!({"shapes": old_shapes}), vec![]);
        let new = node("c", "canvas", json!({"shapes": new_shapes}), vec![]);
        let ops = diff_tree(&old, &new);
        assert!(
            ops.is_empty(),
            "reordered id-keyed lists with identical content should compare equal"
        );
    }

    #[test]
    fn id_keyed_list_different_id_sets_not_equal() {
        // Same length, but one list has an id the other doesn't.
        // `id_keyed_list_equal` must refuse equality so the diff still
        // emits an update_props patch.
        let old_shapes = json!([
            {"id": "s1", "type": "rect"},
            {"id": "s2", "type": "circle"},
        ]);
        let new_shapes = json!([
            {"id": "s1", "type": "rect"},
            {"id": "s3", "type": "circle"},
        ]);
        let old = node("c", "canvas", json!({"shapes": old_shapes}), vec![]);
        let new = node("c", "canvas", json!({"shapes": new_shapes}), vec![]);
        let ops = diff_tree(&old, &new);
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], PatchOp::UpdateProps { .. }));
    }
}
