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
    /// When `window_id` is set, the search is restricted to that
    /// window's subtree.
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
            } => write!(f, "{win}#{widget_id}"),
            Self::Id {
                widget_id,
                window_id: None,
            } => write!(f, "{widget_id}"),
            Self::Text(text) => write!(f, "{{text: {text:?}}}"),
            Self::Role(role) => write!(f, "{{role: {role}}}"),
            Self::Label(label) => write!(f, "{{label: {label:?}}}"),
            Self::Focused => write!(f, "{{focused}}"),
        }
    }
}
