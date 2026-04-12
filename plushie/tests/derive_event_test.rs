//! Integration tests for the WidgetEvent and WidgetCommand derive macros.
//!
//! These tests verify that the generated impls produce the correct
//! wire format and specs with real plushie-core types.

use plushie::{WidgetCommand, WidgetEvent};
use plushie_core::protocol::PropValue;
use plushie_core::spec::{PayloadSpec, ValueType, WidgetCommandEncode};
use plushie_core::types::WidgetEventEncode;

// ---------------------------------------------------------------------------
// Unit variant
// ---------------------------------------------------------------------------

#[derive(WidgetEvent)]
enum UnitEvent {
    Cleared,
    Reset,
}

#[test]
fn unit_variant_produces_null() {
    let (family, value) = UnitEvent::Cleared.to_wire();
    assert_eq!(family, "cleared");
    assert_eq!(value, PropValue::Null);
}

#[test]
fn unit_variant_snake_case() {
    let (family, _) = UnitEvent::Reset.to_wire();
    assert_eq!(family, "reset");
}

// ---------------------------------------------------------------------------
// Single-field tuple variants
// ---------------------------------------------------------------------------

#[derive(WidgetEvent)]
enum TupleEvent {
    Select(u64),
    HoverChanged(bool),
    Renamed(String),
    Scaled(f32),
}

#[test]
fn tuple_u64() {
    let (family, value) = TupleEvent::Select(42).to_wire();
    assert_eq!(family, "select");
    assert_eq!(value, PropValue::U64(42));
}

#[test]
fn tuple_bool() {
    let (family, value) = TupleEvent::HoverChanged(true).to_wire();
    assert_eq!(family, "hover_changed");
    assert_eq!(value, PropValue::Bool(true));
}

#[test]
fn tuple_string() {
    let (family, value) = TupleEvent::Renamed("new_name".to_string()).to_wire();
    assert_eq!(family, "renamed");
    assert_eq!(value, PropValue::Str("new_name".to_string()));
}

#[test]
fn tuple_f32() {
    let (family, value) = TupleEvent::Scaled(2.5).to_wire();
    assert_eq!(family, "scaled");
    assert_eq!(value, PropValue::F64(2.5));
}

// ---------------------------------------------------------------------------
// Named-field (struct) variants
// ---------------------------------------------------------------------------

#[derive(WidgetEvent)]
enum StructEvent {
    Change { x: f32, y: f32 },
}

#[test]
fn struct_variant_produces_object() {
    let (family, value) = StructEvent::Change { x: 1.0, y: 2.5 }.to_wire();
    assert_eq!(family, "change");

    let map = match &value {
        PropValue::Object(m) => m,
        other => panic!("expected Object, got {other:?}"),
    };
    assert_eq!(map.get("x").and_then(PropValue::as_f64), Some(1.0));
    assert_eq!(map.get("y").and_then(PropValue::as_f64), Some(2.5));
}

// ---------------------------------------------------------------------------
// Mixed variants in a single enum
// ---------------------------------------------------------------------------

#[derive(WidgetEvent)]
enum MixedEvent {
    Select(u64),
    Move { x: f32, y: f32 },
    Cleared,
}

#[test]
fn mixed_enum_all_forms() {
    let (f1, v1) = MixedEvent::Select(7).to_wire();
    assert_eq!(f1, "select");
    assert_eq!(v1, PropValue::U64(7));

    let (f2, v2) = MixedEvent::Move { x: 0.0, y: 0.0 }.to_wire();
    assert_eq!(f2, "move");
    assert!(matches!(v2, PropValue::Object(_)));

    let (f3, v3) = MixedEvent::Cleared.to_wire();
    assert_eq!(f3, "cleared");
    assert_eq!(v3, PropValue::Null);
}

// ---------------------------------------------------------------------------
// PascalCase to snake_case edge cases
// ---------------------------------------------------------------------------

#[derive(WidgetEvent)]
enum CaseEvent {
    DragEnd,
    OptionHovered,
    HTMLLoaded,
}

#[test]
fn pascal_case_conversion() {
    assert_eq!(CaseEvent::DragEnd.to_wire().0, "drag_end");
    assert_eq!(CaseEvent::OptionHovered.to_wire().0, "option_hovered");
    assert_eq!(CaseEvent::HTMLLoaded.to_wire().0, "html_loaded");
}

// ---------------------------------------------------------------------------
// Round-trip through serde_json::Value (what EventResult::emit_event does)
// ---------------------------------------------------------------------------

#[test]
fn value_round_trip() {
    let (_, prop_value) = TupleEvent::Select(99).to_wire();
    let json_value = serde_json::Value::from(prop_value);
    assert_eq!(json_value, serde_json::json!(99));
}

#[test]
fn struct_value_round_trip() {
    let (_, prop_value) = StructEvent::Change { x: 3.0, y: 4.0 }.to_wire();
    let json_value = serde_json::Value::from(prop_value);
    let obj = json_value.as_object().unwrap();
    assert_eq!(obj.get("x").and_then(|v| v.as_f64()), Some(3.0));
    assert_eq!(obj.get("y").and_then(|v| v.as_f64()), Some(4.0));
}

// ---------------------------------------------------------------------------
// Event specs generation
// ---------------------------------------------------------------------------

#[test]
fn event_specs_generated_for_all_variants() {
    let specs = UnitEvent::event_specs();
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].family, "cleared");
    assert!(matches!(specs[0].payload, PayloadSpec::None));
    assert_eq!(specs[1].family, "reset");
}

#[test]
fn event_spec_tuple_variant_has_value_type() {
    let specs = TupleEvent::event_specs();
    assert_eq!(specs[0].family, "select");
    match &specs[0].payload {
        PayloadSpec::Value(vt) => assert_eq!(*vt, ValueType::Integer),
        other => panic!("expected Value, got {other:?}"),
    }
}

#[test]
fn event_spec_struct_variant_has_fields() {
    let specs = StructEvent::event_specs();
    assert_eq!(specs[0].family, "change");
    match &specs[0].payload {
        PayloadSpec::Fields { fields, required } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "x");
            assert_eq!(fields[0].1, ValueType::Float);
            assert_eq!(fields[1].0, "y");
            assert_eq!(required, &["x", "y"]);
        }
        other => panic!("expected Fields, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// WidgetCommand derive
// ---------------------------------------------------------------------------

#[derive(WidgetCommand)]
enum TestCommand {
    SetValue(f32),
    Reset,
    SetRange { min: f32, max: f32 },
}

#[test]
fn command_unit_variant() {
    let (op, value) = TestCommand::Reset.to_wire();
    assert_eq!(op, "reset");
    assert_eq!(value, PropValue::Null);
}

#[test]
fn command_tuple_variant() {
    let (op, value) = TestCommand::SetValue(72.0).to_wire();
    assert_eq!(op, "set_value");
    let json = serde_json::Value::from(value);
    assert_eq!(json.as_f64(), Some(72.0));
}

#[test]
fn command_struct_variant() {
    let (op, value) = TestCommand::SetRange {
        min: 0.0,
        max: 100.0,
    }
    .to_wire();
    assert_eq!(op, "set_range");
    let json = serde_json::Value::from(value);
    let obj = json.as_object().unwrap();
    assert_eq!(obj.get("min").and_then(|v| v.as_f64()), Some(0.0));
    assert_eq!(obj.get("max").and_then(|v| v.as_f64()), Some(100.0));
}

#[test]
fn command_specs_generated() {
    let specs = TestCommand::command_specs();
    assert_eq!(specs.len(), 3);

    assert_eq!(specs[0].op, "set_value");
    assert!(matches!(
        specs[0].payload,
        PayloadSpec::Value(ValueType::Float)
    ));

    assert_eq!(specs[1].op, "reset");
    assert!(matches!(specs[1].payload, PayloadSpec::None));

    assert_eq!(specs[2].op, "set_range");
    match &specs[2].payload {
        PayloadSpec::Fields { fields, required } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(required.len(), 2);
        }
        other => panic!("expected Fields, got {other:?}"),
    }
}

#[test]
fn command_spec_validates_correct_payload() {
    let specs = TestCommand::command_specs();
    let set_value_spec = &specs[0];
    assert!(set_value_spec.payload.validate(&serde_json::json!(72.0)));
    assert!(!set_value_spec.payload.validate(&serde_json::json!("wrong")));

    let reset_spec = &specs[1];
    assert!(reset_spec.payload.validate(&serde_json::Value::Null));
    assert!(!reset_spec.payload.validate(&serde_json::json!(42)));
}
