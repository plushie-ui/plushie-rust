//! Integration tests for the WidgetEvent derive macro.
//!
//! These tests verify that the generated WidgetEventEncode impl
//! produces the correct wire format with real plushie-core types.

use plushie::WidgetEvent;
use plushie_core::protocol::{PropMap, PropValue};
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
