//! Alignment types for widget layout.

use serde_json::Value;

use crate::protocol::{PropValue, Props};

use super::PlushieType;

/// Horizontal alignment within a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
}

impl PlushieType for HorizontalAlignment {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Left => "left",
                Self::Center => "center",
                Self::Right => "right",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "horizontal_alignment"
    }
}

/// Vertical alignment within a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}

impl PlushieType for VerticalAlignment {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "top" => Some(Self::Top),
            "center" => Some(Self::Center),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Top => "top",
                Self::Center => "center",
                Self::Bottom => "bottom",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "top" => Some(Self::Top),
            "center" => Some(Self::Center),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "vertical_alignment"
    }
}
