//! Tree normalization: scope prefixing and ID validation.
//!
//! After `App::view()` returns a `View`, normalization walks the tree
//! to apply scoped ID prefixes (containers with explicit IDs prefix
//! their children) and validate ID constraints (no duplicates, no
//! reserved characters).
//!
//! This mirrors the Elixir SDK's `Tree.normalize` function.

use std::collections::HashSet;

use serde_json::Value;

/// Normalize a view tree: apply scope prefixes and validate IDs.
///
/// Returns the normalized tree as a JSON value and any validation
/// warnings (duplicate IDs, reserved characters).
pub fn normalize(view: &Value) -> (Value, Vec<String>) {
    let mut warnings = Vec::new();
    let mut seen_ids = HashSet::new();
    let result = normalize_node(view, &[], &mut seen_ids, &mut warnings);
    (result, warnings)
}

fn normalize_node(
    node: &Value,
    scope: &[&str],
    seen_ids: &mut HashSet<String>,
    warnings: &mut Vec<String>,
) -> Value {
    let id = node["id"].as_str().unwrap_or("");
    let type_name = node["type"].as_str().unwrap_or("");

    // Determine if this node creates a scope (explicit ID, not auto-generated).
    let is_auto = id.starts_with("auto:");
    let is_window = type_name == "window";

    // Build the scoped ID.
    let scoped_id = if scope.is_empty() || is_auto {
        id.to_string()
    } else {
        format!("{}/{}", scope.join("/"), id)
    };

    // Check for duplicate IDs (only for non-auto IDs).
    if !is_auto && !scoped_id.is_empty() && !seen_ids.insert(scoped_id.clone()) {
        warnings.push(format!("duplicate ID: \"{scoped_id}\""));
    }

    // Check for reserved characters in user-provided IDs.
    if !is_auto && id.contains('/') {
        warnings.push(format!(
            "ID \"{id}\" contains reserved character '/'. \
             Use container scoping instead."
        ));
    }

    // Build the new scope for children.
    let child_scope: Vec<&str> = if !is_auto && !is_window && !id.is_empty() {
        let mut s = scope.to_vec();
        s.push(id);
        s
    } else {
        scope.to_vec()
    };

    // Normalize children recursively.
    let children = node["children"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|child| normalize_node(child, &child_scope, seen_ids, warnings))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Rebuild the node with the scoped ID.
    let mut result = node.clone();
    if let Some(obj) = result.as_object_mut() {
        obj.insert("id".to_string(), Value::String(scoped_id));
        obj.insert("children".to_string(), Value::Array(children));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: &str, type_name: &str, children: Vec<Value>) -> Value {
        json!({
            "id": id,
            "type": type_name,
            "props": {},
            "children": children,
        })
    }

    #[test]
    fn flat_tree_preserves_ids() {
        let tree = node("root", "column", vec![
            node("a", "text", vec![]),
            node("b", "text", vec![]),
        ]);
        let (result, warnings) = normalize(&tree);
        assert!(warnings.is_empty());
        assert_eq!(result["children"][0]["id"], "root/a");
        assert_eq!(result["children"][1]["id"], "root/b");
    }

    #[test]
    fn auto_ids_are_not_scoped() {
        let tree = node("auto:col:1:1", "column", vec![
            node("btn", "button", vec![]),
        ]);
        let (result, warnings) = normalize(&tree);
        assert!(warnings.is_empty());
        // Auto-ID container doesn't prefix children
        assert_eq!(result["children"][0]["id"], "btn");
    }

    #[test]
    fn nested_scoping() {
        let tree = node("form", "container", vec![
            node("section", "column", vec![
                node("field", "text_input", vec![]),
            ]),
        ]);
        let (result, warnings) = normalize(&tree);
        assert!(warnings.is_empty());
        assert_eq!(result["children"][0]["id"], "form/section");
        assert_eq!(
            result["children"][0]["children"][0]["id"],
            "form/section/field"
        );
    }

    #[test]
    fn window_does_not_scope_children() {
        let tree = node("main", "window", vec![
            node("col", "column", vec![]),
        ]);
        let (result, warnings) = normalize(&tree);
        assert!(warnings.is_empty());
        // Window nodes don't prefix children
        assert_eq!(result["children"][0]["id"], "col");
    }

    #[test]
    fn duplicate_ids_produce_warning() {
        let tree = node("root", "column", vec![
            node("btn", "button", vec![]),
            node("btn", "button", vec![]),
        ]);
        let (_, warnings) = normalize(&tree);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("duplicate ID"));
    }

    #[test]
    fn reserved_slash_in_id_produces_warning() {
        let tree = node("form/field", "text_input", vec![]);
        let (_, warnings) = normalize(&tree);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("reserved character"));
    }

    #[test]
    fn auto_ids_skip_duplicate_check() {
        let tree = node("root", "column", vec![
            node("auto:text:1:1", "text", vec![]),
            node("auto:text:1:1", "text", vec![]),
        ]);
        let (_, warnings) = normalize(&tree);
        // Auto IDs are allowed to repeat (same call site in a loop)
        assert!(warnings.is_empty());
    }
}
