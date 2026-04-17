//! Tests for the automation module: Selector and Element.

mod common;

use common::{a11y_node, button_node, container_node, text_node, window_node};
use plushie::automation::Element;
use plushie_core::Selector;
use plushie_core::protocol::{Props, TreeNode};

// ---------------------------------------------------------------------------
// Selector::find by ID
// ---------------------------------------------------------------------------

#[test]
fn find_by_exact_id() {
    let tree = container_node("root", vec![text_node("greeting", "Hello")]);
    assert!(Selector::id("greeting").find(&tree).is_some());
    assert!(Selector::id("missing").find(&tree).is_none());
}

#[test]
fn find_by_local_name_in_scoped_tree() {
    let tree = window_node(
        "main",
        vec![container_node(
            "main#app",
            vec![button_node("main#app/save", "Save")],
        )],
    );
    // Find by local name
    let node = Selector::id("save").find(&tree);
    assert!(node.is_some());
    assert_eq!(node.unwrap().id, "main#app/save");
}

#[test]
fn find_by_full_scoped_id() {
    let tree = window_node(
        "main",
        vec![container_node(
            "main#app",
            vec![button_node("main#app/save", "Save")],
        )],
    );
    assert!(Selector::id("main#app/save").find(&tree).is_some());
}

// ---------------------------------------------------------------------------
// Selector::find by text, role, label
// ---------------------------------------------------------------------------

#[test]
fn find_by_text() {
    let tree = container_node(
        "root",
        vec![text_node("a", "Hello"), text_node("b", "World")],
    );
    let found = Selector::text("World").find(&tree);
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "b");
}

#[test]
fn find_by_role() {
    let tree = container_node(
        "root",
        vec![
            a11y_node("heading", "text", "heading"),
            text_node("body", "content"),
        ],
    );
    let found = Selector::role("heading").find(&tree);
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "heading");
}

#[test]
fn find_by_role_falls_back_to_type_name() {
    let tree = container_node("root", vec![button_node("btn", "Click")]);
    // No explicit a11y.role, so matches type_name "button"
    assert!(Selector::role("button").find(&tree).is_some());
}

#[test]
fn find_by_label() {
    let tree = container_node("root", vec![button_node("save", "Save Document")]);
    let found = Selector::label("Save Document").find(&tree);
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "save");
}

#[test]
fn find_all_by_role() {
    let tree = container_node(
        "root",
        vec![
            button_node("a", "First"),
            text_node("t", "text"),
            button_node("b", "Second"),
        ],
    );
    let found = Selector::role("button").find_all(&tree);
    assert_eq!(found.len(), 2);
}

// ---------------------------------------------------------------------------
// Selector::find with window scoping
// ---------------------------------------------------------------------------

#[test]
fn find_with_window_scope() {
    let tree = container_node(
        "root",
        vec![
            window_node("main", vec![button_node("main#btn", "Main")]),
            window_node("dialog", vec![button_node("dialog#btn", "Dialog")]),
        ],
    );
    let found = Selector::id_in_window("main#btn", "main").find(&tree);
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "main#btn");
}

// ---------------------------------------------------------------------------
// Selector wire format
// ---------------------------------------------------------------------------

#[test]
fn selector_wire_round_trip() {
    let cases = vec![
        Selector::id("save"),
        Selector::id("main#form/save"),
        Selector::text("Save"),
        Selector::role("button"),
        Selector::label("Save document"),
        Selector::focused(),
    ];
    for sel in cases {
        let wire = sel.to_wire();
        let parsed = Selector::from_wire(&wire).expect("parse failed");
        assert_eq!(sel, parsed, "round-trip failed for {sel}");
    }
}

// ---------------------------------------------------------------------------
// Element accessors
// ---------------------------------------------------------------------------

#[test]
fn element_text_from_content() {
    let node = text_node("t", "Hello");
    let elem = Element::new(&node);
    assert_eq!(elem.text(), Some("Hello"));
    assert_eq!(elem.id(), "t");
    assert_eq!(elem.widget_type(), "text");
}

#[test]
fn element_text_from_label() {
    let node = button_node("b", "Save");
    let elem = Element::new(&node);
    assert_eq!(elem.text(), Some("Save"));
}

#[test]
fn element_inferred_role_from_a11y() {
    let node = a11y_node("h", "container", "heading");
    let elem = Element::new(&node);
    assert_eq!(elem.inferred_role(), "heading");
}

#[test]
fn element_inferred_role_from_type() {
    let node = button_node("b", "Click");
    let elem = Element::new(&node);
    assert_eq!(elem.inferred_role(), "button");
}

#[test]
fn element_children() {
    let tree = container_node("root", vec![text_node("a", "A"), text_node("b", "B")]);
    let elem = Element::new(&tree);
    let children = elem.children();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].id(), "a");
    assert_eq!(children[1].id(), "b");
}

#[test]
fn element_prop_accessors() {
    let mut props = serde_json::Map::new();
    props.insert("size".to_string(), serde_json::json!(24.0));
    props.insert("disabled".to_string(), serde_json::json!(true));
    props.insert("label".to_string(), serde_json::json!("test"));
    let node = TreeNode {
        id: "n".to_string(),
        type_name: "text".to_string(),
        props: Props::Wire(serde_json::Value::Object(props)),
        children: vec![],
    };
    let elem = Element::new(&node);
    assert_eq!(elem.prop_str("label"), Some("test"));
    assert_eq!(elem.prop_f32("size"), Some(24.0));
    assert!(elem.is_disabled());
}

// ---------------------------------------------------------------------------
// Selector Display
// ---------------------------------------------------------------------------

#[test]
fn selector_display() {
    assert_eq!(Selector::id("save").to_string(), "save");
    assert_eq!(Selector::id("main#save").to_string(), "main#save");
    // id_in_window shows window context even when widget_id lacks #
    assert_eq!(
        Selector::id_in_window("save", "main").to_string(),
        "main#save"
    );
    assert_eq!(Selector::text("Save").to_string(), "{text: \"Save\"}");
    assert_eq!(Selector::role("button").to_string(), "{role: button}");
    assert_eq!(Selector::focused().to_string(), "{focused}");
}
