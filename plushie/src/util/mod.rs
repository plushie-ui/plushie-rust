//! Utility data structures for app state management.
//!
//! Pure data structures with no external dependencies. Available
//! in both direct and wire modes.
//!
//! - [`Selection`]: Single/multi/range selection state for lists and tables.
//! - [`UndoStack`]: Reversible state history with bounded size.
//! - [`Route`]: Navigation stack with parameters.
//! - [`Query`]: Composable filter/sort/paginate pipeline.

mod data;
mod route;
mod selection;
mod undo;

pub use data::{Query, QueryResult};
pub use route::{Route, RouteEntry};
pub use selection::{Selection, SelectionMode};
pub use undo::UndoStack;
