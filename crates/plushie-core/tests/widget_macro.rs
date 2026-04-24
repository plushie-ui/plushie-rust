//! End-to-end test for the `widget!` function-like macro.
//!
//! Exercises the macro as a widget author would: invoke it, call the
//! generated builder, turn the value into a [`TreeNode`] via [`From`],
//! and inspect the resulting props + type_name.

use plushie_core::protocol::{PropValue, TreeNode};
use plushie_core::types::Color;
use plushie_core::{FromNode, PlushieType, WidgetEventEncode, WidgetProps, widget};

widget! {
    /// A gauge widget used only in tests.
    #[widget(type_name = "test_gauge", crate = "test-gauge")]
    pub struct Gauge {
        /// Current value.
        pub value: f32,
        /// Display color.
        pub color: Color,
    }

    events {
        ValueChanged(f32),
        Cleared,
    }
}

#[derive(WidgetProps)]
#[widget(name = "test_gauge")]
#[allow(dead_code)]
struct GaugePropsSource {
    pub value: f32,
    pub color: Color,
}

#[test]
fn builder_sets_typed_props() {
    let g = Gauge::new("g1")
        .value(0.42)
        .color(Color::rgb(0.1, 0.2, 0.3));

    assert_eq!(g.id, "g1");
    assert_eq!(g.value, Some(0.42));
    assert!(g.color.is_some());
}

#[test]
fn from_widget_produces_tree_node() {
    let g = Gauge::new("gauge-a").value(0.75);
    let node: TreeNode = g.into();

    assert_eq!(node.id, "gauge-a");
    assert_eq!(node.type_name, "test_gauge");
    assert!(node.children.is_empty());

    // Only Some() fields flow into props.
    let v = node.props.get("value").cloned();
    assert!(matches!(v, Some(PropValue::F64(_))));
    assert!(node.props.get("color").is_none());
}

#[test]
fn type_name_constant() {
    assert_eq!(Gauge::type_name(), "test_gauge");
}

#[test]
fn metadata_const_is_valid_json() {
    let parsed: serde_json::Value =
        serde_json::from_str(PLUSHIE_WIDGET_METADATA).expect("metadata parses as JSON");
    assert_eq!(parsed["type_name"], "test_gauge");
    assert_eq!(parsed["crate"], "test-gauge");
    assert_eq!(parsed["struct"], "Gauge");
    // The constructor now lives only in Cargo.toml, not in the macro
    // output, so the emitted metadata does not carry it.
    assert!(parsed.get("constructor").is_none());
}

#[test]
fn events_block_generates_enum() {
    // The events block expands to a `GaugeEvent` enum with the
    // WidgetEvent derive applied. Build a variant and round-trip it
    // through the wire encoder.
    let (family, payload) = GaugeEvent::ValueChanged(1.5).to_wire();
    assert_eq!(family, "value_changed");
    assert!(matches!(payload, PropValue::F64(_)));

    let (family, payload) = GaugeEvent::Cleared.to_wire();
    assert_eq!(family, "cleared");
    assert!(matches!(payload, PropValue::Null));
}

#[test]
fn crate_root_exports_macro_support_surface() {
    assert!(matches!(
        <f32 as PlushieType>::wire_encode(&0.5),
        PropValue::F64(_)
    ));

    let node: TreeNode = Gauge::new("gauge-props")
        .value(0.25)
        .color(Color::rgb(0.4, 0.5, 0.6))
        .into();

    let props = <GaugePropsSourceProps as FromNode>::from_node(&node);
    assert_eq!(props.value, Some(0.25));
    assert!(props.color.is_some());
}
