//! Interaction-related types.

use serde_json::Value;

use crate::protocol::{PropValue, Props};

use super::PlushieType;

/// Mouse cursor style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl PlushieType for CursorStyle {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "pointer" => Some(Self::Pointer),
            "grab" => Some(Self::Grab),
            "grabbing" => Some(Self::Grabbing),
            "crosshair" => Some(Self::Crosshair),
            "text" => Some(Self::Text),
            "move" => Some(Self::Move),
            "not_allowed" => Some(Self::NotAllowed),
            "progress" => Some(Self::Progress),
            "wait" => Some(Self::Wait),
            "help" => Some(Self::Help),
            "cell" => Some(Self::Cell),
            "copy" => Some(Self::Copy),
            "alias" => Some(Self::Alias),
            "no_drop" => Some(Self::NoDrop),
            "all_scroll" => Some(Self::AllScroll),
            "zoom_in" => Some(Self::ZoomIn),
            "zoom_out" => Some(Self::ZoomOut),
            "context_menu" => Some(Self::ContextMenu),
            "resizing_horizontally" => Some(Self::ResizingHorizontally),
            "resizing_vertically" => Some(Self::ResizingVertically),
            "resizing_diagonally_up" => Some(Self::ResizingDiagonallyUp),
            "resizing_diagonally_down" => Some(Self::ResizingDiagonallyDown),
            "resizing_column" => Some(Self::ResizingColumn),
            "resizing_row" => Some(Self::ResizingRow),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Pointer => "pointer",
                Self::Grab => "grab",
                Self::Grabbing => "grabbing",
                Self::Crosshair => "crosshair",
                Self::Text => "text",
                Self::Move => "move",
                Self::NotAllowed => "not_allowed",
                Self::Progress => "progress",
                Self::Wait => "wait",
                Self::Help => "help",
                Self::Cell => "cell",
                Self::Copy => "copy",
                Self::Alias => "alias",
                Self::NoDrop => "no_drop",
                Self::AllScroll => "all_scroll",
                Self::ZoomIn => "zoom_in",
                Self::ZoomOut => "zoom_out",
                Self::ContextMenu => "context_menu",
                Self::ResizingHorizontally => "resizing_horizontally",
                Self::ResizingVertically => "resizing_vertically",
                Self::ResizingDiagonallyUp => "resizing_diagonally_up",
                Self::ResizingDiagonallyDown => "resizing_diagonally_down",
                Self::ResizingColumn => "resizing_column",
                Self::ResizingRow => "resizing_row",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "pointer" => Some(Self::Pointer),
            "grab" => Some(Self::Grab),
            "grabbing" => Some(Self::Grabbing),
            "crosshair" => Some(Self::Crosshair),
            "text" => Some(Self::Text),
            "move" => Some(Self::Move),
            "not_allowed" => Some(Self::NotAllowed),
            "progress" => Some(Self::Progress),
            "wait" => Some(Self::Wait),
            "help" => Some(Self::Help),
            "cell" => Some(Self::Cell),
            "copy" => Some(Self::Copy),
            "alias" => Some(Self::Alias),
            "no_drop" => Some(Self::NoDrop),
            "all_scroll" => Some(Self::AllScroll),
            "zoom_in" => Some(Self::ZoomIn),
            "zoom_out" => Some(Self::ZoomOut),
            "context_menu" => Some(Self::ContextMenu),
            "resizing_horizontally" => Some(Self::ResizingHorizontally),
            "resizing_vertically" => Some(Self::ResizingVertically),
            "resizing_diagonally_up" => Some(Self::ResizingDiagonallyUp),
            "resizing_diagonally_down" => Some(Self::ResizingDiagonallyDown),
            "resizing_column" => Some(Self::ResizingColumn),
            "resizing_row" => Some(Self::ResizingRow),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "cursor_style"
    }
}
