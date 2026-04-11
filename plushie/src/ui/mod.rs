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
mod canvas;
mod memo;
mod table;

pub use layout::*;
pub use display::*;
pub use input::*;
pub use interactive::*;
pub use canvas::*;
pub use memo::*;
pub use table::*;

pub(crate) use plushie_core::protocol::{PropMap, PropValue};
use serde_json::{Value, json};

use crate::View;
use crate::types::*;

// ---------------------------------------------------------------------------
// View construction helpers
// ---------------------------------------------------------------------------

/// Create a View (TreeNode) with children.
pub(crate) fn view_node(id: String, type_name: &str, props: PropMap, children: Vec<View>) -> View {
    View {
        id,
        type_name: type_name.to_string(),
        props: plushie_core::protocol::Props::Typed(props),
        children,
    }
}

/// Create a leaf View (no children).
pub(crate) fn view_leaf(id: String, type_name: &str, props: PropMap) -> View {
    view_node(id, type_name, props, vec![])
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

/// Set a prop value on a PropMap.
pub(crate) fn set_prop(props: &mut PropMap, key: &str, value: impl Into<PropValue>) {
    props.insert(key, value.into());
}

/// Set a prop only if the value is Some.
pub(crate) fn set_opt<T: Into<PropValue>>(props: &mut PropMap, key: &str, value: Option<T>) {
    if let Some(v) = value {
        props.insert(key, v.into());
    }
}

/// Convert a Length to a PropValue.
pub(crate) fn length_to_value(l: Length) -> PropValue {
    match l {
        Length::Fill => PropValue::Str("fill".into()),
        Length::Shrink => PropValue::Str("shrink".into()),
        Length::FillPortion(n) => {
            let mut m = PropMap::new();
            m.insert("fill_portion", PropValue::U64(n as u64));
            PropValue::Object(m)
        }
        Length::Fixed(f) => PropValue::F64(f as f64),
    }
}

/// Convert a Padding to a PropValue.
pub(crate) fn padding_to_value(p: Padding) -> PropValue {
    if p.top == p.bottom && p.left == p.right && p.top == p.left {
        PropValue::F64(p.top as f64)
    } else if p.top == p.bottom && p.left == p.right {
        PropValue::Array(vec![
            PropValue::F64(p.top as f64),
            PropValue::F64(p.left as f64),
        ])
    } else {
        let mut m = PropMap::new();
        m.insert("top", PropValue::F64(p.top as f64));
        m.insert("right", PropValue::F64(p.right as f64));
        m.insert("bottom", PropValue::F64(p.bottom as f64));
        m.insert("left", PropValue::F64(p.left as f64));
        PropValue::Object(m)
    }
}

/// Convert a Style to a PropValue.
pub(crate) fn style_to_value(s: &Style) -> PropValue {
    match s {
        Style::Preset(name) => PropValue::Str(name.clone()),
        Style::Custom(map) => {
            PropValue::from(serde_json::to_value(map).unwrap_or(Value::Null))
        }
    }
}

/// Convert a Color to a PropValue.
pub(crate) fn color_to_value(c: &Color) -> PropValue {
    PropValue::Str(c.as_hex().to_string())
}

/// Convert an Align to a horizontal alignment string for the renderer.
pub(crate) fn halign_to_value(a: Align) -> Value {
    match a {
        Align::Start => json!("left"),
        Align::Center => json!("center"),
        Align::End => json!("right"),
    }
}

/// Convert an Align to a vertical alignment string for the renderer.
pub(crate) fn valign_to_value(a: Align) -> Value {
    match a {
        Align::Start => json!("top"),
        Align::Center => json!("center"),
        Align::End => json!("bottom"),
    }
}

/// Convert an Align to a cross-axis alignment string (start/center/end).
/// Used by overlay's `align` prop which has its own parser.
pub(crate) fn cross_align_to_value(a: Align) -> Value {
    match a {
        Align::Start => json!("start"),
        Align::Center => json!("center"),
        Align::End => json!("end"),
    }
}
