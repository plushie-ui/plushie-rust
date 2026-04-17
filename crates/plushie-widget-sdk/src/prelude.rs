//! Common re-exports for widget authors.
//!
//! Import the entire prelude to get the types, traits, and helpers
//! needed to implement [`PlushieWidget`]:
//!
//! ```ignore
//! use plushie_widget_sdk::prelude::*;
//! ```
//!
//! For iced types not covered here (e.g. `canvas::Path`, advanced
//! layout widgets), use `plushie_widget_sdk::iced::*` instead of adding a
//! direct `iced` dependency. This avoids version conflicts when
//! plushie-core bumps its iced version.

// -- Renderer trait alias + Element shorthand --
pub use crate::{PlushieElement, PlushieRenderer};

// -- PlushieWidget trait, registry, and lifecycle types --
pub use crate::registry::{
    GenerationCounter, HandleResult, InitCtx, PlushieWidget, PlushieWidgetRender, SubscribeCtx,
    WidgetRegistry, WidgetSet, WidgetSubscription,
};

// -- Derive macros for widget props, events, and commands --
pub use crate::{WidgetCommand, WidgetEvent, WidgetProps};
// Derive macro re-export. Note: the macro and the
// `registry::PlushieWidget` trait share the name `PlushieWidget`,
// but Rust keeps macros and traits in separate namespaces so
// `#[derive(PlushieWidget)]` and `impl PlushieWidget<R>` both work
// from a single glob import.
pub use plushie_core_macros::PlushieWidget;

// -- Rendering context --
pub use crate::render_ctx::RenderCtx;

// -- Image registry (widgets that render images read from ctx.images) --
pub use crate::image_registry::ImageRegistry;

// -- A11y types (needed for PlushieWidget::infer_a11y) --
pub use crate::a11y::A11yOverrides;

// -- Canvas engine (for composing canvas-based widgets) --
pub use crate::canvas_engine::CanvasEngine;

// -- Wire protocol types --
pub use crate::message::Message;
pub use crate::protocol::{CoalesceHint, OutgoingEvent, TreeNode};

// -- Prop extraction helpers --
pub use crate::prop_helpers::*;

// -- Type conversions (plushie-core -> iced) --
pub use crate::iced_convert;

// -- Widget helpers (parsing, style application) --
pub use crate::widget::helpers::{
    StyleMapFields, StyleOverrides, container_style_from_base, get_style_overrides,
    parse_style_map_fields, parse_style_overrides, style_overrides_from_style_map,
};

// -- Commonly needed iced types --
//
// Note: `column` and `row` are deliberately excluded. They conflict
// with the `column!` and `row!` macros when glob-imported. Widget
// authors should use the macros directly (available via
// `plushie_widget_sdk::iced`) or import explicitly:
//   use plushie_widget_sdk::iced::widget::{column, row};
pub use crate::iced::widget::{
    button, canvas, checkbox, container, image, pick_list, progress_bar, rule, scrollable, slider,
    space, stack, text, toggler, tooltip,
};
pub use crate::iced::{
    Color, Element, Font, Length, Padding, Pixels, Point, Size, Theme, alignment,
};

// -- JSON (widgets parse props from serde_json::Value) --
pub use serde_json::Value;
