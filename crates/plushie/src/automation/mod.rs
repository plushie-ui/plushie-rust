//! Automation primitives for programmatic UI interaction.
//!
//! This module provides production-capable building blocks for
//! driving plushie apps from external agents, accessibility
//! harnesses, test frameworks, or automation scripts. It is NOT
//! test-only infrastructure.
//!
//! # Key types
//!
//! - [`Selector`] identifies widgets by ID, text, role, label, or
//!   focus state. Supports window-qualified IDs (`"main#save"`).
//! - [`Element`] is a typed wrapper over tree nodes with accessors
//!   for text content, accessibility properties, and widget props.
//!
//! # Usage
//!
//! ```ignore
//! use plushie::automation::{Selector, Element};
//!
//! // Find a widget by role
//! let sel = Selector::role("button");
//! if let Some(elem) = sel.find(&tree).map(Element::new) {
//!     println!("Found button: {:?}", elem.text());
//! }
//!
//! // Find by visible text
//! let all = Selector::text("Save").find_all(&tree);
//! ```

pub mod cli;
mod element;
pub mod file;
pub mod runner;
#[cfg(feature = "wire")]
pub mod runner_wire;

pub use element::Element;
pub use plushie_core::Selector;

/// Renderer backend an automation script runs against.
///
/// Mirrors the `backend:` header field parsed by
/// [`file::Header::backend`] and the equivalent routing in Elixir's
/// `Plushie.Automation.Runner`:
///
/// - [`Backend::Mock`] runs the script against a headless
///   [`crate::test::TestSession`], no renderer subprocess. Same
///   semantics as Elixir's `--mock` pool.
/// - [`Backend::Headless`] runs against a [`crate::test::TestSession`]
///   too. A real headless renderer (tiny-skia, no display server) is
///   only needed for screenshot capture; the MVU-exercise path is
///   identical to mock.
/// - [`Backend::Windowed`] spawns the real `plushie-renderer` binary
///   in windowed mode so the user can watch the script execute. The
///   runner drives the MVU loop locally and mirrors tree snapshots to
///   the renderer subprocess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// No renderer. Script exercises the MVU cycle in-process.
    Mock,
    /// Headless rendering. Today behaves the same as [`Backend::Mock`]
    /// for automation purposes; the distinction exists so future
    /// screenshot / tree-hash capture can route through a real
    /// tiny-skia renderer.
    Headless,
    /// Windowed rendering. A real renderer subprocess is spawned and
    /// the scripted interactions mirror to it so the user sees the
    /// script execute visually.
    Windowed,
}

impl Backend {
    /// Parse the value of a `backend:` header field. Unknown values
    /// return `None`; the caller decides whether to error out or fall
    /// back to a default.
    pub fn from_header(value: &str) -> Option<Self> {
        match value {
            "mock" => Some(Backend::Mock),
            "headless" => Some(Backend::Headless),
            "windowed" => Some(Backend::Windowed),
            _ => None,
        }
    }
}
