//! Common re-exports for widget authors.
//!
//! Import the entire prelude to get the types, traits, and helpers
//! needed to implement [`PlushieWidget`]:
//!
//! ```ignore
//! use plushie_ext::prelude::*;
//! ```
//!
//! For iced types not covered here (e.g. `canvas::Path`, advanced
//! layout widgets), use `plushie_ext::iced::*` instead of adding a
//! direct `iced` dependency. This avoids version conflicts when
//! plushie-core bumps its iced version.

// -- Renderer trait alias --
pub use crate::PlushieRenderer;

// -- PlushieWidget trait, registry, and lifecycle types --
pub use crate::registry::{GenerationCounter, InitCtx, PlushieWidget, WidgetRegistry, WidgetSet};

// -- Rendering context --
pub use crate::render_ctx::RenderCtx;

// -- A11y types (needed for PlushieWidget::infer_a11y) --
pub use crate::a11y::A11yOverrides;

// -- Canvas engine (for composing canvas-based widgets) --
pub use crate::canvas_engine::CanvasEngine;

// -- Wire protocol types --
pub use crate::message::Message;
pub use crate::protocol::{CoalesceHint, OutgoingEvent, TreeNode};

// -- Prop extraction helpers --
pub use crate::prop_helpers::*;

// -- Widget helpers (parsing, style application) --
pub use crate::widget::helpers::{
    StyleMapFields, StyleOverrides, container_style_from_base, get_style_overrides,
    parse_background, parse_border, parse_color, parse_font, parse_padding_value, parse_shadow,
    parse_style_map_fields, parse_style_overrides,
};

// -- Commonly needed iced types --
//
// Note: `column` and `row` are deliberately excluded. They conflict
// with the `column!` and `row!` macros when glob-imported. Widget
// authors should use the macros directly (available via
// `plushie_ext::iced`) or import explicitly:
//   use plushie_ext::iced::widget::{column, row};
pub use crate::iced::widget::{
    button, canvas, checkbox, container, image, pick_list, progress_bar, rule, scrollable, slider,
    space, stack, text, toggler, tooltip,
};
pub use crate::iced::{
    Color, Element, Font, Length, Padding, Pixels, Point, Size, Theme, alignment,
};

// -- JSON (widgets parse props from serde_json::Value) --
pub use serde_json::Value;
