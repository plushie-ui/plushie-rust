//! App rating page with form validation and reviews.
//!
//! Demonstrates custom widget composition (star rating), styled
//! containers, form validation with error states, and a scrollable
//! review list.
//!
//! Run with: `cargo run -p plushie --example rate_plushie`

use plushie::prelude::*;

struct RatePlushie {
    rating: usize,
    dark_mode: bool,
    reviews: Vec<Review>,
    review_name: String,
    review_comment: String,
    errors: Vec<String>,
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
        (RatePlushie {
            rating: 0,
            dark_mode: false,
            reviews: vec![
                Review { stars: 5, user: "elixir_fan_42".into(), time: "2d ago".into(),
                    text: "Finally, native GUIs that don't make me want to cry.".into() },
                Review { stars: 5, user: "beam_me_up".into(), time: "3d ago".into(),
                    text: "The Elm architecture feels right at home here.".into() },
                Review { stars: 4, user: "rustacean".into(), time: "5d ago".into(),
                    text: "Solid Iced wrapper. Docked a star because I had to write Elixir.".into() },
                Review { stars: 3, user: "web_refugee".into(), time: "1w ago".into(),
                    text: "Where is my CSS grid? Also it works perfectly. Three stars.".into() },
                Review { stars: 1, user: "electron_mass".into(), time: "2w ago".into(),
                    text: "No browser engine. No JavaScript runtime. What am I even paying for?".into() },
            ],
            review_name: String::new(),
            review_comment: String::new(),
            errors: vec![],
        }, Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            // Star rating (buttons named "star-0" through "star-4")
            Some(Click(id)) if id.starts_with("star-") => {
                if let Ok(n) = id["star-".len()..].parse::<usize>() {
                    model.rating = n + 1;
                    model.errors.retain(|e| e != "rating");
                }
            }
            // Theme toggle
            Some(Toggle("theme-toggle", dark)) => {
                model.dark_mode = dark;
            }
            // Form inputs
            Some(Input("review-name", name)) => {
                model.review_name = name.to_string();
                model.errors.retain(|e| e != "name");
            }
            Some(Input("review-comment", comment)) => {
                model.review_comment = comment.to_string();
                model.errors.retain(|e| e != "comment");
            }
            // Submit
            Some(Click("submit-review")) | Some(Submit("review-name", _)) => {
                submit_review(model);
            }
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self) -> View {
        let star_row = row().spacing(4.0).children(
            (0..5).map(|i| {
                let filled = i < model.rating;
                let label = if filled { "★" } else { "☆" };
                button(&format!("star-{i}"), label)
                    .style(if filled { Style::warning() } else { Style::text() })
            })
        );

        let has_error = |field: &str| model.errors.iter().any(|e| e == field);

        let name_style = if has_error("name") {
            Style::custom().border(Border::new().color(Color::red()).width(2.0)).into()
        } else {
            Style::primary()
        };

        let mut form = column().spacing(12.0)
            .child(text("Rate Plushie").size(28.0))
            .child(row().spacing(12.0)
                .child(text("Rating:"))
                .child(star_row)
                .child(toggler("theme-toggle", model.dark_mode).label("Dark mode"))
            )
            .child(text_input("review-name", &model.review_name)
                .placeholder("Your name")
                .style(name_style))
            .child(text_input("review-comment", &model.review_comment)
                .placeholder("Write a review..."))
            .child(button("submit-review", "Submit Review").style(Style::primary()));

        if !model.errors.is_empty() {
            form = form.child(text(&format!("Errors: {}", model.errors.join(", ")))
                .color(Color::red()));
        }

        let reviews = column().spacing(8.0).children(
            model.reviews.iter().map(|r| {
                let stars: String = (0..5).map(|i| if i < r.stars { '★' } else { '☆' }).collect();
                container().padding(12)
                    .border(Border::new().color("#dee2e6").width(1.0).radius(4.0))
                    .child(column().spacing(4.0)
                        .child(row().spacing(8.0)
                            .child(text(&stars).color(Color::gold()))
                            .child(text(&r.user).size(14.0))
                            .child(text(&r.time).size(12.0).color(Color::gray()))
                        )
                        .child(text(&r.text))
                    )
            })
        );

        window("main").title("Rate Plushie").child(
            scrollable().child(
                column().spacing(20.0).padding(20)
                    .child(form)
                    .child(rule())
                    .child(text("Reviews").size(20.0))
                    .child(reviews)
            )
        ).into()
    }
}

fn submit_review(model: &mut RatePlushie) {
    let mut errors = vec![];
    if model.rating == 0 { errors.push("rating".to_string()); }
    if model.review_name.trim().is_empty() { errors.push("name".to_string()); }
    if model.review_comment.trim().is_empty() { errors.push("comment".to_string()); }

    if errors.is_empty() {
        model.reviews.insert(0, Review {
            stars: model.rating,
            user: model.review_name.clone(),
            time: "just now".into(),
            text: model.review_comment.clone(),
        });
        model.review_name.clear();
        model.review_comment.clear();
        model.rating = 0;
    }
    model.errors = errors;
}

fn main() -> plushie::Result {
    plushie::run::<RatePlushie>()
}
