//! Custom theme example.
//!
//! Demonstrates using Style presets and custom StyleMaps with
//! status overrides (hovered, pressed, disabled).
//!
//! Run with: `cargo run -p plushie --example custom_theme`

use plushie::prelude::*;

struct ThemeApp {
    active_tab: String,
}

impl App for ThemeApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (ThemeApp { active_tab: "home".to_string() }, Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Click(id)) => model.active_tab = id.to_string(),
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self) -> View {
        let tab_style = |id: &str| -> Style {
            if model.active_tab == id {
                Style::custom()
                    .background(Color::hex("#3498db"))
                    .text_color(Color::white())
                    .border(Border::new().radius(4.0))
                    .into()
            } else {
                Style::custom()
                    .background(Color::hex("#ecf0f1"))
                    .text_color(Color::hex("#2c3e50"))
                    .border(Border::new().radius(4.0))
                    .hovered(|s| s
                        .background(Color::hex("#bdc3c7"))
                    )
                    .into()
            }
        };

        window("main").title("Custom Theme").child(
            column().spacing(16.0).padding(20)
                .child(text("Custom Styled Tabs").size(24.0))
                .child(row().spacing(4.0).children([
                    button("home", "Home").style(tab_style("home")),
                    button("profile", "Profile").style(tab_style("profile")),
                    button("settings", "Settings").style(tab_style("settings")),
                ]))
                .child(container()
                    .padding(16)
                    .style(Style::custom()
                        .background(Color::hex("#f8f9fa"))
                        .border(Border::new().color(Color::hex("#dee2e6")).width(1.0).radius(8.0))
                    )
                    .child(text(&format!("Active tab: {}", model.active_tab)))
                )
        ).into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<ThemeApp>()
}
