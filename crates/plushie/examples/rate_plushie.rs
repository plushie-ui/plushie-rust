//! App rating page with composite widgets, form validation, and reviews.
//!
//! Demonstrates the Widget trait for reusable components:
//!
//! - `StarRating`: a row of star buttons that tracks hover state
//!   internally and emits a "select" event with the chosen rating.
//! - `ThemeToggle`: wraps a toggler, emitting "toggle" with the
//!   new boolean state.
//!
//! The app never touches the widgets' internal children directly.
//! It receives high-level semantic events via `as_widget()`.
//!
//! Run with: `cargo run -p plushie --example rate_plushie`

use std::collections::HashMap;

use plushie::prelude::*;
use plushie::widget::{EventResult, Widget, WidgetView};

// ---------------------------------------------------------------------------
// StarRating widget
// ---------------------------------------------------------------------------

struct StarRating;

#[derive(WidgetEvent)]
enum StarRatingEvent {
    /// User selected a rating.
    Select(u64),
}

#[derive(Default)]
struct StarState {
    hover: Option<usize>,
}

impl Widget for StarRating {
    type State = StarState;
    type Props = UntypedProps;

    fn view(id: &str, props: &UntypedProps, state: &Self::State) -> View {
        let rating = props
            .0
            .get("rating")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        let display = state.hover.unwrap_or(rating);

        row()
            .id(id)
            .spacing(4.0)
            .children((0..5).map(|i| {
                let filled = i < display;
                let label = if filled { "\u{2605}" } else { "\u{2606}" };
                button(&format!("star-{i}"), label).style(if filled {
                    Style::warning()
                } else {
                    Style::text()
                })
            }))
            .into()
    }

    fn handle_event(event: &Event, state: &mut Self::State) -> EventResult {
        match event.widget_match() {
            Some(Click(id)) if id.starts_with("star-") => {
                if let Ok(n) = id["star-".len()..].parse::<usize>() {
                    state.hover = None;
                    EventResult::emit_event(StarRatingEvent::Select((n + 1) as u64))
                } else {
                    EventResult::Ignored
                }
            }
            Some(Enter(id, _)) if id.starts_with("star-") => {
                state.hover = id["star-".len()..].parse::<usize>().ok().map(|n| n + 1);
                EventResult::Consumed
            }
            Some(Exit(..)) => {
                state.hover = None;
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

// ---------------------------------------------------------------------------
// ThemeToggle widget
// ---------------------------------------------------------------------------

struct ThemeToggle;

#[derive(WidgetEvent)]
enum ThemeToggleEvent {
    /// Theme toggled on or off.
    Toggle(bool),
}

/// ThemeToggle has no internal state; it's a pure view wrapper
/// that transforms toggler events into "toggle" emissions.
#[derive(Default)]
struct ToggleState;

impl Widget for ThemeToggle {
    type State = ToggleState;
    type Props = UntypedProps;

    fn view(id: &str, props: &UntypedProps, _state: &Self::State) -> View {
        let is_dark = props
            .0
            .get("dark")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        row()
            .id(id)
            .align_y(Align::Center)
            .child(space().width(Fill))
            .child(text("Dark humor").id("label").color(Color::hex(if is_dark {
                "#9999bb"
            } else {
                "#666666"
            })))
            .child(toggler("switch", is_dark))
            .into()
    }

    fn handle_event(event: &Event, _state: &mut Self::State) -> EventResult {
        match event.widget_match() {
            Some(Toggle("switch", on)) => EventResult::emit_event(ThemeToggleEvent::Toggle(on)),
            _ => EventResult::Ignored,
        }
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

struct RatePlushie {
    rating: usize,
    dark_mode: bool,
    reviews: Vec<Review>,
    review_name: String,
    review_comment: String,
    errors: HashMap<String, String>,
}

struct Review {
    stars: usize,
    user: String,
    time: String,
    text: String,
}

impl App for RatePlushie {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            RatePlushie {
                rating: 0,
                dark_mode: false,
                reviews: vec![
                    Review {
                        stars: 5,
                        user: "elixir_fan_42".into(),
                        time: "2d ago".into(),
                        text: "Finally, native GUIs that don't make me want to cry.".into(),
                    },
                    Review {
                        stars: 5,
                        user: "beam_me_up".into(),
                        time: "3d ago".into(),
                        text: "The Elm architecture feels right at home here.".into(),
                    },
                    Review {
                        stars: 4,
                        user: "rustacean".into(),
                        time: "5d ago".into(),
                        text: "Solid Iced wrapper. Docked a star because I had to write Elixir."
                            .into(),
                    },
                    Review {
                        stars: 3,
                        user: "web_refugee".into(),
                        time: "1w ago".into(),
                        text: "Where is my CSS grid? Also it works perfectly. Three stars.".into(),
                    },
                    Review {
                        stars: 5,
                        user: "otp_enjoyer".into(),
                        time: "1w ago".into(),
                        text: "Let it crash, but make it beautiful.".into(),
                    },
                    Review {
                        stars: 1,
                        user: "electron_mass".into(),
                        time: "2w ago".into(),
                        text:
                            "No browser engine. No JavaScript runtime. What am I even paying for?"
                                .into(),
                    },
                ],
                review_name: String::new(),
                review_comment: String::new(),
                errors: HashMap::new(),
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        // Widget events arrive via as_widget() since the emitted
        // families ("select", "toggle") aren't all covered by
        // WidgetMatch's typed variants.
        if let Some(w) = event.as_widget() {
            match w.scoped_id.id.as_str() {
                "stars" => {
                    if let Some(n) = w.value.as_u64() {
                        model.rating = n as usize;
                        model.errors.remove("rating");
                    }
                }
                "theme-toggle" => {
                    if let Some(dark) = w.value.as_bool() {
                        model.dark_mode = dark;
                    }
                }
                _ => {}
            }
        }

        // Standard widget_match for the form controls.
        match event.widget_match() {
            Some(Input("review-name", name)) => {
                model.review_name = name.to_string();
                model.errors.remove("name");
            }
            Some(Input("review-comment", comment)) => {
                model.review_comment = comment.to_string();
                model.errors.remove("comment");
            }
            Some(Click("submit-review")) | Some(Submit("review-name", _)) => {
                submit_review(model);
            }
            _ => {}
        }

        Command::none()
    }

    fn view(model: &Self, widgets: &mut WidgetRegistrar) -> Option<View> {
        let p: f64 = if model.dark_mode { 1.0 } else { 0.0 };
        let t = theme(p);

        Some(
            window("main")
                .title("Rate Plushie")
                .child(
                    scrollable().child(
                        column()
                            .id("page")
                            .spacing(24.0)
                            .padding(Padding::new(32.0, 24.0, 32.0, 24.0))
                            .width(Fill)
                            .child(
                                text("Rate Plushie")
                                    .id("heading")
                                    .size(28.0)
                                    .color(Color::hex(&t.text))
                                    .a11y(&A11y::new().role(Role::Heading).level(1)),
                            )
                            .child(rating_card(model, &t, widgets))
                            .child(
                                text("Reviews")
                                    .id("reviews-heading")
                                    .size(20.0)
                                    .color(Color::hex(&t.text))
                                    .a11y(&A11y::new().role(Role::Heading).level(2)),
                            )
                            .child(reviews_list(&model.reviews, &t)),
                    ),
                )
                .into(),
        )
    }
}

// -- Rating card --------------------------------------------------------------

fn rating_card(model: &RatePlushie, t: &AppTheme, widgets: &mut WidgetRegistrar) -> View {
    let mut card_col = column().spacing(20.0);

    card_col = card_col.child(
        text("How would you rate Plushie?")
            .id("prompt")
            .size(14.0)
            .color(Color::hex(&t.text_secondary)),
    );

    // Star rating widget
    let mut stars_group = column().id("stars-group").spacing(4.0);
    stars_group = stars_group.child(
        WidgetView::<StarRating>::new("stars")
            .prop("rating", model.rating as u64)
            .register(widgets),
    );
    if let Some(err) = model.errors.get("rating") {
        stars_group = stars_group.child(
            text(err)
                .id("stars-error")
                .size(12.0)
                .color(Color::hex(&t.error_text))
                .a11y(&A11y::new().role(Role::Alert).live(Live::Polite)),
        );
    }
    card_col = card_col.child(stars_group);

    card_col = card_col.child(rule());
    card_col = card_col.child(review_form(model, t));

    // Theme toggle widget
    card_col = card_col.child(
        WidgetView::<ThemeToggle>::new("theme-toggle")
            .prop("dark", model.dark_mode)
            .register(widgets),
    );

    container()
        .id("rating-card")
        .padding(24)
        .width(Fill)
        .border(Border::new().color(&*t.card_border).width(1.0).radius(12.0))
        .background(Color::hex(&t.card_bg))
        .child(card_col)
        .into()
}

// -- Review form --------------------------------------------------------------

fn review_form(model: &RatePlushie, t: &AppTheme) -> View {
    let name_err = model.errors.get("name");
    let comment_err = model.errors.get("comment");

    let error_border = Border::new().color(&*t.error_border).width(2.0).radius(4.0);
    let error_style: Style = Style::custom()
        .border(error_border)
        .background(Color::hex(&t.error_bg))
        .focused(|s| s.border(Border::new().color(&*t.error_border).width(2.0).radius(4.0)))
        .into();

    let mut form = column().id("review-form").spacing(12.0).width(Fill);

    // Name field
    let mut name_col = column().id("name-field").spacing(4.0).width(Fill);
    let mut name_input = text_input("review-name", &model.review_name)
        .placeholder("Your name")
        .on_submit(true)
        .a11y(&{
            let mut a = A11y::new()
                .label("Your name")
                .required(true)
                .invalid(name_err.is_some());
            if name_err.is_some() {
                a = a.error_message("review-name-error");
            }
            a
        });
    if name_err.is_some() {
        name_input = name_input.style(error_style.clone());
    }
    name_col = name_col.child(name_input);
    if let Some(err) = name_err {
        name_col = name_col.child(
            text(err)
                .id("review-name-error")
                .size(12.0)
                .color(Color::hex(&t.error_text))
                .a11y(&A11y::new().role(Role::Alert).live(Live::Polite)),
        );
    }
    form = form.child(name_col);

    // Comment field
    let mut comment_col = column().id("comment-field").spacing(4.0).width(Fill);
    let mut comment_input = text_editor("review-comment", &model.review_comment)
        .placeholder("Write your review...")
        .height(80.0)
        .a11y(&{
            let mut a = A11y::new()
                .label("Review text")
                .required(true)
                .invalid(comment_err.is_some());
            if comment_err.is_some() {
                a = a.error_message("review-comment-error");
            }
            a
        });
    if comment_err.is_some() {
        comment_input = comment_input.style(error_style);
    }
    comment_col = comment_col.child(comment_input);
    if let Some(err) = comment_err {
        comment_col = comment_col.child(
            text(err)
                .id("review-comment-error")
                .size(12.0)
                .color(Color::hex(&t.error_text))
                .a11y(&A11y::new().role(Role::Alert).live(Live::Polite)),
        );
    }
    form = form.child(comment_col);

    form = form.child(button("submit-review", "Submit Review"));

    form.into()
}

// -- Reviews list -------------------------------------------------------------

fn reviews_list(reviews: &[Review], t: &AppTheme) -> View {
    let mut list = column().id("reviews").spacing(0.0).width(Fill);

    for (i, review) in reviews.iter().enumerate() {
        if i > 0 {
            list = list.child(rule().id(&format!("sep-{i}")));
        }
        list = list.child(review_card(review, i, t));
    }

    list.into()
}

fn review_card(review: &Review, i: usize, t: &AppTheme) -> View {
    let stars: String = (0..5)
        .map(|j| {
            if j < review.stars {
                '\u{2605}'
            } else {
                '\u{2606}'
            }
        })
        .collect();

    column()
        .id(&format!("review-{i}"))
        .spacing(4.0)
        .padding(12)
        .width(Fill)
        .child(
            row()
                .id(&format!("rhdr-{i}"))
                .spacing(8.0)
                .align_y(Align::Center)
                .child(text(&stars).id(&format!("rstars-{i}")).color(Color::gold()))
                .child(
                    text(&review.user)
                        .id(&format!("rname-{i}"))
                        .size(12.0)
                        .color(Color::hex(&t.text_secondary)),
                )
                .child(space().id(&format!("rsp-{i}")).width(Fill))
                .child(
                    text(&review.time)
                        .id(&format!("rtime-{i}"))
                        .size(12.0)
                        .color(Color::hex(&t.text_muted)),
                ),
        )
        .child(
            text(&format!("\u{201c}{}\u{201d}", review.text))
                .id(&format!("rtext-{i}"))
                .size(14.0)
                .color(Color::hex(&t.text)),
        )
        .into()
}

// -- Submit / Validation ------------------------------------------------------

fn submit_review(model: &mut RatePlushie) {
    let errors = validate_review(model);

    if errors.is_empty() {
        let name = model.review_name.trim().to_string();
        let comment = model.review_comment.trim().to_string();
        model.reviews.insert(
            0,
            Review {
                stars: model.rating,
                user: name,
                time: "just now".into(),
                text: comment,
            },
        );
        model.review_name.clear();
        model.review_comment.clear();
        model.rating = 0;
        model.errors.clear();
    } else {
        model.errors = errors;
    }
}

fn validate_review(model: &RatePlushie) -> HashMap<String, String> {
    let mut errors = HashMap::new();

    if model.review_name.trim().is_empty() {
        errors.insert("name".into(), "Name is required".into());
    }
    if model.review_comment.trim().is_empty() {
        errors.insert("comment".into(), "Review text is required".into());
    }
    if model.rating == 0 {
        errors.insert("rating".into(), "Please select a rating".into());
    }

    errors
}

// -- Theme --------------------------------------------------------------------

struct AppTheme {
    card_bg: String,
    card_border: String,
    text: String,
    text_secondary: String,
    text_muted: String,
    error_text: String,
    error_border: String,
    error_bg: String,
}

fn theme(p: f64) -> AppTheme {
    AppTheme {
        card_bg: fade((255, 255, 255), (28, 28, 50), p),
        card_border: fade((224, 224, 224), (42, 42, 74), p),
        text: fade((26, 26, 26), (240, 240, 245), p),
        text_secondary: fade((102, 102, 102), (153, 153, 187), p),
        text_muted: fade((170, 170, 170), (85, 85, 119), p),
        error_text: fade((185, 28, 28), (255, 100, 100), p),
        error_border: fade((220, 38, 38), (255, 80, 80), p),
        error_bg: fade((254, 242, 242), (50, 20, 20), p),
    }
}

fn fade(from: (i32, i32, i32), to: (i32, i32, i32), t: f64) -> String {
    let r = (from.0 as f64 + (to.0 - from.0) as f64 * t).round() as u8;
    let g = (from.1 as f64 + (to.1 - from.1) as f64 * t).round() as u8;
    let b = (from.2 as f64 + (to.2 - from.2) as f64 * t).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn main() -> plushie::Result {
    plushie::run::<RatePlushie>()
}
