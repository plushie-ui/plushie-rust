//! Application and window configuration.
//!
//! Configuration types are defined in [`plushie_core::settings`]
//! and re-exported here. The surface is enumerated (not glob) so
//! new plushie-core settings items don't silently leak into the
//! plushie public API.

pub use plushie_core::settings::{
    // Wire-mode renderer lifecycle
    ExitReason,
    RestartPolicy,
    // App-level settings
    Settings,
    // Per-window configuration
    WindowConfig,
};
