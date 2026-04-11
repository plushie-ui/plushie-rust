//! Integration tests for the PlushieWidget derive macro.
//!
//! These tests verify that the generated Props struct and type_name
//! method work correctly with real plushie-core types.

#![allow(dead_code)] // derive test structs have fields read only via generated code

use plushie::PlushieWidget;
use plushie_core::protocol::{Props, TreeNode};
use plushie_core::types::Color;
use serde_json::json;

// ---------------------------------------------------------------------------
// Test widget with primitive types
// ---------------------------------------------------------------------------

#[derive(PlushieWidget)]
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
        props: Props::Wire(json!({"label": "hello", "size": 14.0, "visible": true})),
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
        props: Props::Wire(json!({"label": "partial"})),
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
        props: Props::Wire(json!({})),
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
        props: Props::Wire(json!({"label": 42, "size": "not a number", "visible": "yes"})),
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

#[derive(PlushieWidget)]
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
        props: Props::Wire(json!({
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
        props: Props::Wire(json!({"label": "debug_me"})),
        children: vec![],
    };

    let props = TestWidgetProps::from_node(&node);
    let debug = format!("{:?}", props);
    assert!(debug.contains("TestWidgetProps"));
    assert!(debug.contains("label"));
}
