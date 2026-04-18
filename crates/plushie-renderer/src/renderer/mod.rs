//! iced::daemon application entry point.
//!
//! Most runtime logic lives in `plushie-renderer-lib` so it can be
//! shared with the WASM entry point. This module provides the native
//! entry point (`run`) and stdin I/O, then delegates to
//! `plushie-renderer-lib` for the iced daemon, event handling, and
//! output.

mod run;
pub(crate) mod stdin;

pub(crate) use run::run;
