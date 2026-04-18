//! Alignment types for widget layout.

use crate::PlushieEnum;

/// Horizontal alignment within a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "horizontal_alignment")]
pub enum HorizontalAlignment {
    /// Left.
    Left,
    /// Center.
    Center,
    /// Right.
    Right,
}

/// Vertical alignment within a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "vertical_alignment")]
pub enum VerticalAlignment {
    /// Top.
    Top,
    /// Center.
    Center,
    /// Bottom.
    Bottom,
}
