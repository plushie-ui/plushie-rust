//! Wire protocol types for the UI tree.
//!
//! These types define the structure of the retained UI tree that the
//! host sends and the renderer maintains. [`TreeNode`] is the recursive
//! tree structure used in snapshot messages. [`PatchOp`] is the
//! incremental update format used in patch messages.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A single node in the UI tree.
///
/// Each node has a unique `id` (scoped to the tree, assigned by the host),
/// a `type_name` that determines which widget renders it, a `props` map
/// of widget-specific properties, and optional `children` for container
/// widgets.
///
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TreeNode {
    /// Unique identifier for this node within the tree.
    pub id: String,

    /// Widget type name (e.g. `"button"`, `"text"`, `"slider"`).
    #[serde(rename = "type")]
    pub type_name: String,

    /// Widget-specific properties.
    #[serde(default)]
    pub props: super::Props,

    /// Child nodes for container widgets.
    #[serde(default)]
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Get a string prop by key.
    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.props.get_str(key)
    }

    /// Get an f32 prop by key.
    pub fn prop_f32(&self, key: &str) -> Option<f32> {
        self.props.get_f32(key)
    }

    /// Get a bool prop by key.
    pub fn prop_bool(&self, key: &str) -> Option<bool> {
        self.props.get_bool(key)
    }

    /// Compute the canonical tree hash used across SDKs.
    ///
    /// The hash input is recursively key-sorted JSON, so semantically
    /// identical trees hash the same regardless of object insertion order.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the tree cannot be converted to JSON.
    pub fn canonical_hash(&self) -> Result<String, serde_json::Error> {
        let json = self.canonical_json()?;
        Ok(format!("{:x}", Sha256::digest(json.as_bytes())))
    }

    fn canonical_json(&self) -> Result<String, serde_json::Error> {
        let value = serde_json::to_value(self)?;
        let mut out = String::new();
        write_canonical_json(&value, &mut out)?;
        Ok(out)
    }
}

/// Compute the canonical cross-SDK tree hash for an optional root node.
///
/// The hash input is recursively key-sorted JSON, then SHA-256 hex.
/// A missing root produces the empty string so renderer-side queries,
/// local test harnesses, and sibling SDKs can share one empty-tree policy.
///
/// # Errors
///
/// Returns a serialization error if the tree cannot be converted to JSON.
pub fn canonical_tree_hash(root: Option<&TreeNode>) -> Result<String, serde_json::Error> {
    match root {
        Some(root) => root.canonical_hash(),
        None => Ok(String::new()),
    }
}

fn write_canonical_json(value: &Value, out: &mut String) -> Result<(), serde_json::Error> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            out.push_str(&serde_json::to_string(value)?);
            Ok(())
        }
        Value::Array(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                write_canonical_json(item, out)?;
            }
            out.push(']');
            Ok(())
        }
        Value::Object(map) => {
            out.push('{');
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
            for (idx, (key, item)) in entries.into_iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key)?);
                out.push(':');
                write_canonical_json(item, out)?;
            }
            out.push('}');
            Ok(())
        }
    }
}

/// A single patch operation applied incrementally to the retained tree.
///
/// The `op` field discriminates the operation type. The `path` field
/// identifies the target node as a sequence of child indices from the
/// root. Operation-specific fields are captured in `rest` via
/// `#[serde(flatten)]`.
///
/// Supported operations: `replace_node`, `update_props`,
/// `insert_child`, `remove_child`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PatchOp {
    /// Operation type (e.g. `"replace_node"`, `"update_props"`).
    pub op: String,

    /// Path from the tree root to the target node, as a sequence of
    /// child indices. An empty path targets the root.
    pub path: Vec<usize>,

    /// Operation-specific fields (e.g. `node`, `props`, `index`).
    #[serde(flatten)]
    pub rest: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{PropMap, PropValue, Props};
    use serde_json::json;

    // -- TreeNode deserialization ---------------------------------------------

    #[test]
    fn tree_node_full() {
        let val = json!({
            "id": "root",
            "type": "column",
            "props": {"spacing": 10},
            "children": [
                {"id": "c1", "type": "text", "props": {"content": "hi"}, "children": []}
            ]
        });
        let node: TreeNode = serde_json::from_value(val).unwrap();
        assert_eq!(node.id, "root");
        assert_eq!(node.type_name, "column");
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].id, "c1");
        assert_eq!(node.props.get_f64("spacing"), Some(10.0));
    }

    #[test]
    fn tree_node_defaults_props_and_children() {
        let node: TreeNode = serde_json::from_value(json!({"id": "x", "type": "text"})).unwrap();
        assert_eq!(node.id, "x");
        assert_eq!(node.type_name, "text");
        assert!(node.children.is_empty());
        // props defaults to an empty map
        assert!(node.props.as_prop_map().is_empty());
    }

    #[test]
    fn tree_node_deeply_nested() {
        let val = json!({
            "id": "a", "type": "column", "children": [
                {"id": "b", "type": "row", "children": [
                    {"id": "c", "type": "text"}
                ]}
            ]
        });
        let node: TreeNode = serde_json::from_value(val).unwrap();
        assert_eq!(node.children[0].children[0].id, "c");
    }

    #[test]
    fn canonical_hash_ignores_prop_key_insertion_order() {
        let mut left_inner = PropMap::new();
        left_inner.insert("zebra", 3_i64);
        left_inner.insert("apple", 1_i64);
        let mut left_props = PropMap::new();
        left_props.insert("style", PropValue::Object(left_inner));
        left_props.insert("label", "hello");
        let left = TreeNode {
            id: "root".to_string(),
            type_name: "text".to_string(),
            props: Props::from(left_props),
            children: vec![],
        };

        let mut right_inner = PropMap::new();
        right_inner.insert("apple", 1_i64);
        right_inner.insert("zebra", 3_i64);
        let mut right_props = PropMap::new();
        right_props.insert("label", "hello");
        right_props.insert("style", PropValue::Object(right_inner));
        let right = TreeNode {
            id: "root".to_string(),
            type_name: "text".to_string(),
            props: Props::from(right_props),
            children: vec![],
        };

        assert_eq!(
            left.canonical_hash().unwrap(),
            right.canonical_hash().unwrap()
        );
    }

    #[test]
    fn canonical_hash_matches_expected_json_contract() {
        let mut child_props = PropMap::new();
        child_props.insert("text", "hello");
        let mut root_props = PropMap::new();
        root_props.insert("z", true);
        root_props.insert("a", 1_i64);

        let tree = TreeNode {
            id: "root".to_string(),
            type_name: "column".to_string(),
            props: Props::from(root_props),
            children: vec![TreeNode {
                id: "child".to_string(),
                type_name: "text".to_string(),
                props: Props::from(child_props),
                children: vec![],
            }],
        };

        let expected_json = concat!(
            r#"{"children":[{"children":[],"id":"child","props":{"text":"hello"},"type":"text"}],"#,
            r#""id":"root","props":{"a":1,"z":true},"type":"column"}"#
        );

        assert_eq!(tree.canonical_json().unwrap(), expected_json);
        assert_eq!(
            tree.canonical_hash().unwrap(),
            format!("{:x}", Sha256::digest(expected_json.as_bytes()))
        );
    }

    #[test]
    fn canonical_tree_hash_empty_tree_is_empty_string() {
        assert_eq!(canonical_tree_hash(None).unwrap(), "");
    }

    // -- PatchOp deserialization ----------------------------------------------

    #[test]
    fn patch_op_replace_node() {
        let val =
            json!({"op": "replace_node", "path": [1, 2], "node": {"id": "n", "type": "text"}});
        let op: PatchOp = serde_json::from_value(val).unwrap();
        assert_eq!(op.op, "replace_node");
        assert_eq!(op.path, vec![1, 2]);
        assert!(op.rest.get("node").is_some());
    }

    #[test]
    fn patch_op_update_props() {
        let val = json!({"op": "update_props", "path": [0], "props": {"color": "red"}});
        let op: PatchOp = serde_json::from_value(val).unwrap();
        assert_eq!(op.op, "update_props");
        assert_eq!(op.rest["props"]["color"], "red");
    }

    #[test]
    fn patch_op_insert_child() {
        let val = json!({"op": "insert_child", "path": [], "index": 0, "node": {"id": "new", "type": "button"}});
        let op: PatchOp = serde_json::from_value(val).unwrap();
        assert_eq!(op.op, "insert_child");
        assert!(op.path.is_empty());
        assert_eq!(op.rest["index"], 0);
    }

    #[test]
    fn patch_op_remove_child() {
        let val = json!({"op": "remove_child", "path": [0], "index": 1});
        let op: PatchOp = serde_json::from_value(val).unwrap();
        assert_eq!(op.op, "remove_child");
        assert_eq!(op.rest["index"], 1);
    }
}
