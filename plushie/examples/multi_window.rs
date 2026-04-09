//! Multi-window example.
//!
//! Demonstrates creating multiple windows from a single view
//! function. Each window has independent content.
//!
//! Run with: `cargo run -p plushie --example multi_window`

use plushie::prelude::*;

struct MultiWindowApp {
    main_count: i32,
    settings_open: bool,
    theme_name: String,
}

impl App for MultiWindowApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (MultiWindowApp {
            main_count: 0,
            settings_open: false,
            theme_name: "Dark".to_string(),
        }, Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Click("inc")) => model.main_count += 1,
            Some(Click("dec")) => model.main_count -= 1,
            Some(Click("open_settings")) => model.settings_open = true,
            Some(Click("close_settings")) => model.settings_open = false,
            Some(Select("theme", name)) => model.theme_name = name.to_string(),
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self) -> View {
        // Main window is always present.
        let mut root = column()
            .child(window("main").title("Main Window").child(
                column().spacing(8.0).padding(16)
                    .child(text(&format!("Count: {}", model.main_count)).size(24.0))
                    .child(row().spacing(8.0).children([
                        button("inc", "+"),
                        button("dec", "-"),
                    ]))
                    .child(button("open_settings", "Open Settings"))
            ));

        // Settings window is conditionally rendered.
        if model.settings_open {
            root = root.child(window("settings").title("Settings").child(
                column().spacing(8.0).padding(16)
                    .child(text("Theme").size(18.0))
                    .child(pick_list("theme", &["Light", "Dark", "Nord"], Some(&model.theme_name)))
                    .child(button("close_settings", "Close"))
            ));
        }

        root.into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<MultiWindowApp>()
}
