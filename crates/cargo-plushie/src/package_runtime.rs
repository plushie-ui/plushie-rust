//! Runtime support for launching Plushie package payloads.
//!
//! The self-contained portable launcher embeds the package manifest and
//! payload archive. The reusable `plushie-launcher` binary consumes the
//! same manifest shape from disk. Both paths use this module's contract:
//! validate the manifest, extract a content-addressed payload cache, set
//! launcher-owned environment variables, then start the SDK host command.

use crate::platform;
use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const COMPLETE_MARKER: &str = ".plushie-complete";
const EMBEDDED_PACKAGE_MAGIC: &[u8] = b"\nPLUSHIE_EMBEDDED_PACKAGE_V1\n";
const EXPECTED_SCHEMA_VERSION: u32 = 1;
const EXPECTED_PROTOCOL_VERSION: u32 = plushie_core::protocol::PROTOCOL_VERSION;
const EXPECTED_PLUSHIE_RUST_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Environment variable set by the launcher to pass a readiness-file
/// path to the SDK host. Defined here (the launcher consumer) and
/// re-exported for use by the packaging build side.
pub const PACKAGE_READY_FILE_ENV: &str = "PLUSHIE_PACKAGE_READY_FILE";

/// Append package data to a reusable `plushie-launcher` binary.
///
/// # Errors
///
/// Returns an error when the package data length cannot be represented
/// in the embedded trailer or writing fails.
pub fn append_embedded_package(
    mut writer: impl Write,
    manifest_text: &str,
    payload_bytes: &[u8],
) -> Result<()> {
    let manifest_len = u64::try_from(manifest_text.len()).context("manifest is too large")?;
    let payload_len = u64::try_from(payload_bytes.len()).context("payload is too large")?;
    writer.write_all(manifest_text.as_bytes())?;
    writer.write_all(payload_bytes)?;
    writer.write_all(&manifest_len.to_le_bytes())?;
    writer.write_all(&payload_len.to_le_bytes())?;
    writer.write_all(EMBEDDED_PACKAGE_MAGIC)?;
    Ok(())
}

/// Return whether an executable already contains an embedded Plushie package.
///
/// # Errors
///
/// Returns an error when the executable cannot be read.
pub fn has_embedded_package(executable_path: &Path) -> Result<bool> {
    let bytes = std::fs::read(executable_path)
        .with_context(|| format!("read executable `{}`", executable_path.display()))?;
    Ok(bytes.ends_with(EMBEDDED_PACKAGE_MAGIC))
}

/// Run embedded package data from an executable.
///
/// Returns `Ok(None)` when the executable has no embedded Plushie
/// package data.
///
/// # Errors
///
/// Returns an error when embedded package data is malformed or the host
/// process cannot be started.
pub fn run_embedded_package(executable_path: &Path) -> Result<Option<u8>> {
    run_embedded_package_with_mode(executable_path, false)
}

/// Validate and extract embedded package data from an executable.
///
/// Returns `Ok(None)` when the executable has no embedded Plushie
/// package data.
///
/// # Errors
///
/// Returns an error when embedded package data is malformed or cannot be
/// extracted.
pub fn postcheck_embedded_package(executable_path: &Path) -> Result<Option<u8>> {
    run_embedded_package_with_mode(executable_path, true)
}

fn run_embedded_package_with_mode(executable_path: &Path, postcheck: bool) -> Result<Option<u8>> {
    let Some(package) = read_embedded_package(executable_path)? else {
        return Ok(None);
    };
    let manifest: Manifest =
        toml::from_str(&package.manifest_text).context("parse embedded package manifest")?;
    validate_manifest(&manifest)?;
    let code = run_package(PackageInput {
        manifest_text: &package.manifest_text,
        manifest: &manifest,
        payload_bytes: &package.payload_bytes,
        postcheck,
        cache_root: None,
    })?;
    Ok(Some(code))
}

struct EmbeddedPackage {
    manifest_text: String,
    payload_bytes: Vec<u8>,
}

fn read_embedded_package(executable_path: &Path) -> Result<Option<EmbeddedPackage>> {
    let bytes = std::fs::read(executable_path)
        .with_context(|| format!("read executable `{}`", executable_path.display()))?;
    let trailer_len = EMBEDDED_PACKAGE_MAGIC.len() + 16;
    if bytes.len() < trailer_len || !bytes.ends_with(EMBEDDED_PACKAGE_MAGIC) {
        return Ok(None);
    }

    let lengths_end = bytes.len() - EMBEDDED_PACKAGE_MAGIC.len();
    let lengths_start = lengths_end - 16;
    let manifest_len = u64::from_le_bytes(
        bytes[lengths_start..lengths_start + 8]
            .try_into()
            .expect("slice length is fixed"),
    );
    let payload_len = u64::from_le_bytes(
        bytes[lengths_start + 8..lengths_end]
            .try_into()
            .expect("slice length is fixed"),
    );
    let manifest_len = usize::try_from(manifest_len).context("embedded manifest is too large")?;
    let payload_len = usize::try_from(payload_len).context("embedded payload is too large")?;
    let data_len = manifest_len
        .checked_add(payload_len)
        .context("embedded package length overflow")?;
    let data_start = lengths_start
        .checked_sub(data_len)
        .context("embedded package trailer points before executable start")?;
    let payload_start = data_start + manifest_len;
    let manifest_text = std::str::from_utf8(&bytes[data_start..payload_start])
        .context("embedded manifest is not UTF-8")?
        .to_string();
    let payload_bytes = bytes[payload_start..lengths_start].to_vec();
    anyhow::ensure!(
        payload_bytes.len() == payload_len,
        "embedded payload length mismatch"
    );
    Ok(Some(EmbeddedPackage {
        manifest_text,
        payload_bytes,
    }))
}

/// Run an on-disk package manifest and payload archive.
///
/// Returns the host process exit code. A missing process exit code maps
/// to `1`, matching the portable launcher.
///
/// # Errors
///
/// Returns an error when the manifest is invalid, the payload cannot be
/// extracted, or the host process cannot be started.
pub fn run_external_package(manifest_path: &Path) -> Result<u8> {
    run_external_package_with_mode(manifest_path, false)
}

/// Validate and extract an on-disk package without starting the host.
///
/// This is the reusable launcher's equivalent of the portable launcher's
/// postcheck mode.
///
/// # Errors
///
/// Returns an error when the manifest is invalid or the payload cannot be
/// extracted.
pub fn postcheck_external_package(manifest_path: &Path) -> Result<u8> {
    run_external_package_with_mode(manifest_path, true)
}

fn run_external_package_with_mode(manifest_path: &Path, postcheck: bool) -> Result<u8> {
    run_external_package_with_options(manifest_path, postcheck, None)
}

fn run_external_package_with_options(
    manifest_path: &Path,
    postcheck: bool,
    cache_root: Option<&Path>,
) -> Result<u8> {
    let manifest_path = manifest_path
        .canonicalize()
        .with_context(|| format!("canonicalize manifest `{}`", manifest_path.display()))?;
    let manifest_dir = manifest_path
        .parent()
        .context("package manifest path has no parent")?
        .to_path_buf();
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("read package manifest `{}`", manifest_path.display()))?;
    let manifest: Manifest = toml::from_str(&manifest_text).context("parse package manifest")?;
    validate_manifest(&manifest)?;

    let payload_archive = manifest_dir.join(&manifest.payload.archive);
    let payload_bytes = std::fs::read(&payload_archive)
        .with_context(|| format!("read payload archive `{}`", payload_archive.display()))?;

    run_package(PackageInput {
        manifest_text: &manifest_text,
        manifest: &manifest,
        payload_bytes: &payload_bytes,
        postcheck,
        cache_root,
    })
}

struct PackageInput<'a> {
    manifest_text: &'a str,
    manifest: &'a Manifest,
    payload_bytes: &'a [u8],
    postcheck: bool,
    cache_root: Option<&'a Path>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    app_id: String,
    app_version: String,
    target: String,
    plushie_rust_version: String,
    protocol_version: u32,
    start: Start,
    renderer: Renderer,
    payload: Payload,
}

#[derive(Debug, Deserialize)]
struct Start {
    working_dir: String,
    command: Vec<String>,
    forward_env: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Renderer {
    path: String,
    kind: String,
}

#[derive(Debug, Deserialize)]
struct Payload {
    archive: String,
    hash: String,
    size: Option<u64>,
}

struct PayloadRoot {
    path: PathBuf,
    reused: bool,
}

struct ExtractionLock {
    path: PathBuf,
}

fn run_package(input: PackageInput<'_>) -> Result<u8> {
    let hash = payload_hash(input.manifest)?;
    let payload_root = ensure_payload(&input)?;
    let root = payload_root.path;
    let renderer = absolute_payload_path(&root, &input.manifest.renderer.path);
    let working_dir = absolute_payload_path(&root, &input.manifest.start.working_dir);
    let host_program = input
        .manifest
        .start
        .command
        .first()
        .context("start.command is empty")?;
    let host_program = absolute_payload_path(&root, host_program);
    validate_extracted_paths(&renderer, &host_program, &working_dir)?;

    eprintln!(
        "plushie launcher: app={} version={} payload=sha256:{} cache={} cache_status={} renderer={} host={}",
        input.manifest.app_id,
        input.manifest.app_version,
        hash,
        root.display(),
        if payload_root.reused {
            "reused"
        } else {
            "extracted"
        },
        renderer.display(),
        host_program.display()
    );

    if input.postcheck {
        eprintln!("plushie launcher: postcheck ok");
        return Ok(0);
    }

    let mut command = Command::new(&host_program);
    command.current_dir(&working_dir).env_clear();

    for name in &input.manifest.start.forward_env {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }

    command
        .env("PLUSHIE_PACKAGE_DIR", &root)
        .env("PLUSHIE_BINARY_PATH", &renderer);

    if let Some(value) = std::env::var_os(PACKAGE_READY_FILE_ENV) {
        command.env(PACKAGE_READY_FILE_ENV, value);
    }

    for arg in input.manifest.start.command.iter().skip(1) {
        command.arg(arg);
    }

    let status = command
        .status()
        .with_context(|| format!("start host `{}`", host_program.display()))?;
    eprintln!("plushie launcher: host exited with {status}");
    if status.success()
        && let Err(err) = prune_cache(input.manifest, hash)
    {
        eprintln!("plushie launcher: cache pruning failed: {err:#}");
    }
    Ok(status.code().unwrap_or(1).try_into().unwrap_or(1))
}

fn validate_extracted_paths(
    renderer: &Path,
    host_program: &Path,
    working_dir: &Path,
) -> Result<()> {
    anyhow::ensure!(
        renderer.is_file(),
        "renderer.path does not exist after extraction: `{}`",
        renderer.display()
    );
    anyhow::ensure!(
        host_program.is_file(),
        "start.command[0] does not exist after extraction: `{}`",
        host_program.display()
    );
    anyhow::ensure!(
        working_dir.is_dir(),
        "start.working_dir does not exist after extraction: `{}`",
        working_dir.display()
    );
    Ok(())
}

fn ensure_payload(input: &PackageInput<'_>) -> Result<PayloadRoot> {
    verify_payload(input.manifest, input.payload_bytes)?;
    let hash = payload_hash(input.manifest)?;
    let root = app_cache_root(input.manifest, input.cache_root)?;
    let dest = root.join(hash);

    if cache_entry_is_complete(&dest, input.manifest_text) {
        return Ok(PayloadRoot {
            path: dest,
            reused: true,
        });
    }

    std::fs::create_dir_all(&root)?;
    let _lock = acquire_extraction_lock(&root, hash, &dest, input.manifest_text)?;
    if cache_entry_is_complete(&dest, input.manifest_text) {
        return Ok(PayloadRoot {
            path: dest,
            reused: true,
        });
    }

    let tmp = root.join(format!(".{hash}.{}.tmp", std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::create_dir_all(&tmp)?;

    if let Err(err) = extract_payload(&tmp, input) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(err);
    }

    make_executable(&absolute_payload_path(&tmp, &input.manifest.renderer.path))?;
    if let Some(program) = input.manifest.start.command.first() {
        let path = absolute_payload_path(&tmp, program);
        if path.is_file() {
            make_executable(&path)?;
        }
    }

    if dest.exists() {
        std::fs::remove_dir_all(&dest).context("remove incomplete payload cache")?;
    }
    std::fs::rename(&tmp, &dest).context("install extracted payload")?;
    Ok(PayloadRoot {
        path: dest,
        reused: false,
    })
}

fn payload_hash(manifest: &Manifest) -> Result<&str> {
    manifest
        .payload
        .hash
        .strip_prefix("sha256:")
        .context("payload hash missing sha256 prefix")
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    anyhow::ensure!(
        manifest.schema_version == EXPECTED_SCHEMA_VERSION,
        "unsupported package manifest schema_version {}",
        manifest.schema_version
    );
    anyhow::ensure!(
        manifest.target == current_package_target(),
        "target `{}` does not match current runtime host `{}`; cross-target packages are not supported yet",
        manifest.target,
        current_package_target()
    );
    anyhow::ensure!(
        manifest.protocol_version == EXPECTED_PROTOCOL_VERSION,
        "protocol_version mismatch: package expects {}, launcher supports {}",
        manifest.protocol_version,
        EXPECTED_PROTOCOL_VERSION
    );
    anyhow::ensure!(
        manifest.plushie_rust_version == EXPECTED_PLUSHIE_RUST_VERSION,
        "plushie_rust_version mismatch: package expects {}, launcher is {}",
        manifest.plushie_rust_version,
        EXPECTED_PLUSHIE_RUST_VERSION
    );
    validate_app_id(&manifest.app_id)?;
    validate_payload_relative_path("payload.archive", &manifest.payload.archive, false)?;
    validate_payload_relative_path("renderer.path", &manifest.renderer.path, false)?;
    let host_program = manifest
        .start
        .command
        .first()
        .context("start.command is empty")?;
    validate_payload_relative_path("start.command[0]", host_program, false)?;
    validate_payload_relative_path("start.working_dir", &manifest.start.working_dir, true)?;
    anyhow::ensure!(
        manifest.renderer.kind == "stock" || manifest.renderer.kind == "custom",
        "renderer.kind must be `stock` or `custom`, got `{}`",
        manifest.renderer.kind
    );
    if manifest
        .start
        .forward_env
        .iter()
        .any(|name| name.trim().is_empty() || name.contains([',', '=']))
    {
        anyhow::bail!(
            "start.forward_env must contain only non-empty variable names without `,` or `=`"
        );
    }
    if manifest.start.forward_env.iter().any(|name| {
        name == "PLUSHIE_BINARY_PATH"
            || name == "PLUSHIE_PACKAGE_DIR"
            || name == PACKAGE_READY_FILE_ENV
    }) {
        anyhow::bail!("start.forward_env must not include launcher-owned package variables");
    }
    Ok(())
}

fn verify_payload(manifest: &Manifest, payload_bytes: &[u8]) -> Result<()> {
    if let Some(size) = manifest.payload.size {
        anyhow::ensure!(
            payload_bytes.len() as u64 == size,
            "payload size mismatch: manifest expected {size} bytes, archive has {} bytes",
            payload_bytes.len()
        );
    }
    let expected = payload_hash(manifest)?;
    let actual = format!("{:x}", Sha256::digest(payload_bytes));
    anyhow::ensure!(
        actual == expected,
        "payload sha256 mismatch: expected {expected}, got {actual}"
    );
    Ok(())
}

fn extract_payload(tmp: &Path, input: &PackageInput<'_>) -> Result<()> {
    let decoder = zstd::stream::read::Decoder::new(input.payload_bytes)
        .context("open zstd payload archive")?;
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries().context("read payload archive entries")? {
        let mut entry = entry.context("read payload archive entry")?;
        let path = entry.path().context("read payload archive entry path")?;
        validate_payload_relative_path("payload archive entry", &path.to_string_lossy(), true)?;

        let entry_type = entry.header().entry_type();
        anyhow::ensure!(
            !entry_type.is_symlink() && !entry_type.is_hard_link(),
            "payload archive entry `{}` must not be a link",
            path.display()
        );
        anyhow::ensure!(
            entry_type.is_file() || entry_type.is_dir(),
            "payload archive entry `{}` has unsupported type",
            path.display()
        );

        let dest = tmp.join(&path);
        if entry_type.is_dir() {
            std::fs::create_dir_all(&dest)
                .with_context(|| format!("create directory `{}`", dest.display()))?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create directory `{}`", parent.display()))?;
            }
            entry
                .unpack(&dest)
                .with_context(|| format!("extract `{}`", dest.display()))?;
        }
    }

    std::fs::write(tmp.join("plushie-package.toml"), input.manifest_text)?;
    std::fs::write(
        tmp.join(COMPLETE_MARKER),
        format!(
            "app_id={}\napp_version={}\npayload_hash={}\nrenderer.path={}\nstart.command={}\n",
            input.manifest.app_id,
            input.manifest.app_version,
            input.manifest.payload.hash,
            input.manifest.renderer.path,
            input.manifest.start.command[0]
        ),
    )?;
    Ok(())
}

/// Age after which a lock directory is considered abandoned and safe to
/// reclaim. A hard-killed process cannot clean up the lock dir; waiting
/// forever is worse than reclaiming and re-extracting.
const STALE_LOCK_AGE: Duration = Duration::from_secs(120);

fn acquire_extraction_lock(
    root: &Path,
    hash: &str,
    dest: &Path,
    manifest_text: &str,
) -> Result<ExtractionLock> {
    let lock = root.join(format!(".{hash}.lock"));
    let start = Instant::now();
    loop {
        match std::fs::create_dir(&lock) {
            Ok(()) => return Ok(ExtractionLock { path: lock }),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if cache_entry_is_complete(dest, manifest_text) {
                    return Ok(ExtractionLock {
                        path: PathBuf::new(),
                    });
                }

                // Reclaim a lock that outlived any plausible extraction: a
                // SIGKILL or panic during extract_payload leaves the lock dir
                // behind with no holder to clean it up. If the mtime is older
                // than STALE_LOCK_AGE, delete and retry immediately.
                if let Ok(meta) = std::fs::metadata(&lock)
                    && let Ok(modified) = meta.modified()
                    && modified.elapsed().unwrap_or(STALE_LOCK_AGE) >= STALE_LOCK_AGE
                {
                    eprintln!(
                        "plushie launcher: reclaiming stale extraction lock `{}`",
                        lock.display()
                    );
                    let _ = std::fs::remove_dir(&lock);
                    continue;
                }

                anyhow::ensure!(
                    start.elapsed() < Duration::from_secs(60),
                    "timed out waiting for payload extraction lock `{}`",
                    lock.display()
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("create extraction lock `{}`", lock.display()));
            }
        }
    }
}

impl Drop for ExtractionLock {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
            let _ = std::fs::remove_dir(&self.path);
        }
    }
}

fn cache_entry_is_complete(dest: &Path, manifest_text: &str) -> bool {
    let manifest_path = dest.join("plushie-package.toml");
    let marker_path = dest.join(COMPLETE_MARKER);
    if !manifest_path.is_file() || !marker_path.is_file() {
        return false;
    }
    if !manifest_text.is_empty() {
        match std::fs::read_to_string(&manifest_path) {
            Ok(text) if text == manifest_text => {}
            _ => return false,
        }
    }
    let Ok(manifest) = std::fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|text| toml::from_str::<Manifest>(&text).ok())
        .ok_or(())
    else {
        return false;
    };
    absolute_payload_path(dest, &manifest.renderer.path).is_file()
        && manifest
            .start
            .command
            .first()
            .map(|program| absolute_payload_path(dest, program).is_file())
            .unwrap_or(false)
}

fn prune_cache(manifest: &Manifest, current_hash: &str) -> Result<()> {
    let root = app_cache_root(manifest, None)?;
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Ok(());
    };

    let mut old_payloads = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == current_hash || name.starts_with('.') {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        old_payloads.push((modified, entry.path()));
    }

    old_payloads.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in old_payloads.into_iter().skip(1) {
        std::fs::remove_dir_all(&path)
            .with_context(|| format!("remove old payload cache `{}`", path.display()))?;
    }

    Ok(())
}

fn app_cache_root(manifest: &Manifest, override_root: Option<&Path>) -> Result<PathBuf> {
    let root = match override_root {
        Some(root) => absolutize(root.to_path_buf())?,
        None => cache_root()?,
    };
    Ok(root
        .join("plushie/apps")
        .join(app_cache_name(&manifest.app_id)))
}

fn cache_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("PLUSHIE_CACHE_DIR") {
        return absolutize(PathBuf::from(path));
    }
    if cfg!(windows) {
        if let Some(path) = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|home| PathBuf::from(home).join("AppData/Local").into_os_string())
            })
        {
            return absolutize(PathBuf::from(path));
        }
    } else if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
        return absolutize(PathBuf::from(path));
    } else if let Some(home) = std::env::var_os("HOME") {
        return absolutize(PathBuf::from(home).join(".cache"));
    }
    absolutize(std::env::temp_dir())
}

fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn absolute_payload_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn validate_payload_relative_path(name: &str, value: &str, allow_dot: bool) -> Result<()> {
    let path = Path::new(value);
    anyhow::ensure!(
        !path.is_absolute(),
        "{name} must be payload-relative, got absolute path `{value}`"
    );

    let mut has_normal_component = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => has_normal_component = true,
            Component::CurDir => {}
            Component::ParentDir => {
                anyhow::bail!("{name} must not contain parent traversal: `{value}`");
            }
            Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("{name} must be payload-relative: `{value}`");
            }
        }
    }

    anyhow::ensure!(
        has_normal_component || allow_dot,
        "{name} must name a payload file path"
    );
    Ok(())
}

fn validate_app_id(value: &str) -> Result<()> {
    let safe = safe_name(value);
    anyhow::ensure!(
        safe != "." && safe != "..",
        "app_id must not map to a path-control component"
    );
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

fn current_package_target() -> String {
    format!("{}-{}", platform::os_name(), platform::arch_name())
}

fn app_cache_name(app_id: &str) -> String {
    let hash = Sha256::digest(app_id.as_bytes());
    format!(
        "{}-{:016x}",
        safe_name(app_id),
        u64::from_be_bytes(hash[..8].try_into().expect("sha256 digest is long enough"))
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn postchecks_external_package_manifest() {
        let dir = tempdir().unwrap();
        let payload_root = dir.path().join("payload");
        std::fs::create_dir_all(payload_root.join("bin")).unwrap();
        std::fs::write(payload_root.join("bin/host"), "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::write(payload_root.join("bin/plushie-renderer"), "renderer").unwrap();

        let archive = dir.path().join("payload.tar.zst");
        write_archive(&payload_root, &archive);
        let bytes = std::fs::read(&archive).unwrap();
        let hash = format!("sha256:{:x}", Sha256::digest(&bytes));

        let manifest = format!(
            r#"
schema_version = 1
app_id = "com.example.launcher"
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "test"
plushie_rust_version = "{EXPECTED_PLUSHIE_RUST_VERSION}"
protocol_version = {EXPECTED_PROTOCOL_VERSION}

[start]
working_dir = "."
command = ["bin/host"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "{hash}"
size = {}
"#,
            bytes.len()
        );
        let manifest_path = dir.path().join("plushie-package.toml");
        std::fs::write(&manifest_path, manifest).unwrap();

        let cache = tempdir().unwrap();
        let result =
            run_external_package_with_options(&manifest_path, true, Some(cache.path())).unwrap();
        assert_eq!(result, 0);
        assert!(cache.path().join("plushie/apps").exists());
    }

    #[test]
    fn postcheck_rejects_missing_extracted_host() {
        let dir = tempdir().unwrap();
        let payload_root = dir.path().join("payload");
        std::fs::create_dir_all(payload_root.join("bin")).unwrap();
        std::fs::write(payload_root.join("bin/plushie-renderer"), "renderer").unwrap();

        let archive = dir.path().join("payload.tar.zst");
        let file = std::fs::File::create(&archive).unwrap();
        let encoder = zstd::stream::write::Encoder::new(file, 0).unwrap();
        let mut builder = tar::Builder::new(encoder);
        builder
            .append_path_with_name(
                payload_root.join("bin/plushie-renderer"),
                "bin/plushie-renderer",
            )
            .unwrap();
        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap();
        let bytes = std::fs::read(&archive).unwrap();
        let hash = format!("sha256:{:x}", Sha256::digest(&bytes));

        let manifest = format!(
            r#"
schema_version = 1
app_id = "com.example.launcher"
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "test"
plushie_rust_version = "{EXPECTED_PLUSHIE_RUST_VERSION}"
protocol_version = {EXPECTED_PROTOCOL_VERSION}

[start]
working_dir = "."
command = ["bin/host"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "{hash}"
size = {}
"#,
            bytes.len()
        );
        let manifest_path = dir.path().join("plushie-package.toml");
        std::fs::write(&manifest_path, manifest).unwrap();

        let cache = tempdir().unwrap();
        let err = run_external_package_with_options(&manifest_path, true, Some(cache.path()))
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("start.command[0] does not exist after extraction")
        );
    }

    #[test]
    fn rejects_manifest_for_different_target() {
        let dir = tempdir().unwrap();
        let payload_root = dir.path().join("payload");
        std::fs::create_dir_all(payload_root.join("bin")).unwrap();
        std::fs::write(payload_root.join("bin/host"), "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::write(payload_root.join("bin/plushie-renderer"), "renderer").unwrap();

        let archive = dir.path().join("payload.tar.zst");
        write_archive(&payload_root, &archive);
        let bytes = std::fs::read(&archive).unwrap();
        let hash = format!("sha256:{:x}", Sha256::digest(&bytes));
        let other_target = if current_package_target() == "linux-x86_64" {
            "darwin-x86_64"
        } else {
            "linux-x86_64"
        };

        let manifest = format!(
            r#"
schema_version = 1
app_id = "com.example.launcher"
app_version = "0.1.0"
target = "{other_target}"
host_sdk = "test"
plushie_rust_version = "{EXPECTED_PLUSHIE_RUST_VERSION}"
protocol_version = {EXPECTED_PROTOCOL_VERSION}

[start]
working_dir = "."
command = ["bin/host"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "{hash}"
size = {}
"#,
            bytes.len()
        );
        let manifest_path = dir.path().join("plushie-package.toml");
        std::fs::write(&manifest_path, manifest).unwrap();

        let err = run_external_package_with_options(&manifest_path, true, None).unwrap_err();
        assert!(
            err.to_string()
                .contains("cross-target packages are not supported yet")
        );
    }

    #[test]
    fn cache_entry_requires_matching_manifest_text() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("payload");
        std::fs::create_dir_all(dest.join("bin")).unwrap();
        std::fs::write(dest.join("bin/host"), "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::write(dest.join("bin/plushie-renderer"), "renderer").unwrap();
        let manifest_text = format!(
            r#"
schema_version = 1
app_id = "com.example.launcher"
app_version = "0.1.0"
target = "{}"
plushie_rust_version = "{EXPECTED_PLUSHIE_RUST_VERSION}"
protocol_version = {EXPECTED_PROTOCOL_VERSION}

[start]
working_dir = "."
command = ["bin/host"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "sha256:deadbeef"
"#,
            current_package_target()
        );
        std::fs::write(dest.join("plushie-package.toml"), &manifest_text).unwrap();
        std::fs::write(dest.join(COMPLETE_MARKER), "ok").unwrap();

        assert!(cache_entry_is_complete(&dest, &manifest_text));
        assert!(!cache_entry_is_complete(
            &dest,
            &manifest_text.replace("bin/host", "bin/other-host")
        ));
    }

    fn write_archive(payload_root: &Path, archive: &Path) {
        let file = std::fs::File::create(archive).unwrap();
        let encoder = zstd::stream::write::Encoder::new(file, 0).unwrap();
        let mut builder = tar::Builder::new(encoder);
        builder
            .append_path_with_name(payload_root.join("bin/host"), "bin/host")
            .unwrap();
        builder
            .append_path_with_name(
                payload_root.join("bin/plushie-renderer"),
                "bin/plushie-renderer",
            )
            .unwrap();
        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap();
    }
}
