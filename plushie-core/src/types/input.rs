//! Input-related enum types.

use serde_json::Value;

use crate::protocol::{PropValue, Props};

use super::PlushieType;

/// Purpose hint for a text input, affecting virtual keyboard layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl PlushieType for InputPurpose {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "normal" => Some(Self::Normal),
            "secure" => Some(Self::Secure),
            "terminal" => Some(Self::Terminal),
            "number" => Some(Self::Number),
            "decimal" => Some(Self::Decimal),
            "phone" => Some(Self::Phone),
            "email" => Some(Self::Email),
            "url" => Some(Self::Url),
            "search" => Some(Self::Search),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Normal => "normal",
                Self::Secure => "secure",
                Self::Terminal => "terminal",
                Self::Number => "number",
                Self::Decimal => "decimal",
                Self::Phone => "phone",
                Self::Email => "email",
                Self::Url => "url",
                Self::Search => "search",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "normal" => Some(Self::Normal),
            "secure" => Some(Self::Secure),
            "terminal" => Some(Self::Terminal),
            "number" => Some(Self::Number),
            "decimal" => Some(Self::Decimal),
            "phone" => Some(Self::Phone),
            "email" => Some(Self::Email),
            "url" => Some(Self::Url),
            "search" => Some(Self::Search),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "input_purpose"
    }
}

/// Image filter/interpolation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMethod {
    /// Nearest-neighbor interpolation (pixelated).
    Nearest,
    /// Bilinear interpolation (smooth).
    Linear,
}

impl PlushieType for FilterMethod {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "nearest" => Some(Self::Nearest),
            "linear" => Some(Self::Linear),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Nearest => "nearest",
                Self::Linear => "linear",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "nearest" => Some(Self::Nearest),
            "linear" => Some(Self::Linear),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "filter_method"
    }
}

/// QR code error correction level.
///
/// ## Wire format
/// Snake_case string: `"low"`, `"medium"`, `"quartile"`, `"high"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCorrection {
    Low,
    Medium,
    Quartile,
    High,
}

impl PlushieType for ErrorCorrection {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "quartile" => Some(Self::Quartile),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(
            match self {
                Self::Low => "low",
                Self::Medium => "medium",
                Self::Quartile => "quartile",
                Self::High => "high",
            }
            .into(),
        )
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        match props.get_str(key)? {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "quartile" => Some(Self::Quartile),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    fn type_name() -> &'static str {
        "error_correction"
    }
}
