//! Widget selector for automation and tree search.
//!
//! Selectors identify widgets in the UI tree by various criteria:
//! ID, visible text, accessibility role, accessibility label, or
//! focus state. They are the addressing mechanism for the
//! automation layer, used by both SDK-side tree search and
//! renderer-side interact handling.
//!
//! # Selector formats
//!
//! ```ignore
//! Selector::id("save")              // by widget ID
//! Selector::id("form/save")         // by scoped ID path
//! Selector::id("main#save")         // window-qualified ID
//! Selector::text("Save")            // by visible text content
//! Selector::role("button")          // by accessibility role
//! Selector::label("Save document")  // by accessibility label
//! Selector::focused()               // currently focused widget
//! ```
//!
//! # Wire format
//!
//! Over the wire protocol, selectors are JSON objects:
//!
//! ```json
//! {"by": "id", "value": "save"}
//! {"by": "id", "value": "main#save", "window_id": "main"}
//! {"by": "text", "value": "Save"}
//! {"by": "role", "value": "button"}
//! {"by": "label", "value": "Save document"}
//! {"by": "focused"}
//! ```

use serde_json::Value;
use std::fmt;

use crate::protocol::TreeNode;

/// A selector that identifies a widget in the UI tree.
///
/// Used by the automation layer to target interactions (click,
/// type_text, etc.) and queries (find, assert). The selector is
/// resolved against the current widget tree to locate the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// Match a widget by its ID (local or scoped path).
    ///
    /// The `widget_id` may be a bare local name (`"save"`), a scoped
    /// path (`"form/save"`), or a window-qualified ID (`"main#save"`).
    /// Bare names and partial scoped paths also match as trailing
    /// segments, so `"form/save"` finds a node with the fully
    /// qualified id `"main#form/save"`. When `window_id` is set, the
    /// search is restricted to that window's subtree.
    Id {
        widget_id: String,
        window_id: Option<String>,
    },
    /// Match a widget by its visible text content.
    ///
    /// Searches the `content`, `label`, `value`, and `placeholder`
    /// props for a matching string.
    Text(String),
    /// Match a widget by its accessibility role.
    Role(String),
    /// Match a widget by its accessibility label.
    Label(String),
    /// Match the widget that currently has keyboard focus.
    Focused,
}

impl Selector {
    /// Create an ID selector.
    ///
    /// If the ID contains `#`, the prefix is extracted as the
    /// window ID for scoped search.
    pub fn id(id: &str) -> Self {
        let window_id = id
            .split_once('#')
            .filter(|(win, _)| !win.is_empty())
            .map(|(win, _)| win.to_string());
        Self::Id {
            widget_id: id.to_string(),
            window_id,
        }
    }

    /// Create an ID selector with an explicit window scope.
    pub fn id_in_window(id: &str, window_id: &str) -> Self {
        Self::Id {
            widget_id: id.to_string(),
            window_id: Some(window_id.to_string()),
        }
    }

    /// Create a text content selector.
    pub fn text(text: &str) -> Self {
        Self::Text(text.to_string())
    }

    /// Create an accessibility role selector.
    pub fn role(role: &str) -> Self {
        Self::Role(role.to_string())
    }

    /// Create an accessibility label selector.
    pub fn label(label: &str) -> Self {
        Self::Label(label.to_string())
    }

    /// Create a focused widget selector.
    pub fn focused() -> Self {
        Self::Focused
    }

    /// Parse a selector from the wire protocol JSON format.
    ///
    /// Expected format: `{"by": "id"|"text"|"role"|"label"|"focused", "value": "...", "window_id": "..."}`
    pub fn from_wire(value: &Value) -> Option<Self> {
        let by = value.get("by")?.as_str()?;
        match by {
            "focused" => Some(Self::Focused),
            _ => {
                let raw_value = value.get("value")?.as_str()?.to_string();
                let explicit_window = value
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                match by {
                    "id" => {
                        let window_id = raw_value
                            .split_once('#')
                            .filter(|(win, _)| !win.is_empty())
                            .map(|(win, _)| win.to_string())
                            .or(explicit_window);
                        Some(Self::Id {
                            widget_id: raw_value,
                            window_id,
                        })
                    }
                    "text" => Some(Self::Text(raw_value)),
                    "role" => Some(Self::Role(raw_value)),
                    "label" => Some(Self::Label(raw_value)),
                    _ => None,
                }
            }
        }
    }

    /// Encode this selector to the wire protocol JSON format.
    pub fn to_wire(&self) -> Value {
        match self {
            Self::Id {
                widget_id,
                window_id,
            } => {
                let mut obj = serde_json::json!({"by": "id", "value": widget_id});
                if let Some(win) = window_id {
                    obj["window_id"] = Value::String(win.clone());
                }
                obj
            }
            Self::Text(text) => serde_json::json!({"by": "text", "value": text}),
            Self::Role(role) => serde_json::json!({"by": "role", "value": role}),
            Self::Label(label) => serde_json::json!({"by": "label", "value": label}),
            Self::Focused => serde_json::json!({"by": "focused"}),
        }
    }
}

/// Convenience conversion from `&str` to `Selector::Id`.
///
/// Bare strings are treated as ID selectors. Supports the `#`
/// syntax for window qualification (`"main#save"`).
impl From<&str> for Selector {
    fn from(s: &str) -> Self {
        Self::id(s)
    }
}

impl From<String> for Selector {
    fn from(s: String) -> Self {
        Self::id(&s)
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id {
                widget_id,
                window_id: Some(win),
            } if !widget_id.starts_with(&format!("{win}#")) => {
                write!(f, "{win}#{widget_id}")
            }
            Self::Id { widget_id, .. } => write!(f, "{widget_id}"),
            Self::Text(text) => write!(f, "{{text: {text:?}}}"),
            Self::Role(role) => write!(f, "{{role: {role}}}"),
            Self::Label(label) => write!(f, "{{label: {label:?}}}"),
            Self::Focused => write!(f, "{{focused}}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tree search
// ---------------------------------------------------------------------------

/// Maximum recursion depth for tree traversal.
const MAX_SEARCH_DEPTH: usize = 256;

impl Selector {
    /// Find the first matching node in the tree.
    ///
    /// Returns a reference to the matching `TreeNode`, or `None` if
    /// no node matches the selector criteria.
    pub fn find<'a>(&self, root: &'a TreeNode) -> Option<&'a TreeNode> {
        match self {
            Self::Id {
                widget_id,
                window_id,
            } => find_by_id(root, widget_id, window_id.as_deref(), None, 0),
            Self::Text(text) => search(root, 0, &|n| matches_text(n, text)),
            Self::Role(role) => search(root, 0, &|n| matches_role(n, role)),
            Self::Label(label) => search(root, 0, &|n| matches_label(n, label)),
            Self::Focused => search(root, 0, &is_focused),
        }
    }

    /// Find all matching nodes in the tree.
    ///
    /// Returns a Vec of references to every `TreeNode` that matches.
    pub fn find_all<'a>(&self, root: &'a TreeNode) -> Vec<&'a TreeNode> {
        let mut results = Vec::new();
        match self {
            Self::Text(text) => search_all(root, 0, &|n| matches_text(n, text), &mut results),
            Self::Role(role) => search_all(root, 0, &|n| matches_role(n, role), &mut results),
            Self::Label(label) => search_all(root, 0, &|n| matches_label(n, label), &mut results),
            Self::Focused => search_all(root, 0, &is_focused, &mut results),
            Self::Id {
                widget_id,
                window_id,
            } => {
                // ID selectors match at most one node.
                if let Some(node) = find_by_id(root, widget_id, window_id.as_deref(), None, 0) {
                    results.push(node);
                }
            }
        }
        results
    }
}

// -- Depth-first search helpers ----------------------------------------------

fn search<'a>(
    node: &'a TreeNode,
    depth: usize,
    predicate: &dyn Fn(&TreeNode) -> bool,
) -> Option<&'a TreeNode> {
    if depth > MAX_SEARCH_DEPTH {
        return None;
    }
    if predicate(node) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| search(child, depth + 1, predicate))
}

fn search_all<'a>(
    node: &'a TreeNode,
    depth: usize,
    predicate: &dyn Fn(&TreeNode) -> bool,
    results: &mut Vec<&'a TreeNode>,
) {
    if depth > MAX_SEARCH_DEPTH {
        return;
    }
    if predicate(node) {
        results.push(node);
    }
    for child in &node.children {
        search_all(child, depth + 1, predicate, results);
    }
}

/// Find a node by ID, optionally scoped to a specific window.
///
/// Matches against the full scoped ID (`main#form/email`), the
/// local name (the segment after the last `/` or `#`), and any
/// trailing scoped-path suffix (so target `"todo-1/done"` matches
/// a node with id `"main#todo-1/done"`). This lets callers use
/// bare names, partial scoped paths, or fully qualified ids
/// interchangeably.
fn find_by_id<'a>(
    node: &'a TreeNode,
    target_id: &str,
    target_window: Option<&str>,
    current_window: Option<&'a str>,
    depth: usize,
) -> Option<&'a TreeNode> {
    if depth > MAX_SEARCH_DEPTH {
        return None;
    }

    let current_window = if node.type_name == "window" {
        Some(node.id.as_str())
    } else {
        current_window
    };

    let matches_id = node.id == target_id
        || local_name(&node.id) == target_id
        || node.id.ends_with(&format!("/{target_id}"))
        || node.id.ends_with(&format!("#{target_id}"));
    if matches_id && target_window.is_none_or(|win| current_window == Some(win)) {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_by_id(child, target_id, target_window, current_window, depth + 1))
}

/// Extract the local name from a scoped ID.
///
/// `"main#form/email"` -> `"email"`
/// `"form/email"` -> `"email"`
/// `"email"` -> `"email"`
fn local_name(id: &str) -> &str {
    id.rsplit_once('/')
        .or_else(|| id.rsplit_once('#'))
        .map(|(_, local)| local)
        .unwrap_or(id)
}

// -- Node predicates ---------------------------------------------------------

/// Match against text content in `content`, `label`, `value`, and
/// `placeholder` props.
fn matches_text(node: &TreeNode, text: &str) -> bool {
    for key in &["content", "label", "value", "placeholder"] {
        if node.props.get_str(key) == Some(text) {
            return true;
        }
    }
    false
}

/// Match by explicit `a11y.role`, falling back to `type_name` when
/// no `a11y` prop is present.
fn matches_role(node: &TreeNode, role: &str) -> bool {
    if let Some(a11y) = node.props.get_value("a11y") {
        a11y.get("role").and_then(|v| v.as_str()) == Some(role)
    } else {
        node.type_name == role
    }
}

/// Match by explicit `a11y.label`, falling back to `label` and
/// `content` props.
fn matches_label(node: &TreeNode, label: &str) -> bool {
    if let Some(a11y) = node.props.get_value("a11y")
        && a11y.get("label").and_then(|v| v.as_str()) == Some(label)
    {
        return true;
    }
    for key in &["label", "content"] {
        if node.props.get_str(key) == Some(label) {
            return true;
        }
    }
    false
}

/// Match nodes with `props.focused == true` or `a11y.focused == true`.
fn is_focused(node: &TreeNode) -> bool {
    if node.props.get_bool("focused") == Some(true) {
        return true;
    }
    if let Some(a11y) = node.props.get_value("a11y")
        && a11y.get("focused").and_then(|v| v.as_bool()) == Some(true)
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Props;

    /// Construct a minimal [`TreeNode`] for tree-search tests.
    fn node(id: &str, type_name: &str) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: type_name.to_string(),
            props: Props::default(),
            children: vec![],
        }
    }

    /// Construct a [`TreeNode`] with children.
    fn node_with_children(id: &str, type_name: &str, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: type_name.to_string(),
            props: Props::default(),
            children,
        }
    }

    #[test]
    fn find_by_id_matches_exact_id() {
        let root = node_with_children(
            "main",
            "window",
            vec![node("main#save", "button"), node("main#cancel", "button")],
        );
        let sel = Selector::id("main#save");
        let found = sel.find(&root).expect("exact id match");
        assert_eq!(found.id, "main#save");
    }

    #[test]
    fn find_by_id_matches_local_name() {
        let root = node_with_children(
            "main",
            "window",
            vec![node("main#save", "button"), node("main#cancel", "button")],
        );
        let sel = Selector::id("save");
        let found = sel.find(&root).expect("local-name match");
        assert_eq!(found.id, "main#save");
    }

    #[test]
    fn find_by_id_matches_scoped_path_suffix() {
        let root = node_with_children(
            "main",
            "window",
            vec![node_with_children(
                "main#todos",
                "column",
                vec![node("main#todo-1/done", "checkbox")],
            )],
        );
        let sel = Selector::id("todo-1/done");
        let found = sel
            .find(&root)
            .expect("scoped-path suffix should match trailing segments");
        assert_eq!(found.id, "main#todo-1/done");
    }

    #[test]
    fn find_by_id_matches_deeply_nested_scoped_suffix() {
        let root = node_with_children(
            "main",
            "window",
            vec![node_with_children(
                "main#page-theme",
                "column",
                vec![node_with_children(
                    "main#page-theme/page",
                    "column",
                    vec![node_with_children(
                        "main#page-theme/page/rating-card",
                        "column",
                        vec![node("main#page-theme/page/rating-card/stars", "canvas")],
                    )],
                )],
            )],
        );
        let sel = Selector::id("page-theme/page/rating-card/stars");
        let found = sel
            .find(&root)
            .expect("deep scoped-path suffix should match");
        assert_eq!(found.id, "main#page-theme/page/rating-card/stars");
    }

    #[test]
    fn find_by_id_local_name_still_matches_for_bare_target() {
        // Target "done" should still be resolvable via the local-name
        // rule even when the only candidate is a sibling subtree whose
        // scoped suffix does not line up on a `/` boundary.
        let root = node_with_children(
            "main",
            "window",
            vec![node("main#unrelated/done", "checkbox")],
        );
        let sel = Selector::id("done");
        let found = sel
            .find(&root)
            .expect("local-name rule should still hit here");
        assert_eq!(found.id, "main#unrelated/done");
    }

    #[test]
    fn find_by_id_does_not_match_mid_segment_substring() {
        // The suffix rule requires a `/` or `#` boundary. Target
        // "ne/done" must not match "main#unrelated/done" just because
        // it appears as a raw substring.
        let root = node_with_children(
            "main",
            "window",
            vec![node("main#unrelated/done", "checkbox")],
        );
        let sel = Selector::id("ne/done");
        assert!(
            sel.find(&root).is_none(),
            "suffix match must respect segment boundaries"
        );
    }
}
