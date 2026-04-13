//! Text-related enum types.

use crate::PlushieEnum;

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
