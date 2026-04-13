//! iced::daemon application entry point.
//!
//! The core renderer logic lives in the `plushie-renderer` crate. This
//! module provides the native entry point (`run`) and stdin I/O, then
//! delegates to plushie-renderer for the iced daemon, event handling,
//! and output.

mod run;
pub(crate) mod stdin;

pub(crate) use run::run;
