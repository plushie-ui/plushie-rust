pub use crate::shared_state::*;

use serde_json::Value;

use crate::protocol::TreeNode;

/// Reconstruct a shape JSON Value from a tree node.
///
/// Shape nodes have `{id, type, props, children}`. This converts back to
/// the `{type: type, id: id, ...props, children: [...]}` format that the
/// canvas rendering and hit-testing code expects.
fn tree_node_to_shape_value(node: &TreeNode) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String(node.type_name.clone()));

    // Copy all props into the shape map
    if let Some(obj) = node.props.as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), v.clone());
        }
    }

    // Recursively convert children (for group shapes)
    if !node.children.is_empty() {
        let child_shapes: Vec<Value> = node.children.iter().map(tree_node_to_shape_value).collect();
        map.insert("children".to_string(), Value::Array(child_shapes));
    }

    Value::Object(map)
}

/// Extract canvas layer data from a node's children. Returns owned Values
/// suitable for hashing and rendering.
///
/// Canvas nodes carry shapes as tree children:
/// - `__layer__` children with a `name` prop and shape children (layered)
/// - Direct shape children without a layer wrapper (flat, treated as "default" layer)
///
/// Returns a BTreeMap so layer order is deterministic (alphabetical by name).
pub(crate) fn canvas_layers_from_node(
    node: &TreeNode,
) -> std::collections::BTreeMap<String, Value> {
    let mut map = std::collections::BTreeMap::new();

    // Check if children are __layer__ containers
    let has_layers = node.children.iter().any(|c| c.type_name == "__layer__");

    if has_layers {
        for child in &node.children {
            if child.type_name == "__layer__" {
                let layer_name = child
                    .props
                    .as_object()
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();

                let shapes: Vec<Value> = child
                    .children
                    .iter()
                    .map(tree_node_to_shape_value)
                    .collect();
                map.insert(layer_name, Value::Array(shapes));
            }
        }
    } else if !node.children.is_empty() {
        // Direct shape children (flat canvas)
        let shapes: Vec<Value> = node.children.iter().map(tree_node_to_shape_value).collect();
        map.insert("default".to_string(), Value::Array(shapes));
    }

    map
}
