//! Wire protocol types for the UI tree.
//!
//! These types define the structure of the retained UI tree that the
//! host sends and the renderer maintains. [`TreeNode`] is the recursive
//! tree structure used in snapshot messages. [`PatchOp`] is the
//! incremental update format used in patch messages.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    ///
    /// [`Props::Typed`] in direct mode (SDK builders produce typed
    /// values), [`Props::Wire`] in wire mode (props arrive as JSON).
    #[serde(default = "empty_wire_props")]
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
}

/// Default for TreeNode.props during deserialization: an empty Wire
/// object (not Typed, since it comes from JSON).
fn empty_wire_props() -> super::Props {
    super::Props::Wire(serde_json::Value::Object(serde_json::Map::new()))
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
        // props defaults to an empty object, not null
        assert!(node.props.is_object());
        assert!(node.props.as_object().unwrap().is_empty());
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
