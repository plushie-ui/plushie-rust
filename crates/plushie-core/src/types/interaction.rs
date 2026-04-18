//! Interaction-related types.

use crate::PlushieEnum;

/// Mouse cursor style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "cursor_style")]
pub enum CursorStyle {
    /// Pointer.
    Pointer,
    /// Grab.
    Grab,
    /// Grabbing.
    Grabbing,
    /// Crosshair.
    Crosshair,
    /// Text.
    Text,
    /// Move.
    Move,
    /// Not Allowed.
    NotAllowed,
    /// Progress.
    Progress,
    /// Wait.
    Wait,
    /// Help.
    Help,
    /// Cell.
    Cell,
    /// Copy.
    Copy,
    /// Alias.
    Alias,
    /// No Drop.
    NoDrop,
    /// All Scroll.
    AllScroll,
    /// Zoom In.
    ZoomIn,
    /// Zoom Out.
    ZoomOut,
    /// Context Menu.
    ContextMenu,
    /// Resizing Horizontally.
    ResizingHorizontally,
    /// Resizing Vertically.
    ResizingVertically,
    /// Resizing Diagonally Up.
    ResizingDiagonallyUp,
    /// Resizing Diagonally Down.
    ResizingDiagonallyDown,
    /// Resizing Column.
    ResizingColumn,
    /// Resizing Row.
    ResizingRow,
}
