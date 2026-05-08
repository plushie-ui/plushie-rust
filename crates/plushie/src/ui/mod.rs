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

mod canvas;
mod display;
mod input;
mod interactive;
mod layout;
mod memo;
mod table;

pub use canvas::*;
pub use display::*;
pub use input::*;
pub use interactive::*;
pub use layout::*;
pub use memo::*;
pub use table::*;

pub(crate) use plushie_core::protocol::{PropMap, PropValue};

use crate::View;
use crate::derive_support::PlushieType;
use crate::types::*;

// ---------------------------------------------------------------------------
// View construction helpers
// ---------------------------------------------------------------------------

/// Create a View with children.
pub(crate) fn view_node(id: String, type_name: &str, props: PropMap, children: Vec<View>) -> View {
    View::new(
        id,
        type_name,
        plushie_core::protocol::Props::from(props),
        children,
    )
}

/// Create a leaf View (no children).
pub(crate) fn view_leaf(id: String, type_name: &str, props: PropMap) -> View {
    view_node(id, type_name, props, vec![])
}

// ---------------------------------------------------------------------------
// Auto-ID helper
// ---------------------------------------------------------------------------

/// Generate a stable auto-ID from the call site.
///
/// The file path is normalised to forward slashes so the ID is the
/// same on Windows (where `loc.file()` returns backslash-separated
/// paths) as on Unix. Cross-SDK tree hashes and golden snapshots
/// depend on this stability.
#[track_caller]
pub(crate) fn auto_id(prefix: &str) -> String {
    let loc = std::panic::Location::caller();
    let file = loc.file().replace('\\', "/");
    format!("auto:{prefix}:{file}:{}", loc.line())
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

/// Convert a Length to a PropValue via PlushieType.
pub(crate) fn length_to_value(l: Length) -> PropValue {
    l.wire_encode()
}

/// Convert a Padding to a PropValue via PlushieType.
pub(crate) fn padding_to_value(p: Padding) -> PropValue {
    p.wire_encode()
}

/// Convert a Style to a PropValue via PlushieType.
pub(crate) fn style_to_value(s: &Style) -> PropValue {
    s.wire_encode()
}

/// Convert a Color to a PropValue via PlushieType.
pub(crate) fn color_to_value(c: &Color) -> PropValue {
    c.wire_encode()
}

/// Convert a Background (solid color or gradient) to a PropValue
/// via PlushieType.
pub(crate) fn background_to_value(bg: &Background) -> PropValue {
    bg.wire_encode()
}

/// Convert an Align to a horizontal alignment string for the renderer.
pub(crate) fn halign_to_value(a: Align) -> PropValue {
    PropValue::Str(
        match a {
            Align::Start => "left",
            Align::Center => "center",
            Align::End => "right",
        }
        .into(),
    )
}

/// Convert an Align to a vertical alignment string for the renderer.
pub(crate) fn valign_to_value(a: Align) -> PropValue {
    PropValue::Str(
        match a {
            Align::Start => "top",
            Align::Center => "center",
            Align::End => "bottom",
        }
        .into(),
    )
}

/// Convert an Align to a cross-axis alignment string (start/center/end).
/// Used by overlay's `align` prop which has its own parser.
pub(crate) fn cross_align_to_value(a: Align) -> PropValue {
    PropValue::Str(
        match a {
            Align::Start => "start",
            Align::Center => "center",
            Align::End => "end",
        }
        .into(),
    )
}
