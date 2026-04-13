//! Input-related enum types.

use crate::PlushieEnum;

/// Purpose hint for a text input, affecting virtual keyboard layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "input_purpose")]
pub enum InputPurpose {
    Normal,
    Secure,
    Terminal,
    Number,
    Decimal,
    Phone,
    Email,
    Url,
    Search,
}

/// Image filter/interpolation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "filter_method")]
pub enum FilterMethod {
    /// Nearest-neighbor interpolation (pixelated).
    Nearest,
    /// Bilinear interpolation (smooth).
    Linear,
}

/// QR code error correction level.
///
/// ## Wire format
/// Snake_case string: `"low"`, `"medium"`, `"quartile"`, `"high"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PlushieEnum)]
#[plushie_type(name = "error_correction")]
pub enum ErrorCorrection {
    Low,
    Medium,
    Quartile,
    High,
}
