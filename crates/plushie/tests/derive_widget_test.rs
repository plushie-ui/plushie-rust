//! Integration tests for the WidgetProps derive macro.
//!
//! These tests verify that the generated Props struct, type_name
//! method, and typed builder work correctly with real plushie-core types.

#![allow(dead_code)] // derive test structs have fields read only via generated code

use plushie::WidgetProps;
use plushie_core::protocol::{Props, TreeNode};
use plushie_core::types::Color;
use serde_json::json;

// ---------------------------------------------------------------------------
// Test widget with primitive types
// ---------------------------------------------------------------------------

#[derive(WidgetProps)]
#[widget(name = "test_widget")]
struct TestWidget {
    label: String,
    size: f32,
    visible: bool,
}

#[test]
fn type_name() {
    assert_eq!(TestWidget::type_name(), "test_widget");
}

#[test]
fn props_from_node_all_present() {
    let node = TreeNode {
        id: "w1".to_string(),
        type_name: "test_widget".to_string(),
        props: Props::from_json(json!({"label": "hello", "size": 14.0, "visible": true})),
        children: vec![],
    };

    let props = TestWidgetProps::from_node(&node);
    assert_eq!(props.label, Some("hello".to_string()));
    assert_eq!(props.size, Some(14.0));
    assert_eq!(props.visible, Some(true));
}

#[test]
fn props_from_node_partial() {
    let node = TreeNode {
        id: "w2".to_string(),
        type_name: "test_widget".to_string(),
        props: Props::from_json(json!({"label": "partial"})),
        children: vec![],
    };

    let props = TestWidgetProps::from_node(&node);
    assert_eq!(props.label, Some("partial".to_string()));
    assert_eq!(props.size, None);
    assert_eq!(props.visible, None);
}

#[test]
fn props_from_node_empty() {
    let node = TreeNode {
        id: "w3".to_string(),
        type_name: "test_widget".to_string(),
        props: Props::from_json(json!({})),
        children: vec![],
    };

    let props = TestWidgetProps::from_node(&node);
    assert_eq!(props.label, None);
    assert_eq!(props.size, None);
    assert_eq!(props.visible, None);
}

#[test]
fn props_from_node_type_mismatch() {
    let node = TreeNode {
        id: "w4".to_string(),
        type_name: "test_widget".to_string(),
        props: Props::from_json(json!({"label": 42, "size": "not a number", "visible": "yes"})),
        children: vec![],
    };

    let props = TestWidgetProps::from_node(&node);
    // Type mismatches result in None (extract returns None for wrong types)
    assert_eq!(props.label, None);
    assert_eq!(props.size, None);
    assert_eq!(props.visible, None);
}

// ---------------------------------------------------------------------------
// Test widget with complex types
// ---------------------------------------------------------------------------

#[derive(WidgetProps)]
#[widget(name = "color_box")]
struct ColorBox {
    /// The fill color.
    color: Color,
    opacity: f32,
    count: i32,
}

#[test]
fn complex_type_extraction() {
    let node = TreeNode {
        id: "cb1".to_string(),
        type_name: "color_box".to_string(),
        props: Props::from_json(json!({
            "color": "#ff0000",
            "opacity": 0.8,
            "count": 3
        })),
        children: vec![],
    };

    let props = ColorBoxProps::from_node(&node);
    assert!(props.color.is_some());
    assert_eq!(props.opacity, Some(0.8));
    assert_eq!(props.count, Some(3));
}

#[test]
fn color_box_type_name() {
    assert_eq!(ColorBox::type_name(), "color_box");
}

// ---------------------------------------------------------------------------
// Test Debug impl
// ---------------------------------------------------------------------------

#[test]
fn props_debug_format() {
    let node = TreeNode {
        id: "d1".to_string(),
        type_name: "test_widget".to_string(),
        props: Props::from_json(json!({"label": "debug_me"})),
        children: vec![],
    };

    let props = TestWidgetProps::from_node(&node);
    let debug = format!("{:?}", props);
    assert!(debug.contains("TestWidgetProps"));
    assert!(debug.contains("label"));
}

// ---------------------------------------------------------------------------
// Builder: typed construction
// ---------------------------------------------------------------------------

#[test]
fn builder_sets_id_and_type_name() {
    let b = TestWidget::builder("tw1");
    assert_eq!(b.0.id, "tw1");
    assert_eq!(b.0.type_name, "test_widget");
}

#[test]
fn builder_typed_setters() {
    let b = TestWidget::builder("tw2")
        .label("hello".to_string())
        .size(14.0)
        .visible(true);

    assert_eq!(b.0.props.get("label").unwrap().as_str(), Some("hello"));
    assert_eq!(b.0.props.get("size").unwrap().as_f64(), Some(14.0));
    assert_eq!(b.0.props.get("visible").unwrap().as_bool(), Some(true));
}

#[test]
fn builder_untyped_fallback() {
    let b = TestWidget::builder("tw3").prop("custom", "value");

    assert_eq!(b.0.props.get("custom").unwrap().as_str(), Some("value"));
}

#[test]
fn builder_complex_type() {
    let b = ColorBox::builder("cb2")
        .color(Color::red())
        .opacity(0.5)
        .count(7);

    // Color encodes as a hex string via PlushieType::wire_encode.
    assert_eq!(b.0.props.get("color").unwrap().as_str(), Some("#ff0000"));
    assert_eq!(b.0.props.get("opacity").unwrap().as_f64(), Some(0.5));
    // i32 encodes as I64 via PlushieType::wire_encode.
    assert_eq!(b.0.props.get("count").unwrap().as_i64(), Some(7));
}

#[test]
fn builder_roundtrip_through_props() {
    // Build with typed builder, then extract with generated Props.
    let b = TestWidget::builder("rt1")
        .label("roundtrip".to_string())
        .size(20.0)
        .visible(false);

    let node = TreeNode {
        id: b.0.id.clone(),
        type_name: b.0.type_name.to_string(),
        props: Props::from(b.0.props),
        children: vec![],
    };

    let props = TestWidgetProps::from_node(&node);
    assert_eq!(props.label, Some("roundtrip".to_string()));
    assert_eq!(props.size, Some(20.0));
    assert_eq!(props.visible, Some(false));
}
