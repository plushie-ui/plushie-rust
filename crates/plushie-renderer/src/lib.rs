//! # plushie-renderer
//!
//! Native GUI renderer binary. Three execution modes:
//!
//! - **Windowed (default):** `plushie-renderer` - full iced rendering
//!   with real windows and GPU. Production mode. Reports
//!   `"mode": "windowed"`.
//! - **Headless:** `plushie-renderer --headless` - no display server.
//!   Real rendering via tiny-skia with persistent widget state.
//!   Accurate screenshots after interactions. For CI with visual
//!   verification.
//! - **Mock:** `plushie-renderer --mock` - no rendering. Core + wire
//!   protocol only. Stub screenshots. For fast protocol-level testing
//!   from any language.
//!
//! All modes handle scripting messages (Query, Interact, TreeHash,
//! Screenshot, Reset) for programmatic inspection and interaction.
//!
//! Wire codec auto-detection: the first byte of stdin determines the format
//! (`{` = JSON, anything else = MessagePack). Override with `--json` or
//! `--msgpack`.

mod env;
mod headless;
mod output;
mod renderer;
mod startup;
pub(crate) mod transport;

/// Entry point for the plushie renderer.
///
/// Widget packages create a `PlushieAppBuilder`, register their widgets,
/// and pass it here. The default `main.rs` simply passes an empty builder.
pub fn run(builder: plushie_widget_sdk::app::PlushieAppBuilder) -> iced::Result {
    renderer::run(builder)
}
