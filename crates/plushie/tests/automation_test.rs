//! Tests for the automation module: Selector and Element.

mod common;

use common::{a11y_node, button_node, container_node, text_node, window_node};
use plushie::automation::{Element, file, runner};
use plushie::prelude::*;
use plushie::test::TestSession;
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
        props: Props::from_json(serde_json::Value::Object(props)),
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

// ---------------------------------------------------------------------------
// Script runner failures and captures
// ---------------------------------------------------------------------------

struct AutomationCounter {
    count: i32,
    viewport: Option<(f32, f32)>,
}

impl App for AutomationCounter {
    type Model = Self;

    fn init() -> (Self::Model, Command) {
        (
            Self {
                count: 0,
                viewport: None,
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self::Model, event: Event) -> Command {
        match event {
            Event::Widget(widget) if widget.scoped_id.id == "inc" => {
                model.count += 1;
            }
            Event::Window(window) => {
                if let (Some(width), Some(height)) = (window.width, window.height) {
                    model.viewport = Some((width, height));
                }
            }
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self::Model, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .child(
                column()
                    .child(text(&format!("{}", model.count)).id("display"))
                    .child(button("inc", "+")),
            )
            .into()
    }
}

#[test]
fn missing_interaction_target_is_line_failure() {
    let script = file::parse("app: Counter\n-----\nclick \"missing\"\n").unwrap();
    let mut session = TestSession::<AutomationCounter>::start().allow_diagnostics();

    let result = runner::run(&script, &mut session);

    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.failures[0].0, 3);
    assert!(result.failures[0].1.contains("target not found: missing"));
}

#[test]
fn tree_hash_instruction_records_capture() {
    let script = file::parse("app: Counter\n-----\ntree_hash \"after_init\"\n").unwrap();
    let mut session = TestSession::<AutomationCounter>::start().allow_diagnostics();
    let expected = session.tree_hash();

    let result = runner::run(&script, &mut session);

    assert!(result.is_ok(), "got failures: {:?}", result.failures);
    assert_eq!(result.captures.len(), 1);
    assert_eq!(result.captures[0].kind, "tree_hash");
    assert_eq!(result.captures[0].name, "after_init");
    assert_eq!(result.captures[0].value, expected);
}

#[test]
fn screenshot_instruction_is_explicitly_unsupported_in_test_session_runner() {
    let script = file::parse("app: Counter\n-----\nscreenshot \"snap\"\n").unwrap();
    let mut session = TestSession::<AutomationCounter>::start().allow_diagnostics();

    let result = runner::run(&script, &mut session);

    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.failures[0].0, 3);
    assert!(result.failures[0].1.contains("unsupported"));
}

#[test]
fn explicit_viewport_header_is_applied_as_resize_event() {
    let script =
        file::parse("app: Counter\nviewport: 320x240\n-----\nassert_exists \"display\"\n").unwrap();
    let mut session = TestSession::<AutomationCounter>::start().allow_diagnostics();

    let result = runner::run(&script, &mut session);

    assert!(result.is_ok(), "got failures: {:?}", result.failures);
    assert_eq!(session.model().viewport, Some((320.0, 240.0)));
}
