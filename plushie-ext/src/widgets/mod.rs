//! Widget rendering: tree node to iced element mapping.
//!
//! The public API is `render()` (immutable dispatch)
//! (mutable cache pre-population). See `SharedState` for shared renderer state.

pub(crate) mod a11y;
pub mod builtins;
pub(crate) mod caches;
pub mod canvas;
mod display;
pub mod helpers;
mod input;
mod interactive;
mod layout;
pub(crate) mod overlay;
pub mod render;
mod table;
pub mod validate;

// --- Public re-exports -----------------------------------------------------

pub(crate) use caches::MAX_TREE_DEPTH;
pub use caches::SharedState;
pub(crate) use caches::hash_json_value;
pub use helpers::parse_padding_value;
pub use render::render;
pub use validate::{is_validate_props_enabled, set_validate_props};
