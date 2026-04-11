//! Text-related enum types.

use serde_json::Value;

use crate::protocol::{PropValue, Props};

use super::PlushieType;

/// How text wraps when it exceeds the available width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrapping {
    /// No wrapping. Text overflows.
    None,
    /// Wrap at word boundaries.
    Word,
    /// Wrap at glyph boundaries.
    Glyph,
    /// Try word boundaries first, then glyph boundaries.
    WordOrGlyph,
}

impl PlushieType for Wrapping {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "none" => Some(Self::None),
            "word" => Some(Self::Word),
            "glyph" => Some(Self::Glyph),
            "word_or_glyph" => Some(Self::WordOrGlyph),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::None => "none",
                Self::Word => "word",
                Self::Glyph => "glyph",
                Self::WordOrGlyph => "word_or_glyph",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "none" => Some(Self::None),
            "word" => Some(Self::Word),
            "glyph" => Some(Self::Glyph),
            "word_or_glyph" => Some(Self::WordOrGlyph),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "wrapping"
    }
}

/// Text shaping engine selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shaping {
    /// Basic shaping (fast, ASCII-only).
    Basic,
    /// Advanced shaping (HarfBuzz, handles complex scripts).
    Advanced,
    /// Automatic detection based on content.
    Auto,
}

impl PlushieType for Shaping {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "basic" => Some(Self::Basic),
            "advanced" => Some(Self::Advanced),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Basic => "basic",
                Self::Advanced => "advanced",
                Self::Auto => "auto",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "basic" => Some(Self::Basic),
            "advanced" => Some(Self::Advanced),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "shaping"
    }
}

/// How text is truncated when it overflows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ellipsis {
    /// No truncation.
    None,
    /// Truncate at the start, showing "...end".
    Start,
    /// Truncate in the middle, showing "sta...nd".
    Middle,
    /// Truncate at the end, showing "start...".
    End,
}

impl PlushieType for Ellipsis {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "none" => Some(Self::None),
            "start" => Some(Self::Start),
            "middle" => Some(Self::Middle),
            "end" => Some(Self::End),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::None => "none",
                Self::Start => "start",
                Self::Middle => "middle",
                Self::End => "end",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "none" => Some(Self::None),
            "start" => Some(Self::Start),
            "middle" => Some(Self::Middle),
            "end" => Some(Self::End),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "ellipsis"
    }
}
