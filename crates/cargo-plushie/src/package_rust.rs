//! Rust SDK-owned standalone package staging.
//!
//! This module builds the Rust app host binary with wire support,
//! stages it with a payload-local renderer, writes the shared
//! `plushie-package.toml`, and produces the payload archive consumed
//! by [`crate::package`].

use crate::{Error, Result, default_icons, generator, package, patch_config, platform};
use anyhow::Context;
use cargo_metadata::{CargoOpt, Metadata, Package, Target, TargetKind};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

const PACKAGE_MANIFEST: &str = "plushie-package.toml";
const PAYLOAD_ARCHIVE: &str = "payload.tar.zst";
const PAYLOAD_DIR: &str = "payload-root";
const DEFAULT_ICON_NAME: &str = "plushie-checkbox-512x512.png";
const HOST_SDK: &str = "rust";
const RENDERER_SOURCE: &str = "local-build";

/// Options for staging a Rust SDK standalone package.
#[derive(Debug)]
pub struct RustPackageOpts<'a> {
    /// Path to the Rust app manifest.
    pub manifest_path: &'a Path,
    /// Path to the renderer binary to copy into the payload.
    pub renderer_path: &'a Path,
    /// Optional plushie-rust checkout used to patch host dependencies.
    pub source_path: Option<&'a Path>,
    /// Directory receiving the payload root, archive, and manifest.
    pub out_dir: &'a Path,
    /// Optional Cargo binary target name for the host app.
    pub bin: Option<&'a str>,
    /// Optional package application ID.
    pub app_id: Option<&'a str>,
    /// Optional human-readable application name.
    pub app_name: Option<&'a str>,
    /// Optional app icon to copy into the payload.
    pub icon: Option<&'a Path>,
    /// Cargo features to enable when building the host.
    pub features: &'a [String],
    /// Disable default features when building the host.
    pub no_default_features: bool,
    /// Enable all features when building the host.
    pub all_features: bool,
    /// Build the host with Cargo's release profile.
    pub release: bool,
    /// Print the underlying Cargo command.
    pub verbose: bool,
}

/// Result of staging a Rust SDK standalone package.
#[derive(Debug)]
pub struct RustPackageResult {
    /// Generated package manifest path.
    pub manifest_path: PathBuf,
    /// Generated payload archive path.
    pub payload_archive_path: PathBuf,
    /// Payload staging directory.
    pub payload_dir: PathBuf,
    /// Built host binary copied into the payload.
    pub host_payload_path: PathBuf,
    /// Renderer binary copied into the payload.
    pub renderer_payload_path: PathBuf,
    /// Payload icon path referenced by `[platform].icon`.
    pub icon_payload_path: PathBuf,
}

#[derive(Debug)]
struct AppInfo {
    package_name: String,
    package_version: String,
    app_id: String,
    app_name: Option<String>,
    bin_name: String,
    host_binary_path: PathBuf,
    plushie_version: String,
}

#[derive(Serialize)]
struct PackageManifest {
    schema_version: u32,
    app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_name: Option<String>,
    app_version: String,
    target: String,
    host_sdk: String,
    host_sdk_version: String,
    plushie_rust_version: String,
    protocol_version: u32,
    start: StartManifest,
    renderer: RendererManifest,
    platform: PlatformManifest,
    payload: PayloadManifest,
}

#[derive(Serialize)]
struct StartManifest {
    working_dir: String,
    command: Vec<String>,
    forward_env: Vec<String>,
}

#[derive(Serialize)]
struct RendererManifest {
    path: String,
    kind: String,
    source: String,
}

#[derive(Serialize)]
struct PlatformManifest {
    icon: String,
}

#[derive(Serialize)]
struct PayloadManifest {
    archive: String,
    hash: String,
    size: u64,
}

/// Build and stage a Rust SDK standalone package payload.
///
/// # Errors
///
/// Returns an error when Cargo metadata fails, the host build fails,
/// files cannot be copied, the payload cannot be archived, or the
/// generated shared package manifest does not pass precheck.
pub fn stage_rust_package(opts: &RustPackageOpts<'_>) -> Result<RustPackageResult> {
    if let Some(source_path) = opts.source_path {
        write_host_cargo_config(opts.manifest_path, source_path)?;
    }
    let metadata = cargo_metadata(opts)?;
    let package = package_for_manifest(&metadata, opts.manifest_path)?;
    let app_info = app_info(&metadata, package, opts)?;
    build_host(opts, &app_info)?;

    let out_dir = absolutize(opts.out_dir)?;
    std::fs::create_dir_all(&out_dir)?;
    let payload_dir = out_dir.join(PAYLOAD_DIR);
    reset_dir(&payload_dir)?;

    let host_payload_path = copy_payload_binary(
        &app_info.host_binary_path,
        &payload_dir,
        &format!("bin/{}", executable_name(&app_info.bin_name)),
    )?;
    let renderer_payload_path = copy_payload_binary(
        opts.renderer_path,
        &payload_dir,
        &format!("bin/plushie-renderer{}", platform::exe_suffix()),
    )?;
    let icon_payload_path = materialize_icon(opts.icon, &payload_dir)?;

    let payload_archive_path = out_dir.join(PAYLOAD_ARCHIVE);
    write_payload_archive(&payload_dir, &payload_archive_path)?;
    let payload_bytes = std::fs::read(&payload_archive_path)?;
    let hash = format!("sha256:{:x}", Sha256::digest(&payload_bytes));

    let manifest = PackageManifest {
        schema_version: 1,
        app_id: app_info.app_id,
        app_name: app_info.app_name,
        app_version: app_info.package_version,
        target: package_target(),
        host_sdk: HOST_SDK.to_string(),
        host_sdk_version: app_info.plushie_version.clone(),
        plushie_rust_version: app_info.plushie_version,
        protocol_version: plushie_core::protocol::PROTOCOL_VERSION,
        start: StartManifest {
            working_dir: ".".to_string(),
            command: vec![payload_relative_string(&payload_dir, &host_payload_path)?],
            forward_env: default_forward_env(),
        },
        renderer: RendererManifest {
            path: payload_relative_string(&payload_dir, &renderer_payload_path)?,
            kind: "custom".to_string(),
            source: RENDERER_SOURCE.to_string(),
        },
        platform: PlatformManifest {
            icon: payload_relative_string(&payload_dir, &icon_payload_path)?,
        },
        payload: PayloadManifest {
            archive: PAYLOAD_ARCHIVE.to_string(),
            hash,
            size: payload_bytes.len() as u64,
        },
    };

    let manifest_text =
        toml::to_string_pretty(&manifest).with_context(|| "serialize package manifest")?;
    let manifest_path = out_dir.join(PACKAGE_MANIFEST);
    generator::write_if_changed(&manifest_path, &manifest_text)?;
    package::precheck_package(&manifest_path)?;

    Ok(RustPackageResult {
        manifest_path,
        payload_archive_path,
        payload_dir,
        host_payload_path,
        renderer_payload_path,
        icon_payload_path,
    })
}

fn default_forward_env() -> Vec<String> {
    [
        "PATH",
        "HOME",
        "LANG",
        "LC_ALL",
        "XDG_RUNTIME_DIR",
        "WAYLAND_DISPLAY",
        "DISPLAY",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn write_host_cargo_config(manifest_path: &Path, source_path: &Path) -> Result<()> {
    let manifest_dir = manifest_path
        .parent()
        .ok_or_else(|| Error::Other(anyhow::anyhow!("manifest path has no parent directory")))?;
    let mut entries = vec![(
        "plushie".to_string(),
        source_path.join("crates").join("plushie"),
    )];
    entries.extend(
        patch_config::all_patches(source_path)
            .into_iter()
            .filter(|(name, _)| name != "plushie-renderer"),
    );
    let body = format!(
        "# Auto-generated by `cargo plushie package-rust`. Do not edit.\n\
         # Redirects Rust host Plushie deps to a local checkout.\n\n\
         {}",
        patch_config::render_patch_block(&entries)
    );
    generator::write_if_changed(&manifest_dir.join(".cargo/config.toml"), &body)
}

fn cargo_metadata(opts: &RustPackageOpts<'_>) -> Result<Metadata> {
    let manifest_dir = opts
        .manifest_path
        .parent()
        .ok_or_else(|| Error::Other(anyhow::anyhow!("manifest path has no parent directory")))?;
    let mut cmd = cargo_metadata::MetadataCommand::new();
    cmd.manifest_path(opts.manifest_path)
        .current_dir(manifest_dir);
    apply_feature_opts(&mut cmd, opts);
    cmd.exec()
        .with_context(|| "cargo metadata failed")
        .map_err(Error::from)
}

fn package_for_manifest<'a>(metadata: &'a Metadata, manifest_path: &Path) -> Result<&'a Package> {
    let expected = std::fs::canonicalize(manifest_path)
        .with_context(|| format!("manifest path `{}` not found", manifest_path.display()))?;
    for package in &metadata.packages {
        if std::fs::canonicalize(package.manifest_path.as_std_path())
            .map(|path| path == expected)
            .unwrap_or(false)
        {
            return Ok(package);
        }
    }

    Err(Error::Other(anyhow::anyhow!(
        "`{}` is not a package manifest; pass the Cargo.toml for the Rust app package",
        manifest_path.display()
    )))
}

fn apply_feature_opts(cmd: &mut cargo_metadata::MetadataCommand, opts: &RustPackageOpts<'_>) {
    let features = host_features(opts);
    if !features.is_empty() {
        cmd.features(CargoOpt::SomeFeatures(features));
    }
    if opts.no_default_features {
        cmd.features(CargoOpt::NoDefaultFeatures);
    }
    if opts.all_features {
        cmd.features(CargoOpt::AllFeatures);
    }
}

fn app_info(metadata: &Metadata, package: &Package, opts: &RustPackageOpts<'_>) -> Result<AppInfo> {
    let target = select_bin_target(package, opts.bin)?;
    let app_id = opts
        .app_id
        .map(str::to_string)
        .or_else(|| plushie_metadata_string(package, "app_id"))
        .unwrap_or_else(|| package.name.to_string());
    let app_name = opts
        .app_name
        .map(str::to_string)
        .or_else(|| plushie_metadata_string(package, "app_name"));

    let host_binary_path = metadata
        .target_directory
        .as_std_path()
        .join(if opts.release { "release" } else { "debug" })
        .join(executable_name(&target.name));

    let plushie_version = metadata
        .packages
        .iter()
        .find(|package| package.name == "plushie")
        .map(|package| package.version.to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    Ok(AppInfo {
        package_name: package.name.to_string(),
        package_version: package.version.to_string(),
        app_id,
        app_name,
        bin_name: target.name.clone(),
        host_binary_path,
        plushie_version,
    })
}

fn select_bin_target<'a>(package: &'a Package, requested: Option<&str>) -> Result<&'a Target> {
    let bins: Vec<_> = package
        .targets
        .iter()
        .filter(|target| target.kind.iter().any(|kind| kind == &TargetKind::Bin))
        .collect();

    if let Some(requested) = requested {
        return bins
            .into_iter()
            .find(|target| target.name == requested)
            .ok_or_else(|| {
                Error::Other(anyhow::anyhow!(
                    "package `{}` does not define a binary target named `{requested}`",
                    package.name
                ))
            });
    }

    match bins.as_slice() {
        [target] => Ok(target),
        [] => Err(Error::Other(anyhow::anyhow!(
            "package `{}` does not define a binary target",
            package.name
        ))),
        _ => Err(Error::Other(anyhow::anyhow!(
            "package `{}` defines multiple binary targets; pass --bin",
            package.name
        ))),
    }
}

fn plushie_metadata_string(package: &Package, key: &str) -> Option<String> {
    package
        .metadata
        .get("plushie")
        .and_then(|value| {
            value
                .get("package")
                .and_then(|table| table.get(key))
                .or_else(|| value.get(key))
        })
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn build_host(opts: &RustPackageOpts<'_>, app_info: &AppInfo) -> Result<()> {
    let manifest_dir = opts
        .manifest_path
        .parent()
        .ok_or_else(|| Error::Other(anyhow::anyhow!("manifest path has no parent directory")))?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut cmd = std::process::Command::new(cargo);
    cmd.current_dir(manifest_dir)
        .arg("build")
        .arg("--manifest-path")
        .arg(opts.manifest_path)
        .arg("--bin")
        .arg(&app_info.bin_name);
    if opts.release {
        cmd.arg("--release");
    }
    if opts.no_default_features {
        cmd.arg("--no-default-features");
    }
    if opts.all_features {
        cmd.arg("--all-features");
    }

    let features = host_features(opts);
    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
    }

    if opts.verbose {
        eprintln!(
            "running: cargo build --manifest-path {} --bin {}{}{}{} --features {}",
            opts.manifest_path.display(),
            app_info.bin_name,
            if opts.release { " --release" } else { "" },
            if opts.no_default_features {
                " --no-default-features"
            } else {
                ""
            },
            if opts.all_features {
                " --all-features"
            } else {
                ""
            },
            features.join(",")
        );
    }

    let status = cmd.status().with_context(|| {
        format!(
            "failed to run cargo build for Rust package host `{}`",
            app_info.package_name
        )
    })?;
    if !status.success() {
        return Err(Error::CargoBuildFailed(status));
    }
    if !app_info.host_binary_path.is_file() {
        return Err(Error::Other(anyhow::anyhow!(
            "host build did not produce `{}`",
            app_info.host_binary_path.display()
        )));
    }
    Ok(())
}

fn host_features(opts: &RustPackageOpts<'_>) -> Vec<String> {
    let mut features = opts.features.to_vec();
    if !features.iter().any(|feature| feature == "plushie/wire") {
        features.push("plushie/wire".to_string());
    }
    features
}

fn copy_payload_binary(source: &Path, payload_dir: &Path, relative: &str) -> Result<PathBuf> {
    if !source.is_file() {
        return Err(Error::Other(anyhow::anyhow!(
            "payload binary `{}` does not exist",
            source.display()
        )));
    }
    let dest = payload_dir.join(clean_payload_relative_path(relative)?);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, &dest).with_context(|| {
        format!(
            "copy payload binary `{}` to `{}`",
            source.display(),
            dest.display()
        )
    })?;
    make_executable(&dest)?;
    Ok(dest)
}

fn materialize_icon(icon: Option<&Path>, payload_dir: &Path) -> Result<PathBuf> {
    let assets_dir = payload_dir.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    if let Some(icon) = icon {
        let file_name = icon
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                Error::Other(anyhow::anyhow!(
                    "icon path `{}` must have a UTF-8 file name",
                    icon.display()
                ))
            })?;
        let relative = clean_payload_relative_path(&format!("assets/{file_name}"))?;
        let dest = payload_dir.join(relative);
        std::fs::copy(icon, &dest)
            .with_context(|| format!("copy app icon `{}`", icon.display()))?;
        return Ok(dest);
    }

    default_icons::write_default_icons(&assets_dir)?;
    Ok(assets_dir.join(DEFAULT_ICON_NAME))
}

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

fn reset_dir(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(Error::Io(err)),
    }
    std::fs::create_dir_all(path)?;
    Ok(())
}

fn absolutize(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn payload_relative_string(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).with_context(|| {
        format!(
            "payload path `{}` is not under `{}`",
            path.display(),
            root.display()
        )
    })?;
    Ok(relative
        .components()
        .map(|component| match component {
            Component::Normal(part) => Ok(part.to_string_lossy().into_owned()),
            _ => Err(Error::Other(anyhow::anyhow!(
                "payload path `{}` is not cleanly relative",
                relative.display()
            ))),
        })
        .collect::<Result<Vec<_>>>()?
        .join("/"))
}

fn clean_payload_relative_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(Error::Other(anyhow::anyhow!(
            "payload path must be relative, got `{value}`"
        )));
    }

    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => cleaned.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::Other(anyhow::anyhow!(
                    "payload path must not escape the payload root: `{value}`"
                )));
            }
        }
    }
    Ok(cleaned)
}

fn package_target() -> String {
    format!("{}-{}", platform::os_name(), platform::arch_name())
}

fn executable_name(name: &str) -> String {
    format!("{name}{}", platform::exe_suffix())
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn package_manifest_records_rust_wire_payload() {
        let dir = tempdir().unwrap();
        let payload_dir = dir.path().join(PAYLOAD_DIR);
        std::fs::create_dir_all(payload_dir.join("bin")).unwrap();
        std::fs::create_dir_all(payload_dir.join("assets")).unwrap();
        std::fs::write(payload_dir.join("bin/app"), b"host").unwrap();
        std::fs::write(payload_dir.join("bin/plushie-renderer"), b"renderer").unwrap();
        std::fs::write(payload_dir.join("assets/app.png"), b"\x89PNG\r\n\x1a\n").unwrap();

        let archive = dir.path().join(PAYLOAD_ARCHIVE);
        write_payload_archive(&payload_dir, &archive).unwrap();
        let bytes = std::fs::read(&archive).unwrap();
        let manifest = PackageManifest {
            schema_version: 1,
            app_id: "com.example.demo".to_string(),
            app_name: Some("Demo".to_string()),
            app_version: "0.1.0".to_string(),
            target: package_target(),
            host_sdk: HOST_SDK.to_string(),
            host_sdk_version: env!("CARGO_PKG_VERSION").to_string(),
            plushie_rust_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: plushie_core::protocol::PROTOCOL_VERSION,
            start: StartManifest {
                working_dir: ".".to_string(),
                command: vec!["bin/app".to_string()],
                forward_env: Vec::new(),
            },
            renderer: RendererManifest {
                path: "bin/plushie-renderer".to_string(),
                kind: "custom".to_string(),
                source: RENDERER_SOURCE.to_string(),
            },
            platform: PlatformManifest {
                icon: "assets/app.png".to_string(),
            },
            payload: PayloadManifest {
                archive: PAYLOAD_ARCHIVE.to_string(),
                hash: format!("sha256:{:x}", Sha256::digest(&bytes)),
                size: bytes.len() as u64,
            },
        };
        let manifest_path = dir.path().join(PACKAGE_MANIFEST);
        std::fs::write(&manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();

        let precheck = package::precheck_package(&manifest_path).unwrap();

        assert_eq!(precheck.app_id, "com.example.demo");
        let text = std::fs::read_to_string(&manifest_path).unwrap();
        assert!(text.contains("host_sdk = \"rust\""));
        assert!(text.contains("app_name = \"Demo\""));
        assert!(text.contains("icon = \"assets/app.png\""));
        assert!(text.contains("source = \"local-build\""));
    }

    #[test]
    fn materializes_default_icon_inside_payload() {
        let dir = tempdir().unwrap();
        let icon = materialize_icon(None, dir.path()).unwrap();

        assert_eq!(icon, dir.path().join("assets").join(DEFAULT_ICON_NAME));
        assert!(icon.is_file());
    }

    #[test]
    fn copies_app_icon_inside_payload_assets() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("icon.png");
        std::fs::write(&source, b"custom-icon").unwrap();
        let payload = dir.path().join("payload");

        let icon = materialize_icon(Some(&source), &payload).unwrap();

        assert_eq!(icon, payload.join("assets/icon.png"));
        assert_eq!(std::fs::read(icon).unwrap(), b"custom-icon");
    }

    #[test]
    fn rejects_virtual_workspace_manifest() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app/src")).unwrap();
        std::fs::create_dir_all(dir.path().join("plushie/src")).unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"[workspace]
members = ["app", "plushie"]
resolver = "3"
"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("app/Cargo.toml"),
            r#"[package]
name = "demo-app"
version = "0.1.0"
edition = "2024"

[dependencies]
plushie = { path = "../plushie" }
"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("app/src/lib.rs"), "").unwrap();
        std::fs::write(
            dir.path().join("plushie/Cargo.toml"),
            r#"[package]
name = "plushie"
version = "0.1.0"
edition = "2024"

[features]
wire = []
"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("plushie/src/lib.rs"), "").unwrap();

        let manifest_path = dir.path().join("Cargo.toml");
        let renderer_path = dir.path().join("renderer");
        let out_dir = dir.path().join("dist");
        let opts = RustPackageOpts {
            manifest_path: &manifest_path,
            renderer_path: &renderer_path,
            source_path: None,
            out_dir: &out_dir,
            bin: None,
            app_id: None,
            app_name: None,
            icon: None,
            features: &[],
            no_default_features: false,
            all_features: false,
            release: false,
            verbose: false,
        };

        let metadata = cargo_metadata(&opts).unwrap();
        let err = package_for_manifest(&metadata, opts.manifest_path).unwrap_err();

        assert!(err.to_string().contains("is not a package manifest"));
    }

    #[test]
    fn host_features_always_enable_plushie_wire() {
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join("Cargo.toml");
        let renderer_path = dir.path().join("renderer");
        let features = vec![String::from("demo/extra")];
        let opts = RustPackageOpts {
            manifest_path: &manifest_path,
            renderer_path: &renderer_path,
            source_path: None,
            out_dir: dir.path(),
            bin: None,
            app_id: None,
            app_name: None,
            icon: None,
            features: &features,
            no_default_features: false,
            all_features: false,
            release: false,
            verbose: false,
        };

        assert_eq!(host_features(&opts), ["demo/extra", "plushie/wire"]);
    }
}
