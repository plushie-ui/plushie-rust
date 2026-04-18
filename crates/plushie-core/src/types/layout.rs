//! Layout-related enum types.

use crate::PlushieEnum;

/// Direction of scrolling or layout.
///
/// ## Wire format
/// Snake_case string: `"horizontal"`, `"vertical"`, `"both"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "direction")]
pub enum Direction {
    /// Horizontal.
    Horizontal,
    /// Vertical.
    Vertical,
    /// Both.
    Both,
}

/// Anchor point for positioning.
///
/// ## Wire format
/// Snake_case string: `"start"`, `"end"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "anchor")]
pub enum Anchor {
    /// Start.
    Start,
    /// End.
    End,
}

/// Position of a tooltip or overlay relative to its target.
///
/// ## Wire format
/// Snake_case string: `"below"`, `"above"`, `"left"`, `"right"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "position")]
pub enum Position {
    /// Below.
    Below,
    /// Above.
    Above,
    /// Left.
    Left,
    /// Right.
    Right,
}

/// How an image or content should be fit within its container.
///
/// ## Wire format
/// Snake_case string: `"contain"`, `"cover"`, `"fill"`, `"none"`, `"scale_down"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "content_fit")]
pub enum ContentFit {
    /// Scale to fit entirely within the container, preserving aspect ratio.
    Contain,
    /// Scale to cover the container, preserving aspect ratio (may crop).
    Cover,
    /// Stretch to fill the container exactly (may distort).
    Fill,
    /// Like Contain, but never scales up beyond the original size.
    ScaleDown,
    /// No scaling, display at original size.
    None,
}

/// Arrow key navigation mode for canvas interactive elements.
///
/// ## Wire format
/// Snake_case string: `"wrap"`, `"clamp"`, `"linear"`, `"none"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "arrow_mode")]
pub enum ArrowMode {
    /// Wrap.
    Wrap,
    /// Clamp.
    Clamp,
    /// Linear.
    Linear,
    /// None.
    None,
}

/// Sort direction for table columns.
///
/// ## Wire format
/// Snake_case string: `"asc"` or `"desc"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "sort_order")]
pub enum SortOrder {
    /// Ascending order (A-Z, 0-9).
    Asc,
    /// Descending order (Z-A, 9-0).
    Desc,
}
