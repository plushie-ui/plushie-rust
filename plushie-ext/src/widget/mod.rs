//! Widget implementations and rendering infrastructure.
//!
//! Each widget type has its own `_widget.rs` module containing
//! a full [`PlushieWidget`](crate::registry::PlushieWidget)
//! implementation. The [`widget_set`] module provides the default
//! [`IcedWidgetSet`](widget_set::IcedWidgetSet) that registers all
//! built-in widgets.
//!
//! The [`render`] function is the entry point for the immutable
//! view phase: it dispatches through the registry, applies a11y
//! wrapping, and bounds recursion depth.

// -- Infrastructure ----------------------------------------------------------

pub mod canvas;
pub mod helpers;
pub(crate) mod overlay;
pub mod render;
pub mod widget_set;

// Re-export for backward compatibility (these types moved to crate root).
pub use crate::shared_state::SharedState;
pub use crate::validate::{is_validate_props_enabled, set_validate_props};
pub use helpers::parse_padding_value;
pub use render::render;
pub use widget_set::{IcedWidgetSet, iced_widget_set};

// -- Widget modules ----------------------------------------------------------

pub(crate) mod button_widget;
pub(crate) mod canvas_widget;
pub(crate) mod checkbox_widget;
pub(crate) mod column_widget;
pub(crate) mod combo_box_widget;
pub(crate) mod container_widget;
pub(crate) mod float_widget;
pub(crate) mod grid_widget;
pub(crate) mod image_widget;
pub(crate) mod keyed_column_widget;
pub(crate) mod markdown_widget;
pub(crate) mod overlay_widget;
pub(crate) mod pane_grid_widget;
pub(crate) mod pick_list_widget;
pub(crate) mod pin_widget;
pub(crate) mod pointer_area_widget;
pub(crate) mod progress_bar_widget;
pub(crate) mod qr_code_widget;
pub(crate) mod radio_widget;
pub(crate) mod responsive_widget;
pub(crate) mod rich_text_widget;
pub(crate) mod row_widget;
pub(crate) mod rule_widget;
pub(crate) mod scrollable_widget;
pub(crate) mod sensor_widget;
pub(crate) mod slider_widget;
pub(crate) mod space_widget;
pub(crate) mod stack_widget;
pub(crate) mod svg_widget;
pub(crate) mod table_widget;
pub(crate) mod text_editor_widget;
pub(crate) mod text_input_widget;
pub(crate) mod text_widget;
pub(crate) mod themer_widget;
pub(crate) mod toggler_widget;
pub(crate) mod tooltip_widget;
pub(crate) mod window_widget;
