//! Widget gallery demonstrating common widget types.
//!
//! Showcases: text, button (with style variants), text_input,
//! checkbox, toggler, slider, pick_list, radio, progress_bar,
//! rule, and styled text.
//!
//! Run with: `cargo run -p plushie --example gallery`

use plushie::prelude::*;

#[derive(Clone)]
struct Gallery {
    input_value: String,
    submit_value: String,
    checked: bool,
    toggled: bool,
    slider_value: f32,
    selected: String,
    radio: String,
}

impl App for Gallery {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            Gallery {
                input_value: String::new(),
                submit_value: String::new(),
                checked: false,
                toggled: false,
                slider_value: 50.0,
                selected: "Apple".to_string(),
                radio: "A".to_string(),
            },
            Command::none(),
        )
    }

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        match event.widget_match() {
            Some(Input("input", v)) => next.input_value = v.to_string(),
            Some(Input("submit-input", v)) => next.submit_value = v.to_string(),
            Some(Submit("submit-input", _)) => next.submit_value.clear(),
            Some(Toggle("check", v)) => next.checked = v,
            Some(Toggle("toggler", v)) => next.toggled = v,
            Some(Slide("slide", v)) => next.slider_value = v as f32,
            Some(Select("pick", v)) => next.selected = v.to_string(),
            Some(Select(id, v)) if id.starts_with("radio-") => {
                next.radio = v.to_string();
            }
            _ => {}
        }
        (next, Command::none())
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        let section = |label: &str| text(label).size(14.0).color(Color::hex("#888888"));

        window("main")
            .title("Widget Gallery")
            .child(
                scrollable().id("gallery").height(Fill).child(
                    column()
                        .padding(16)
                        .spacing(16.0)
                        .child(text("Widget Gallery").id("title").size(20.0))
                        // Buttons
                        .child(section("Buttons"))
                        .child(row().spacing(8.0).children([
                            button("btn-default", "Default"),
                            button("btn-primary", "Primary").style(Style::primary()),
                            button("btn-danger", "Danger").style(Style::danger()),
                            button("btn-text", "Text").style(Style::text()),
                        ]))
                        // Text input
                        .child(section("Text Input"))
                        .child(text_input("input", &model.input_value).placeholder("Type here..."))
                        .child(
                            text_input("submit-input", &model.submit_value)
                                .placeholder("Press Enter (on_submit)")
                                .on_submit(true),
                        )
                        // Toggles
                        .child(section("Toggles"))
                        .child(
                            row()
                                .spacing(16.0)
                                .child(checkbox("check", model.checked).label("I agree"))
                                .child(toggler("toggler", model.toggled).label("Enable")),
                        )
                        // Slider
                        .child(section("Slider"))
                        .child(slider("slide", (0.0, 100.0), model.slider_value))
                        .child(
                            text(&format!("Value: {}", model.slider_value as i32))
                                .id("slide-value")
                                .size(12.0),
                        )
                        // Selection
                        .child(section("Selection"))
                        .child(pick_list(
                            "pick",
                            &["Apple", "Banana", "Cherry"],
                            Some(&model.selected),
                        ))
                        .child(
                            row()
                                .spacing(8.0)
                                .child(radio("radio-a", "A", Some(&model.radio)).label("A"))
                                .child(radio("radio-b", "B", Some(&model.radio)).label("B"))
                                .child(radio("radio-c", "C", Some(&model.radio)).label("C")),
                        )
                        // Display
                        .child(section("Display"))
                        .child(progress_bar((0.0, 100.0), 65.0).id("progress"))
                        .child(rule())
                        .child(
                            text("Styled text")
                                .id("styled")
                                .size(18.0)
                                .color(Color::hex("#3b82f6")),
                        ),
                ),
            )
            .into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<Gallery>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie::test::TestSession;

    #[test]
    fn title_renders() {
        let session = TestSession::<Gallery>::start();
        session.assert_text("title", "Widget Gallery");
    }

    #[test]
    fn text_input_updates_model() {
        let mut session = TestSession::<Gallery>::start();
        session.type_text("input", "hello");
        assert_eq!(session.model().input_value, "hello");
    }

    #[test]
    fn checkbox_toggles() {
        let mut session = TestSession::<Gallery>::start();
        assert!(!session.model().checked);
        session.set_toggle("check", true);
        assert!(session.model().checked);
    }

    #[test]
    fn toggler_toggles() {
        let mut session = TestSession::<Gallery>::start();
        assert!(!session.model().toggled);
        session.set_toggle("toggler", true);
        assert!(session.model().toggled);
    }

    #[test]
    fn slider_updates_value() {
        let mut session = TestSession::<Gallery>::start();
        session.slide("slide", 75.0);
        assert!((session.model().slider_value - 75.0).abs() < 0.1);
    }

    #[test]
    fn pick_list_selects_value() {
        let mut session = TestSession::<Gallery>::start();
        session.select("pick", "Banana");
        assert_eq!(session.model().selected, "Banana");
    }

    #[test]
    fn radio_selects_value() {
        let mut session = TestSession::<Gallery>::start();
        session.select("radio-b", "B");
        assert_eq!(session.model().radio, "B");
    }
}
