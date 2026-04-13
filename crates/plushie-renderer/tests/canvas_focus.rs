use iced::widget::{button, canvas, column, text_input};
use iced::{Element, Length, Theme};

struct FocusableCanvas;

impl canvas::Program<String> for FocusableCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry<iced::Renderer>> {
        let _ = (renderer, bounds);
        vec![]
    }

    fn is_focusable(&self, _state: &()) -> bool {
        true
    }
}

fn view() -> Element<'static, String> {
    column![
        text_input("Name", ""),
        canvas(FocusableCanvas)
            .width(Length::Fixed(200.0))
            .height(Length::Fixed(50.0)),
        button("Submit"),
    ]
    .into()
}

#[test]
fn tab_reaches_canvas() {
    use iced::keyboard::key::Named;
    use iced_test::simulator::Simulator;

    let mut sim = Simulator::new(view());

    // Tab through widgets
    for i in 1..=6 {
        let status = sim.tap_key(Named::Tab);
        eprintln!("Tab {i}: {status:?}");
    }
}
