//! View builders for constructing UI trees.
//!
//! Each widget has a builder function that produces a [`View`].
//! Container widgets use `.child()` and `.children()` to nest content.
//! Display widgets are leaf nodes.
//!
//! ```ignore
//! use plushie::prelude::*;
//!
//! let view = window("main").title("My App").child(
//!     column().spacing(8).padding(16).children([
//!         text("Hello, world!"),
//!         button("ok", "OK").style(Style::primary()),
//!     ])
//! );
//! ```

mod layout;
mod display;
mod input;
mod interactive;

pub use layout::*;
pub use display::*;
pub use input::*;
pub use interactive::*;

use serde_json::{Map, Value, json};

use crate::View;
use crate::types::*;

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

impl View {
    /// Create a View from a TreeNode-like structure.
    pub(crate) fn node(id: String, type_name: &str, props: Map<String, Value>, children: Vec<View>) -> Self {
        View(json!({
            "id": id,
            "type": type_name,
            "props": Value::Object(props),
            "children": children.into_iter().map(|v| v.0).collect::<Vec<_>>(),
        }))
    }

    /// Create a leaf View (no children).
    pub(crate) fn leaf(id: String, type_name: &str, props: Map<String, Value>) -> Self {
        Self::node(id, type_name, props, vec![])
    }
}

// ---------------------------------------------------------------------------
// Auto-ID helper
// ---------------------------------------------------------------------------

/// Generate a stable auto-ID from the call site.
#[track_caller]
pub(crate) fn auto_id(prefix: &str) -> String {
    let loc = std::panic::Location::caller();
    format!("auto:{prefix}:{}:{}", loc.file(), loc.line())
}

// ---------------------------------------------------------------------------
// Prop helpers
// ---------------------------------------------------------------------------

/// Set an optional prop on a JSON map.
pub(crate) fn set_prop(props: &mut Map<String, Value>, key: &str, value: impl Into<Value>) {
    props.insert(key.to_string(), value.into());
}

/// Set a prop only if the value is Some.
pub(crate) fn set_opt<T: Into<Value>>(props: &mut Map<String, Value>, key: &str, value: Option<T>) {
    if let Some(v) = value {
        props.insert(key.to_string(), v.into());
    }
}

/// Convert a Length to a JSON value.
pub(crate) fn length_to_value(l: Length) -> Value {
    match l {
        Length::Fill => json!("fill"),
        Length::Shrink => json!("shrink"),
        Length::FillPortion(n) => json!({"fill_portion": n}),
        Length::Fixed(f) => json!(f),
    }
}

/// Convert a Padding to a JSON value.
pub(crate) fn padding_to_value(p: Padding) -> Value {
    if p.top == p.bottom && p.left == p.right && p.top == p.left {
        json!(p.top)
    } else if p.top == p.bottom && p.left == p.right {
        json!([p.top, p.left])
    } else {
        json!({"top": p.top, "right": p.right, "bottom": p.bottom, "left": p.left})
    }
}

/// Convert a Style to a JSON value.
pub(crate) fn style_to_value(s: &Style) -> Value {
    match s {
        Style::Preset(name) => json!(name),
        Style::Custom(map) => serde_json::to_value(map).unwrap_or(Value::Null),
    }
}

/// Convert a Color to a JSON value.
pub(crate) fn color_to_value(c: &Color) -> Value {
    json!(c.as_hex())
}

/// Convert an Align to a JSON string.
pub(crate) fn align_to_value(a: Align) -> Value {
    match a {
        Align::Start => json!("start"),
        Align::Center => json!("center"),
        Align::End => json!("end"),
    }
}
