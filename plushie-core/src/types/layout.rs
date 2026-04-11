//! Layout-related enum types.

use serde_json::Value;

use crate::protocol::{PropValue, Props};

use super::PlushieType;

/// Direction of scrolling or layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Horizontal,
    Vertical,
    Both,
}

impl PlushieType for Direction {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "horizontal" => Some(Self::Horizontal),
            "vertical" => Some(Self::Vertical),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Horizontal => "horizontal",
                Self::Vertical => "vertical",
                Self::Both => "both",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "horizontal" => Some(Self::Horizontal),
            "vertical" => Some(Self::Vertical),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "direction"
    }
}

/// Anchor point for positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    End,
}

impl PlushieType for Anchor {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "start" => Some(Self::Start),
            "end" => Some(Self::End),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Start => "start",
                Self::End => "end",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "start" => Some(Self::Start),
            "end" => Some(Self::End),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "anchor"
    }
}

/// Position of a tooltip or overlay relative to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Below,
    Above,
    Left,
    Right,
}

impl PlushieType for Position {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "below" => Some(Self::Below),
            "above" => Some(Self::Above),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Below => "below",
                Self::Above => "above",
                Self::Left => "left",
                Self::Right => "right",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "below" => Some(Self::Below),
            "above" => Some(Self::Above),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "position"
    }
}

/// How an image or content should be fit within its container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl PlushieType for ContentFit {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "contain" => Some(Self::Contain),
            "cover" => Some(Self::Cover),
            "fill" => Some(Self::Fill),
            "scale_down" => Some(Self::ScaleDown),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Contain => "contain",
                Self::Cover => "cover",
                Self::Fill => "fill",
                Self::ScaleDown => "scale_down",
                Self::None => "none",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "contain" => Some(Self::Contain),
            "cover" => Some(Self::Cover),
            "fill" => Some(Self::Fill),
            "scale_down" => Some(Self::ScaleDown),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "content_fit"
    }
}

/// Arrow key navigation mode for canvas interactive elements.
///
/// ## Wire format
/// Snake_case string: `"wrap"`, `"clamp"`, `"linear"`, `"none"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowMode {
    Wrap,
    Clamp,
    Linear,
    None,
}

impl PlushieType for ArrowMode {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "wrap" => Some(Self::Wrap),
            "clamp" => Some(Self::Clamp),
            "linear" => Some(Self::Linear),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Wrap => "wrap",
                Self::Clamp => "clamp",
                Self::Linear => "linear",
                Self::None => "none",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "wrap" => Some(Self::Wrap),
            "clamp" => Some(Self::Clamp),
            "linear" => Some(Self::Linear),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "arrow_mode"
    }
}

/// Sort direction for table columns.
///
/// ## Wire format
/// Snake_case string: `"asc"` or `"desc"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Ascending order (A-Z, 0-9).
    Asc,
    /// Descending order (Z-A, 9-0).
    Desc,
}

impl PlushieType for SortOrder {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "asc" => Some(Self::Asc),
            "desc" => Some(Self::Desc),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Asc => "asc",
                Self::Desc => "desc",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "asc" => Some(Self::Asc),
            "desc" => Some(Self::Desc),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "sort_order"
    }
}
