//! Widget discovery via `cargo metadata`.
//!
//! Ported from the Elixir implementation at
//! `lib/mix/tasks/plushie.build.ex` (widget discovery + three
//! collision checks). Rust's path here uses `cargo_metadata` to walk
//! the full dep graph and filter packages that declare
//! `[package.metadata.plushie.widget]`.
//!
//! See the crate-level rustdoc for the metadata schema.

use crate::{Error, Result, WidgetMetadata};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Walk the cargo metadata dep graph and return every package that
/// carries a `[package.metadata.plushie.widget]` table.
///
/// `manifest_dir` is the directory containing the app crate's
/// `Cargo.toml`. Metadata is resolved via `cargo_metadata`; the call
/// requires `cargo` on `PATH`.
///
/// # Errors
///
/// Returns [`Error::CargoMetadata`] when `cargo metadata` fails, and
/// [`Error::InvalidWidgetMetadata`] when a declared table is missing
/// required keys (`type_name`, `constructor`).
pub fn discover_widgets(manifest_dir: &Path) -> Result<Vec<WidgetMetadata>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .exec()
        .map_err(|e| Error::CargoMetadata(e.to_string()))?;

    let mut widgets = Vec::new();

    for pkg in &metadata.packages {
        let Some(widget_meta) = pkg.metadata.get("plushie").and_then(|v| v.get("widget")) else {
            continue;
        };

        let type_name = widget_meta
            .get("type_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidWidgetMetadata {
                crate_name: pkg.name.to_string(),
                reason: "missing `type_name` field".to_string(),
            })?
            .to_string();

        let constructor = widget_meta
            .get("constructor")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidWidgetMetadata {
                crate_name: pkg.name.to_string(),
                reason: "missing `constructor` field".to_string(),
            })?
            .to_string();

        let crate_path = PathBuf::from(&pkg.manifest_path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();

        widgets.push(WidgetMetadata {
            crate_name: pkg.name.to_string(),
            crate_path,
            type_name,
            constructor,
        });
    }

    // Deterministic order for reproducible output.
    widgets.sort_by(|a, b| a.crate_name.cmp(&b.crate_name));
    Ok(widgets)
}

/// Fail if any two widgets share a `type_name`.
///
/// # Errors
///
/// Returns [`Error::DuplicateTypeName`] on the first collision
/// detected.
pub fn check_type_name_collisions(widgets: &[WidgetMetadata]) -> Result<()> {
    let mut by_type: HashMap<&str, Vec<&str>> = HashMap::new();
    for w in widgets {
        by_type.entry(&w.type_name).or_default().push(&w.crate_name);
    }
    for (type_name, crates) in by_type {
        if crates.len() > 1 {
            return Err(Error::DuplicateTypeName {
                type_name: type_name.to_string(),
                crates: crates.join(", "),
            });
        }
    }
    Ok(())
}

/// Fail if any widget shadows a built-in name.
///
/// `builtins` is the renderer's reserved list (usually
/// `plushie_widget_sdk::BUILTIN_TYPE_NAMES`). The build tool accepts
/// it as a slice so library consumers can inject a mock list in tests.
///
/// # Errors
///
/// Returns [`Error::BuiltinCollision`] on the first collision
/// detected.
pub fn check_builtin_collisions(widgets: &[WidgetMetadata], builtins: &[&str]) -> Result<()> {
    for w in widgets {
        if builtins.contains(&w.type_name.as_str()) {
            return Err(Error::BuiltinCollision {
                crate_name: w.crate_name.clone(),
                type_name: w.type_name.clone(),
            });
        }
    }
    Ok(())
}

/// Fail if any two widgets produce the same crate basename.
///
/// Two widgets at `native/widget/` and `other/widget/` both produce a
/// `widget` crate directory and cannot coexist in a Cargo workspace.
///
/// # Errors
///
/// Returns [`Error::DuplicateCrateBasename`] on the first collision.
pub fn check_crate_basename_collisions(widgets: &[WidgetMetadata]) -> Result<()> {
    let mut by_base: HashMap<String, Vec<String>> = HashMap::new();
    for w in widgets {
        let base = w
            .crate_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| w.crate_name.clone());
        by_base.entry(base).or_default().push(w.crate_name.clone());
    }
    for (basename, crates) in by_base {
        if crates.len() > 1 {
            return Err(Error::DuplicateCrateBasename {
                basename,
                crates: crates.join(", "),
            });
        }
    }
    Ok(())
}

/// Run every discovery-time collision check in one call.
///
/// # Errors
///
/// Propagates the first failing check's error (duplicate type name,
/// built-in shadow, or duplicate crate basename).
pub fn check_all_collisions(widgets: &[WidgetMetadata], builtins: &[&str]) -> Result<()> {
    check_type_name_collisions(widgets)?;
    check_builtin_collisions(widgets, builtins)?;
    check_crate_basename_collisions(widgets)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wm(crate_name: &str, type_name: &str, path: &str) -> WidgetMetadata {
        WidgetMetadata {
            crate_name: crate_name.to_string(),
            crate_path: PathBuf::from(path),
            type_name: type_name.to_string(),
            constructor: format!("{crate_name}::new()"),
        }
    }

    #[test]
    fn accepts_unique_widgets() {
        let widgets = vec![
            wm("my-gauge", "my_gauge", "native/gauge"),
            wm("my-sparkline", "my_sparkline", "native/sparkline"),
        ];
        assert!(check_all_collisions(&widgets, &["button", "text"]).is_ok());
    }

    #[test]
    fn detects_duplicate_type_names() {
        let widgets = vec![
            wm("pkg-a", "dup", "native/a"),
            wm("pkg-b", "dup", "native/b"),
        ];
        let err = check_type_name_collisions(&widgets).unwrap_err();
        assert!(matches!(err, Error::DuplicateTypeName { .. }));
    }

    #[test]
    fn detects_builtin_shadow() {
        let widgets = vec![wm("shadow-pkg", "button", "native/shadow")];
        let err = check_builtin_collisions(&widgets, &["button", "text"]).unwrap_err();
        assert!(matches!(err, Error::BuiltinCollision { .. }));
    }

    #[test]
    fn detects_duplicate_crate_basenames() {
        let widgets = vec![
            wm("pkg-a", "a", "native/widget"),
            wm("pkg-b", "b", "other/widget"),
        ];
        let err = check_crate_basename_collisions(&widgets).unwrap_err();
        assert!(matches!(err, Error::DuplicateCrateBasename { .. }));
    }
}
