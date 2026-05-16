//! Shared types and utilities for `cargo plushie`.
//!
//! The crate is split between thin binary wrappers, CLI parsing, and
//! this library module. Keeping the logic in a library makes
//! integration testing possible without spawning the binary.
//!
//! Commands:
//!
//! - `cargo plushie build` - generate a custom renderer workspace
//!   under `target/plushie-renderer/` with every native widget in the
//!   dep graph registered, then run `cargo build`.
//! - `cargo plushie download` - fetch a precompiled stock renderer
//!   from GitHub releases and place it under `bin/`.
//! - `cargo plushie package portable` - build a standalone launcher from a
//!   Plushie package manifest and payload archive.
//! - `cargo plushie package assemble` - complete a partial SDK manifest,
//!   archive the payload dir, and hand off to `package portable`.
//! - `cargo plushie package-rust assemble` - build a wire-mode Rust app
//!   payload and hand it to the shared package launcher.

#![deny(missing_docs)]

/// Metadata describing a single native widget as declared in its
/// `[package.metadata.plushie.widget]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetMetadata {
    /// Cargo package name.
    pub crate_name: String,
    /// On-disk path to the widget crate's manifest directory.
    pub crate_path: std::path::PathBuf,
    /// Wire-protocol type name registered by the widget.
    pub type_name: String,
    /// Rust constructor expression (e.g. `my_gauge::Gauge::new()`).
    pub constructor: String,
}

/// Error type returned by library functions in this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Cargo metadata failed to produce a dep graph.
    #[error("cargo metadata failed: {0}")]
    CargoMetadata(String),
    /// A widget crate declared an incomplete or invalid
    /// `[package.metadata.plushie.widget]` table.
    #[error("invalid widget metadata for `{crate_name}`: {reason}")]
    InvalidWidgetMetadata {
        /// Cargo package whose manifest was malformed.
        crate_name: String,
        /// Human-readable description of what was missing or wrong.
        reason: String,
    },
    /// Two widgets declared the same `type_name`.
    #[error("widget type name `{type_name}` is registered by multiple crates: {crates}")]
    DuplicateTypeName {
        /// The colliding wire-protocol type name.
        type_name: String,
        /// Comma-separated list of crate names in collision.
        crates: String,
    },
    /// A widget declared a type name that is already registered by the
    /// built-in iced widget set.
    #[error(
        "widget `{crate_name}` declares type name `{type_name}` which shadows a built-in widget"
    )]
    BuiltinCollision {
        /// Cargo package responsible for the collision.
        crate_name: String,
        /// Wire-protocol type name that conflicts with a built-in.
        type_name: String,
    },
    /// Two widgets produced the same crate basename.
    #[error("widget crate basename `{basename}` is shared by multiple crates: {crates}")]
    DuplicateCrateBasename {
        /// The crate basename that collided.
        basename: String,
        /// Comma-separated list of crate names / paths in collision.
        crates: String,
    },
    /// A widget's `constructor` expression does not look like a valid
    /// Rust path or zero-argument call.
    #[error(
        "widget `{crate_name}` constructor `{constructor}` is not a valid Rust \
         path or zero-argument call expression"
    )]
    InvalidConstructor {
        /// Widget crate name.
        crate_name: String,
        /// Constructor expression that failed validation.
        constructor: String,
    },
    /// Generic I/O failure during build or download.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Inner `cargo build` invocation exited with a non-zero status.
    #[error("`cargo build` failed with status {0}")]
    CargoBuildFailed(std::process::ExitStatus),
    /// A `cargo plushie download` call found native widgets in the
    /// dep graph; the stock binary cannot register them.
    #[error(
        "native widgets detected ({widgets}); stock binary cannot register them. \
         Run `cargo plushie build` to produce a custom renderer instead."
    )]
    DownloadWithNativeWidgets {
        /// Comma-separated list of native widget crate names.
        widgets: String,
    },
    /// Any other error surfaced through `anyhow`.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Convenience `Result` alias.
pub type Result<T> = std::result::Result<T, Error>;

pub mod cli;

/// Print the version identity for a native Plushie tool.
///
/// Used by thin binary wrappers that are built and uploaded as release
/// assets alongside the renderer.
///
/// # Errors
///
/// Returns an error when the version payload cannot be serialized.
pub fn print_tool_version(tool: &str, json: bool) -> anyhow::Result<()> {
    tool_identity::print_current_version(tool, json)
}
pub mod default_icons;
pub mod discover;
pub mod doctor;
pub mod download;
pub mod generator;
pub mod package;
pub mod package_assemble;
pub mod package_runtime;
pub mod package_rust;
pub mod patch_config;
pub mod platform;
pub mod scaffold;
mod tool_identity;
