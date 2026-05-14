//! Shared plumbing for `[patch.crates-io]` overrides that redirect
//! plushie-rust crates to a local checkout.
//!
//! Two consumers share this logic:
//!
//! 1. The generated renderer workspace under
//!    `target/plushie-renderer/Cargo.toml`: it pulls `plushie-widget-sdk`
//!    and `plushie-renderer` in as path deps and needs the transitive
//!    internal crates patched too so `cargo build` resolves against the
//!    local checkout instead of crates.io.
//!
//! 2. The host-SDK "spec" manifest sitting in a scratch directory
//!    (e.g. `_build/plushie-renderer-spec/Cargo.toml`). Host SDKs
//!    point at widget crates as path deps; those widget crates declare
//!    `plushie-widget-sdk = "0.6"` and friends as registry deps.
//!    `cargo metadata` on the spec manifest walks the full dep graph
//!    and fails on unpublished workspace versions unless a
//!    `.cargo/config.toml` alongside the manifest redirects every
//!    plushie-rust crate to the local checkout. `cargo plushie build`
//!    writes that file before invoking discovery and runs cargo with
//!    CWD set to the manifest directory so cargo's config walk picks
//!    it up.
//!
//! Both consumers also forward any additional `[patch.crates-io]`
//! entries already declared at the plushie-rust workspace root (the
//! committed `Cargo.toml` plus any gitignored `.cargo/config.toml`
//! overrides, typically redirecting `plushie-iced-*` crates to a
//! sibling checkout). Keeping forwarding here means the rule "the
//! patches the host workspace uses are the patches the generated
//! workspace and spec manifest use" lives in one place.

use crate::Result;
use crate::generator::write_if_changed;
use std::path::{Path, PathBuf};

/// Every plushie-rust crate that gets published to crates.io and
/// therefore needs a `[patch.crates-io]` override when building
/// against a local checkout. Order is stable for reproducible output.
pub const PLUSHIE_RUST_CRATES: &[&str] = &[
    "plushie-core",
    "plushie-core-macros",
    "plushie-renderer",
    "plushie-renderer-lib",
    "plushie-widget-sdk",
];

/// Parse `[patch.crates-io]` entries from the plushie-rust source tree.
///
/// Reads both `<source>/Cargo.toml` (the committed workspace manifest)
/// and `<source>/.cargo/config.toml` (a gitignored local-dev overrides
/// file, e.g. redirecting `plushie-iced-*` crates to a sibling
/// checkout). Entries from the committed manifest come first; any
/// additional names found in the local config are appended. A name
/// declared in both files keeps the first occurrence (Cargo.toml).
///
/// Returns `(name, resolved_path)` pairs for every entry whose `path`
/// resolves to an existing directory relative to `source_path`.
///
/// Callers that need to emit patches for the plushie-rust crates
/// themselves must drop any entry whose name appears in
/// [`PLUSHIE_RUST_CRATES`]: this helper reports them faithfully, but
/// the generated workspace and scratch config always write those
/// entries explicitly from the canonical source-path layout.
pub fn forwarded_patches(source_path: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let sources = [
        source_path.join("Cargo.toml"),
        source_path.join(".cargo/config.toml"),
    ];
    for manifest in &sources {
        let Ok(contents) = std::fs::read_to_string(manifest) else {
            continue;
        };
        let Ok(parsed) = contents.parse::<toml_edit::DocumentMut>() else {
            continue;
        };
        let Some(patch) = parsed.get("patch").and_then(|p| p.get("crates-io")) else {
            continue;
        };
        let Some(table) = patch.as_table() else {
            continue;
        };
        for (name, item) in table.iter() {
            if out.iter().any(|(existing, _)| existing == name) {
                continue;
            }
            let entry = item.as_inline_table().map(|t| t.clone().into_table());
            let Some(entry) = entry else {
                continue;
            };
            let Some(path_value) = entry.get("path").and_then(|v| v.as_str()) else {
                continue;
            };
            let resolved = source_path.join(path_value);
            if resolved.is_dir() {
                out.push((name.to_string(), resolved));
            }
        }
    }
    out
}

/// Produce every `(crate_name, absolute_path)` pair that should appear
/// in a `[patch.crates-io]` block redirecting plushie-rust crates to
/// the local checkout.
///
/// The returned vec starts with the published plushie-rust crates
/// (resolved to `<source_path>/crates/<crate>`) and appends any
/// additional non-plushie forwarded patches declared at the source
/// workspace root. This keeps the renderer-workspace `Cargo.toml` and
/// the scratch `.cargo/config.toml` aligned on exactly the same set of
/// overrides.
pub fn all_patches(source_path: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = PLUSHIE_RUST_CRATES
        .iter()
        .map(|name| {
            let path = source_path.join("crates").join(name);
            ((*name).to_string(), path)
        })
        .collect();
    for (name, path) in forwarded_patches(source_path) {
        if PLUSHIE_RUST_CRATES.contains(&name.as_str()) {
            continue;
        }
        out.push((name, path));
    }
    out
}

/// Render a `[patch.crates-io]` block given pre-resolved
/// `(name, absolute_path)` pairs. The caller controls ordering; this
/// helper just emits stable TOML without re-sorting.
pub(crate) fn render_patch_block(entries: &[(String, PathBuf)]) -> String {
    let mut out = String::from("[patch.crates-io]\n");
    for (name, path) in entries {
        out.push_str(&format!(
            "{name} = {{ path = {:?} }}\n",
            path.display().to_string()
        ));
    }
    out
}

/// Write `<spec_manifest_dir>/.cargo/config.toml` containing a
/// `[patch.crates-io]` block that redirects every plushie-rust crate
/// to `source_path` and forwards any additional patches declared at
/// the source workspace root.
///
/// Cargo's config file walk starts from the current working directory
/// (not the manifest directory), so a caller that wants this file
/// picked up must invoke cargo with `current_dir(spec_manifest_dir)`.
/// `cargo plushie build` does exactly that in `discover::discover_widgets`
/// so the host-SDK spec manifest can keep its widget deps as plain
/// `plushie-widget-sdk = "0.6"` version pins without leaking the
/// plushie-rust checkout path into every widget crate.
///
/// The write is idempotent via [`write_if_changed`]: a no-op
/// re-invocation of `cargo plushie build` does not bump the mtime.
///
/// # Errors
///
/// Propagates `std::fs` failures from directory creation or the
/// underlying write.
pub fn write_scratch_cargo_config(spec_manifest_dir: &Path, source_path: &Path) -> Result<()> {
    let entries = all_patches(source_path);
    let body = format!(
        "# Auto-generated by `cargo plushie build`. Do not edit.\n\
         # Redirects plushie-rust crates.io deps to a local checkout so\n\
         # `cargo metadata` can resolve unpublished workspace versions.\n\n\
         {}",
        render_patch_block(&entries)
    );
    let path = spec_manifest_dir.join(".cargo/config.toml");
    write_if_changed(&path, &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Populate a fake plushie-rust checkout with the published crate
    /// directories so [`all_patches`] can resolve them.
    fn populate_source_checkout(source_root: &Path) {
        for name in PLUSHIE_RUST_CRATES {
            std::fs::create_dir_all(source_root.join("crates").join(name)).unwrap();
        }
    }

    #[test]
    fn all_patches_emits_every_plushie_rust_crate() {
        let dir = tempdir().unwrap();
        populate_source_checkout(dir.path());
        let entries = all_patches(dir.path());
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
        for crate_name in PLUSHIE_RUST_CRATES {
            assert!(
                names.contains(crate_name),
                "missing patch for `{crate_name}`: got {names:?}"
            );
        }
        // Absolute paths point at the populated checkout.
        for (name, path) in &entries {
            assert_eq!(
                path,
                &dir.path().join("crates").join(name),
                "path for `{name}` should resolve under source checkout"
            );
        }
    }

    #[test]
    fn all_patches_forwards_non_plushie_entries() {
        let dir = tempdir().unwrap();
        populate_source_checkout(dir.path());
        std::fs::create_dir_all(dir.path().join("../plushie-iced-sibling")).unwrap();

        let config_toml = r#"
[patch.crates-io]
plushie-iced = { path = "../plushie-iced-sibling" }
"#;
        std::fs::create_dir_all(dir.path().join(".cargo")).unwrap();
        std::fs::write(dir.path().join(".cargo/config.toml"), config_toml).unwrap();

        let entries = all_patches(dir.path());
        let iced = entries
            .iter()
            .find(|(n, _)| n == "plushie-iced")
            .expect("plushie-iced forwarded");
        assert!(
            iced.1.ends_with("plushie-iced-sibling"),
            "forwarded path resolves relative to source root: {:?}",
            iced.1
        );
    }

    #[test]
    fn all_patches_drops_plushie_rust_entries_from_forwarded_sources() {
        // Even if the source workspace declares an explicit
        // `[patch.crates-io].plushie-widget-sdk` entry, the canonical
        // `crates/plushie-widget-sdk` path always wins. Otherwise we
        // could silently emit duplicate patch entries and have cargo
        // complain.
        let dir = tempdir().unwrap();
        populate_source_checkout(dir.path());
        let cargo_toml = r#"
[workspace]
members = []

[patch.crates-io]
plushie-widget-sdk = { path = "some/weird/other/path" }
"#;
        std::fs::create_dir_all(dir.path().join("some/weird/other/path")).unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), cargo_toml).unwrap();

        let entries = all_patches(dir.path());
        let sdk_entries: Vec<&PathBuf> = entries
            .iter()
            .filter(|(n, _)| n == "plushie-widget-sdk")
            .map(|(_, p)| p)
            .collect();
        assert_eq!(
            sdk_entries.len(),
            1,
            "only the canonical plushie-widget-sdk patch survives"
        );
        assert_eq!(
            sdk_entries[0],
            &dir.path().join("crates/plushie-widget-sdk")
        );
    }

    #[test]
    fn write_scratch_cargo_config_emits_all_plushie_patches() {
        let source = tempdir().unwrap();
        populate_source_checkout(source.path());
        let spec = tempdir().unwrap();

        write_scratch_cargo_config(spec.path(), source.path()).unwrap();

        let config_path = spec.path().join(".cargo/config.toml");
        let body = std::fs::read_to_string(&config_path).unwrap();

        assert!(body.contains("[patch.crates-io]"));
        for name in PLUSHIE_RUST_CRATES {
            let expected_path = source.path().join("crates").join(name);
            let expected_line = format!(
                "{name} = {{ path = {:?} }}",
                expected_path.display().to_string()
            );
            assert!(
                body.contains(&expected_line),
                "config should contain `{expected_line}`\nactual:\n{body}"
            );
        }
    }

    #[test]
    fn write_scratch_cargo_config_forwards_plushie_iced_patch() {
        let source = tempdir().unwrap();
        populate_source_checkout(source.path());
        std::fs::create_dir_all(source.path().join("../plushie-iced-sibling")).unwrap();
        std::fs::create_dir_all(source.path().join(".cargo")).unwrap();
        let src_config = r#"
[patch.crates-io]
plushie-iced = { path = "../plushie-iced-sibling" }
"#;
        std::fs::write(source.path().join(".cargo/config.toml"), src_config).unwrap();

        let spec = tempdir().unwrap();
        write_scratch_cargo_config(spec.path(), source.path()).unwrap();

        let body = std::fs::read_to_string(spec.path().join(".cargo/config.toml")).unwrap();
        assert!(
            body.contains("plushie-iced = {"),
            "forwarded plushie-iced patch should appear in scratch config:\n{body}"
        );
    }

    #[test]
    fn write_scratch_cargo_config_is_idempotent() {
        let source = tempdir().unwrap();
        populate_source_checkout(source.path());
        let spec = tempdir().unwrap();

        write_scratch_cargo_config(spec.path(), source.path()).unwrap();
        let config_path = spec.path().join(".cargo/config.toml");
        let mtime1 = std::fs::metadata(&config_path).unwrap().modified().unwrap();

        std::thread::sleep(std::time::Duration::from_millis(20));
        write_scratch_cargo_config(spec.path(), source.path()).unwrap();
        let mtime2 = std::fs::metadata(&config_path).unwrap().modified().unwrap();

        assert_eq!(
            mtime1, mtime2,
            "write_if_changed must skip identical content to preserve mtime"
        );
    }
}
