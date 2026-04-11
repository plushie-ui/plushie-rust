//! Theme specification type.

use serde_json::Value;

use crate::protocol::PropValue;

use super::PlushieType;

/// Theme specification.
///
/// ## Wire format
///
/// A string: `"system"` for OS-detected theme, or a named theme
/// like `"dark"`, `"light"`, `"dracula"`, etc.
#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    /// A named built-in theme (e.g., "dark", "light", "dracula").
    Named(String),
    /// System theme (follows OS setting).
    System,
}

impl PlushieType for Theme {
    fn wire_decode(value: &Value) -> Option<Self> {
        let s = value.as_str()?;
        if s == "system" {
            Some(Self::System)
        } else {
            Some(Self::Named(s.to_string()))
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::System => PropValue::Str("system".to_string()),
            Self::Named(name) => PropValue::Str(name.clone()),
        }
    }

    fn type_name() -> &'static str {
        "theme"
    }
}

impl From<&str> for Theme {
    fn from(s: &str) -> Self {
        if s == "system" {
            Theme::System
        } else {
            Theme::Named(s.to_string())
        }
    }
}
