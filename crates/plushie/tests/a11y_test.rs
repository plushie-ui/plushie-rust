//! Integration tests for accessibility behaviour exposed through
//! TestSession.
//!
//! These tests pin the SDK-level a11y contract:
//!
//! - The normalizer auto-populates `a11y.role` from the widget type
//!   when the author doesn't set one.
//! - `resolved_a11y` composes explicit + inferred fields so
//!   `placeholder` survives even when `a11y.label` is set explicitly.
//! - Image/SVG `alt` flows into `a11y.label` via the widget-sdk
//!   fallback when the author didn't set a label.
//! - Author overrides win per field (precedence pin).
//! - Ctrl+Tab escapes a focus-capturing widget (fork behaviour
//!   visible from the SDK).

use plushie::prelude::*;
use plushie::test::TestSession;
use plushie_core::types::PlushieType;

// ---------------------------------------------------------------------------
// Test app: a couple of widgets covering the merge + infer paths
// ---------------------------------------------------------------------------

struct A11yHarness;

impl App for A11yHarness {
    type Model = Self;

    fn init() -> (Self, Command) {
        (A11yHarness, Command::none())
    }

    fn update(_model: &mut Self, _event: Event) -> Command {
        Command::none()
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> Option<View> {
        Some(
            window("main")
                .child(
                    column()
                        // text_input with placeholder + explicit a11y.label.
                        // Precedence: explicit label wins, inferred
                        // description (from placeholder) is preserved.
                        .child(
                            text_input("search", "")
                                .placeholder("Search...")
                                .a11y(&A11y::new().label("Global search")),
                        )
                        // text_editor with placeholder only. Description
                        // should come from inference.
                        .child(text_editor("notes", "").placeholder("Write here..."))
                        // image with both alt and explicit a11y.label.
                        // Precedence pin: explicit label wins over alt.
                        .child(
                            image("logo.png")
                                .id("photo")
                                .alt("Auto alt")
                                .a11y(&A11y::new().label("Explicit label")),
                        )
                        // image with alt only. Label should be inferred.
                        .child(image("icon.png").id("icon").alt("Settings icon")),
                )
                .into(),
        )
    }
}

#[test]
fn normalizer_auto_populates_role_for_text_input() {
    let session = TestSession::<A11yHarness>::start();
    let a11y = session
        .resolved_a11y("search")
        .expect("text_input should have resolved a11y");
    assert_eq!(
        a11y.role,
        Some(plushie_core::types::a11y::Role::TextInput),
        "normalizer should auto-populate role on built-in widgets"
    );
}

#[test]
fn text_input_placeholder_flows_to_description_via_inference() {
    let session = TestSession::<A11yHarness>::start();
    let a11y = session.resolved_a11y("notes").unwrap();
    assert_eq!(
        a11y.description.as_deref(),
        Some("Write here..."),
        "placeholder should flow to a11y.description"
    );
}

#[test]
fn explicit_label_and_inferred_description_coexist_after_merge() {
    // Before the merge fix, setting a11y.label on a text_input would
    // silently discard the placeholder-derived description. This test
    // pins the fix.
    let session = TestSession::<A11yHarness>::start();
    let a11y = session.resolved_a11y("search").unwrap();
    assert_eq!(a11y.label.as_deref(), Some("Global search"));
    assert_eq!(a11y.description.as_deref(), Some("Search..."));
}

#[test]
fn image_alt_flows_to_label_when_unset() {
    let session = TestSession::<A11yHarness>::start();
    let a11y = session.resolved_a11y("icon").unwrap();
    assert_eq!(
        a11y.label.as_deref(),
        Some("Settings icon"),
        "image alt should flow to a11y.label"
    );
}

#[test]
fn explicit_a11y_label_wins_over_image_alt() {
    // Precedence pin: author's explicit a11y.label takes precedence
    // over the alt-derived default.
    let session = TestSession::<A11yHarness>::start();
    let a11y = session.resolved_a11y("photo").unwrap();
    assert_eq!(a11y.label.as_deref(), Some("Explicit label"));
}

#[test]
fn assert_a11y_matches_resolved_values() {
    let session = TestSession::<A11yHarness>::start();
    // Both role (from normalizer) and description (from inference) are
    // visible to assert_a11y even though neither appears on the raw
    // author-provided a11y prop.
    session.assert_a11y(
        "notes",
        &serde_json::json!({
            "role": "multiline_text_input",
            "description": "Write here...",
        }),
    );
}

// ---------------------------------------------------------------------------
// resolved_a11y encoded round-trips through the core A11y type
// ---------------------------------------------------------------------------

#[test]
fn resolved_a11y_round_trips_via_wire() {
    let session = TestSession::<A11yHarness>::start();
    let a11y = session.resolved_a11y("search").unwrap();
    let wire = serde_json::Value::from(a11y.wire_encode());
    let decoded = plushie_core::types::a11y::A11y::wire_decode(&wire).expect("should decode back");
    assert_eq!(decoded.label.as_deref(), Some("Global search"));
    assert_eq!(decoded.description.as_deref(), Some("Search..."));
}

// ---------------------------------------------------------------------------
// Ctrl+Tab escape coverage
// ---------------------------------------------------------------------------
//
// text_editor captures Tab to insert a literal tab. Ctrl+Tab escapes
// out of the widget so keyboard users aren't trapped. The fork
// implements this; this test pins the SDK's ability to drive both
// key presses through TestSession so a fork-level regression surfaces
// at the SDK boundary.

struct TabHarness {
    notes: String,
}

impl App for TabHarness {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            TabHarness {
                notes: String::new(),
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Some(Input("notes", text)) = event.widget_match() {
            model.notes = text.to_string();
        }
        Command::none()
    }

    fn view(_model: &Self, _widgets: &mut WidgetRegistrar) -> Option<View> {
        Some(
            window("main")
                .child(
                    column()
                        .child(text_editor("notes", "").placeholder("Write here..."))
                        .child(button("ok", "OK")),
                )
                .into(),
        )
    }
}

#[test]
fn tab_and_ctrl_tab_dispatch_cleanly_through_test_session() {
    // The fork captures Tab inside text_editor (so it inserts a tab
    // character) and lets Ctrl+Tab escape out to the next focusable
    // widget. TestSession can't observe the fork's focus tracker
    // directly, but it must be able to dispatch both KeyPresses
    // without panic and without mutating unrelated state. This is the
    // SDK-level anchor point; cross-testing against the real fork's
    // focus tracker is deferred to the rendering tests.
    let mut session = TestSession::<TabHarness>::start();
    session.press("Tab");
    session.press("Ctrl+Tab");
    assert_eq!(
        session.model().notes,
        "",
        "TestSession synthesises key events; text_editor content is \
         driven by Input events, not raw Tab presses, so the model \
         stays untouched"
    );
}
