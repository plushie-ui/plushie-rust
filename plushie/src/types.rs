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
    Background, Border, Color, Font, FontStretch, FontStyle, FontWeight,
    Gradient, GradientStop, Length, Padding, PlushieType, Radius, Shadow,
    Style, StyleMap,
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
// SDK-specific: KeyModifiers
// -------------------------------------------------------------------------

/// Keyboard modifier state exposed to app `update/2`.
///
/// This is the SDK's user-facing type, separate from the protocol-level
/// `plushie_core::protocol::KeyModifiers` which has Serialize/Deserialize
/// for wire transport. The SDK version adds `Copy` and `Eq` for
/// ergonomic pattern matching in event handlers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
    pub command: bool,
}
