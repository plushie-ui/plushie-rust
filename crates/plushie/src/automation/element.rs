//! Typed wrapper over tree nodes for automation queries.
//!
//! [`Element`] provides ergonomic access to widget properties
//! without requiring callers to navigate raw JSON. It wraps a
//! reference to a [`TreeNode`] and exposes typed accessors for
//! common properties.

use plushie_core::protocol::TreeNode;
use serde_json::Value;

/// A reference to a widget in the UI tree with typed accessors.
///
/// Created from a `TreeNode` reference (typically obtained via
/// [`Selector::find`](plushie_core::Selector::find)). Provides
/// ergonomic access to text content, accessibility properties,
/// and widget-specific props.
///
/// # Examples
///
/// ```ignore
/// let elem = Element::new(node);
/// assert_eq!(elem.widget_type(), "button");
/// assert_eq!(elem.text(), Some("Save"));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Element<'a> {
    node: &'a TreeNode,
}

impl<'a> Element<'a> {
    /// Wrap a tree node reference as an Element.
    pub fn new(node: &'a TreeNode) -> Self {
        Self { node }
    }

    /// The widget's scoped ID.
    pub fn id(&self) -> &str {
        &self.node.id
    }

    /// The widget type name (e.g. "button", "text", "container").
    pub fn widget_type(&self) -> &str {
        &self.node.type_name
    }

    /// The underlying tree node.
    pub fn node(&self) -> &'a TreeNode {
        self.node
    }

    /// Child elements.
    pub fn children(&self) -> Vec<Element<'a>> {
        self.node.children.iter().map(Element::new).collect()
    }

    /// The visible text content of this widget.
    ///
    /// Checks `content`, `label`, `value`, and `placeholder` props
    /// in that order, returning the first non-empty string found.
    pub fn text(&self) -> Option<&'a str> {
        for key in &["content", "label", "value", "placeholder"] {
            if let Some(text) = self.node.props.get_str(key) {
                return Some(text);
            }
        }
        None
    }

    /// Get a string property by key.
    pub fn prop_str(&self, key: &str) -> Option<&'a str> {
        self.node.props.get_str(key)
    }

    /// Get a float property by key.
    pub fn prop_f32(&self, key: &str) -> Option<f32> {
        self.node.props.get_f32(key)
    }

    /// Get a boolean property by key.
    pub fn prop_bool(&self, key: &str) -> Option<bool> {
        self.node.props.get_bool(key)
    }

    /// Get a property as an owned JSON Value.
    ///
    /// Works with both wire (JSON) and typed (PropValue) prop
    /// representations. For simple types, prefer the typed
    /// accessors (`prop_str`, `prop_f32`, `prop_bool`).
    pub fn prop(&self, key: &str) -> Option<Value> {
        self.node.props.get_value(key)
    }

    /// The accessibility properties for this widget, if any.
    pub fn a11y(&self) -> Option<Value> {
        self.node.props.get_value("a11y")
    }

    /// The inferred accessibility role of this widget.
    ///
    /// Returns the explicit `a11y.role` if set, otherwise falls
    /// back to the widget type name. Returns an owned String
    /// because the a11y role may be extracted from a temporary.
    pub fn inferred_role(&self) -> String {
        if let Some(a11y) = self.node.props.get_value("a11y")
            && let Some(role) = a11y.get("role").and_then(|v| v.as_str())
        {
            return role.to_string();
        }
        self.node.type_name.clone()
    }

    /// Whether this widget currently has keyboard focus.
    pub fn is_focused(&self) -> bool {
        if self.node.props.get_bool("focused") == Some(true) {
            return true;
        }
        if let Some(a11y) = self.node.props.get_value("a11y")
            && a11y.get("focused").and_then(|v| v.as_bool()) == Some(true)
        {
            return true;
        }
        false
    }

    /// Whether this widget is disabled.
    pub fn is_disabled(&self) -> bool {
        self.node.props.get_bool("disabled") == Some(true)
    }
}

impl<'a> From<&'a TreeNode> for Element<'a> {
    fn from(node: &'a TreeNode) -> Self {
        Self::new(node)
    }
}
