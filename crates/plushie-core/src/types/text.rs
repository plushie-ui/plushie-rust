//! Text-related enum types.

use crate::PlushieEnum;

/// Horizontal alignment for rendered text content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "text_alignment")]
pub enum TextAlignment {
    /// Use the renderer's default text alignment.
    Default,
    /// Align text to the physical left edge.
    Left,
    /// Center text.
    Center,
    /// Align text to the physical right edge.
    Right,
    /// Align text to the logical start edge.
    Start,
    /// Align text to the logical end edge.
    End,
    /// Justify text.
    Justified,
}

/// Text direction used to resolve logical text operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "text_direction")]
pub enum TextDirection {
    /// Use the renderer's default direction handling.
    Auto,
    /// Left-to-right text.
    Ltr,
    /// Right-to-left text.
    Rtl,
}

/// Logical text editor cursor motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "text_motion")]
pub enum TextMotion {
    /// Move backward in logical text order.
    Backward,
    /// Move forward in logical text order.
    Forward,
    /// Move up.
    Up,
    /// Move down.
    Down,
    /// Move one word backward in logical text order.
    WordBackward,
    /// Move one word forward in logical text order.
    WordForward,
    /// Move to the start of the current line.
    LineStart,
    /// Move to the end of the current line.
    LineEnd,
    /// Move one page up.
    PageUp,
    /// Move one page down.
    PageDown,
    /// Move to the start of the document.
    DocumentStart,
    /// Move to the end of the document.
    DocumentEnd,
}

impl From<super::HorizontalAlignment> for TextAlignment {
    fn from(value: super::HorizontalAlignment) -> Self {
        match value {
            super::HorizontalAlignment::Left => Self::Left,
            super::HorizontalAlignment::Center => Self::Center,
            super::HorizontalAlignment::Right => Self::Right,
        }
    }
}

/// How text wraps when it exceeds the available width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "wrapping")]
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

/// Text shaping engine selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "shaping")]
pub enum Shaping {
    /// Basic shaping (fast, ASCII-only).
    Basic,
    /// Advanced shaping (HarfBuzz, handles complex scripts).
    Advanced,
    /// Automatic detection based on content.
    Auto,
}

/// How text is truncated when it overflows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "ellipsis")]
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::protocol::PropValue;
    use crate::types::PlushieType;

    #[test]
    fn text_alignment_wire_encode_decode() {
        let cases = [
            (TextAlignment::Default, "default"),
            (TextAlignment::Left, "left"),
            (TextAlignment::Center, "center"),
            (TextAlignment::Right, "right"),
            (TextAlignment::Start, "start"),
            (TextAlignment::End, "end"),
            (TextAlignment::Justified, "justified"),
        ];

        for (variant, wire) in cases {
            assert_eq!(TextAlignment::wire_decode(&json!(wire)), Some(variant));
            assert_eq!(variant.wire_encode(), PropValue::Str(wire.into()));
        }
    }

    #[test]
    fn text_direction_wire_encode_decode() {
        let cases = [
            (TextDirection::Auto, "auto"),
            (TextDirection::Ltr, "ltr"),
            (TextDirection::Rtl, "rtl"),
        ];

        for (variant, wire) in cases {
            assert_eq!(TextDirection::wire_decode(&json!(wire)), Some(variant));
            assert_eq!(variant.wire_encode(), PropValue::Str(wire.into()));
        }
    }

    #[test]
    fn text_motion_wire_encode_decode() {
        let cases = [
            (TextMotion::Backward, "backward"),
            (TextMotion::Forward, "forward"),
            (TextMotion::Up, "up"),
            (TextMotion::Down, "down"),
            (TextMotion::WordBackward, "word_backward"),
            (TextMotion::WordForward, "word_forward"),
            (TextMotion::LineStart, "line_start"),
            (TextMotion::LineEnd, "line_end"),
            (TextMotion::PageUp, "page_up"),
            (TextMotion::PageDown, "page_down"),
            (TextMotion::DocumentStart, "document_start"),
            (TextMotion::DocumentEnd, "document_end"),
        ];

        for (variant, wire) in cases {
            assert_eq!(TextMotion::wire_decode(&json!(wire)), Some(variant));
            assert_eq!(variant.wire_encode(), PropValue::Str(wire.into()));
        }
    }

    #[test]
    fn text_motion_rejects_legacy_physical_names() {
        for wire in ["left", "right", "word_left", "word_right", "home", "end"] {
            assert_eq!(TextMotion::wire_decode(&json!(wire)), None);
        }
    }
}
