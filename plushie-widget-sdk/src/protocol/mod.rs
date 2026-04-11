//! Wire protocol types for host-renderer communication.
//!
//! [`IncomingMessage`] is deserialized from the host. [`OutgoingEvent`]
//! and response types are serialized back. The transport (stdin/stdout,
//! socket, test harness) is handled by the binary crate, not here.
//!
//! Every wire message carries a `session` field identifying the logical
//! session it belongs to. [`SessionMessage`] pairs a session ID with a
//! deserialized [`IncomingMessage`]. All outgoing types include a
//! `session` field that echoes the originating session ID back.

//! Wire protocol types for host-renderer communication.
//!
//! All protocol types are defined in [`plushie_core::protocol`] and
//! re-exported here. This module adds iced-dependent extension methods
//! (keyboard event constructors).

// Iced-dependent extension methods on core protocol types.
mod outgoing_ext;

// Re-export all protocol types from plushie-core.
pub use plushie_core::protocol::*;

// Re-export the extension trait so callers get the keyboard event
// constructors via `use plushie_widget_sdk::protocol::*`.
pub use outgoing_ext::OutgoingEventKeyExt;
