//! Tree normalization: scope prefixing and ID validation.
//!
//! After `App::view()` returns a `View` (TreeNode), normalization
//! walks the tree to apply scoped ID prefixes (containers with explicit
//! IDs prefix their children) and validate ID constraints (no duplicates,
//! no reserved characters).

use std::collections::HashSet;

use plushie_core::protocol::TreeNode;

/// Normalize a view tree: apply scope prefixes and validate IDs.
///
/// Returns the normalized tree and any validation warnings
/// (duplicate IDs, reserved characters).
pub fn normalize(tree: &TreeNode) -> (TreeNode, Vec<String>) {
    let mut warnings = Vec::new();
    let mut seen_ids = HashSet::new();
    let result = normalize_node(tree, &[], &mut seen_ids, &mut warnings);
    (result, warnings)
}

fn normalize_node(
    node: &TreeNode,
    scope: &[&str],
    seen_ids: &mut HashSet<String>,
    warnings: &mut Vec<String>,
) -> TreeNode {
    let id = &node.id;
    let type_name = &node.type_name;

    let is_auto = id.starts_with("auto:");
    let is_window = type_name == "window";

    // Build the scoped ID.
    let scoped_id = if scope.is_empty() || is_auto {
        id.clone()
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
    let children = node.children.iter()
        .map(|child| normalize_node(child, &child_scope, seen_ids, warnings))
        .collect();

    TreeNode {
        id: scoped_id,
        type_name: node.type_name.clone(),
        props: node.props.clone(),
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: &str, type_name: &str, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: type_name.to_string(),
            props: json!({}),
            children,
        }
    }

    #[test]
    fn flat_tree_preserves_ids() {
        let tree = node("root", "column", vec![
            node("a", "text", vec![]),
            node("b", "text", vec![]),
        ]);
        let (result, warnings) = normalize(&tree);
        assert!(warnings.is_empty());
        assert_eq!(result.children[0].id, "root/a");
        assert_eq!(result.children[1].id, "root/b");
    }

    #[test]
    fn auto_ids_are_not_scoped() {
        let tree = node("auto:col:1:1", "column", vec![
            node("btn", "button", vec![]),
        ]);
        let (result, warnings) = normalize(&tree);
        assert!(warnings.is_empty());
        assert_eq!(result.children[0].id, "btn");
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
        assert_eq!(result.children[0].id, "form/section");
        assert_eq!(result.children[0].children[0].id, "form/section/field");
    }

    #[test]
    fn window_does_not_scope_children() {
        let tree = node("main", "window", vec![
            node("col", "column", vec![]),
        ]);
        let (result, warnings) = normalize(&tree);
        assert!(warnings.is_empty());
        assert_eq!(result.children[0].id, "col");
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
        assert!(warnings.is_empty());
    }
}
