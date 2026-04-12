//! Payload specifications for events and commands.
//!
//! Describes the expected shape of an event's or command's payload
//! so it can be validated at runtime. The same spec structure is
//! used for both events and commands, mirroring Elixir's
//! `Plushie.Event.BuiltinSpecs` system.
//!
//! # Spec shapes
//!
//! - [`PayloadSpec::None`] - no payload (e.g., click, reset)
//! - [`PayloadSpec::Value`] - single typed value (e.g., slide -> f32)
//! - [`PayloadSpec::Fields`] - named typed fields (e.g., drag -> {x, y})
//!
//! # Usage
//!
//! Widget renderers declare specs alongside their event/command
//! definitions. The registry validates payloads against specs
//! when converting messages to outgoing events.

use serde_json::Value;

/// The type of a single value in a payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// JSON string
    String,
    /// JSON number (f64)
    Float,
    /// JSON integer
    Integer,
    /// JSON boolean
    Bool,
    /// Any JSON value (no type constraint)
    Any,
}

impl ValueType {
    /// Check if a JSON value matches this type.
    pub fn matches(&self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Float => value.is_f64(),
            Self::Integer => value.is_i64() || value.is_u64(),
            Self::Bool => value.is_boolean(),
            Self::Any => true,
        }
    }
}

/// Describes the expected shape of a message payload.
#[derive(Debug, Clone)]
pub enum PayloadSpec {
    /// No payload expected (value should be null).
    None,
    /// A single typed value.
    Value(ValueType),
    /// Named fields with types. Some fields may be required.
    Fields {
        fields: Vec<(String, ValueType)>,
        required: Vec<String>,
    },
}

impl PayloadSpec {
    /// Validate a JSON value against this spec.
    ///
    /// Returns true if the value conforms to the expected shape.
    pub fn validate(&self, value: &Value) -> bool {
        match self {
            Self::None => value.is_null(),
            Self::Value(vt) => vt.matches(value),
            Self::Fields { fields, required } => {
                let obj = match value.as_object() {
                    Some(o) => o,
                    None => return false,
                };

                // All required fields must be present
                let has_required = required.iter().all(|r| obj.contains_key(r));

                // Present fields must have correct types
                let types_ok = fields
                    .iter()
                    .all(|(name, vt)| obj.get(name).map(|v| vt.matches(v)).unwrap_or(true));

                has_required && types_ok
            }
        }
    }
}

/// Spec for a widget event.
#[derive(Debug, Clone)]
pub struct EventSpec {
    /// Event family name (e.g., "slide", "click", "change").
    pub family: String,
    /// Expected payload shape.
    pub payload: PayloadSpec,
}

/// Spec for a widget command.
#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// Command family name (e.g., "set_value", "reset").
    pub family: String,
    /// Expected payload shape.
    pub payload: PayloadSpec,
}

/// Trait for types that can encode widget commands for the wire.
///
/// Mirrors [`crate::types::WidgetEventEncode`] for commands.
/// Typically derived via `#[derive(WidgetCommand)]`.
pub trait WidgetCommandEncode {
    /// Encode this command to its wire representation.
    ///
    /// Returns `(family, value)`.
    fn to_wire(&self) -> (&'static str, crate::protocol::PropValue);

    /// Return the specs for all command variants.
    fn command_specs() -> Vec<CommandSpec>
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn none_spec_validates_null() {
        assert!(PayloadSpec::None.validate(&Value::Null));
        assert!(!PayloadSpec::None.validate(&json!("hello")));
        assert!(!PayloadSpec::None.validate(&json!(42)));
    }

    #[test]
    fn value_spec_validates_type() {
        let spec = PayloadSpec::Value(ValueType::Float);
        assert!(spec.validate(&json!(2.5)));
        assert!(!spec.validate(&json!("hello")));
        assert!(!spec.validate(&Value::Null));

        let spec = PayloadSpec::Value(ValueType::String);
        assert!(spec.validate(&json!("hello")));
        assert!(!spec.validate(&json!(42)));

        let spec = PayloadSpec::Value(ValueType::Bool);
        assert!(spec.validate(&json!(true)));
        assert!(!spec.validate(&json!("true")));

        let spec = PayloadSpec::Value(ValueType::Any);
        assert!(spec.validate(&json!(42)));
        assert!(spec.validate(&json!("hello")));
        assert!(spec.validate(&Value::Null));
    }

    #[test]
    fn fields_spec_validates_structure() {
        let spec = PayloadSpec::Fields {
            fields: vec![
                ("x".into(), ValueType::Float),
                ("y".into(), ValueType::Float),
            ],
            required: vec!["x".into(), "y".into()],
        };

        assert!(spec.validate(&json!({"x": 1.0, "y": 2.0})));
        assert!(spec.validate(&json!({"x": 1.0, "y": 2.0, "extra": true})));
        assert!(!spec.validate(&json!({"x": 1.0})));
        assert!(!spec.validate(&json!({"x": "wrong", "y": 2.0})));
        assert!(!spec.validate(&json!("not an object")));
    }

    #[test]
    fn fields_spec_optional_fields() {
        let spec = PayloadSpec::Fields {
            fields: vec![
                ("x".into(), ValueType::Float),
                ("label".into(), ValueType::String),
            ],
            required: vec!["x".into()],
        };

        assert!(spec.validate(&json!({"x": 1.0})));
        assert!(spec.validate(&json!({"x": 1.0, "label": "hello"})));
        assert!(!spec.validate(&json!({"label": "hello"})));
    }

    #[test]
    fn integer_type_matches_both_signed_and_unsigned() {
        let spec = PayloadSpec::Value(ValueType::Integer);
        assert!(spec.validate(&json!(42)));
        assert!(spec.validate(&json!(-5)));
        assert!(!spec.validate(&json!(2.5)));
        assert!(!spec.validate(&json!("42")));
    }
}
