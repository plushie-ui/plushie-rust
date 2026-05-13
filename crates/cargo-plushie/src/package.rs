//! Standalone package command support.
//!
//! The SDKs own host-language packaging. This module owns the shared
//! Plushie wrapper step: validate a package manifest, embed its payload
//! archive in a generated Rust launcher, and build that launcher.

use crate::{Error, Result, generator};
use anyhow::Context;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const GENERATED_MANIFEST: &str = "plushie-package.toml";
const GENERATED_PAYLOAD: &str = "payload.tar.zst";

/// Options for building a standalone launcher from a package manifest.
#[derive(Debug)]
pub struct PackageOpts<'a> {
    /// Path to the Plushie package manifest.
    pub manifest_path: &'a Path,
    /// Optional final launcher output path.
    pub out_path: Option<&'a Path>,
    /// Build the generated launcher with Cargo's release profile.
    pub release: bool,
    /// Print the generated Cargo command.
    pub verbose: bool,
}

/// Result of building a standalone launcher.
#[derive(Debug)]
pub struct PackageResult {
    /// Generated launcher crate directory.
    pub launcher_crate_dir: PathBuf,
    /// Final launcher executable path.
    pub binary_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct PackageManifest {
    schema_version: u32,
    app_id: String,
    app_name: Option<String>,
    app_version: String,
    target: Option<String>,
    host_sdk: String,
    host_sdk_version: Option<String>,
    plushie_rust_version: String,
    protocol_version: u32,
    renderer_path: String,
    host_command: Vec<String>,
    working_dir: Option<String>,
    #[serde(default)]
    exec_env: Vec<String>,
    payload: PayloadManifest,
}

#[derive(Debug, Deserialize)]
struct PayloadManifest {
    archive: String,
    hash: String,
    size: Option<u64>,
}

struct PreparedLauncher {
    crate_dir: PathBuf,
    package_name: String,
    output_path: PathBuf,
}

/// Build the generated launcher and copy it to the requested output.
///
/// # Errors
///
/// Returns an error when manifest validation fails, Cargo fails, or
/// the final binary cannot be copied.
pub fn build_launcher(opts: &PackageOpts<'_>) -> Result<PackageResult> {
    let prepared = prepare_launcher_crate(opts)?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = std::process::Command::new(cargo);
    cmd.current_dir(&prepared.crate_dir).arg("build");
    if opts.release {
        cmd.arg("--release");
    }
    if opts.verbose {
        eprintln!(
            "running: cargo build{}",
            if opts.release { " --release" } else { "" }
        );
    }
    let status = cmd
        .status()
        .with_context(|| "failed to run cargo build for generated launcher")?;
    if !status.success() {
        return Err(Error::CargoBuildFailed(status));
    }

    let profile = if opts.release { "release" } else { "debug" };
    let built = prepared
        .crate_dir
        .join("target")
        .join(profile)
        .join(executable_name(&prepared.package_name));
    if !built.is_file() {
        return Err(Error::Other(anyhow::anyhow!(
            "generated launcher did not produce `{}`",
            built.display()
        )));
    }

    if let Some(parent) = prepared.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&built, &prepared.output_path)?;
    make_executable(&prepared.output_path)?;

    Ok(PackageResult {
        launcher_crate_dir: prepared.crate_dir,
        binary_path: prepared.output_path,
    })
}

fn prepare_launcher_crate(opts: &PackageOpts<'_>) -> Result<PreparedLauncher> {
    let manifest_path = std::fs::canonicalize(opts.manifest_path).with_context(|| {
        format!(
            "package manifest `{}` not found",
            opts.manifest_path.display()
        )
    })?;
    let manifest_dir = manifest_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("package manifest has no parent directory"))?;
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read `{}`", manifest_path.display()))?;
    let manifest = parse_manifest(&manifest_text)?;
    let archive_path = manifest_dir.join(&manifest.payload.archive);
    let payload = std::fs::read(&archive_path)
        .with_context(|| format!("failed to read payload `{}`", archive_path.display()))?;
    validate_payload(&manifest, &payload)?;

    let target_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target"));
    let package_name = package_name(&manifest.app_id);
    let crate_dir = target_root.join("plushie-package").join(&package_name);
    let output_path = opts.out_path.map(Path::to_path_buf).unwrap_or_else(|| {
        target_root
            .join("plushie/package")
            .join(executable_name(&safe_name(&manifest.app_id)))
    });

    std::fs::create_dir_all(crate_dir.join("src"))?;
    generator::write_if_changed(
        &crate_dir.join("Cargo.toml"),
        &launcher_cargo_toml(&package_name),
    )?;
    generator::write_if_changed(&crate_dir.join("src/main.rs"), LAUNCHER_MAIN)?;
    generator::write_if_changed(&crate_dir.join(GENERATED_MANIFEST), &manifest_text)?;
    std::fs::write(crate_dir.join(GENERATED_PAYLOAD), payload)?;

    Ok(PreparedLauncher {
        crate_dir,
        package_name,
        output_path,
    })
}

fn parse_manifest(text: &str) -> Result<PackageManifest> {
    let manifest: PackageManifest =
        toml::from_str(text).with_context(|| "failed to parse package manifest")?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &PackageManifest) -> Result<()> {
    if manifest.schema_version != 1 {
        return Err(Error::Other(anyhow::anyhow!(
            "unsupported package manifest schema_version {}",
            manifest.schema_version
        )));
    }
    require_nonempty("app_id", &manifest.app_id)?;
    if let Some(app_name) = &manifest.app_name {
        require_nonempty("app_name", app_name)?;
    }
    require_nonempty("app_version", &manifest.app_version)?;
    if let Some(target) = &manifest.target {
        require_nonempty("target", target)?;
    }
    require_nonempty("host_sdk", &manifest.host_sdk)?;
    if let Some(host_sdk_version) = &manifest.host_sdk_version {
        require_nonempty("host_sdk_version", host_sdk_version)?;
    }
    require_nonempty("plushie_rust_version", &manifest.plushie_rust_version)?;
    require_nonempty("renderer_path", &manifest.renderer_path)?;
    if let Some(working_dir) = &manifest.working_dir {
        require_nonempty("working_dir", working_dir)?;
    }
    require_nonempty("payload.archive", &manifest.payload.archive)?;
    if manifest.host_command.is_empty() || manifest.host_command.iter().any(|arg| arg.is_empty()) {
        return Err(Error::Other(anyhow::anyhow!(
            "host_command must contain a non-empty argv"
        )));
    }
    if manifest.exec_env.iter().any(|name| name.trim().is_empty()) {
        return Err(Error::Other(anyhow::anyhow!(
            "exec_env must contain only non-empty variable names"
        )));
    }
    if manifest.protocol_version == 0 {
        return Err(Error::Other(anyhow::anyhow!(
            "protocol_version must be greater than zero"
        )));
    }
    validate_sha256_field(&manifest.payload.hash)?;
    Ok(())
}

fn validate_payload(manifest: &PackageManifest, payload: &[u8]) -> Result<()> {
    if let Some(size) = manifest.payload.size
        && payload.len() as u64 != size
    {
        return Err(Error::Other(anyhow::anyhow!(
            "payload size mismatch: manifest expected {size} bytes, archive has {} bytes",
            payload.len()
        )));
    }

    let expected = manifest
        .payload
        .hash
        .strip_prefix("sha256:")
        .expect("validated hash prefix");
    let actual = format!("{:x}", Sha256::digest(payload));
    if actual != expected {
        return Err(Error::Other(anyhow::anyhow!(
            "payload sha256 mismatch: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn validate_sha256_field(hash: &str) -> Result<()> {
    let Some(hex) = hash.strip_prefix("sha256:") else {
        return Err(Error::Other(anyhow::anyhow!(
            "payload.hash must start with `sha256:`"
        )));
    };
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Other(anyhow::anyhow!(
            "payload.hash must contain a 64-character hex SHA-256 digest"
        )));
    }
    Ok(())
}

fn require_nonempty(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::Other(anyhow::anyhow!("{name} must not be empty")));
    }
    Ok(())
}

fn safe_name(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "app".to_string()
    } else {
        out
    }
}

fn package_name(app_id: &str) -> String {
    format!("plushie-package-{}", safe_name(app_id).replace('.', "-"))
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn launcher_cargo_toml(package_name: &str) -> String {
    format!(
        r#"[package]
name = "{package_name}"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
anyhow = "1"
serde = {{ version = "1", features = ["derive"] }}
sha2 = "0.10"
tar = "0.4"
toml = "0.8"
zstd = "0.13"
"#
    )
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

const LAUNCHER_MAIN: &str = r###"use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const MANIFEST_TEXT: &str = include_str!("../plushie-package.toml");
const PAYLOAD_BYTES: &[u8] = include_bytes!("../payload.tar.zst");

#[derive(Debug, Deserialize)]
struct Manifest {
    app_id: String,
    renderer_path: String,
    host_command: Vec<String>,
    working_dir: Option<String>,
    #[serde(default)]
    exec_env: Vec<String>,
    payload: Payload,
}

#[derive(Debug, Deserialize)]
struct Payload {
    hash: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("plushie launcher: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<u8> {
    let manifest: Manifest = toml::from_str(MANIFEST_TEXT).context("parse embedded manifest")?;
    let root = ensure_payload(&manifest)?;
    let renderer = absolute_payload_path(&root, &manifest.renderer_path);
    let working_dir = manifest
        .working_dir
        .as_deref()
        .map(|path| absolute_payload_path(&root, path))
        .unwrap_or_else(|| root.clone());
    let host_program = manifest
        .host_command
        .first()
        .context("host_command is empty")?;

    let mut command = Command::new(&renderer);
    command
        .current_dir(&working_dir)
        .arg("--listen")
        .arg("--exec-bin")
        .arg(host_program)
        .env("PLUSHIE_PACKAGE_DIR", &root);

    if !manifest.exec_env.is_empty() {
        command.arg("--exec-env").arg(manifest.exec_env.join(","));
    }

    for arg in manifest.host_command.iter().skip(1) {
        command.arg("--exec-arg").arg(arg);
    }

    let status = command
        .status()
        .with_context(|| format!("start renderer `{}`", renderer.display()))?;
    Ok(status.code().unwrap_or(1).try_into().unwrap_or(1))
}

fn ensure_payload(manifest: &Manifest) -> Result<PathBuf> {
    verify_payload_hash(&manifest.payload.hash)?;
    let hash = manifest
        .payload
        .hash
        .strip_prefix("sha256:")
        .context("payload hash missing sha256 prefix")?;
    let root = cache_root().join("plushie/apps").join(safe_name(&manifest.app_id));
    let dest = root.join(hash);

    if dest.join("plushie-package.toml").is_file() {
        return Ok(dest);
    }

    std::fs::create_dir_all(&root)?;
    let tmp = root.join(format!(".{hash}.{}.tmp", std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::create_dir_all(&tmp)?;

    let decoder = zstd::stream::read::Decoder::new(PAYLOAD_BYTES)
        .context("open embedded zstd payload")?;
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(&tmp).context("extract embedded payload")?;
    std::fs::write(tmp.join("plushie-package.toml"), MANIFEST_TEXT)?;

    make_executable(&absolute_payload_path(&tmp, &manifest.renderer_path))?;
    if let Some(program) = manifest.host_command.first() {
        let path = absolute_payload_path(&tmp, program);
        if path.is_file() {
            make_executable(&path)?;
        }
    }

    match std::fs::rename(&tmp, &dest) {
        Ok(()) => Ok(dest),
        Err(err) if dest.join("plushie-package.toml").is_file() => {
            let _ = std::fs::remove_dir_all(&tmp);
            eprintln!("plushie launcher: using concurrently extracted payload after rename failed: {err}");
            Ok(dest)
        }
        Err(err) => Err(err).context("install extracted payload"),
    }
}

fn verify_payload_hash(expected: &str) -> Result<()> {
    let expected = expected
        .strip_prefix("sha256:")
        .context("payload hash missing sha256 prefix")?;
    let actual = format!("{:x}", Sha256::digest(PAYLOAD_BYTES));
    anyhow::ensure!(
        actual == expected,
        "embedded payload sha256 mismatch: expected {expected}, got {actual}"
    );
    Ok(())
}

fn cache_root() -> PathBuf {
    if let Some(path) = std::env::var_os("PLUSHIE_CACHE_DIR") {
        return PathBuf::from(path);
    }
    if cfg!(windows) {
        if let Some(path) = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .or_else(|| std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("AppData/Local").into_os_string()))
        {
            return PathBuf::from(path);
        }
    } else if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(path);
    } else if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache");
    }
    std::env::temp_dir()
}

fn absolute_payload_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn safe_name(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "app".to_string() } else { out }
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path.is_file() {
            let mut permissions = std::fs::metadata(path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(path, permissions)?;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}
"###;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_valid_manifest() {
        let payload = b"payload";
        let hash = format!("sha256:{:x}", Sha256::digest(payload));
        let text = format!(
            r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
host_sdk = "python"
plushie_rust_version = "0.7.1"
protocol_version = 1
renderer_path = "bin/plushie-renderer"
host_command = ["bin/notes"]

[payload]
archive = "payload.tar.zst"
hash = "{hash}"
size = 7
"#
        );

        let manifest = parse_manifest(&text).unwrap();
        assert_eq!(manifest.app_id, "com.example.notes");
        assert_eq!(manifest.host_command, ["bin/notes"]);
    }

    #[test]
    fn rejects_empty_host_command() {
        let text = r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
host_sdk = "python"
plushie_rust_version = "0.7.1"
protocol_version = 1
renderer_path = "bin/plushie-renderer"
host_command = []

[payload]
archive = "payload.tar.zst"
hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
"#;

        let err = parse_manifest(text).unwrap_err();
        assert!(err.to_string().contains("host_command"));
    }

    #[test]
    fn verifies_payload_hash_and_size() {
        let payload = b"payload";
        let hash = format!("sha256:{:x}", Sha256::digest(payload));
        let manifest = PackageManifest {
            schema_version: 1,
            app_id: "com.example.notes".to_string(),
            app_name: None,
            app_version: "0.1.0".to_string(),
            target: None,
            host_sdk: "python".to_string(),
            host_sdk_version: None,
            plushie_rust_version: "0.7.1".to_string(),
            protocol_version: 1,
            renderer_path: "bin/plushie-renderer".to_string(),
            host_command: vec!["bin/notes".to_string()],
            working_dir: None,
            exec_env: Vec::new(),
            payload: PayloadManifest {
                archive: "payload.tar.zst".to_string(),
                hash,
                size: Some(payload.len() as u64),
            },
        };

        validate_payload(&manifest, payload).unwrap();
    }

    #[test]
    fn prepares_generated_launcher_crate() {
        let dir = tempdir().unwrap();
        let payload = b"payload";
        let archive = dir.path().join("payload.tar.zst");
        std::fs::write(&archive, payload).unwrap();
        let hash = format!("sha256:{:x}", Sha256::digest(payload));
        let manifest = dir.path().join("plushie-package.toml");
        std::fs::write(
            &manifest,
            format!(
                r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
host_sdk = "python"
plushie_rust_version = "0.7.1"
protocol_version = 1
renderer_path = "bin/plushie-renderer"
host_command = ["bin/notes"]

[payload]
archive = "payload.tar.zst"
hash = "{hash}"
"#
            ),
        )
        .unwrap();

        let opts = PackageOpts {
            manifest_path: &manifest,
            out_path: None,
            release: false,
            verbose: false,
        };
        let prepared = prepare_launcher_crate(&opts).unwrap();
        assert!(prepared.crate_dir.join("Cargo.toml").is_file());
        assert!(prepared.crate_dir.join("src/main.rs").is_file());
        assert!(prepared.crate_dir.join(GENERATED_MANIFEST).is_file());
        assert!(prepared.crate_dir.join(GENERATED_PAYLOAD).is_file());
    }
}
