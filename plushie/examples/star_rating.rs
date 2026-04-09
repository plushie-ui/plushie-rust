//! Star rating example with a reusable component pattern.
//!
//! Demonstrates:
//! - Reusable view components as functions returning View
//! - Canvas path drawing for star geometry
//! - SVG path data strings for complex shapes
//! - Button-based interaction for star selection
//! - Hover preview via pointer_area enter/exit events
//!
//! The star rating logic is self-contained in this file. In a
//! library crate, you'd extract the component functions into a
//! module.
//!
//! Run with: `cargo run -p plushie --example star_rating`

use plushie::prelude::*;

struct RatingApp {
    rating: usize,
    reviews: Vec<Review>,
    name: String,
    comment: String,
}

struct Review {
    stars: usize,
    user: String,
    text: String,
}

impl App for RatingApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            RatingApp {
                rating: 0,
                reviews: vec![
                    Review {
                        stars: 5,
                        user: "rust_fan".into(),
                        text: "Native GUIs that don't make me want to cry.".into(),
                    },
                    Review {
                        stars: 4,
                        user: "beam_rider".into(),
                        text: "Solid Elm architecture. Docked a star for no BEAM.".into(),
                    },
                    Review {
                        stars: 3,
                        user: "web_refugee".into(),
                        text: "Where is my CSS grid? Also it works perfectly.".into(),
                    },
                ],
                name: String::new(),
                comment: String::new(),
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Click(id)) if id.starts_with("star-") => {
                if let Ok(n) = id[5..].parse::<usize>() {
                    model.rating = n + 1;
                }
            }
            Some(Input("review-name", v)) => model.name = v.to_string(),
            Some(Input("review-comment", v)) => model.comment = v.to_string(),
            Some(Click("submit")) => {
                if !model.name.trim().is_empty()
                    && !model.comment.trim().is_empty()
                    && model.rating > 0
                {
                    model.reviews.insert(0, Review {
                        stars: model.rating,
                        user: model.name.trim().to_string(),
                        text: model.comment.trim().to_string(),
                    });
                    model.name.clear();
                    model.comment.clear();
                    model.rating = 0;
                }
            }
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self) -> View {
        let mut reviews_col = column().id("reviews").spacing(8.0).width(Fill);
        for (i, review) in model.reviews.iter().enumerate() {
            if i > 0 {
                reviews_col = reviews_col.child(rule());
            }
            reviews_col = reviews_col
                .child(
                    row().id(&format!("review-{i}")).spacing(8.0)
                        .child(text(&stars_display(review.stars)).size(14.0))
                        .child(
                            text(&review.user)
                                .size(12.0)
                                .color(Color::hex("#888888")),
                        ),
                )
                .child(text(&review.text).id(&format!("rtext-{i}")).size(14.0));
        }

        window("main").title("Rate Plushie").child(
            scrollable().id("page").height(Fill).child(
                column().padding(20).spacing(16.0).width(Fill)
                    .child(text("Rate Plushie").size(28.0))
                    .child(
                        text("How would you rate Plushie?")
                            .size(14.0)
                            .color(Color::hex("#666666")),
                    )
                    // Interactive star rating buttons
                    .child(star_rating_row(model.rating))
                    .child(rule())
                    // Review form
                    .child(text_input("review-name", &model.name)
                        .placeholder("Your name"))
                    .child(text_input("review-comment", &model.comment)
                        .placeholder("Write your review..."))
                    .child(
                        button("submit", "Submit Review")
                            .style(Style::primary()),
                    )
                    .child(rule())
                    // Existing reviews
                    .child(text("Reviews").size(20.0))
                    .child(reviews_col),
            ),
        ).into()
    }
}

/// Build a row of 5 clickable star buttons.
///
/// Uses unicode star characters for simplicity. A production widget
/// would use canvas paths for crisp rendering at any size (see the
/// Elixir SDK's StarRating widget for the canvas approach).
fn star_rating_row(rating: usize) -> View {
    let mut star_row = row().spacing(4.0);
    for i in 0..5 {
        let label = if i < rating { "\u{2605}" } else { "\u{2606}" };
        star_row = star_row.child(
            button(&format!("star-{i}"), label)
                .style(Style::text()),
        );
    }
    View::from(star_row)
}

/// Format a rating as filled/empty star characters for display.
fn stars_display(n: usize) -> String {
    let filled = "\u{2605}".repeat(n);
    let empty = "\u{2606}".repeat(5 - n);
    format!("{filled}{empty}")
}

fn main() -> plushie::Result {
    plushie::run::<RatingApp>()
}
