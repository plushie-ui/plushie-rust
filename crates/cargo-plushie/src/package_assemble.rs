//! Cross-SDK generic package assembly.
//!
//! SDKs build their payload directory and write a partial
//! `plushie-package.toml`, then call `cargo plushie package assemble
//! --manifest <path> --payload-dir <path>` to complete the manifest,
//! create the deterministic archive, and hand off to
//! `cargo plushie package portable`.
//!
//! This module owns that final step: walking the payload dir, materializing
//! a default icon when none is declared, writing the deterministic tar.zst
//! archive, computing its SHA-256, and overwriting the partial manifest with
//! a complete one containing `[start]` and `[payload]`.

use crate::{Error, Result, default_icons, generator, package, platform};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

const PAYLOAD_ARCHIVE: &str = "payload.tar.zst";
const DEFAULT_ICON_NAME: &str = "default-app-icon-512.png";
const DEFAULT_ICON_PAYLOAD_PATH: &str = "assets/default-app-icon-512.png";

/// Options for the cross-SDK generic package assembly step.
#[derive(Debug)]
pub struct AssembleOpts<'a> {
    /// Path to the partial `plushie-package.toml` written by the SDK.
    pub manifest_path: &'a Path,
    /// Path to the fully assembled payload directory tree.
    pub payload_dir: &'a Path,
    /// Optional developer-owned package config. Defaults to
    /// `plushie-package.config.toml` next to the manifest when present.
    pub package_config: Option<&'a Path>,
}

/// Result of the cross-SDK generic package assembly step.
#[derive(Debug)]
pub struct AssembleResult {
    /// Final package manifest path (overwritten in place).
    pub manifest_path: PathBuf,
    /// Payload archive written next to the manifest.
    pub payload_archive_path: PathBuf,
}

// ---------------------------------------------------------------------------
// Partial manifest schema
//
// Mirrors the complete manifest but with [start] and [payload] optional so
// the SDK can omit them. All other required fields must be present.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialManifest {
    schema_version: u32,
    app_id: String,
    app_name: Option<String>,
    app_version: String,
    target: Option<String>,
    host_sdk: String,
    host_sdk_version: Option<String>,
    plushie_rust_version: String,
    protocol_version: u32,
    start: Option<PartialStartManifest>,
    renderer: PartialRendererManifest,
    platform: Option<PartialPlatformManifest>,
    updates: Option<PartialUpdatesManifest>,
    signing: Option<PartialSigningManifest>,
    licenses: Option<Vec<PartialLicenseEntry>>,
    sbom: Option<Vec<PartialSbomEntry>>,
    // Accepted but discarded; a fresh [payload] is always computed.
    #[allow(dead_code)]
    payload: Option<PartialPayloadManifest>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialStartManifest {
    working_dir: String,
    command: Vec<String>,
    forward_env: Vec<String>,
}

impl PartialStartManifest {
    fn to_start_config(&self) -> package::PackageStartConfig {
        package::PackageStartConfig {
            working_dir: self.working_dir.clone(),
            command: self.command.clone(),
            forward_env: self.forward_env.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialRendererManifest {
    path: String,
    kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialPlatformManifest {
    publisher: Option<String>,
    bundle_id: Option<String>,
    icon: Option<String>,
    copyright: Option<String>,
    category: Option<String>,
    description: Option<String>,
    macos: Option<PartialPlatformMacosManifest>,
    windows: Option<PartialPlatformWindowsManifest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialPlatformMacosManifest {
    bundle_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialPlatformWindowsManifest {
    install_scope: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialUpdatesManifest {
    channel: String,
    feed_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialSigningManifest {
    #[serde(default)]
    hooks: Vec<PartialSigningHookManifest>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialSigningHookManifest {
    phase: String,
    command: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialLicenseEntry {
    name: String,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialSbomEntry {
    format: String,
    path: String,
}

// Accepted during deserialization so that deny_unknown_fields does not
// reject a partial manifest that already has a [payload] section.
// The fields are intentionally discarded; assembly always writes a fresh
// [payload] from the newly created archive.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct PartialPayloadManifest {
    archive: Option<String>,
}

// ---------------------------------------------------------------------------
// Final manifest schema (serialized after assembly is complete)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct FinalManifest {
    schema_version: u32,
    app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_name: Option<String>,
    app_version: String,
    target: String,
    host_sdk: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_sdk_version: Option<String>,
    plushie_rust_version: String,
    protocol_version: u32,
    start: FinalStartManifest,
    renderer: FinalRendererManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    platform: Option<FinalPlatformManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updates: Option<FinalUpdatesManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signing: Option<FinalSigningManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    licenses: Option<Vec<FinalLicenseEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sbom: Option<Vec<FinalSbomEntry>>,
    payload: FinalPayloadManifest,
}

#[derive(Serialize)]
struct FinalStartManifest {
    working_dir: String,
    command: Vec<String>,
    forward_env: Vec<String>,
}

#[derive(Serialize)]
struct FinalRendererManifest {
    path: String,
    kind: String,
}

#[derive(Serialize)]
struct FinalPlatformManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    copyright: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    macos: Option<FinalPlatformMacosManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    windows: Option<FinalPlatformWindowsManifest>,
}

#[derive(Serialize)]
struct FinalPlatformMacosManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_version: Option<String>,
}

#[derive(Serialize)]
struct FinalPlatformWindowsManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    install_scope: Option<String>,
}

#[derive(Serialize)]
struct FinalUpdatesManifest {
    channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    feed_url: Option<String>,
}

#[derive(Serialize)]
struct FinalSigningManifest {
    hooks: Vec<FinalSigningHookManifest>,
}

#[derive(Serialize)]
struct FinalSigningHookManifest {
    phase: String,
    command: Vec<String>,
}

#[derive(Serialize)]
struct FinalLicenseEntry {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct FinalSbomEntry {
    format: String,
    path: String,
}

#[derive(Serialize)]
struct FinalPayloadManifest {
    archive: String,
    hash: String,
    size: u64,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Complete a partial SDK-written manifest by archiving the payload dir and
/// filling in `[start]` and `[payload]`.
///
/// # Errors
///
/// Returns an error when the manifest is missing required fields, the payload
/// dir contains unsafe entries (symlinks, hardlinks, special files), archiving
/// fails, or the resulting manifest fails precheck.
pub fn assemble_package(opts: &AssembleOpts<'_>) -> Result<AssembleResult> {
    let manifest_path = std::fs::canonicalize(opts.manifest_path).with_context(|| {
        format!(
            "partial manifest `{}` not found",
            opts.manifest_path.display()
        )
    })?;
    let manifest_dir = manifest_path
        .parent()
        .ok_or_else(|| Error::Other(anyhow::anyhow!("manifest path has no parent directory")))?;
    let payload_dir = std::fs::canonicalize(opts.payload_dir)
        .with_context(|| format!("payload dir `{}` not found", opts.payload_dir.display()))?;

    let manifest_text = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read `{}`", manifest_path.display()))?;
    let partial: PartialManifest =
        toml::from_str(&manifest_text).with_context(|| "failed to parse partial manifest")?;

    validate_partial_manifest(&partial)?;

    let resolved_source_config = resolve_source_config(opts, manifest_dir)?;
    let source_config = resolved_source_config.as_ref().map(|r| &r.config);

    let start = resolve_start(partial.start.as_ref(), source_config)?;
    package::validate_start_config(&start)?;

    copy_bundled_assets(resolved_source_config.as_ref(), manifest_dir, &payload_dir)?;

    let mut platform = partial.platform.clone();
    materialize_default_icon_if_needed(&mut platform, &payload_dir)?;

    validate_payload_dir(&payload_dir)?;

    let archive_path = manifest_dir.join(PAYLOAD_ARCHIVE);
    write_payload_archive(&payload_dir, &archive_path)?;
    let archive_bytes = std::fs::read(&archive_path)?;
    let hash = format!("sha256:{:x}", Sha256::digest(&archive_bytes));
    let size = archive_bytes.len() as u64;

    let final_manifest = FinalManifest {
        schema_version: partial.schema_version,
        app_id: partial.app_id,
        app_name: partial.app_name,
        app_version: partial.app_version,
        target: partial.target.unwrap_or_default(),
        host_sdk: partial.host_sdk,
        host_sdk_version: partial.host_sdk_version,
        plushie_rust_version: partial.plushie_rust_version,
        protocol_version: partial.protocol_version,
        start: FinalStartManifest {
            working_dir: start.working_dir,
            command: start.command,
            forward_env: start.forward_env,
        },
        renderer: FinalRendererManifest {
            path: partial.renderer.path,
            kind: partial.renderer.kind,
        },
        platform: platform.map(|p| FinalPlatformManifest {
            publisher: p.publisher,
            bundle_id: p.bundle_id,
            icon: p.icon,
            copyright: p.copyright,
            category: p.category,
            description: p.description,
            macos: p.macos.map(|m| FinalPlatformMacosManifest {
                bundle_version: m.bundle_version,
            }),
            windows: p.windows.map(|w| FinalPlatformWindowsManifest {
                install_scope: w.install_scope,
            }),
        }),
        updates: partial.updates.map(|u| FinalUpdatesManifest {
            channel: u.channel,
            feed_url: u.feed_url,
        }),
        signing: partial.signing.map(|s| FinalSigningManifest {
            hooks: s
                .hooks
                .into_iter()
                .map(|h| FinalSigningHookManifest {
                    phase: h.phase,
                    command: h.command,
                })
                .collect(),
        }),
        licenses: partial.licenses.map(|ls| {
            ls.into_iter()
                .map(|l| FinalLicenseEntry {
                    name: l.name,
                    path: l.path,
                })
                .collect()
        }),
        sbom: partial.sbom.map(|ss| {
            ss.into_iter()
                .map(|s| FinalSbomEntry {
                    format: s.format,
                    path: s.path,
                })
                .collect()
        }),
        payload: FinalPayloadManifest {
            archive: PAYLOAD_ARCHIVE.to_string(),
            hash,
            size,
        },
    };

    let final_text = toml::to_string_pretty(&final_manifest)
        .with_context(|| "serialize completed package manifest")?;
    generator::write_if_changed(&manifest_path, &final_text)?;

    package::precheck_package(&manifest_path)?;

    Ok(AssembleResult {
        manifest_path: manifest_path.to_path_buf(),
        payload_archive_path: archive_path,
    })
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

const MANIFEST_SCHEMA_VERSION: u32 = 1;
const EXPECTED_PLUSHIE_RUST_VERSION: &str = env!("CARGO_PKG_VERSION");
const EXPECTED_PROTOCOL_VERSION: u32 = plushie_core::protocol::PROTOCOL_VERSION;

fn validate_partial_manifest(manifest: &PartialManifest) -> Result<()> {
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(Error::Other(anyhow::anyhow!(
            "unsupported package manifest schema_version {}",
            manifest.schema_version
        )));
    }
    require_nonempty("app_id", &manifest.app_id)?;
    validate_app_id(&manifest.app_id)?;
    if let Some(app_name) = &manifest.app_name {
        require_nonempty("app_name", app_name)?;
    }
    require_nonempty("app_version", &manifest.app_version)?;
    let target = manifest
        .target
        .as_deref()
        .ok_or_else(|| Error::Other(anyhow::anyhow!("target must be set")))?;
    require_nonempty("target", target)?;
    validate_package_target(target)?;
    validate_current_package_target(target)?;
    require_nonempty("host_sdk", &manifest.host_sdk)?;
    if let Some(host_sdk_version) = &manifest.host_sdk_version {
        require_nonempty("host_sdk_version", host_sdk_version)?;
    }
    require_nonempty("plushie_rust_version", &manifest.plushie_rust_version)?;
    if manifest.plushie_rust_version != EXPECTED_PLUSHIE_RUST_VERSION {
        return Err(Error::Other(anyhow::anyhow!(
            "plushie_rust_version mismatch: package expects {}, cargo-plushie is {}",
            manifest.plushie_rust_version,
            EXPECTED_PLUSHIE_RUST_VERSION
        )));
    }
    if manifest.protocol_version != EXPECTED_PROTOCOL_VERSION {
        return Err(Error::Other(anyhow::anyhow!(
            "protocol_version mismatch: package expects {}, cargo-plushie supports {}",
            manifest.protocol_version,
            EXPECTED_PROTOCOL_VERSION
        )));
    }
    require_nonempty("renderer.path", &manifest.renderer.path)?;
    validate_payload_relative_path("renderer.path", &manifest.renderer.path, false)?;
    require_nonempty("renderer.kind", &manifest.renderer.kind)?;
    match manifest.renderer.kind.as_str() {
        "stock" | "custom" => {}
        value => {
            return Err(Error::Other(anyhow::anyhow!(
                "renderer.kind must be `stock` or `custom`, got `{value}`"
            )));
        }
    }
    validate_partial_platform_metadata(manifest)?;
    Ok(())
}

fn validate_partial_platform_metadata(manifest: &PartialManifest) -> Result<()> {
    let Some(platform) = &manifest.platform else {
        return Ok(());
    };
    if let Some(publisher) = &platform.publisher {
        require_nonempty("platform.publisher", publisher)?;
    }
    if let Some(bundle_id) = &platform.bundle_id {
        require_nonempty("platform.bundle_id", bundle_id)?;
    }
    if let Some(icon) = &platform.icon {
        require_nonempty("platform.icon", icon)?;
        validate_payload_relative_path("platform.icon", icon, false)?;
    }
    if let Some(copyright) = &platform.copyright {
        require_nonempty("platform.copyright", copyright)?;
    }
    if let Some(category) = &platform.category {
        require_nonempty("platform.category", category)?;
    }
    if let Some(description) = &platform.description {
        require_nonempty("platform.description", description)?;
    }
    if let Some(macos) = &platform.macos
        && let Some(bundle_version) = &macos.bundle_version
    {
        require_nonempty("platform.macos.bundle_version", bundle_version)?;
    }
    if let Some(windows) = &platform.windows
        && let Some(install_scope) = &windows.install_scope
    {
        require_nonempty("platform.windows.install_scope", install_scope)?;
        match install_scope.as_str() {
            "perUser" | "perMachine" => {}
            value => {
                return Err(Error::Other(anyhow::anyhow!(
                    "platform.windows.install_scope must be `perUser` or `perMachine`, got `{value}`"
                )));
            }
        }
    }
    Ok(())
}

fn require_nonempty(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::Other(anyhow::anyhow!("{name} must not be empty")));
    }
    Ok(())
}

fn validate_app_id(value: &str) -> Result<()> {
    let is_valid = {
        let mut segments = value.split('.');
        let all_valid = segments.by_ref().all(|seg| {
            !seg.is_empty()
                && seg.starts_with(|c: char| c.is_ascii_lowercase())
                && seg
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        });
        let segment_count = value.split('.').count();
        all_valid && segment_count >= 2
    };
    if !is_valid {
        return Err(Error::Other(anyhow::anyhow!(
            "app_id must be lowercase reverse-DNS like 'com.example.appname', got: '{value}'"
        )));
    }
    Ok(())
}

fn validate_package_target(value: &str) -> Result<()> {
    let Some((os, arch)) = value.split_once('-') else {
        return Err(Error::Other(anyhow::anyhow!(
            "target must use `<os>-<arch>`, got `{value}`"
        )));
    };
    if !matches!(os, "linux" | "darwin" | "windows") {
        return Err(Error::Other(anyhow::anyhow!(
            "target OS must be linux, darwin, or windows, got `{os}`"
        )));
    }
    if !matches!(arch, "x86_64" | "aarch64") {
        return Err(Error::Other(anyhow::anyhow!(
            "target architecture must be x86_64 or aarch64, got `{arch}`"
        )));
    }
    Ok(())
}

fn validate_current_package_target(value: &str) -> Result<()> {
    let current = format!("{}-{}", platform::os_name(), platform::arch_name());
    if value != current {
        return Err(Error::Other(anyhow::anyhow!(
            "target `{value}` does not match current build host `{current}`; cross-target package manifests are not supported yet"
        )));
    }
    Ok(())
}

fn validate_payload_relative_path(name: &str, value: &str, allow_dot: bool) -> Result<()> {
    let path = clean_relative_path(name, value)?;
    if path.as_os_str().is_empty() && !allow_dot {
        return Err(Error::Other(anyhow::anyhow!(
            "{name} must name a payload file path"
        )));
    }
    Ok(())
}

fn clean_relative_path(name: &str, value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(Error::Other(anyhow::anyhow!(
            "{name} must be payload-relative, got absolute path `{value}`"
        )));
    }
    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => cleaned.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(Error::Other(anyhow::anyhow!(
                    "{name} must not contain parent traversal: `{value}`"
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(Error::Other(anyhow::anyhow!(
                    "{name} must be payload-relative: `{value}`"
                )));
            }
        }
    }
    Ok(cleaned)
}

// ---------------------------------------------------------------------------
// Source config resolution
// ---------------------------------------------------------------------------

/// Resolved source config plus the directory it was loaded from.
///
/// The directory is the base used to resolve project-relative paths in
/// the config (e.g. `[assets].dir`).
struct ResolvedSourceConfig {
    config: package::PackageSourceConfig,
    config_dir: PathBuf,
}

fn resolve_source_config(
    opts: &AssembleOpts<'_>,
    manifest_dir: &Path,
) -> Result<Option<ResolvedSourceConfig>> {
    match opts.package_config {
        Some(path) => {
            let config = package::load_source_config(path)?;
            let config_dir = path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            // Canonicalize so relative source paths resolve consistently.
            let config_dir = std::fs::canonicalize(&config_dir).unwrap_or(config_dir);
            Ok(Some(ResolvedSourceConfig { config, config_dir }))
        }
        None => match package::load_default_source_config(manifest_dir)? {
            Some(config) => Ok(Some(ResolvedSourceConfig {
                config,
                config_dir: manifest_dir.to_path_buf(),
            })),
            None => Ok(None),
        },
    }
}

// ---------------------------------------------------------------------------
// Bundled assets copy
// ---------------------------------------------------------------------------

const DEFAULT_ASSETS_DIR_NAME: &str = "package_assets";

/// Copy a project-relative directory verbatim into the payload root.
///
/// Resolution rules:
/// - If `[assets].dir` is set in the source config, use that path
///   (relative to the config file's directory). The directory must
///   exist; raise a clear error if not.
/// - Otherwise (no `[assets]` section, or no source config), look for
///   the convention default `package_assets/` next to the source config
///   (or, when no config exists, next to the manifest). Use it if
///   present; otherwise no-op.
///
/// Existing files in `payload_dir` are overwritten by matching asset
/// files. Empty source directories are a no-op (not an error).
fn copy_bundled_assets(
    resolved_source_config: Option<&ResolvedSourceConfig>,
    manifest_dir: &Path,
    payload_dir: &Path,
) -> Result<()> {
    let (source_dir, explicit) = match resolved_source_config {
        Some(resolved) => match resolved.config.assets.as_ref() {
            Some(assets) => (resolved.config_dir.join(&assets.dir), true),
            None => (resolved.config_dir.join(DEFAULT_ASSETS_DIR_NAME), false),
        },
        // No package config: fall back to the convention default next
        // to the manifest dir. Same opt-in semantics: only takes effect
        // if the directory exists.
        None => (manifest_dir.join(DEFAULT_ASSETS_DIR_NAME), false),
    };

    if !source_dir.exists() {
        if explicit {
            return Err(Error::Other(anyhow::anyhow!(
                "[assets].dir `{}` does not exist",
                source_dir.display()
            )));
        }
        return Ok(());
    }

    if !source_dir.is_dir() {
        return Err(Error::Other(anyhow::anyhow!(
            "assets path `{}` is not a directory",
            source_dir.display()
        )));
    }

    copy_dir_recursive(&source_dir, payload_dir)
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source)
        .with_context(|| format!("read assets dir `{}`", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("read entry in assets dir `{}`", source.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("stat assets entry `{}`", entry.path().display()))?;
        let dest_path = dest.join(entry.file_name());

        if file_type.is_dir() {
            std::fs::create_dir_all(&dest_path)
                .with_context(|| format!("create payload dir `{}`", dest_path.display()))?;
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create payload dir `{}`", parent.display()))?;
            }
            std::fs::copy(entry.path(), &dest_path).with_context(|| {
                format!(
                    "copy asset `{}` to `{}`",
                    entry.path().display(),
                    dest_path.display()
                )
            })?;
        } else {
            return Err(Error::Other(anyhow::anyhow!(
                "assets entry `{}` must be a plain file or directory",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn resolve_start(
    partial_start: Option<&PartialStartManifest>,
    source_config: Option<&package::PackageSourceConfig>,
) -> Result<package::PackageStartConfig> {
    // Source config takes precedence over the partial manifest's [start].
    if let Some(config) = source_config {
        return Ok(config.start.clone());
    }
    partial_start.map(|s| s.to_start_config()).ok_or_else(|| {
        Error::Other(anyhow::anyhow!(
            "manifest has no [start] section and no package config was found; \
                 add [start] to the partial manifest or provide a plushie-package.config.toml"
        ))
    })
}

// ---------------------------------------------------------------------------
// Default icon materialization
// ---------------------------------------------------------------------------

fn materialize_default_icon_if_needed(
    platform: &mut Option<PartialPlatformManifest>,
    payload_dir: &Path,
) -> Result<()> {
    let icon_already_set = platform.as_ref().and_then(|p| p.icon.as_deref()).is_some();

    if icon_already_set {
        return Ok(());
    }

    let default_icon_path = payload_dir.join(DEFAULT_ICON_PAYLOAD_PATH);
    if default_icon_path.is_file() {
        // Icon file exists at the default location; just record it.
        let p = platform.get_or_insert_with(|| PartialPlatformManifest {
            publisher: None,
            bundle_id: None,
            icon: None,
            copyright: None,
            category: None,
            description: None,
            macos: None,
            windows: None,
        });
        p.icon = Some(DEFAULT_ICON_PAYLOAD_PATH.to_string());
        return Ok(());
    }

    // Write the bundled default icon.
    let assets_dir = payload_dir.join("assets");
    std::fs::create_dir_all(&assets_dir)
        .with_context(|| format!("create payload assets dir `{}`", assets_dir.display()))?;
    let icon_dest = assets_dir.join(DEFAULT_ICON_NAME);
    let icon_bytes = default_icons::default_icons()
        .iter()
        .find(|i| i.name == DEFAULT_ICON_NAME)
        .expect("default icon 512 is always present")
        .bytes;
    std::fs::write(&icon_dest, icon_bytes)
        .with_context(|| format!("write default icon `{}`", icon_dest.display()))?;

    let p = platform.get_or_insert_with(|| PartialPlatformManifest {
        publisher: None,
        bundle_id: None,
        icon: None,
        copyright: None,
        category: None,
        description: None,
        macos: None,
        windows: None,
    });
    p.icon = Some(DEFAULT_ICON_PAYLOAD_PATH.to_string());
    Ok(())
}

// ---------------------------------------------------------------------------
// Payload directory validation
// ---------------------------------------------------------------------------

fn validate_payload_dir(dir: &Path) -> Result<()> {
    validate_payload_dir_recursive(dir, dir)
}

fn validate_payload_dir_recursive(root: &Path, dir: &Path) -> Result<()> {
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("read payload dir `{}`", dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("read entry in payload dir `{}`", dir.display()))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .with_context(|| format!("stat payload entry `{}`", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(Error::Other(anyhow::anyhow!(
                "payload entry `{}` must not be a symlink",
                path.display()
            )));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() > 1 && metadata.is_file() {
                return Err(Error::Other(anyhow::anyhow!(
                    "payload entry `{}` must not be a hard link (nlink > 1)",
                    path.display()
                )));
            }
        }
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(Error::Other(anyhow::anyhow!(
                "payload entry `{}` must be a plain file or directory",
                path.display()
            )));
        }
        // Validate the path component is safe.
        let relative = path.strip_prefix(root).expect("entry is under root");
        for component in relative.components() {
            match component {
                Component::Normal(_) => {}
                _ => {
                    return Err(Error::Other(anyhow::anyhow!(
                        "payload entry `{}` has an unsafe path component",
                        path.display()
                    )));
                }
            }
        }
        if metadata.is_dir() {
            validate_payload_dir_recursive(root, &path)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Deterministic archive creation
// ---------------------------------------------------------------------------

fn write_payload_archive(payload_dir: &Path, archive_path: &Path) -> Result<()> {
    if let Some(parent) = archive_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        append_dir_entries(&mut builder, payload_dir, payload_dir)?;
        builder.finish()?;
    }
    let encoded = zstd::stream::encode_all(tar_bytes.as_slice(), 0)
        .with_context(|| "compress payload archive")?;
    std::fs::write(archive_path, encoded)?;
    Ok(())
}

fn append_dir_entries(
    builder: &mut tar::Builder<&mut Vec<u8>>,
    root: &Path,
    dir: &Path,
) -> Result<()> {
    let mut entries = std::fs::read_dir(dir)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root).expect("entry under root");
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            append_directory(builder, relative, &metadata)?;
            append_dir_entries(builder, root, &path)?;
        } else if metadata.is_file() {
            append_regular_file(builder, relative, &path, &metadata)?;
        } else {
            return Err(Error::Other(anyhow::anyhow!(
                "payload entry `{}` must be a plain file or directory",
                path.display()
            )));
        }
    }
    Ok(())
}

fn append_directory(
    builder: &mut tar::Builder<&mut Vec<u8>>,
    relative: &Path,
    metadata: &std::fs::Metadata,
) -> Result<()> {
    let mut header = tar::Header::new_ustar();
    header.set_path(relative)?;
    header.set_entry_type(tar::EntryType::Directory);
    header.set_size(0);
    header.set_mode(payload_mode(metadata, true));
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    builder.append(&header, std::io::empty())?;
    Ok(())
}

fn append_regular_file(
    builder: &mut tar::Builder<&mut Vec<u8>>,
    relative: &Path,
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut header = tar::Header::new_ustar();
    header.set_path(relative)?;
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(metadata.len());
    header.set_mode(payload_mode(metadata, false));
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    builder.append(&header, &mut file)?;
    Ok(())
}

fn payload_mode(metadata: &std::fs::Metadata, is_dir: bool) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = is_dir;
        metadata.permissions().mode() & 0o777
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        if is_dir { 0o755 } else { 0o644 }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn current_target() -> String {
        format!("{}-{}", platform::os_name(), platform::arch_name())
    }

    fn write_minimal_payload(dir: &Path) {
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin/host"), b"host").unwrap();
        std::fs::write(dir.join("bin/plushie-renderer"), b"renderer").unwrap();
    }

    fn write_partial_manifest(dir: &Path, start: Option<&str>, platform: Option<&str>) -> PathBuf {
        let start_section = start.unwrap_or(
            r#"[start]
working_dir = "."
command = ["bin/host"]
forward_env = []
"#,
        );
        let platform_section = platform.unwrap_or("");
        let text = format!(
            r#"schema_version = 1
app_id = "com.example.test"
app_version = "0.1.0"
target = "{target}"
host_sdk = "elixir"
host_sdk_version = "0.1.0"
plushie_rust_version = "{version}"
protocol_version = {proto}

{start_section}

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

{platform_section}
"#,
            target = current_target(),
            version = env!("CARGO_PKG_VERSION"),
            proto = plushie_core::protocol::PROTOCOL_VERSION,
        );
        let path = dir.join("plushie-package.toml");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn assembles_complete_manifest_from_partial() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        let result = assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap();

        assert!(result.manifest_path.is_file());
        assert!(result.payload_archive_path.is_file());
        let text = std::fs::read_to_string(&result.manifest_path).unwrap();
        assert!(text.contains("[payload]"), "manifest has [payload] section");
        assert!(text.contains("archive = \"payload.tar.zst\""));
        assert!(text.contains("hash = \"sha256:"));
        assert!(text.contains("size ="));
        assert!(text.contains("[start]"), "manifest has [start] section");
    }

    #[test]
    fn materializes_default_icon_when_none_declared() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        let result = assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap();

        let text = std::fs::read_to_string(&result.manifest_path).unwrap();
        assert!(
            text.contains("icon = \"assets/default-app-icon-512.png\""),
            "default icon set in manifest"
        );
        assert!(
            payload_dir
                .join("assets/default-app-icon-512.png")
                .is_file(),
            "default icon written into payload dir"
        );
    }

    #[test]
    fn does_not_overwrite_declared_icon() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        std::fs::create_dir_all(payload_dir.join("assets")).unwrap();
        std::fs::write(payload_dir.join("assets/my-icon.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        let manifest_path = write_partial_manifest(
            dir.path(),
            None,
            Some("[platform]\nicon = \"assets/my-icon.png\""),
        );

        let result = assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap();

        let text = std::fs::read_to_string(&result.manifest_path).unwrap();
        assert!(
            text.contains("icon = \"assets/my-icon.png\""),
            "custom icon preserved"
        );
        assert!(
            !text.contains("default-app-icon"),
            "default icon not inserted"
        );
    }

    #[test]
    fn archive_hash_matches_written_file() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        let result = assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap();

        let archive_bytes = std::fs::read(&result.payload_archive_path).unwrap();
        let actual_hash = format!("sha256:{:x}", Sha256::digest(&archive_bytes));
        let manifest_text = std::fs::read_to_string(&result.manifest_path).unwrap();
        assert!(
            manifest_text.contains(&actual_hash),
            "manifest hash matches archive"
        );
    }

    #[test]
    fn source_config_overrides_partial_start() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        // Partial manifest has a [start] that source config will override.
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        // Write a source config with a different command.
        let config_path = dir.path().join("plushie-package.config.toml");
        std::fs::write(
            &config_path,
            "config_version = 1\n\n[start]\nworking_dir = \".\"\ncommand = [\"bin/host\", \"--extra-arg\"]\nforward_env = [\"PATH\"]\n",
        )
        .unwrap();

        let result = assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: Some(&config_path),
        })
        .unwrap();

        let text = std::fs::read_to_string(&result.manifest_path).unwrap();
        assert!(
            text.contains("--extra-arg"),
            "source config command used in final manifest"
        );
    }

    #[test]
    fn rejects_missing_required_field_app_id() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        let path = dir.path().join("plushie-package.toml");
        std::fs::write(
            &path,
            format!(
                r#"schema_version = 1
app_id = ""
app_version = "0.1.0"
target = "{target}"
host_sdk = "elixir"
plushie_rust_version = "{version}"
protocol_version = {proto}

[start]
working_dir = "."
command = ["bin/host"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"
"#,
                target = current_target(),
                version = env!("CARGO_PKG_VERSION"),
                proto = plushie_core::protocol::PROTOCOL_VERSION,
            ),
        )
        .unwrap();

        let err = assemble_package(&AssembleOpts {
            manifest_path: &path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap_err();
        assert!(
            err.to_string().contains("app_id"),
            "error mentions app_id: {err}"
        );
    }

    #[test]
    fn rejects_invalid_app_id_format() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        let path = dir.path().join("plushie-package.toml");
        std::fs::write(
            &path,
            format!(
                r#"schema_version = 1
app_id = "notreversedns"
app_version = "0.1.0"
target = "{target}"
host_sdk = "elixir"
plushie_rust_version = "{version}"
protocol_version = {proto}

[start]
working_dir = "."
command = ["bin/host"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"
"#,
                target = current_target(),
                version = env!("CARGO_PKG_VERSION"),
                proto = plushie_core::protocol::PROTOCOL_VERSION,
            ),
        )
        .unwrap();

        let err = assemble_package(&AssembleOpts {
            manifest_path: &path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap_err();
        assert!(
            err.to_string().contains("reverse-DNS"),
            "error mentions reverse-DNS format: {err}"
        );
    }

    #[test]
    fn rejects_payload_with_symlink() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", payload_dir.join("bin/evil")).unwrap();
            let manifest_path = write_partial_manifest(dir.path(), None, None);

            let err = assemble_package(&AssembleOpts {
                manifest_path: &manifest_path,
                payload_dir: &payload_dir,
                package_config: None,
            })
            .unwrap_err();
            assert!(
                err.to_string().contains("symlink"),
                "error mentions symlink: {err}"
            );
        }
        #[cfg(not(unix))]
        {
            let _ = payload_dir;
        }
    }

    fn write_config(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("plushie-package.config.toml");
        std::fs::write(&path, body).unwrap();
        path
    }

    fn baseline_start_section() -> &'static str {
        "[start]\nworking_dir = \".\"\ncommand = [\"bin/host\"]\nforward_env = []\n"
    }

    #[test]
    fn copies_convention_default_package_assets_directory() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        // Convention default lives next to the manifest when no explicit
        // package config is given.
        std::fs::create_dir_all(dir.path().join("package_assets")).unwrap();
        std::fs::write(
            dir.path().join("package_assets/branding.txt"),
            b"hello assets",
        )
        .unwrap();
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap();

        let copied = payload_dir.join("branding.txt");
        assert!(copied.is_file(), "convention asset copied into payload");
        assert_eq!(std::fs::read(&copied).unwrap(), b"hello assets");
    }

    #[test]
    fn copies_explicit_assets_dir_from_source_config() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);

        std::fs::create_dir_all(dir.path().join("branding")).unwrap();
        std::fs::write(dir.path().join("branding/logo.svg"), b"<svg/>").unwrap();
        let config_path = write_config(
            dir.path(),
            &format!(
                "config_version = 1\n\n{}\n[assets]\ndir = \"branding\"\n",
                baseline_start_section()
            ),
        );
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: Some(&config_path),
        })
        .unwrap();

        let copied = payload_dir.join("logo.svg");
        assert!(copied.is_file(), "explicit asset copied into payload");
        assert_eq!(std::fs::read(&copied).unwrap(), b"<svg/>");
    }

    #[test]
    fn explicit_missing_assets_dir_is_clear_error() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);

        let config_path = write_config(
            dir.path(),
            &format!(
                "config_version = 1\n\n{}\n[assets]\ndir = \"missing-dir\"\n",
                baseline_start_section()
            ),
        );
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        let err = assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: Some(&config_path),
        })
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("missing-dir"),
            "error names missing dir: {msg}"
        );
        assert!(msg.contains("does not exist"), "error explains: {msg}");
    }

    #[test]
    fn asset_icon_suppresses_default_icon_materialization() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);

        // package_assets/icon.png -> payload/icon.png, which leaves the
        // default-icon materializer's check at assets/default-app-icon-512.png
        // untouched (default still gets written), but the explicit case is the
        // one this test verifies: a user-supplied icon at the default path is
        // honored.
        std::fs::create_dir_all(dir.path().join("package_assets/assets")).unwrap();
        let user_icon = b"\x89PNG\r\n\x1a\nuser-supplied";
        std::fs::write(
            dir.path()
                .join("package_assets/assets/default-app-icon-512.png"),
            user_icon,
        )
        .unwrap();
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        let result = assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap();

        let dest = payload_dir.join("assets/default-app-icon-512.png");
        let bytes = std::fs::read(&dest).unwrap();
        assert_eq!(
            bytes, user_icon,
            "user-supplied icon at default path is not overwritten by the materializer"
        );
        let text = std::fs::read_to_string(&result.manifest_path).unwrap();
        assert!(
            text.contains("icon = \"assets/default-app-icon-512.png\""),
            "manifest still records the icon path"
        );
    }

    #[test]
    fn preserves_nested_asset_directories() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);

        std::fs::create_dir_all(dir.path().join("package_assets/fonts")).unwrap();
        std::fs::write(
            dir.path().join("package_assets/fonts/inter.ttf"),
            b"ttf-bytes",
        )
        .unwrap();
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap();

        let copied = payload_dir.join("fonts/inter.ttf");
        assert!(copied.is_file(), "nested asset preserved");
        assert_eq!(std::fs::read(&copied).unwrap(), b"ttf-bytes");
    }

    #[test]
    fn empty_package_assets_directory_is_noop() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        std::fs::create_dir_all(dir.path().join("package_assets")).unwrap();
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap();
        // No assertion needed beyond not erroring; ensure payload still has
        // the baseline host binary.
        assert!(payload_dir.join("bin/host").is_file());
    }

    #[test]
    fn asset_overwrites_existing_payload_file() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join("payload");
        write_minimal_payload(&payload_dir);
        std::fs::write(payload_dir.join("note.txt"), b"original").unwrap();

        std::fs::create_dir_all(dir.path().join("package_assets")).unwrap();
        std::fs::write(dir.path().join("package_assets/note.txt"), b"replaced").unwrap();
        let manifest_path = write_partial_manifest(dir.path(), None, None);

        assemble_package(&AssembleOpts {
            manifest_path: &manifest_path,
            payload_dir: &payload_dir,
            package_config: None,
        })
        .unwrap();

        assert_eq!(
            std::fs::read(payload_dir.join("note.txt")).unwrap(),
            b"replaced",
            "asset overwrites existing payload file"
        );
    }
}
