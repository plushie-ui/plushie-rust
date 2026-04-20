//! Input-related enum types.

use crate::PlushieEnum;
use crate::protocol::PropValue;

/// Form-validation state for input-accepting widgets.
///
/// Projects onto accessibility props on the normalized tree:
///
/// - [`Validation::Valid`]: `a11y.invalid = false`.
/// - [`Validation::Pending`]: no a11y projection (validation in flight).
/// - [`Validation::Invalid { message }`]: `a11y.invalid = true` and
///   `a11y.error_message = message`.
///
/// The renderer only reads the resulting `a11y.*` fields. Host SDKs
/// across all languages share the same projection rules; this enum
/// is the Rust builder-side shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    /// Valid input.
    Valid,
    /// Validation in progress (e.g. awaiting server confirmation).
    Pending,
    /// Invalid input with a human-readable explanation.
    Invalid {
        /// Error message announced by assistive tech and available to
        /// apps that want to display it.
        message: String,
    },
}

impl Validation {
    /// Construct an [`Validation::Invalid`] variant from any string-like.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
        }
    }

    /// Wire encode. Shape:
    ///
    /// - `Valid`   -> `"valid"`
    /// - `Pending` -> `"pending"`
    /// - `Invalid { message }` -> `{"state": "invalid", "message": message}`
    ///
    /// The normalize pass accepts this shape (plus a few legacy aliases
    /// the other SDKs already emit) and projects onto `a11y.invalid` +
    /// `a11y.error_message`.
    pub fn wire_encode(&self) -> PropValue {
        match self {
            Self::Valid => PropValue::Str("valid".into()),
            Self::Pending => PropValue::Str("pending".into()),
            Self::Invalid { message } => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "state".to_string(),
                    serde_json::Value::String("invalid".into()),
                );
                map.insert(
                    "message".to_string(),
                    serde_json::Value::String(message.clone()),
                );
                PropValue::from(serde_json::Value::Object(map))
            }
        }
    }
}

/// Purpose hint for a text input, affecting virtual keyboard layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "input_purpose")]
pub enum InputPurpose {
    /// Normal.
    Normal,
    /// Secure.
    Secure,
    /// Terminal.
    Terminal,
    /// Number.
    Number,
    /// Decimal.
    Decimal,
    /// Phone.
    Phone,
    /// Email.
    Email,
    /// Url.
    Url,
    /// Search.
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
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// Quartile.
    Quartile,
    /// High.
    High,
}
