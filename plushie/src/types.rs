//! Shared property types for building views.
//!
//! Most types are re-exported from [`plushie_core::types`] which owns the
//! canonical definitions and wire encode/decode logic. This module adds
//! SDK-specific ergonomic types (`Align`, `KeyModifiers`) that only make
//! sense in the builder/event context.

// -------------------------------------------------------------------------
// Re-exports from plushie-core
// -------------------------------------------------------------------------

pub use plushie_core::types::{
    // Core trait
    PlushieType,
    // Animation
    Animatable,
    // Visual
    Color, Background, Gradient, GradientStop,
    // Geometry
    Length, Padding, Border, Radius, Shadow,
    // Typography
    Font, FontWeight, FontStyle, FontStretch,
    // Text layout
    Wrapping, Shaping, Ellipsis, LineHeight,
    // Alignment
    HorizontalAlignment, VerticalAlignment,
    // Layout
    Direction, Anchor, ArrowMode, Position, ContentFit, SortOrder,
    // Input
    InputPurpose, ErrorCorrection, FilterMethod, CursorStyle,
    // Style
    Style, StyleMap,
    // Value
    Range,
    // Theme
    Theme,
    // A11y
    A11y, Role, Live, Orientation, HasPopup,
};

// -------------------------------------------------------------------------
// Re-exports from plushie-core (canvas)
// -------------------------------------------------------------------------

pub use plushie_core::types::canvas::{
    DragAxis, FillRule, LineCap, LineJoin,
};

// -------------------------------------------------------------------------
// SDK-specific: Alignment
// -------------------------------------------------------------------------

/// Horizontal or vertical alignment.
///
/// Maps to different wire strings depending on context:
/// horizontal uses "left"/"center"/"right", vertical uses
/// "top"/"center"/"bottom", and cross-axis uses
/// "start"/"center"/"end".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Align to the start (left or top).
    Start,
    /// Align to the center.
    Center,
    /// Align to the end (right or bottom).
    End,
}

// -------------------------------------------------------------------------
// Re-export: KeyModifiers
// -------------------------------------------------------------------------

/// Keyboard modifier state exposed to app `update/2`.
///
/// Re-exported from `plushie_core::protocol::KeyModifiers`. The canonical
/// definition lives in plushie-core with all necessary derives (Copy, Eq,
/// Serialize, Deserialize).
pub use plushie_core::protocol::KeyModifiers;

// -------------------------------------------------------------------------
// Re-export: WindowLevel
// -------------------------------------------------------------------------

/// Window stacking level.
///
/// Re-exported from `plushie_core::ops::WindowLevel`.
pub use plushie_core::ops::WindowLevel;
