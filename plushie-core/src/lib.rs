//! Core types and protocol for Plushie.
//!
//! This crate contains the shared data types used by both the Plushie
//! SDK and the renderer. It has no iced dependency, making it suitable
//! for wire-mode apps, FFI bindings, and tooling that doesn't need
//! the full rendering stack.

pub mod animation;
pub mod ops;
pub mod protocol;
pub mod settings;
pub mod types;
