//! Keyboard shortcut logger showing a scrollable list of key presses.
//!
//! Demonstrates:
//! - `Subscription::on_key_press` for global keyboard events
//! - Accessing `KeyEvent` fields (key, modifiers, repeat)
//! - Scrollable container with a dynamic list
//! - Capped log buffer
//!
//! Run with: `cargo run -p plushie --example shortcuts`

use plushie::prelude::*;

const MAX_LOG_ENTRIES: usize = 50;

struct Shortcuts {
    log: Vec<String>,
    count: usize,
}

impl App for Shortcuts {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            Shortcuts {
                log: Vec::new(),
                count: 0,
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Some(key_event) = event.as_key_press() {
            model.count += 1;
            let entry = format_key_event(key_event, model.count);
            model.log.insert(0, entry);
            model.log.truncate(MAX_LOG_ENTRIES);
        }
        Command::none()
    }

    fn subscribe(_model: &Self) -> Vec<Subscription> {
        vec![Subscription::on_key_press()]
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        let mut log_col = column().spacing(2.0).width(Fill);
        for (i, entry) in model.log.iter().enumerate() {
            log_col = log_col.child(text(entry).id(&format!("log_{i}")).size(13.0));
        }

        window("main")
            .title("Keyboard Shortcuts")
            .child(
                column()
                    .padding(16)
                    .spacing(12.0)
                    .width(Fill)
                    .child(text("Press any key").id("header").size(20.0))
                    .child(
                        text(&format!("{} key events captured", model.count))
                            .id("count")
                            .size(12.0)
                            .color(Color::hex("#888888")),
                    )
                    .child(rule())
                    .child(scrollable().id("log").height(Fill).child(log_col)),
            )
            .into()
    }
}

fn format_key_event(event: &plushie::event::KeyEvent, n: usize) -> String {
    let mods = format_modifiers(&event.modifiers);
    let prefix = if mods.is_empty() {
        String::new()
    } else {
        format!("{mods}+")
    };
    format!("#{n}: {prefix}{}", event.key)
}

fn format_modifiers(m: &KeyModifiers) -> String {
    let mut parts = Vec::new();
    if m.ctrl {
        parts.push("Ctrl");
    }
    if m.alt {
        parts.push("Alt");
    }
    if m.shift {
        parts.push("Shift");
    }
    if m.logo {
        parts.push("Super");
    }
    parts.join("+")
}

fn main() -> plushie::Result {
    plushie::run::<Shortcuts>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie::test::TestSession;

    #[test]
    fn starts_with_empty_log_and_zero_count() {
        let session = TestSession::<Shortcuts>::start();
        assert!(session.model().log.is_empty());
        assert_eq!(session.model().count, 0);
    }

    #[test]
    fn header_text_is_present() {
        let session = TestSession::<Shortcuts>::start();
        session.assert_text("header", "Press any key");
    }

    #[test]
    fn count_label_displays_zero_initially() {
        let session = TestSession::<Shortcuts>::start();
        session.assert_text("count", "0 key events captured");
    }

    #[test]
    fn scrollable_log_container_exists() {
        let session = TestSession::<Shortcuts>::start();
        session.assert_exists("log");
    }
}
