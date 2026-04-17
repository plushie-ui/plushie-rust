//! Plushie: a desktop GUI framework for Rust.
//!
//! Build native desktop applications with the Elm architecture:
//! define your model, handle events in `update`, and describe
//! your UI in `view`.
//!
//! # Quick start
//!
//! ```ignore
//! use plushie::prelude::*;
//!
//! struct Counter { count: i32 }
//!
//! impl App for Counter {
//!     type Model = Self;
//!
//!     fn init() -> (Self, Command) {
//!         (Counter { count: 0 }, Command::none())
//!     }
//!
//!     fn update(model: &mut Self, event: Event) -> Command {
//!         match event.widget_match() {
//!             Some(Click("inc")) => model.count += 1,
//!             Some(Click("dec")) => model.count -= 1,
//!             _ => {}
//!         }
//!         Command::none()
//!     }
//!
//!     fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {
//!         window("main").title("Counter").child(
//!             column().spacing(8).padding(16).children([
//!                 text(&format!("Count: {}", model.count)),
//!                 row().spacing(8).children([
//!                     button("inc", "+"),
//!                     button("dec", "-"),
//!                 ]),
//!             ])
//!         )
//!     }
//! }
//!
//! fn main() -> plushie::Result {
//!     plushie::run::<Counter>()
//! }
//! ```
//!
//! # Two modes
//!
//! - **Direct mode** (`plushie::run`): Renders in-process using iced.
//!   No subprocess, no serialization. Default.
//! - **Wire mode** (`plushie::run_wire`): Spawns a renderer binary
//!   and communicates over stdin/stdout. Same API, same behavior,
//!   higher latency.

pub mod animation;
pub mod automation;
pub mod command;
pub mod event;
pub mod prelude;
pub mod query;
pub mod route;
pub mod runner;
pub(crate) mod runtime;
pub mod selection;
pub mod settings;
pub mod state;
pub mod subscription;
pub mod test;
pub mod types;
pub mod ui;
pub mod undo;
pub mod widget;

// Re-export the widget SDK for widget authors who also use the app SDK.
pub use plushie_widget_sdk as widget_sdk;

// Re-export the derive macros for widget authoring.
pub use plushie_core_macros::{PlushieEnum, WidgetCommand, WidgetEvent, WidgetProps};

// ---------------------------------------------------------------------------
// App trait
// ---------------------------------------------------------------------------

use command::Command;
use event::Event;
use settings::{ExitReason, Settings, WindowConfig};
use subscription::Subscription;

/// A view tree returned from [`App::view`].
///
/// This is a type alias for [`plushie_core::protocol::TreeNode`].
/// Built using UI builder functions (`window`, `column`, `button`,
/// `text`, etc.).
pub type View = plushie_core::protocol::TreeNode;

/// The core trait for plushie applications.
///
/// Implement `init`, `update`, and `view` to create an app.
/// The runtime calls these in a loop: events flow in through
/// `update`, state changes flow out through `view`.
///
/// # Required methods
///
/// - [`init`](App::init): Create the initial model and startup commands.
/// - [`update`](App::update): Handle events and produce side effects.
/// - [`view`](App::view): Build the view tree from the current model.
///
/// # Optional methods
///
/// - [`subscribe`](App::subscribe): Declare active event subscriptions.
/// - [`settings`](App::settings): Application-level configuration.
/// - [`window_config`](App::window_config): Per-window defaults.
/// - [`handle_renderer_exit`](App::handle_renderer_exit): React to
///   renderer crashes (wire mode only).
pub trait App: Send + 'static {
    /// Application state. Owned by the runtime, passed to all callbacks.
    type Model: Send + 'static;

    /// Initialize the app. Returns the initial model and any
    /// startup commands (e.g., fetch data, start timers).
    fn init() -> (Self::Model, Command);

    /// Handle an event. Mutate the model and return commands
    /// for side effects. Called once per event.
    fn update(model: &mut Self::Model, event: Event) -> Command;

    /// Build the view tree from the current model. Called after
    /// every update. Return a tree built from UI builder functions
    /// (`window`, `column`, `button`, `text`, etc.).
    ///
    /// Use `widgets` to register composite widgets:
    /// ```ignore
    /// fn view(model: &Self, widgets: &mut WidgetRegistrar) -> View {
    ///     window("main").child(
    ///         WidgetView::<MyWidget>::new("w1").register(widgets)
    ///     ).into()
    /// }
    /// ```
    fn view(model: &Self::Model, widgets: &mut widget::WidgetRegistrar) -> View;

    /// Active subscriptions. Called after every update. The runtime
    /// diffs the returned list and starts/stops subscriptions as
    /// needed. Default: no subscriptions.
    fn subscribe(_model: &Self::Model) -> Vec<Subscription> {
        vec![]
    }

    /// Application-level settings (theme, fonts, text defaults).
    /// Called once at startup. Default: renderer defaults.
    fn settings() -> Settings {
        Settings::default()
    }

    /// Per-window defaults (title, size, position).
    /// Called once at startup. Default: renderer defaults.
    fn window_config(_model: &Self::Model) -> WindowConfig {
        WindowConfig::default()
    }

    /// Called when the renderer process exits unexpectedly.
    /// Wire mode only; direct mode never calls this.
    fn handle_renderer_exit(_model: &mut Self::Model, _reason: ExitReason) {}
}

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

/// Result type for plushie entry points.
pub type Result = std::result::Result<(), Box<dyn std::error::Error>>;

// ---------------------------------------------------------------------------
// Entry points (stubs until runners are implemented)
// ---------------------------------------------------------------------------

/// Run the app in direct mode (in-process renderer).
///
/// This is the default and most common entry point. The renderer
/// runs in the same process with no subprocess or serialization.
///
/// # Errors
///
/// Returns an error if iced fails to initialize the event loop, the
/// app's [`App::init`] panics, or the renderer encounters an
/// unrecoverable window-system failure.
#[cfg(feature = "direct")]
pub fn run<A: App>() -> Result {
    runner::direct::run::<A>()
}

/// Run the app in wire mode (subprocess renderer).
///
/// Spawns the renderer binary at `binary_path` and communicates
/// over stdin/stdout using the plushie wire protocol.
///
/// # Errors
///
/// Returns an error if the renderer binary cannot be spawned, the
/// protocol handshake fails (version mismatch or malformed hello),
/// or stdin/stdout I/O fails during the session.
#[cfg(feature = "wire")]
pub fn run_wire<A: App>(binary_path: &str) -> Result {
    runner::wire::run_wire::<A>(binary_path)
}
