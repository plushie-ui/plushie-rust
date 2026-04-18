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
/// Shorthand for iced `Element` parameterized with the plushie message type.
pub use crate::PlushieElement;
/// Renderer trait alias implemented for `iced::Renderer` and `()`.
pub use crate::PlushieRenderer;

// -- PlushieWidget trait, registry, and lifecycle types --
/// Monotonic counter used to freshen cached widget state.
pub use crate::registry::GenerationCounter;
/// Outcome of a widget message handler.
pub use crate::registry::HandleResult;
/// Context passed to [`PlushieWidget::init`].
pub use crate::registry::InitCtx;
/// Core trait implemented by every built-in and custom widget.
pub use crate::registry::PlushieWidget;
/// Object-safe render slice used by the widget dispatcher.
pub use crate::registry::PlushieWidgetRender;
/// Context passed to `PlushieWidget::subscribe`.
pub use crate::registry::SubscribeCtx;
/// Registry of widget implementations keyed by type name.
pub use crate::registry::WidgetRegistry;
/// Grouping trait for a reusable bundle of widgets.
pub use crate::registry::WidgetSet;
/// Declared widget subscription returned from `subscribe`.
pub use crate::registry::WidgetSubscription;

// -- Derive macros for widget props, events, and commands --
/// Derive macro generating widget command enums.
pub use crate::WidgetCommand;
/// Derive macro generating widget event enums.
pub use crate::WidgetEvent;
/// Derive macro generating typed widget prop structs.
pub use crate::WidgetProps;
// Derive macro re-export. Note: the macro and the
// `registry::PlushieWidget` trait share the name `PlushieWidget`,
// but Rust keeps macros and traits in separate namespaces so
// `#[derive(PlushieWidget)]` and `impl PlushieWidget<R>` both work
// from a single glob import.
/// Derive macro for the [`PlushieWidget`] trait on widget factories.
pub use plushie_core_macros::PlushieWidget;

// -- Rendering context --
/// Render-time context passed to [`PlushieWidget::render`].
pub use crate::render_ctx::RenderCtx;

// -- Image registry (widgets that render images read from ctx.images) --
/// In-memory image handle store accessed via `ctx.images`.
pub use crate::image_registry::ImageRegistry;

// -- A11y types (needed for PlushieWidget::infer_a11y) --
/// Accumulator for `PlushieWidget::infer_a11y` output.
pub use crate::a11y::A11yOverrides;

// -- Canvas engine (for composing canvas-based widgets) --
/// Reusable canvas composition engine.
pub use crate::canvas_engine::CanvasEngine;

// -- Wire protocol types --
/// Internal iced Message used by the widget dispatcher.
pub use crate::message::Message;
/// Coalesce hint for outgoing events.
pub use crate::protocol::CoalesceHint;
/// Widget event emitted to the SDK over the wire.
pub use crate::protocol::OutgoingEvent;
/// Retained widget-tree node structure.
pub use crate::protocol::TreeNode;

// -- Prop extraction helpers --
/// Typed prop-extract helpers (color, length, font, a11y, ...).
pub use crate::prop_helpers::*;

// -- Type conversions (plushie-core -> iced) --
/// Conversion helpers from plushie-core types to iced types.
pub use crate::iced_convert;

// -- Widget helpers (parsing, style application) --
/// Parsed style-map field extract.
pub use crate::widget::helpers::StyleMapFields;
/// Resolved style override set for a widget.
pub use crate::widget::helpers::StyleOverrides;
/// Apply plushie-core style overrides to an iced container style.
pub use crate::widget::helpers::container_style_from_base;
/// Look up per-widget style overrides by widget name.
pub use crate::widget::helpers::get_style_overrides;
/// Parse a StyleMap object into typed fields.
pub use crate::widget::helpers::parse_style_map_fields;
/// Parse a JSON value into typed [`StyleOverrides`].
pub use crate::widget::helpers::parse_style_overrides;
/// Build [`StyleOverrides`] from a parsed [`StyleMapFields`].
pub use crate::widget::helpers::style_overrides_from_style_map;

// -- Plushie-core primitive domain types --
//
// These are the canonical wire-aware types used in widget props and
// view trees. Widget authors build UI logic against these. At the
// render boundary, convert to iced types via `iced_convert::*`.
/// Plushie-core `Color` domain type (wire-aware).
pub use plushie_core::types::Color;
/// Plushie-core `Font` domain type (wire-aware).
pub use plushie_core::types::Font;
/// Plushie-core `Length` domain type (wire-aware).
pub use plushie_core::types::Length;
/// Plushie-core `Padding` domain type (wire-aware).
pub use plushie_core::types::Padding;
/// Plushie-core `Theme` domain type (wire-aware).
pub use plushie_core::types::Theme;

// -- Commonly needed iced rendering types --
//
// These are iced-only; plushie-core has no equivalents. Reach into
// `plushie_widget_sdk::iced::*` for anything else iced-specific.
//
// Note: `column` and `row` are deliberately excluded. They conflict
// with the `column!` and `row!` macros when glob-imported. Widget
// authors should use the macros directly (available via
// `plushie_widget_sdk::iced`) or import explicitly:
//   use plushie_widget_sdk::iced::widget::{column, row};
/// Iced `Element` type.
pub use crate::iced::Element;
/// Iced `Pixels` newtype (distinguishes logical vs physical lengths).
pub use crate::iced::Pixels;
/// Iced `Point` type.
pub use crate::iced::Point;
/// Iced `Size` type.
pub use crate::iced::Size;
/// Iced alignment helpers module.
pub use crate::iced::alignment;
/// Iced `button` widget constructor.
pub use crate::iced::widget::button;
/// Iced `canvas` widget constructor.
pub use crate::iced::widget::canvas;
/// Iced `checkbox` widget constructor.
pub use crate::iced::widget::checkbox;
/// Iced `container` widget constructor.
pub use crate::iced::widget::container;
/// Iced `image` widget constructor.
pub use crate::iced::widget::image;
/// Iced `pick_list` widget constructor.
pub use crate::iced::widget::pick_list;
/// Iced `progress_bar` widget constructor.
pub use crate::iced::widget::progress_bar;
/// Iced `rule` widget constructor.
pub use crate::iced::widget::rule;
/// Iced `scrollable` widget constructor.
pub use crate::iced::widget::scrollable;
/// Iced `slider` widget constructor.
pub use crate::iced::widget::slider;
/// Iced `space` widget constructor.
pub use crate::iced::widget::space;
/// Iced `stack` widget constructor.
pub use crate::iced::widget::stack;
/// Iced `text` widget constructor.
pub use crate::iced::widget::text;
/// Iced `toggler` widget constructor.
pub use crate::iced::widget::toggler;
/// Iced `tooltip` widget constructor.
pub use crate::iced::widget::tooltip;

// -- JSON (widgets parse props from serde_json::Value) --
/// Re-export of `serde_json::Value` for prop parsing.
pub use serde_json::Value;
