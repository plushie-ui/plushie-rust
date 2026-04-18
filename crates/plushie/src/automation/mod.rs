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

pub use element::Element;
pub use plushie_core::Selector;
