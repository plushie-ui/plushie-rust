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
//! `plushie::run::<A>()` is feature-agnostic. It dispatches to
//! whichever runner is compiled in:
//!
//! - **Direct mode** (`direct` feature, default): In-process iced
//!   rendering. No subprocess, no serialization.
//! - **Wire mode** (`wire` feature): Spawns a renderer binary and
//!   communicates over stdin/stdout. Auto-discovers the binary via
//!   `PLUSHIE_BINARY_PATH` then `PATH`.
//!
//! When both features are enabled, direct wins. Pass an explicit
//! renderer path via [`run_with_renderer`] to force a specific wire
//! binary.
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![deny(missing_docs)]

pub mod animation;
pub mod automation;
pub mod command;
mod error;
pub mod event;
pub mod prelude;
pub mod query;
pub mod route;
pub mod runner;
pub(crate) mod runtime;

/// Re-exports of runtime internals used by the test harness and
/// property tests.
///
/// - [`SubOp`] and [`SubscriptionManager`] back
///   [`test::TestSession::last_subscription_ops`].
/// - [`diff_tree`] plus [`apply_patch`] let tree-diff proptests
///   round-trip patches against arbitrary `TreeNode` pairs.
///
/// Everything here is `pub(crate)` in its original module; this
/// re-export surface exists for tests, not for day-to-day SDK
/// consumers. The regular SDK user should never need it.
pub mod runtime_internals {
    pub use crate::runtime::subscriptions::{SubOp, SubscriptionManager};
    pub use crate::runtime::tree_diff::{PatchOp, apply_patch, diff_tree};
}
pub mod selection;
pub mod settings;
pub mod state;
pub mod subscription;
pub mod test;
pub mod types;
pub mod ui;
pub mod undo;
pub mod widget;

pub use error::Error;

// Re-export the widget SDK for widget authors who also use the app SDK.
pub use plushie_widget_sdk as widget_sdk;

// Re-export the derive macros for widget authoring.
pub use plushie_core_macros::{PlushieEnum, WidgetCommand, WidgetEvent, WidgetProps};

/// Version string of the renderer this SDK was built against.
///
/// Matches `plushie-renderer-lib`'s `CARGO_PKG_VERSION` at build
/// time. Wire mode compares the string against the renderer's
/// advertised version in the `hello` message; a mismatch does not
/// abort the handshake (the wire-protocol version is separate), but
/// it does get logged so version skew surfaces early.
///
/// Host SDKs in other languages keep their own synced per-SDK
/// `BINARY_VERSION` files. The Rust SDK uses this constant instead.
#[cfg(feature = "direct")]
pub const RENDERER_VERSION: &str = plushie_renderer_lib::RENDERER_VERSION;

/// Version string of the renderer this SDK was built against.
///
/// Wire-only builds don't depend on `plushie-renderer-lib`, so the
/// value comes straight from `CARGO_PKG_VERSION`, which the workspace
/// keeps in lock-step with the renderer crate at release time.
#[cfg(all(feature = "wire", not(feature = "direct")))]
pub const RENDERER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// App trait
// ---------------------------------------------------------------------------

use command::Command;
use event::Event;
use settings::{ExitReason, RestartPolicy, Settings, WindowConfig};
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

    /// Called synchronously before [`run`] (or
    /// [`run_with_renderer`]) returns [`Error::RendererExit`] when
    /// the renderer subprocess exits. Wire mode only; direct mode
    /// never calls this.
    ///
    /// Use this hook to save state, log diagnostics, or clean up
    /// model-side resources. The typed error coordinates
    /// process-level action (retry, exit, surface to user) after the
    /// hook returns.
    ///
    /// When auto-restart is active (see [`App::restart_policy`]), the
    /// hook fires on every restart attempt with the matching
    /// [`ExitReason`], then once more with
    /// [`ExitReason::MaxRestartsReached`] when the limit is hit.
    fn handle_renderer_exit(_model: &mut Self::Model, _reason: ExitReason) {}

    /// Restart policy for wire mode.
    ///
    /// The default policy restarts up to five times with exponential
    /// backoff and a thirty-second heartbeat. Return a custom policy
    /// to adjust limits or disable auto-restart entirely
    /// (`max_restarts: 0`).
    fn restart_policy() -> RestartPolicy {
        RestartPolicy::default()
    }
}

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

/// Result type for plushie entry points.
///
/// The error type is the [`plushie::Error`](crate::Error) enum. Match
/// on specific variants to handle failure modes (spawn failure,
/// protocol mismatch, renderer exit) distinctly.
pub type Result = std::result::Result<(), Error>;

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Run the app.
///
/// Feature-agnostic entry point. The runner is selected at compile
/// time from the enabled features:
///
/// - `direct` (default): in-process iced rendering. No subprocess.
/// - `wire`: spawns a renderer binary and talks stdin/stdout.
///
/// When both features are enabled, `direct` wins. To force wire mode
/// against a specific binary, use [`run_with_renderer`].
///
/// # Wire binary discovery
///
/// Wire mode locates the renderer in this order:
///
/// 1. `PLUSHIE_BINARY_PATH` environment variable.
/// 2. `PATH` search for `plushie-renderer` (on Windows,
///    `plushie-renderer.exe`).
///
/// If neither resolves to an executable, returns
/// [`Error::BinaryNotFound`] with guidance.
///
/// # Errors
///
/// - [`Error::NoRunnerFeature`] if neither `direct` nor `wire` is
///   enabled at compile time.
/// - In direct mode: iced event-loop init failure, `init` panic,
///   or unrecoverable window-system failure.
/// - In wire mode: binary discovery failure, spawn failure, handshake
///   failure, or I/O error during the session.
pub fn run<A: App>() -> Result {
    #[cfg(feature = "direct")]
    {
        runner::direct::run::<A>()
    }
    #[cfg(all(feature = "wire", not(feature = "direct")))]
    {
        let binary = runner::wire_discovery::discover_renderer()?;
        runner::wire::run_wire::<A>(&binary)
    }
    #[cfg(not(any(feature = "direct", feature = "wire")))]
    {
        Err(Error::NoRunnerFeature)
    }
}

/// Run the app in wire mode against a specific renderer binary.
///
/// Escape hatch for apps that ship a custom renderer (for example, a
/// build with additional `PlushieWidget` implementations). The caller
/// supplies the path explicitly; no discovery is attempted.
///
/// Under the default feature set, consider pointing this at the stock
/// `plushie-renderer` binary via `env!("CARGO_BIN_EXE_plushie-renderer")`
/// from a build that depends on `plushie-renderer` as a dev-dep.
///
/// # Errors
///
/// Returns an error if the renderer binary cannot be spawned, the
/// protocol handshake fails (version mismatch or malformed hello),
/// or stdin/stdout I/O fails during the session.
#[cfg(feature = "wire")]
pub fn run_with_renderer<A: App>(binary_path: &str) -> Result {
    runner::wire::run_wire::<A>(binary_path)
}

/// Run the app in wire mode (subprocess renderer).
///
/// Spawns the renderer binary at `binary_path` and communicates
/// over stdin/stdout using the plushie wire protocol. Uses a
/// private 2-worker tokio runtime for SDK-local async work.
///
/// # Errors
///
/// Returns an error if the renderer binary cannot be spawned, the
/// protocol handshake fails (version mismatch or malformed hello),
/// or stdin/stdout I/O fails during the session.
#[doc(hidden)]
#[deprecated(
    since = "0.6.2",
    note = "use `plushie::run_with_renderer(path)` for an explicit path, or \
            `plushie::run()` to trigger auto-discovery"
)]
#[cfg(feature = "wire")]
pub fn run_wire<A: App>(binary_path: &str) -> Result {
    runner::wire::run_wire::<A>(binary_path)
}

/// Run the app in wire mode on a caller-provided tokio runtime.
///
/// Identical to [`run_with_renderer`] except SDK-local async tasks
/// ([`Command::async_task`](crate::command::Command::async_task),
/// streams, delayed events, and effect-timeout deadlines) are
/// spawned on the supplied [`tokio::runtime::Handle`] instead of
/// a privately owned runtime. Use this when the host app already
/// drives its own tokio runtime and wants to avoid a second one
/// being created.
///
/// # Errors
///
/// Same as [`run_with_renderer`].
#[cfg(feature = "wire")]
pub fn run_wire_with_runtime<A: App>(binary_path: &str, runtime: tokio::runtime::Handle) -> Result {
    runner::wire::run_wire_with_runtime::<A>(binary_path, runtime)
}
