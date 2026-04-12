//! Interaction-related types.

use crate::PlushieEnum;

/// Mouse cursor style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "cursor_style")]
pub enum CursorStyle {
    Pointer,
    Grab,
    Grabbing,
    Crosshair,
    Text,
    Move,
    NotAllowed,
    Progress,
    Wait,
    Help,
    Cell,
    Copy,
    Alias,
    NoDrop,
    AllScroll,
    ZoomIn,
    ZoomOut,
    ContextMenu,
    ResizingHorizontally,
    ResizingVertically,
    ResizingDiagonallyUp,
    ResizingDiagonallyDown,
    ResizingColumn,
    ResizingRow,
}
