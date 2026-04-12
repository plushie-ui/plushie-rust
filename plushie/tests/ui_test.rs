//! Tests for UI view builders.
//!
//! Each test constructs a view using the builder API and verifies
//! the resulting View contains the correct type, props, and children.

use plushie::prelude::*;
use plushie::types::Direction;
use plushie::View;
use serde_json::Value;

/// Extract the JSON value from a View for inspection.
fn view_json(v: impl Into<View>) -> Value {
    let view: View = v.into();
    serde_json::to_value(&view).unwrap()
}

fn get_type(v: &Value) -> &str {
    v["type"].as_str().unwrap_or("")
}

fn get_id(v: &Value) -> &str {
    v["id"].as_str().unwrap_or("")
}

fn get_prop<'a>(v: &'a Value, key: &str) -> &'a Value {
    &v["props"][key]
}

fn child_count(v: &Value) -> usize {
    v["children"].as_array().map(|a| a.len()).unwrap_or(0)
}

fn child_at(v: &Value, idx: usize) -> &Value {
    &v["children"][idx]
}

// ---------------------------------------------------------------------------
// Layout builders
// ---------------------------------------------------------------------------

#[test]
fn window_has_type_and_id() {
    let v = view_json(window("main"));
    assert_eq!(get_type(&v), "window");
    assert_eq!(get_id(&v), "main");
}

#[test]
fn window_with_title_and_child() {
    let v = view_json(
        window("main").title("My App").child(text("Hello"))
    );
    assert_eq!(get_prop(&v, "title").as_str(), Some("My App"));
    assert_eq!(child_count(&v), 1);
    assert_eq!(get_type(child_at(&v, 0)), "text");
}

#[test]
fn column_auto_id_starts_with_auto() {
    let v = view_json(column());
    assert!(get_id(&v).starts_with("auto:"));
    assert_eq!(get_type(&v), "column");
}

#[test]
fn column_explicit_id() {
    let v = view_json(column().id("form"));
    assert_eq!(get_id(&v), "form");
}

#[test]
fn column_with_spacing_and_children() {
    let v = view_json(
        column().spacing(8.0).children([
            text("A"),
            text("B"),
            text("C"),
        ])
    );
    assert_eq!(get_prop(&v, "spacing"), &serde_json::json!(8.0));
    assert_eq!(child_count(&v), 3);
}

#[test]
fn row_auto_id() {
    let v = view_json(row());
    assert!(get_id(&v).starts_with("auto:"));
    assert_eq!(get_type(&v), "row");
}

#[test]
fn container_with_padding_and_child() {
    let v = view_json(
        container().padding(16).child(text("content"))
    );
    assert_eq!(get_type(&v), "container");
    assert_eq!(get_prop(&v, "padding"), &serde_json::json!(16.0));
    assert_eq!(child_count(&v), 1);
}

#[test]
fn stack_with_children() {
    let v = view_json(
        stack().children([text("back"), text("front")])
    );
    assert_eq!(get_type(&v), "stack");
    assert_eq!(child_count(&v), 2);
}

#[test]
fn scrollable_with_direction() {
    let v = view_json(scrollable().direction(Direction::Horizontal));
    assert_eq!(get_type(&v), "scrollable");
    assert_eq!(get_prop(&v, "direction").as_str(), Some("horizontal"));
}

#[test]
fn pane_grid_requires_id() {
    let v = view_json(pane_grid("editor"));
    assert_eq!(get_id(&v), "editor");
    assert_eq!(get_type(&v), "pane_grid");
}

// ---------------------------------------------------------------------------
// Display builders
// ---------------------------------------------------------------------------

#[test]
fn text_has_content_and_auto_id() {
    let v = view_json(text("Hello, world!"));
    assert_eq!(get_type(&v), "text");
    assert_eq!(get_prop(&v, "content").as_str(), Some("Hello, world!"));
    assert!(get_id(&v).starts_with("auto:"));
}

#[test]
fn text_with_explicit_id() {
    let v = view_json(text("Hi").id("greeting"));
    assert_eq!(get_id(&v), "greeting");
}

#[test]
fn text_with_size_and_color() {
    let v = view_json(
        text("Error").size(24.0).color(Color::red())
    );
    assert_eq!(get_prop(&v, "size"), &serde_json::json!(24.0));
    assert_eq!(get_prop(&v, "color").as_str(), Some("#ff0000"));
}

#[test]
fn space_is_minimal() {
    let v = view_json(space());
    assert_eq!(get_type(&v), "space");
    assert!(get_id(&v).starts_with("auto:"));
}

#[test]
fn image_with_source() {
    let v = view_json(image("logo.png"));
    assert_eq!(get_type(&v), "image");
    assert_eq!(get_prop(&v, "source").as_str(), Some("logo.png"));
}

#[test]
fn progress_bar_with_range_and_value() {
    let v = view_json(progress_bar((0.0, 100.0), 50.0));
    assert_eq!(get_type(&v), "progress_bar");
    assert_eq!(get_prop(&v, "value"), &serde_json::json!(50.0));
}

#[test]
fn markdown_with_content() {
    let v = view_json(markdown("# Hello"));
    assert_eq!(get_type(&v), "markdown");
    assert_eq!(get_prop(&v, "content").as_str(), Some("# Hello"));
}

// ---------------------------------------------------------------------------
// Input builders
// ---------------------------------------------------------------------------

#[test]
fn button_requires_id_and_label() {
    let v = view_json(button("save", "Save"));
    assert_eq!(get_type(&v), "button");
    assert_eq!(get_id(&v), "save");
    assert_eq!(get_prop(&v, "label").as_str(), Some("Save"));
}

#[test]
fn button_with_style() {
    let v = view_json(button("ok", "OK").style(Style::primary()));
    assert_eq!(get_prop(&v, "style").as_str(), Some("primary"));
}

#[test]
fn text_input_with_placeholder() {
    let v = view_json(
        text_input("email", "user@example.com").placeholder("Enter email")
    );
    assert_eq!(get_type(&v), "text_input");
    assert_eq!(get_id(&v), "email");
    assert_eq!(get_prop(&v, "value").as_str(), Some("user@example.com"));
    assert_eq!(get_prop(&v, "placeholder").as_str(), Some("Enter email"));
}

#[test]
fn checkbox_with_label() {
    let v = view_json(checkbox("agree", true).label("I agree"));
    assert_eq!(get_type(&v), "checkbox");
    assert_eq!(get_id(&v), "agree");
    assert_eq!(get_prop(&v, "checked"), &serde_json::json!(true));
    assert_eq!(get_prop(&v, "label").as_str(), Some("I agree"));
}

#[test]
fn slider_with_range_and_value() {
    let v = view_json(slider("vol", (0.0, 100.0), 75.0));
    assert_eq!(get_type(&v), "slider");
    assert_eq!(get_id(&v), "vol");
    assert_eq!(get_prop(&v, "value"), &serde_json::json!(75.0));
}

#[test]
fn pick_list_with_options() {
    let v = view_json(pick_list("lang", &["Rust", "Elixir"], Some("Rust")));
    assert_eq!(get_type(&v), "pick_list");
    assert_eq!(get_id(&v), "lang");
    assert_eq!(get_prop(&v, "selected").as_str(), Some("Rust"));
}

// ---------------------------------------------------------------------------
// Interactive builders
// ---------------------------------------------------------------------------

#[test]
fn tooltip_with_tip_and_child() {
    let v = view_json(
        tooltip("tip", "Click to save").child(button("save", "Save"))
    );
    assert_eq!(get_type(&v), "tooltip");
    assert_eq!(get_prop(&v, "tip").as_str(), Some("Click to save"));
    assert_eq!(child_count(&v), 1);
}

#[test]
fn pointer_area_with_child() {
    let v = view_json(
        pointer_area("area").on_press("click").child(text("hover me"))
    );
    assert_eq!(get_type(&v), "pointer_area");
    assert_eq!(get_prop(&v, "on_press"), &serde_json::json!("click"));
}

// ---------------------------------------------------------------------------
// Composition patterns
// ---------------------------------------------------------------------------

#[test]
fn nested_layout_produces_tree() {
    let v = view_json(
        window("main").title("App").child(
            column().spacing(8.0).padding(16)
                .child(text("Title").size(24.0))
                .child(row().spacing(4.0).children([
                    button("ok", "OK").style(Style::primary()),
                    button("cancel", "Cancel"),
                ]))
        )
    );

    assert_eq!(get_type(&v), "window");
    let col = child_at(&v, 0);
    assert_eq!(get_type(col), "column");
    assert_eq!(child_count(col), 2);

    let title = child_at(col, 0);
    assert_eq!(get_type(title), "text");
    assert_eq!(get_prop(title, "content").as_str(), Some("Title"));

    let row = child_at(col, 1);
    assert_eq!(get_type(row), "row");
    assert_eq!(child_count(row), 2);
    assert_eq!(get_type(child_at(row, 0)), "button");
    assert_eq!(get_type(child_at(row, 1)), "button");
}

#[test]
fn dynamic_list_with_iterator() {
    let items = vec!["Alice", "Bob", "Carol"];
    let v = view_json(
        column().children(
            items.iter().map(|name| text(*name))
        )
    );
    assert_eq!(child_count(&v), 3);
}

#[test]
fn conditional_child_with_option() {
    let show_error = true;
    let mut col = column()
        .child(text("Status: OK"));
    if show_error {
        col = col.child(text("Error details here"));
    }
    let v = view_json(col);
    assert_eq!(child_count(&v), 2);
}
