//! Standalone package command support.
//!
//! The SDKs own host-language packaging. This module owns the shared
//! Plushie wrapper step: validate a package manifest, embed its payload
//! archive in a generated Rust launcher, and build that launcher.

use crate::{Error, Result, generator};
use anyhow::Context;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

const GENERATED_MANIFEST: &str = "plushie-package.toml";
/// Conventional developer-owned source package config filename.
pub const SOURCE_CONFIG: &str = "plushie-package.config.toml";
const GENERATED_PAYLOAD: &str = "payload.tar.zst";
const GENERATED_LOCKFILE: &str = "Cargo.lock";
const SHARED_LOCKFILE: &str = "launcher-Cargo.lock";
const SHARED_LOCKFILE_FINGERPRINT: &str = "launcher-Cargo.lock.sha256";
const LAUNCHER_CRATE_NAME: &str = "plushie-package-launcher";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const EXPECTED_PLUSHIE_RUST_VERSION: &str = env!("CARGO_PKG_VERSION");
const EXPECTED_PROTOCOL_VERSION: u32 = plushie_core::protocol::PROTOCOL_VERSION;
const SOURCE_CONFIG_VERSION: u32 = 1;

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

/// Options for postchecking a generated standalone launcher.
#[derive(Debug)]
pub struct PackagePostcheckOpts<'a> {
    /// Package build options.
    pub package: PackageOpts<'a>,
    /// Maximum time to wait for the postcheck run to exit.
    pub timeout: Duration,
}

/// Result of building a standalone launcher.
#[derive(Debug)]
pub struct PackageResult {
    /// Generated launcher crate directory.
    pub launcher_crate_dir: PathBuf,
    /// Final launcher executable path.
    pub binary_path: PathBuf,
}

/// Result of running the generated launcher's postcheck path.
#[derive(Debug)]
pub struct PackagePostcheckResult {
    /// Generated launcher crate directory.
    pub launcher_crate_dir: PathBuf,
    /// Final launcher executable path.
    pub binary_path: PathBuf,
    /// Isolated cache directory used by the postcheck run.
    pub cache_dir: PathBuf,
    /// Captured launcher stderr.
    pub stderr: String,
}

/// Result of prechecking a standalone package manifest and payload.
#[derive(Debug)]
pub struct PackagePrecheckResult {
    /// Package application ID.
    pub app_id: String,
    /// Package application version.
    pub app_version: String,
    /// Payload SHA-256 field from the manifest.
    pub payload_hash: String,
    /// Non-fatal package issues found during precheck.
    pub warnings: Vec<PackageWarning>,
}

/// Developer-owned package configuration read from source control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceConfig {
    /// Startup configuration for the host process.
    pub start: PackageStartConfig,
}

/// Host startup configuration shared by generated manifests and source config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageStartConfig {
    /// Payload-relative working directory for the host process.
    pub working_dir: String,
    /// Structured argv for the host process. The first value is
    /// payload-relative, remaining values are literal arguments.
    pub command: Vec<String>,
    /// Parent environment variable names forwarded into the host process.
    pub forward_env: Vec<String>,
}

/// Non-fatal package issue found during precheck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageWarning {
    /// The package has no payload-local platform icon.
    MissingPlatformIcon,
}

impl PackageWarning {
    /// Human-readable warning text for CLI output.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::MissingPlatformIcon => {
                "package manifest has no platform.icon; the launcher can still be built, \
                 but platform bundles and update metadata may have no app icon. Add an icon \
                 to the payload and set [platform].icon, or run `cargo plushie default-icons \
                 --out <payload>/assets` while staging."
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
    start: StartManifest,
    renderer: RendererManifest,
    platform: Option<PlatformManifest>,
    updates: Option<UpdatesManifest>,
    signing: Option<SigningManifest>,
    payload: PayloadManifest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartManifest {
    working_dir: String,
    command: Vec<String>,
    forward_env: Vec<String>,
}

impl StartManifest {
    fn to_start_config(&self) -> PackageStartConfig {
        PackageStartConfig {
            working_dir: self.working_dir.clone(),
            command: self.command.clone(),
            forward_env: self.forward_env.clone(),
        }
    }
}

impl From<PackageStartConfig> for StartManifest {
    fn from(config: PackageStartConfig) -> Self {
        Self {
            working_dir: config.working_dir,
            command: config.command,
            forward_env: config.forward_env,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceConfigDocument {
    config_version: u32,
    start: StartManifest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RendererManifest {
    path: String,
    kind: String,
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadManifest {
    archive: String,
    hash: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlatformManifest {
    publisher: Option<String>,
    bundle_id: Option<String>,
    icon: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdatesManifest {
    channel: String,
    feed_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SigningManifest {
    #[serde(default)]
    hooks: Vec<SigningHookManifest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SigningHookManifest {
    stage: String,
    command: Vec<String>,
}

struct PreparedLauncher {
    crate_dir: PathBuf,
    build_target_dir: PathBuf,
    manifest_dir: PathBuf,
    package_name: String,
    output_path: PathBuf,
    shared_lockfile: PathBuf,
    lockfile_reused: bool,
    signing_hooks: Vec<SigningHookManifest>,
}

struct LoadedPackage {
    manifest_dir: PathBuf,
    manifest_text: String,
    manifest: PackageManifest,
    payload: Vec<u8>,
}

/// Precheck a package manifest and payload without building a launcher.
///
/// # Errors
///
/// Returns an error when the manifest is invalid, the payload is
/// missing, the payload hash mismatches, or the archive contains an
/// unsafe entry.
pub fn precheck_package(manifest_path: &Path) -> Result<PackagePrecheckResult> {
    let loaded = load_package(manifest_path)?;
    let warnings = package_warnings(&loaded.manifest);
    Ok(PackagePrecheckResult {
        app_id: loaded.manifest.app_id,
        app_version: loaded.manifest.app_version,
        payload_hash: loaded.manifest.payload.hash,
        warnings,
    })
}

/// Return the default source config path for an app source directory.
#[must_use]
pub fn default_source_config_path(source_dir: &Path) -> PathBuf {
    source_dir.join(SOURCE_CONFIG)
}

/// Return the conventional environment passthrough list for packaged apps.
#[must_use]
pub fn default_forward_env() -> Vec<String> {
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

/// Read and validate a developer-owned package config file.
///
/// # Errors
///
/// Returns an error when the file is missing, invalid TOML, has an
/// unsupported config version, or contains unsafe startup config.
pub fn load_source_config(path: &Path) -> Result<PackageSourceConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read package config `{}`", path.display()))?;
    parse_source_config(&text)
}

/// Read the default package config if it exists.
///
/// # Errors
///
/// Returns an error when the conventional config file exists but cannot
/// be read or validated.
pub fn load_default_source_config(source_dir: &Path) -> Result<Option<PackageSourceConfig>> {
    let path = default_source_config_path(source_dir);
    if !path.is_file() {
        return Ok(None);
    }
    load_source_config(&path).map(Some)
}

/// Write a readable developer-owned package config template.
///
/// # Errors
///
/// Returns an error when the supplied config is invalid or the file
/// cannot be written.
pub fn write_source_config_template(path: &Path, config: &PackageSourceConfig) -> Result<()> {
    validate_start_config(&config.start)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::generator::write_if_changed(path, &render_source_config_template(config))
}

/// Render a readable developer-owned package config template.
#[must_use]
pub fn render_source_config_template(config: &PackageSourceConfig) -> String {
    let mut text = String::new();
    text.push_str("# Plushie standalone package config.\n");
    text.push_str("# Commit this file and edit it when the packaged app needs a\n");
    text.push_str("# different entry point, working directory, or forwarded environment.\n\n");
    text.push_str(&format!("config_version = {SOURCE_CONFIG_VERSION}\n\n"));
    text.push_str("[start]\n");
    text.push_str("# Relative to the extracted app package.\n");
    text.push_str(&format!(
        "working_dir = {}\n",
        toml_string_literal(&config.start.working_dir)
    ));
    text.push_str("# Structured argv. The first item is the packaged host executable.\n");
    text.push_str(&format!(
        "command = {}\n",
        toml_array(&config.start.command)
    ));
    text.push_str("# Environment variable names copied from the parent process.\n");
    text.push_str("forward_env = [\n");
    for name in &config.start.forward_env {
        text.push_str(&format!("  {},\n", toml_string_literal(name)));
    }
    text.push_str("]\n");
    text
}

fn toml_string_literal(value: &str) -> String {
    toml_edit::value(value).to_string()
}

fn toml_array(values: &[String]) -> String {
    let mut array = toml_edit::Array::new();
    for value in values {
        array.push(value.as_str());
    }
    array.to_string()
}

fn parse_source_config(text: &str) -> Result<PackageSourceConfig> {
    let config: SourceConfigDocument =
        toml::from_str(text).with_context(|| "failed to parse package config")?;
    if config.config_version != SOURCE_CONFIG_VERSION {
        return Err(Error::Other(anyhow::anyhow!(
            "unsupported package config config_version {}",
            config.config_version
        )));
    }
    let start = config.start.to_start_config();
    validate_start_config(&start)?;
    Ok(PackageSourceConfig { start })
}

fn package_warnings(manifest: &PackageManifest) -> Vec<PackageWarning> {
    let mut warnings = Vec::new();
    if manifest
        .platform
        .as_ref()
        .and_then(|platform| platform.icon.as_ref())
        .is_none()
    {
        warnings.push(PackageWarning::MissingPlatformIcon);
    }
    warnings
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
    cmd.env("CARGO_TARGET_DIR", &prepared.build_target_dir);
    if prepared.lockfile_reused {
        cmd.arg("--locked");
    }
    if opts.release {
        cmd.arg("--release");
    }
    if opts.verbose {
        let locked = if prepared.lockfile_reused {
            " --locked"
        } else {
            ""
        };
        eprintln!(
            "running: CARGO_TARGET_DIR={} cargo build{locked}{}",
            prepared.build_target_dir.display(),
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
        .build_target_dir
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
    run_signing_hooks(&prepared)?;
    update_shared_launcher_lockfile(&prepared)?;

    Ok(PackageResult {
        launcher_crate_dir: prepared.crate_dir,
        binary_path: prepared.output_path,
    })
}

/// Build a launcher and run its postcheck path with an isolated cache.
///
/// # Errors
///
/// Returns an error when launcher build fails, the postcheck process fails
/// or times out, or expected diagnostics are missing.
pub fn postcheck_package(opts: &PackagePostcheckOpts<'_>) -> Result<PackagePostcheckResult> {
    let result = build_launcher(&opts.package)?;
    let cache_dir = postcheck_cache_dir()?;
    let first = run_postcheck_launcher(&result.binary_path, &cache_dir, opts.timeout)?;
    let first_stderr = validate_postcheck_output(first, "extracted")?;
    let second = run_postcheck_launcher(&result.binary_path, &cache_dir, opts.timeout)?;
    let second_stderr = validate_postcheck_output(second, "reused")?;
    let stderr = format!("{first_stderr}{second_stderr}");

    Ok(PackagePostcheckResult {
        launcher_crate_dir: result.launcher_crate_dir,
        binary_path: result.binary_path,
        cache_dir,
        stderr,
    })
}

fn validate_postcheck_output(output: std::process::Output, cache_status: &str) -> Result<String> {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    if !output.status.success() {
        return Err(Error::Other(anyhow::anyhow!(
            "standalone launcher postcheck failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            stdout,
            stderr
        )));
    }
    if !stdout.trim().is_empty() {
        return Err(Error::Other(anyhow::anyhow!(
            "standalone launcher postcheck wrote to stdout:\n{}",
            stdout
        )));
    }
    let cache_status = format!("cache_status={cache_status}");
    for expected in [
        "plushie launcher: app=",
        cache_status.as_str(),
        "renderer=",
        "host=",
        "plushie launcher: postcheck ok",
    ] {
        if !stderr.contains(expected) {
            return Err(Error::Other(anyhow::anyhow!(
                "standalone launcher postcheck missing diagnostic `{expected}`\nstderr:\n{stderr}"
            )));
        }
    }

    Ok(stderr)
}

fn run_postcheck_launcher(
    binary_path: &Path,
    cache_dir: &Path,
    timeout: Duration,
) -> Result<std::process::Output> {
    let mut child = std::process::Command::new(binary_path)
        .env("PLUSHIE_CACHE_DIR", cache_dir)
        .env("PLUSHIE_PACKAGE_POSTCHECK", "1")
        .env_remove("PLUSHIE_BINARY_PATH")
        .env_remove("PLUSHIE_RUST_SOURCE_PATH")
        .env_remove("PLUSHIE_RENDERER_BINARY")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("start postcheck launcher `{}`", binary_path.display()))?;

    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            let output = child
                .wait_with_output()
                .with_context(|| "read postcheck launcher output")?;
            return Ok(output);
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .with_context(|| "read timed-out postcheck launcher output")?;
            return Err(Error::Other(anyhow::anyhow!(
                "standalone launcher postcheck timed out after {:?}\nstdout:\n{}\nstderr:\n{}",
                timeout,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn postcheck_cache_dir() -> Result<PathBuf> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "plushie-package-postcheck-{}-{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn prepare_launcher_crate(opts: &PackageOpts<'_>) -> Result<PreparedLauncher> {
    let loaded = load_package(opts.manifest_path)?;

    let target_root = package_target_root(&loaded.manifest_dir);
    let package_name = package_name(&loaded.manifest.app_id);
    let package_root = target_root.join("plushie-package");
    let crate_dir = package_root.join(&package_name);
    let build_target_dir = package_root.join("target");
    let shared_lockfile = package_root.join(SHARED_LOCKFILE);
    let shared_lockfile_fingerprint = package_root.join(SHARED_LOCKFILE_FINGERPRINT);
    let output_path = opts.out_path.map(Path::to_path_buf).unwrap_or_else(|| {
        target_root
            .join("plushie/package")
            .join(executable_name(&safe_name(&loaded.manifest.app_id)))
    });

    std::fs::create_dir_all(crate_dir.join("src"))?;
    generator::write_if_changed(
        &crate_dir.join("Cargo.toml"),
        &launcher_cargo_toml(&package_name),
    )?;
    generator::write_if_changed(&crate_dir.join("src/main.rs"), &launcher_main_rs())?;
    generator::write_if_changed(&crate_dir.join(GENERATED_MANIFEST), &loaded.manifest_text)?;
    write_bytes_if_changed(&crate_dir.join(GENERATED_PAYLOAD), &loaded.payload)?;
    let lockfile_reused =
        reuse_shared_launcher_lockfile(&shared_lockfile, &shared_lockfile_fingerprint, &crate_dir)?;

    Ok(PreparedLauncher {
        crate_dir,
        build_target_dir,
        manifest_dir: loaded.manifest_dir,
        package_name,
        output_path,
        shared_lockfile,
        lockfile_reused,
        signing_hooks: loaded
            .manifest
            .signing
            .map(|signing| signing.hooks)
            .unwrap_or_default(),
    })
}

fn run_signing_hooks(prepared: &PreparedLauncher) -> Result<()> {
    for hook in &prepared.signing_hooks {
        let argv: Vec<String> = hook
            .command
            .iter()
            .map(|arg| expand_signing_hook_arg(arg, &prepared.output_path))
            .collect();
        let program = argv.first().expect("validated signing hook argv");
        let status = std::process::Command::new(program)
            .args(&argv[1..])
            .current_dir(&prepared.manifest_dir)
            .status()
            .with_context(|| {
                format!(
                    "failed to run signing hook `{}` for stage `{}`",
                    program, hook.stage
                )
            })?;
        if !status.success() {
            return Err(Error::Other(anyhow::anyhow!(
                "signing hook `{}` for stage `{}` failed with status {}",
                program,
                hook.stage,
                status
            )));
        }
    }
    Ok(())
}

fn expand_signing_hook_arg(arg: &str, launcher_path: &Path) -> String {
    arg.replace("{launcher}", &launcher_path.display().to_string())
}

fn reuse_shared_launcher_lockfile(
    shared_lockfile: &Path,
    shared_lockfile_fingerprint: &Path,
    crate_dir: &Path,
) -> Result<bool> {
    let lockfile = crate_dir.join(GENERATED_LOCKFILE);
    let expected_fingerprint = launcher_lockfile_fingerprint();
    let fingerprint = std::fs::read_to_string(shared_lockfile_fingerprint).unwrap_or_default();

    if shared_lockfile.is_file() && fingerprint.trim() == expected_fingerprint {
        let contents = std::fs::read_to_string(shared_lockfile).with_context(|| {
            format!(
                "failed to read shared launcher lockfile `{}`",
                shared_lockfile.display()
            )
        })?;
        generator::write_if_changed(&lockfile, &contents)?;
        return Ok(true);
    }

    match std::fs::remove_file(&lockfile) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(Error::Other(anyhow::anyhow!(
                "failed to remove stale launcher lockfile `{}`: {err}",
                lockfile.display()
            )));
        }
    }
    Ok(false)
}

fn update_shared_launcher_lockfile(prepared: &PreparedLauncher) -> Result<()> {
    let crate_lockfile = prepared.crate_dir.join(GENERATED_LOCKFILE);
    let contents = std::fs::read_to_string(&crate_lockfile).with_context(|| {
        format!(
            "failed to read generated launcher lockfile `{}`",
            crate_lockfile.display()
        )
    })?;
    generator::write_if_changed(&prepared.shared_lockfile, &contents)?;
    generator::write_if_changed(
        &prepared
            .shared_lockfile
            .with_file_name(SHARED_LOCKFILE_FINGERPRINT),
        &(launcher_lockfile_fingerprint() + "\n"),
    )?;
    Ok(())
}

fn write_bytes_if_changed(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Ok(existing) = std::fs::read(path)
        && existing == content
    {
        return Ok(());
    }
    std::fs::write(path, content)?;
    Ok(())
}

fn package_target_root(manifest_dir: &Path) -> PathBuf {
    package_target_root_from(
        std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        &std::env::current_dir().unwrap_or_else(|_| manifest_dir.to_path_buf()),
        manifest_dir,
    )
}

fn package_target_root_from(
    cargo_target_dir: Option<PathBuf>,
    invocation_dir: &Path,
    manifest_dir: &Path,
) -> PathBuf {
    match cargo_target_dir {
        Some(path) if path.is_absolute() => path,
        Some(path) => invocation_dir.join(path),
        None => manifest_dir.join("target"),
    }
}

fn load_package(manifest_path: &Path) -> Result<LoadedPackage> {
    let manifest_path = std::fs::canonicalize(manifest_path)
        .with_context(|| format!("package manifest `{}` not found", manifest_path.display()))?;
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
    validate_payload_archive(&manifest, &payload)?;

    Ok(LoadedPackage {
        manifest_dir: manifest_dir.to_path_buf(),
        manifest_text,
        manifest,
        payload,
    })
}

fn parse_manifest(text: &str) -> Result<PackageManifest> {
    let manifest: PackageManifest =
        toml::from_str(text).with_context(|| "failed to parse package manifest")?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &PackageManifest) -> Result<()> {
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
    validate_start_config(&manifest.start.to_start_config())?;
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
    if let Some(source) = &manifest.renderer.source {
        require_nonempty("renderer.source", source)?;
    }
    require_nonempty("payload.archive", &manifest.payload.archive)?;
    validate_manifest_relative_path("payload.archive", &manifest.payload.archive, false)?;
    validate_platform_metadata(manifest)?;
    validate_update_metadata(manifest)?;
    validate_signing_metadata(manifest)?;
    if manifest.protocol_version != EXPECTED_PROTOCOL_VERSION {
        return Err(Error::Other(anyhow::anyhow!(
            "protocol_version mismatch: package expects {}, cargo-plushie supports {}",
            manifest.protocol_version,
            EXPECTED_PROTOCOL_VERSION
        )));
    }
    validate_sha256_field(&manifest.payload.hash)?;
    Ok(())
}

/// Validate host startup config shared by source config and manifests.
///
/// # Errors
///
/// Returns an error when paths are unsafe or argv/env values are empty
/// or reserved.
pub fn validate_start_config(start: &PackageStartConfig) -> Result<()> {
    require_nonempty("start.working_dir", &start.working_dir)?;
    validate_payload_relative_path("start.working_dir", &start.working_dir, true)?;
    if start.command.is_empty() || start.command.iter().any(|arg| arg.is_empty()) {
        return Err(Error::Other(anyhow::anyhow!(
            "start.command must contain a non-empty argv"
        )));
    }
    validate_payload_relative_path("start.command[0]", &start.command[0], false)?;
    if start
        .forward_env
        .iter()
        .any(|name| name.trim().is_empty() || name.contains([',', '=']))
    {
        return Err(Error::Other(anyhow::anyhow!(
            "start.forward_env must contain only non-empty variable names without `,` or `=`"
        )));
    }
    if start
        .forward_env
        .iter()
        .any(|name| name == "PLUSHIE_BINARY_PATH" || name == "PLUSHIE_PACKAGE_DIR")
    {
        return Err(Error::Other(anyhow::anyhow!(
            "start.forward_env must not include launcher-owned package variables"
        )));
    }
    Ok(())
}

fn validate_platform_metadata(manifest: &PackageManifest) -> Result<()> {
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
    Ok(())
}

fn validate_update_metadata(manifest: &PackageManifest) -> Result<()> {
    let Some(updates) = &manifest.updates else {
        return Ok(());
    };

    require_nonempty("updates.channel", &updates.channel)?;
    if let Some(feed_url) = &updates.feed_url {
        require_nonempty("updates.feed_url", feed_url)?;
    }
    Ok(())
}

fn validate_signing_metadata(manifest: &PackageManifest) -> Result<()> {
    let Some(signing) = &manifest.signing else {
        return Ok(());
    };

    for hook in &signing.hooks {
        require_nonempty("signing.hooks.stage", &hook.stage)?;
        match hook.stage.as_str() {
            "after-launcher-build" => {}
            value => {
                return Err(Error::Other(anyhow::anyhow!(
                    "signing hook stage must be `after-launcher-build`, got `{value}`"
                )));
            }
        }

        if hook.command.is_empty() || hook.command.iter().any(|arg| arg.is_empty()) {
            return Err(Error::Other(anyhow::anyhow!(
                "signing hook command must contain a non-empty argv"
            )));
        }
    }
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

fn validate_payload_relative_path(name: &str, value: &str, allow_dot: bool) -> Result<()> {
    let path = clean_relative_path(name, value, "payload-relative")?;
    if path.as_os_str().is_empty() && !allow_dot {
        return Err(Error::Other(anyhow::anyhow!(
            "{name} must name a payload file path"
        )));
    }

    Ok(())
}

fn validate_manifest_relative_path(name: &str, value: &str, allow_dot: bool) -> Result<()> {
    let path = clean_relative_path(name, value, "manifest-relative")?;
    if path.as_os_str().is_empty() && !allow_dot {
        return Err(Error::Other(anyhow::anyhow!(
            "{name} must name a manifest-relative file path"
        )));
    }

    Ok(())
}

fn validate_app_id(value: &str) -> Result<()> {
    let safe = safe_name(value);
    if safe == "." || safe == ".." {
        return Err(Error::Other(anyhow::anyhow!(
            "app_id must not map to a path-control component"
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

fn clean_payload_relative_path(name: &str, value: &str) -> Result<PathBuf> {
    clean_relative_path(name, value, "payload-relative")
}

fn clean_relative_path(name: &str, value: &str, relation: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(Error::Other(anyhow::anyhow!(
            "{name} must be {relation}, got absolute path `{value}`"
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
                    "{name} must be {relation}: `{value}`"
                )));
            }
        }
    }
    Ok(cleaned)
}

fn validate_payload_archive(manifest: &PackageManifest, payload: &[u8]) -> Result<()> {
    let renderer_path = clean_payload_relative_path("renderer.path", &manifest.renderer.path)?;
    let host_path = clean_payload_relative_path("start.command[0]", &manifest.start.command[0])?;
    let working_dir =
        clean_payload_relative_path("start.working_dir", &manifest.start.working_dir)?;
    let platform_icon = manifest
        .platform
        .as_ref()
        .and_then(|platform| platform.icon.as_deref())
        .map(|path| clean_payload_relative_path("platform.icon", path))
        .transpose()?;
    let mut found_renderer = false;
    let mut found_host = false;
    let mut found_working_dir = working_dir.as_os_str().is_empty();
    let mut found_platform_icon = platform_icon.is_none();

    let decoder = zstd::stream::read::Decoder::new(payload)
        .with_context(|| "failed to open payload archive as zstd")?;
    let mut archive = tar::Archive::new(decoder);
    for entry in archive
        .entries()
        .with_context(|| "failed to read payload archive entries")?
    {
        let entry = entry.with_context(|| "failed to read payload archive entry")?;
        validate_archive_entry(&entry)?;
        let entry_path = entry
            .path()
            .with_context(|| "failed to read payload archive entry path")?;
        let entry_path =
            clean_payload_relative_path("payload archive entry", &entry_path.to_string_lossy())?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_file() {
            found_renderer |= entry_path == renderer_path;
            found_host |= entry_path == host_path;
            if let Some(platform_icon) = &platform_icon {
                found_platform_icon |= entry_path == *platform_icon;
            }
        }
        if entry_type.is_dir() && !working_dir.as_os_str().is_empty() {
            found_working_dir |= entry_path == working_dir;
        }
    }

    if !found_renderer {
        return Err(Error::Other(anyhow::anyhow!(
            "payload archive does not contain renderer.path `{}`",
            manifest.renderer.path
        )));
    }
    if !found_host {
        return Err(Error::Other(anyhow::anyhow!(
            "payload archive does not contain start.command[0] `{}`",
            manifest.start.command[0]
        )));
    }
    if !found_working_dir {
        return Err(Error::Other(anyhow::anyhow!(
            "payload archive does not contain start.working_dir `{}`",
            manifest.start.working_dir
        )));
    }
    if !found_platform_icon {
        let icon = manifest
            .platform
            .as_ref()
            .and_then(|platform| platform.icon.as_deref())
            .expect("platform icon path is present");
        return Err(Error::Other(anyhow::anyhow!(
            "payload archive does not contain platform.icon `{icon}`"
        )));
    }
    Ok(())
}

fn validate_archive_entry<R: std::io::Read>(entry: &tar::Entry<'_, R>) -> Result<()> {
    let path = entry
        .path()
        .with_context(|| "failed to read payload archive entry path")?;
    validate_payload_relative_path("payload archive entry", &path.to_string_lossy(), true)?;

    let entry_type = entry.header().entry_type();
    if entry_type.is_symlink() || entry_type.is_hard_link() {
        return Err(Error::Other(anyhow::anyhow!(
            "payload archive entry `{}` must not be a link",
            path.display()
        )));
    }
    if !(entry_type.is_file() || entry_type.is_dir()) {
        return Err(Error::Other(anyhow::anyhow!(
            "payload archive entry `{}` has unsupported type",
            path.display()
        )));
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

const LAUNCHER_DEPENDENCIES: &str = r#"[dependencies]
anyhow = "=1.0.102"
serde = { version = "=1.0.228", features = ["derive"] }
sha2 = "=0.10.9"
tar = "=0.4.45"
toml = "=0.8.23"
zstd = "=0.13.3"
"#;

fn launcher_cargo_toml(package_name: &str) -> String {
    format!(
        r#"{}

[[bin]]
name = "{package_name}"
path = "src/main.rs"

{LAUNCHER_DEPENDENCIES}"#,
        launcher_package_toml()
    )
}

fn launcher_package_toml() -> String {
    format!(
        r#"[package]
name = "{LAUNCHER_CRATE_NAME}"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]"#
    )
}

fn launcher_lockfile_fingerprint() -> String {
    let input = format!("{}\n{LAUNCHER_DEPENDENCIES}", launcher_package_toml());
    format!("{:x}", Sha256::digest(input))
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

fn launcher_main_rs() -> String {
    LAUNCHER_MAIN_TEMPLATE
        .replace(
            "__MANIFEST_SCHEMA_VERSION__",
            &MANIFEST_SCHEMA_VERSION.to_string(),
        )
        .replace(
            "__EXPECTED_PROTOCOL_VERSION__",
            &EXPECTED_PROTOCOL_VERSION.to_string(),
        )
        .replace(
            "__EXPECTED_PLUSHIE_RUST_VERSION__",
            EXPECTED_PLUSHIE_RUST_VERSION,
        )
}

const LAUNCHER_MAIN_TEMPLATE: &str = r###"use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

const MANIFEST_TEXT: &str = include_str!("../plushie-package.toml");
const PAYLOAD_BYTES: &[u8] = include_bytes!("../payload.tar.zst");
const COMPLETE_MARKER: &str = ".plushie-complete";
const EXPECTED_SCHEMA_VERSION: u32 = __MANIFEST_SCHEMA_VERSION__;
const EXPECTED_PROTOCOL_VERSION: u32 = __EXPECTED_PROTOCOL_VERSION__;
const EXPECTED_PLUSHIE_RUST_VERSION: &str = "__EXPECTED_PLUSHIE_RUST_VERSION__";

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    app_id: String,
    app_version: String,
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
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Payload {
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
    validate_manifest(&manifest)?;
    let hash = payload_hash(&manifest.payload)?;
    let payload_root = ensure_payload(&manifest)?;
    let root = payload_root.path;
    let renderer = absolute_payload_path(&root, &manifest.renderer.path);
    let working_dir = absolute_payload_path(&root, &manifest.start.working_dir);
    let host_program = manifest.start.command.first().context("start.command is empty")?;
    let host_program = absolute_payload_path(&root, host_program);

    eprintln!(
        "plushie launcher: app={} version={} payload=sha256:{} cache={} cache_status={} renderer={} host={}",
        manifest.app_id,
        manifest.app_version,
        hash,
        root.display(),
        if payload_root.reused { "reused" } else { "extracted" },
        renderer.display(),
        host_program.display()
    );

    if std::env::var_os("PLUSHIE_PACKAGE_POSTCHECK").is_some() {
        eprintln!("plushie launcher: postcheck ok");
        return Ok(0);
    }

    let mut command = Command::new(&host_program);
    command.current_dir(&working_dir).env_clear();

    for name in &manifest.start.forward_env {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }

    command
        .env("PLUSHIE_PACKAGE_DIR", &root)
        .env("PLUSHIE_BINARY_PATH", &renderer);

    for arg in manifest.start.command.iter().skip(1) {
        command.arg(arg);
    }

    let status = command
        .status()
        .with_context(|| format!("start host `{}`", host_program.display()))?;
    eprintln!("plushie launcher: host exited with {status}");
    if status.success() {
        if let Err(err) = prune_cache(&manifest, hash) {
            eprintln!("plushie launcher: cache pruning failed: {err:#}");
        }
    }
    Ok(status.code().unwrap_or(1).try_into().unwrap_or(1))
}

fn ensure_payload(manifest: &Manifest) -> Result<PayloadRoot> {
    verify_payload(&manifest.payload)?;
    let hash = payload_hash(&manifest.payload)?;
    let root = app_cache_root(manifest);
    let dest = root.join(hash);

    if cache_entry_is_complete(&dest) {
        return Ok(PayloadRoot {
            path: dest,
            reused: true,
        });
    }

    std::fs::create_dir_all(&root)?;
    let _lock = acquire_extraction_lock(&root, hash, &dest)?;
    if cache_entry_is_complete(&dest) {
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

    if let Err(err) = extract_payload(&tmp, manifest) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(err);
    }

    make_executable(&absolute_payload_path(&tmp, &manifest.renderer.path))?;
    if let Some(program) = manifest.start.command.first() {
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

fn payload_hash(payload: &Payload) -> Result<&str> {
    payload
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
    if let Some(source) = &manifest.renderer.source {
        anyhow::ensure!(
            !source.trim().is_empty(),
            "renderer.source must not be empty"
        );
    }
    if manifest
        .start
        .forward_env
        .iter()
        .any(|name| name.trim().is_empty() || name.contains(|ch| ch == ',' || ch == '='))
    {
        anyhow::bail!(
            "start.forward_env must contain only non-empty variable names without `,` or `=`"
        );
    }
    if manifest
        .start
        .forward_env
        .iter()
        .any(|name| name == "PLUSHIE_BINARY_PATH" || name == "PLUSHIE_PACKAGE_DIR")
    {
        anyhow::bail!("start.forward_env must not include launcher-owned package variables");
    }
    Ok(())
}

fn verify_payload(payload: &Payload) -> Result<()> {
    if let Some(size) = payload.size {
        anyhow::ensure!(
            PAYLOAD_BYTES.len() as u64 == size,
            "embedded payload size mismatch: manifest expected {size} bytes, archive has {} bytes",
            PAYLOAD_BYTES.len()
        );
    }
    let expected = payload
        .hash
        .strip_prefix("sha256:")
        .context("payload hash missing sha256 prefix")?;
    let actual = format!("{:x}", Sha256::digest(PAYLOAD_BYTES));
    anyhow::ensure!(
        actual == expected,
        "embedded payload sha256 mismatch: expected {expected}, got {actual}"
    );
    Ok(())
}

fn extract_payload(tmp: &Path, manifest: &Manifest) -> Result<()> {
    let decoder = zstd::stream::read::Decoder::new(PAYLOAD_BYTES)
        .context("open embedded zstd payload")?;
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries().context("read embedded payload entries")? {
        let mut entry = entry.context("read embedded payload entry")?;
        let path = entry.path().context("read embedded payload entry path")?;
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

    std::fs::write(tmp.join("plushie-package.toml"), MANIFEST_TEXT)?;
    std::fs::write(
        tmp.join(COMPLETE_MARKER),
        format!(
            "app_id={}\napp_version={}\npayload_hash={}\nrenderer.path={}\nstart.command={}\n",
            manifest.app_id,
            manifest.app_version,
            manifest.payload.hash,
            manifest.renderer.path,
            manifest.start.command[0]
        ),
    )?;
    Ok(())
}

fn acquire_extraction_lock(root: &Path, hash: &str, dest: &Path) -> Result<ExtractionLock> {
    let lock = root.join(format!(".{hash}.lock"));
    let start = Instant::now();
    loop {
        match std::fs::create_dir(&lock) {
            Ok(()) => return Ok(ExtractionLock { path: lock }),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if cache_entry_is_complete(dest) {
                    return Ok(ExtractionLock { path: PathBuf::new() });
                }
                anyhow::ensure!(
                    start.elapsed() < Duration::from_secs(60),
                    "timed out waiting for payload extraction lock `{}`",
                    lock.display()
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(err).with_context(|| format!("create extraction lock `{}`", lock.display())),
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

fn cache_entry_is_complete(dest: &Path) -> bool {
    let manifest_path = dest.join("plushie-package.toml");
    let marker_path = dest.join(COMPLETE_MARKER);
    if !manifest_path.is_file() || !marker_path.is_file() {
        return false;
    }
    match std::fs::read_to_string(&manifest_path) {
        Ok(text) if text == MANIFEST_TEXT => {}
        _ => return false,
    }
    let Ok(manifest) = toml::from_str::<Manifest>(MANIFEST_TEXT) else {
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
    let root = app_cache_root(manifest);
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

fn app_cache_root(manifest: &Manifest) -> PathBuf {
    cache_root().join("plushie/apps").join(safe_name(&manifest.app_id))
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
target = "linux-x86_64"
host_sdk = "python"
plushie_rust_version = "0.7.1"
protocol_version = 1

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "{hash}"
size = 7
"#
        );

        let manifest = parse_manifest(&text).unwrap();
        assert_eq!(manifest.app_id, "com.example.notes");
        assert_eq!(manifest.start.command, ["bin/notes"]);
    }

    #[test]
    fn rejects_empty_start_command() {
        let text = r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "python"
plushie_rust_version = "0.7.1"
protocol_version = 1

[start]
working_dir = "."
command = []
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
"#;

        let err = parse_manifest(text).unwrap_err();
        assert!(err.to_string().contains("start.command"));
    }

    #[test]
    fn rejects_invalid_package_target() {
        for target in [
            "",
            "x86_64-unknown-linux-gnu",
            "linux-x64",
            "freebsd-x86_64",
        ] {
            let text = valid_manifest_text("").replace(
                r#"target = "linux-x86_64""#,
                &format!(r#"target = "{target}""#),
            );

            let err = parse_manifest(&text).unwrap_err();
            assert!(err.to_string().contains("target"));
        }
    }

    #[test]
    fn rejects_missing_package_target() {
        let text = valid_manifest_text("").replace(
            r#"target = "linux-x86_64"
"#,
            "",
        );

        let err = parse_manifest(&text).unwrap_err();
        assert!(err.to_string().contains("target"));
    }

    #[test]
    fn preserves_host_argv_arguments_with_spaces() {
        let text = valid_manifest_text("").replace(
            r#"command = ["bin/notes"]"#,
            r#"command = ["bin/notes", "--project", "Daily Notes", "folder/with space/file.txt"]"#,
        );

        let manifest = parse_manifest(&text).unwrap();
        assert_eq!(
            manifest.start.command,
            [
                "bin/notes",
                "--project",
                "Daily Notes",
                "folder/with space/file.txt"
            ]
        );

        let launcher = launcher_main_rs();
        assert!(launcher.contains("for arg in manifest.start.command.iter().skip(1)"));
        assert!(launcher.contains("command.arg(arg);"));
    }

    #[test]
    fn validates_forward_env_names() {
        let valid = valid_manifest_text("").replace(
            r#"forward_env = []"#,
            r#"forward_env = ["PATH", "PLUSHIE_TOKEN"]"#,
        );
        let manifest = parse_manifest(&valid).unwrap();
        assert_eq!(manifest.start.forward_env, ["PATH", "PLUSHIE_TOKEN"]);

        for forward_env in [
            r#"forward_env = [""]"#,
            r#"forward_env = [" "]"#,
            r#"forward_env = ["NAME=VALUE"]"#,
            r#"forward_env = ["ONE,TWO"]"#,
            r#"forward_env = ["PLUSHIE_BINARY_PATH"]"#,
            r#"forward_env = ["PLUSHIE_PACKAGE_DIR"]"#,
        ] {
            let text = valid_manifest_text("").replace(r#"forward_env = []"#, forward_env);

            let err = parse_manifest(&text).unwrap_err();
            assert!(err.to_string().contains("start.forward_env"));
        }
    }

    #[test]
    fn parses_source_package_config() {
        let config = parse_source_config(
            r#"
config_version = 1

[start]
working_dir = "app"
command = ["bin/notes", "--project", "Daily Notes"]
forward_env = ["PATH", "HOME"]
"#,
        )
        .unwrap();

        assert_eq!(config.start.working_dir, "app");
        assert_eq!(
            config.start.command,
            ["bin/notes", "--project", "Daily Notes"]
        );
        assert_eq!(config.start.forward_env, ["PATH", "HOME"]);
    }

    #[test]
    fn renders_source_package_config_template_with_real_values() {
        let text = render_source_config_template(&PackageSourceConfig {
            start: PackageStartConfig {
                working_dir: ".".to_string(),
                command: vec!["bin/notes".to_string()],
                forward_env: default_forward_env(),
            },
        });

        assert!(text.contains("config_version = 1"));
        assert!(text.contains("[start]"));
        assert!(text.contains(r#"working_dir = ".""#));
        assert!(text.contains(r#"command = ["bin/notes"]"#));
        assert!(text.contains(r#""WAYLAND_DISPLAY""#));
    }

    #[test]
    fn rejects_invalid_source_package_config_start_values() {
        for text in [
            r#"
config_version = 2

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = []
"#,
            r#"
config_version = 1

[start]
working_dir = "../app"
command = ["bin/notes"]
forward_env = []
"#,
            r#"
config_version = 1

[start]
working_dir = "."
command = ["/usr/bin/notes"]
forward_env = []
"#,
            r#"
config_version = 1

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = ["PLUSHIE_BINARY_PATH"]
"#,
        ] {
            assert!(parse_source_config(text).is_err());
        }
    }

    #[test]
    fn generated_launcher_forwards_selected_env() {
        let launcher = launcher_main_rs();

        assert!(launcher.contains("for name in &manifest.start.forward_env"));
        assert!(launcher.contains("command.env(name, value);"));
        assert!(launcher.contains(".env(\"PLUSHIE_BINARY_PATH\", &renderer);"));
    }

    #[test]
    fn generated_launcher_starts_host_first() {
        let launcher = launcher_main_rs();

        assert!(launcher.contains("let mut command = Command::new(&host_program);"));
        assert!(launcher.contains(".env(\"PLUSHIE_BINARY_PATH\", &renderer);"));
        assert!(!launcher.contains("arg(\"--listen\")"));
    }

    #[test]
    fn accepts_renderer_provenance_metadata() {
        let text = valid_manifest_text("").replace(
            r#"kind = "stock""#,
            r#"kind = "custom"
source = "local-build""#,
        );

        let manifest = parse_manifest(&text).unwrap();
        let renderer = manifest.renderer;
        assert_eq!(renderer.kind, "custom");
        assert_eq!(renderer.source.as_deref(), Some("local-build"));
    }

    #[test]
    fn rejects_invalid_renderer_provenance_metadata() {
        for renderer_section in [
            r#"
[renderer]
path = "bin/plushie-renderer"
kind = ""
"#,
            r#"
[renderer]
path = "bin/plushie-renderer"
kind = "downloaded"
"#,
            r#"
[renderer]
path = "bin/plushie-renderer"
kind = "stock"
source = " "
"#,
        ] {
            let text = valid_manifest_text("").replace(
                r#"[renderer]
path = "bin/plushie-renderer"
kind = "stock"
"#,
                renderer_section,
            );

            let err = parse_manifest(&text).unwrap_err();
            assert!(err.to_string().contains("renderer."));
        }
    }

    #[test]
    fn accepts_platform_update_and_signing_metadata() {
        let text = valid_manifest_text(
            r#"
[platform]
publisher = "Example Inc."
bundle_id = "com.example.notes"
icon = "assets/icon.png"

[updates]
channel = "stable"
feed_url = "https://example.com/notes/updates.json"

[[signing.hooks]]
stage = "after-launcher-build"
command = ["codesign", "--sign", "Developer ID Application: Example Inc.", "{launcher}"]
"#,
        );

        let manifest = parse_manifest(&text).unwrap();
        let platform = manifest.platform.unwrap();
        let updates = manifest.updates.unwrap();
        let signing = manifest.signing.unwrap();
        assert_eq!(platform.icon.as_deref(), Some("assets/icon.png"));
        assert_eq!(updates.channel, "stable");
        assert_eq!(
            signing.hooks[0].command,
            [
                "codesign",
                "--sign",
                "Developer ID Application: Example Inc.",
                "{launcher}"
            ]
        );
    }

    #[test]
    fn rejects_invalid_platform_metadata() {
        for (metadata, field) in [
            (
                r#"
[platform]
publisher = ""
"#,
                "platform.publisher",
            ),
            (
                r#"
[platform]
bundle_id = " "
"#,
                "platform.bundle_id",
            ),
            (
                r#"
[platform]
icon = "../icon.png"
"#,
                "platform.icon",
            ),
        ] {
            let text = valid_manifest_text(metadata);

            let err = parse_manifest(&text).unwrap_err();
            assert!(
                format!("{err:?}").contains(field),
                "expected `{field}` in `{err}`"
            );
        }
    }

    #[test]
    fn rejects_invalid_update_metadata() {
        for (metadata, field) in [
            (
                r#"
[updates]
channel = ""
"#,
                "updates.channel",
            ),
            (
                r#"
[updates]
channel = "stable"
feed_url = " "
"#,
                "updates.feed_url",
            ),
        ] {
            let text = valid_manifest_text(metadata);

            let err = parse_manifest(&text).unwrap_err();
            assert!(
                format!("{err:?}").contains(field),
                "expected `{field}` in `{err}`"
            );
        }
    }

    #[test]
    fn rejects_invalid_signing_metadata() {
        for metadata in [
            r#"
[[signing.hooks]]
stage = "before-launcher-build"
command = ["codesign"]
"#,
            r#"
[[signing.hooks]]
stage = "after-launcher-build"
command = []
"#,
            r#"
[[signing.hooks]]
stage = "after-launcher-build"
command = ["codesign", ""]
"#,
        ] {
            let text = valid_manifest_text(metadata);

            let err = parse_manifest(&text).unwrap_err();
            assert!(err.to_string().contains("signing hook"));
        }
    }

    #[test]
    fn rejects_unknown_manifest_metadata_fields() {
        for (metadata, field) in [
            (
                r#"
unexpected = true
"#,
                "unexpected",
            ),
            (
                r#"
[platform]
icons = "assets/icon.png"
"#,
                "icons",
            ),
            (
                r#"
[updates]
channel = "stable"
feed = "https://example.com/updates.json"
"#,
                "feed",
            ),
            (
                r#"
[signing]
hook = []
"#,
                "hook",
            ),
            (
                r#"
[[signing.hooks]]
stage = "after-launcher-build"
command = ["codesign"]
shell = true
"#,
                "shell",
            ),
        ] {
            let text = valid_manifest_text(metadata);

            let err = parse_manifest(&text).unwrap_err();
            assert!(
                format!("{err:?}").contains(field),
                "expected `{field}` in `{err:?}`"
            );
        }
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
            target: Some("linux-x86_64".to_string()),
            host_sdk: "python".to_string(),
            host_sdk_version: None,
            plushie_rust_version: "0.7.1".to_string(),
            protocol_version: 1,
            start: StartManifest {
                working_dir: ".".to_string(),
                command: vec!["bin/notes".to_string()],
                forward_env: Vec::new(),
            },
            renderer: RendererManifest {
                path: "bin/plushie-renderer".to_string(),
                kind: "stock".to_string(),
                source: None,
            },
            platform: None,
            updates: None,
            signing: None,
            payload: PayloadManifest {
                archive: "payload.tar.zst".to_string(),
                hash,
                size: Some(payload.len() as u64),
            },
        };

        validate_payload(&manifest, payload).unwrap();
    }

    #[test]
    fn rejects_payload_hash_mismatch() {
        let payload = b"payload";
        let mut manifest = package_manifest_for_payload(payload);
        manifest.payload.hash =
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string();

        let err = validate_payload(&manifest, payload).unwrap_err();
        assert!(err.to_string().contains("payload sha256 mismatch"));
    }

    #[test]
    fn rejects_payload_size_mismatch() {
        let payload = b"payload";
        let mut manifest = package_manifest_for_payload(payload);
        manifest.payload.size = Some(payload.len() as u64 + 1);

        let err = validate_payload(&manifest, payload).unwrap_err();
        assert!(err.to_string().contains("payload size mismatch"));
    }

    #[test]
    fn prepares_generated_launcher_crate() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());

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
        assert_eq!(
            prepared.build_target_dir,
            dir.path().join("target/plushie-package/target")
        );
        assert!(!prepared.lockfile_reused);
    }

    #[test]
    fn expands_signing_hook_launcher_placeholder() {
        let launcher = Path::new("/tmp/plushie launcher");

        assert_eq!(
            expand_signing_hook_arg("sign:{launcher}", launcher),
            format!("sign:{}", launcher.display())
        );
    }

    #[cfg(unix)]
    #[test]
    fn runs_after_launcher_build_signing_hooks_from_manifest_dir() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("hook.txt");
        let launcher = dir.path().join("dist/notes");
        let prepared = PreparedLauncher {
            crate_dir: dir.path().join("crate"),
            build_target_dir: dir.path().join("target"),
            manifest_dir: dir.path().to_path_buf(),
            package_name: "plushie-package-com-example-notes".to_string(),
            output_path: launcher.clone(),
            shared_lockfile: dir.path().join(SHARED_LOCKFILE),
            lockfile_reused: false,
            signing_hooks: vec![SigningHookManifest {
                stage: "after-launcher-build".to_string(),
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "printf '%s\n%s\n' \"$1\" \"$PWD\" > \"$2\"".to_string(),
                    "signing-test".to_string(),
                    "{launcher}".to_string(),
                    marker.display().to_string(),
                ],
            }],
        };

        run_signing_hooks(&prepared).unwrap();

        let output = std::fs::read_to_string(marker).unwrap();
        let mut lines = output.lines();
        let expected_launcher = launcher.display().to_string();
        let expected_manifest_dir = dir.path().display().to_string();
        assert_eq!(lines.next(), Some(expected_launcher.as_str()));
        assert_eq!(lines.next(), Some(expected_manifest_dir.as_str()));
        assert_eq!(lines.next(), None);
    }

    #[cfg(unix)]
    #[test]
    fn reports_failed_signing_hooks() {
        let dir = tempdir().unwrap();
        let prepared = PreparedLauncher {
            crate_dir: dir.path().join("crate"),
            build_target_dir: dir.path().join("target"),
            manifest_dir: dir.path().to_path_buf(),
            package_name: "plushie-package-com-example-notes".to_string(),
            output_path: dir.path().join("dist/notes"),
            shared_lockfile: dir.path().join(SHARED_LOCKFILE),
            lockfile_reused: false,
            signing_hooks: vec![SigningHookManifest {
                stage: "after-launcher-build".to_string(),
                command: vec!["sh".to_string(), "-c".to_string(), "exit 9".to_string()],
            }],
        };

        let err = run_signing_hooks(&prepared).unwrap_err();

        assert!(err.to_string().contains("signing hook"));
        assert!(err.to_string().contains("failed with status"));
    }

    #[test]
    fn package_target_root_uses_cargo_target_dir_value() {
        let dir = tempdir().unwrap();
        let invocation_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        assert_eq!(
            package_target_root_from(
                Some(target_dir.path().to_path_buf()),
                invocation_dir.path(),
                dir.path()
            ),
            target_dir.path()
        );
        assert_eq!(
            package_target_root_from(None, invocation_dir.path(), dir.path()),
            dir.path().join("target")
        );
    }

    #[test]
    fn package_target_root_normalizes_relative_cargo_target_dir() {
        let dir = tempdir().unwrap();
        let invocation_dir = tempdir().unwrap();

        assert_eq!(
            package_target_root_from(
                Some(PathBuf::from("rel-target")),
                invocation_dir.path(),
                dir.path()
            ),
            invocation_dir.path().join("rel-target")
        );
    }

    #[test]
    fn launcher_manifest_uses_stable_crate_name_and_dynamic_binary_name() {
        let manifest = launcher_cargo_toml("plushie-package-com-example-notes");

        assert!(manifest.contains(r#"name = "plushie-package-launcher""#));
        assert!(manifest.contains(r#"name = "plushie-package-com-example-notes""#));
        assert!(manifest.contains(r#"path = "src/main.rs""#));
        assert!(manifest.contains("[workspace]"));
    }

    #[test]
    fn write_bytes_if_changed_preserves_unchanged_files() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("payload.tar.zst");
        write_bytes_if_changed(&path, b"payload").unwrap();
        let first_modified = std::fs::metadata(&path).unwrap().modified().unwrap();

        write_bytes_if_changed(&path, b"payload").unwrap();

        let second_modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(first_modified, second_modified);
    }

    #[test]
    fn prepares_generated_launcher_crate_with_reused_lockfile() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let package_root = dir.path().join("target/plushie-package");
        std::fs::create_dir_all(&package_root).unwrap();
        let lockfile_text = "# locked by previous package build\n";
        std::fs::write(package_root.join(SHARED_LOCKFILE), lockfile_text).unwrap();
        std::fs::write(
            package_root.join(SHARED_LOCKFILE_FINGERPRINT),
            launcher_lockfile_fingerprint() + "\n",
        )
        .unwrap();
        let opts = PackageOpts {
            manifest_path: &manifest,
            out_path: None,
            release: false,
            verbose: false,
        };

        let prepared = prepare_launcher_crate(&opts).unwrap();

        assert!(prepared.lockfile_reused);
        assert_eq!(
            std::fs::read_to_string(prepared.crate_dir.join(GENERATED_LOCKFILE)).unwrap(),
            lockfile_text
        );
    }

    #[test]
    fn prepares_generated_launcher_crate_discards_stale_lockfile() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let crate_dir = dir
            .path()
            .join("target/plushie-package/plushie-package-com-example-notes");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(crate_dir.join(GENERATED_LOCKFILE), "# stale\n").unwrap();
        let package_root = dir.path().join("target/plushie-package");
        std::fs::write(package_root.join(SHARED_LOCKFILE), "# stale shared\n").unwrap();
        std::fs::write(package_root.join(SHARED_LOCKFILE_FINGERPRINT), "stale\n").unwrap();
        let opts = PackageOpts {
            manifest_path: &manifest,
            out_path: None,
            release: false,
            verbose: false,
        };

        let prepared = prepare_launcher_crate(&opts).unwrap();

        assert!(!prepared.lockfile_reused);
        assert!(!prepared.crate_dir.join(GENERATED_LOCKFILE).exists());
    }

    #[test]
    fn rejects_global_host_program_paths() {
        let hash = format!("sha256:{:x}", Sha256::digest(b"payload"));
        let text = format!(
            r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "python"
plushie_rust_version = "{}"
protocol_version = {}

[start]
working_dir = "."
command = ["/usr/bin/python"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "{hash}"
"#,
            EXPECTED_PLUSHIE_RUST_VERSION, EXPECTED_PROTOCOL_VERSION
        );

        let err = parse_manifest(&text).unwrap_err();
        assert!(err.to_string().contains("start.command[0]"));
    }

    #[test]
    fn rejects_manifest_paths_that_escape_roots() {
        for (field, value) in [
            ("renderer.path", "/tmp/plushie-renderer"),
            ("renderer.path", "../bin/plushie-renderer"),
            ("start.working_dir", "/tmp/app"),
            ("start.working_dir", "../app"),
            ("payload.archive", "/tmp/payload.tar.zst"),
            ("payload.archive", "../payload.tar.zst"),
        ] {
            let text = manifest_with_path(field, value);
            let err = parse_manifest(&text).unwrap_err();
            assert!(
                err.to_string().contains(field),
                "expected `{field}` in `{err}`"
            );
        }
    }

    #[test]
    fn rejects_archive_paths_that_escape_payload_root() {
        let payload = malicious_payload_archive();
        let manifest = package_manifest_for_payload(&payload);
        let err = validate_payload_archive(&manifest, &payload).unwrap_err();
        assert!(err.to_string().contains("parent traversal"));
    }

    #[test]
    fn rejects_payload_missing_manifest_renderer() {
        let payload = payload_archive_with_entries(&[("bin/notes", b"host".as_slice())]);
        let manifest = package_manifest_for_payload(&payload);

        let err = validate_payload_archive(&manifest, &payload).unwrap_err();
        assert!(err.to_string().contains("renderer.path"));
    }

    #[test]
    fn rejects_payload_missing_manifest_host_program() {
        let payload =
            payload_archive_with_entries(&[("bin/plushie-renderer", b"renderer".as_slice())]);
        let manifest = package_manifest_for_payload(&payload);

        let err = validate_payload_archive(&manifest, &payload).unwrap_err();
        assert!(err.to_string().contains("start.command[0]"));
    }

    #[test]
    fn accepts_non_root_payload_working_dir() {
        let payload = payload_archive_with_dirs(
            &[
                ("bin/plushie-renderer", b"renderer".as_slice()),
                ("app/bin/notes", b"host".as_slice()),
            ],
            &["app"],
        );
        let mut manifest = package_manifest_for_payload(&payload);
        manifest.start.command = vec!["app/bin/notes".to_string()];
        manifest.start.working_dir = "app".to_string();

        validate_payload_archive(&manifest, &payload).unwrap();
    }

    #[test]
    fn rejects_payload_missing_non_root_working_dir() {
        let payload = payload_archive_with_entries(&[
            ("bin/plushie-renderer", b"renderer".as_slice()),
            ("app/bin/notes", b"host".as_slice()),
        ]);
        let mut manifest = package_manifest_for_payload(&payload);
        manifest.start.command = vec!["app/bin/notes".to_string()];
        manifest.start.working_dir = "app".to_string();

        let err = validate_payload_archive(&manifest, &payload).unwrap_err();
        assert!(err.to_string().contains("start.working_dir"));
    }

    #[test]
    fn accepts_payload_with_platform_icon() {
        let payload = payload_archive_with_entries(&[
            ("bin/plushie-renderer", b"renderer".as_slice()),
            ("bin/notes", b"host".as_slice()),
            ("assets/icon.png", b"icon".as_slice()),
        ]);
        let mut manifest = package_manifest_for_payload(&payload);
        manifest.platform = Some(PlatformManifest {
            publisher: None,
            bundle_id: None,
            icon: Some("assets/icon.png".to_string()),
        });

        validate_payload_archive(&manifest, &payload).unwrap();
    }

    #[test]
    fn warns_when_platform_icon_is_missing() {
        let payload = sample_payload_archive();
        let manifest = package_manifest_for_payload(&payload);

        assert_eq!(
            package_warnings(&manifest),
            vec![PackageWarning::MissingPlatformIcon]
        );
    }

    #[test]
    fn does_not_warn_when_platform_icon_is_present() {
        let payload = sample_payload_archive();
        let mut manifest = package_manifest_for_payload(&payload);
        manifest.platform = Some(PlatformManifest {
            publisher: None,
            bundle_id: None,
            icon: Some("assets/icon.png".to_string()),
        });

        assert_eq!(package_warnings(&manifest), Vec::new());
    }

    #[test]
    fn rejects_payload_missing_platform_icon() {
        let payload = sample_payload_archive();
        let mut manifest = package_manifest_for_payload(&payload);
        manifest.platform = Some(PlatformManifest {
            publisher: None,
            bundle_id: None,
            icon: Some("assets/icon.png".to_string()),
        });

        let err = validate_payload_archive(&manifest, &payload).unwrap_err();
        assert!(err.to_string().contains("platform.icon"));
    }

    #[test]
    fn rejects_path_control_app_ids() {
        let hash = format!("{:x}", Sha256::digest(b"payload"));
        let text = format!(
            r#"
schema_version = 1
app_id = ".."
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "python"
plushie_rust_version = "{}"
protocol_version = {}

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "sha256:{hash}"
"#,
            EXPECTED_PLUSHIE_RUST_VERSION, EXPECTED_PROTOCOL_VERSION
        );

        let err = parse_manifest(&text).unwrap_err();
        assert!(err.to_string().contains("app_id"));
    }

    #[test]
    fn generated_launcher_reports_and_checks_cache_metadata() {
        let launcher = launcher_main_rs();

        for expected in [
            "cache_status={}",
            "if payload_root.reused { \"reused\" } else { \"extracted\" }",
            "fn cache_entry_is_complete(dest: &Path) -> bool",
            "Ok(text) if text == MANIFEST_TEXT",
            "app_id={}",
            "app_version={}",
            "payload_hash={}",
            "renderer.path={}",
            "start.command={}",
        ] {
            assert!(
                launcher.contains(expected),
                "missing generated launcher text `{expected}`"
            );
        }
    }

    fn sample_payload_archive() -> Vec<u8> {
        payload_archive_with_dirs(
            &[
                ("bin/plushie-renderer", b"renderer".as_slice()),
                ("bin/notes", b"host".as_slice()),
            ],
            &[],
        )
    }

    fn write_sample_package(dir: &Path) -> PathBuf {
        let payload = sample_payload_archive();
        let archive = dir.join("payload.tar.zst");
        std::fs::write(&archive, &payload).unwrap();
        let hash = format!("sha256:{:x}", Sha256::digest(&payload));
        let manifest = dir.join("plushie-package.toml");
        std::fs::write(
            &manifest,
            format!(
                r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "python"
plushie_rust_version = "{EXPECTED_PLUSHIE_RUST_VERSION}"
protocol_version = {EXPECTED_PROTOCOL_VERSION}

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "{hash}"
"#
            ),
        )
        .unwrap();
        manifest
    }

    fn valid_manifest_text(extra: &str) -> String {
        let payload_hash = format!("sha256:{:x}", Sha256::digest(b"payload"));
        format!(
            r#"
schema_version = {MANIFEST_SCHEMA_VERSION}
app_id = "com.example.notes"
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "python"
plushie_rust_version = "{EXPECTED_PLUSHIE_RUST_VERSION}"
protocol_version = {EXPECTED_PROTOCOL_VERSION}

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"
{extra}

[payload]
archive = "payload.tar.zst"
hash = "{payload_hash}"
"#
        )
    }

    fn manifest_with_path(field: &str, value: &str) -> String {
        match field {
            "renderer.path" => valid_manifest_text("").replace(
                r#"path = "bin/plushie-renderer""#,
                &format!(r#"path = "{value}""#),
            ),
            "start.working_dir" => valid_manifest_text("").replace(
                r#"working_dir = ".""#,
                &format!(r#"working_dir = "{value}""#),
            ),
            "payload.archive" => {
                let payload_hash = format!("sha256:{:x}", Sha256::digest(b"payload"));
                format!(
                    r#"
schema_version = {MANIFEST_SCHEMA_VERSION}
app_id = "com.example.notes"
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "python"
plushie_rust_version = "{EXPECTED_PLUSHIE_RUST_VERSION}"
protocol_version = {EXPECTED_PROTOCOL_VERSION}

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "{value}"
hash = "{payload_hash}"
"#
                )
            }
            _ => unreachable!("unknown path field"),
        }
    }

    fn package_manifest_for_payload(payload: &[u8]) -> PackageManifest {
        PackageManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            app_id: "com.example.notes".to_string(),
            app_name: None,
            app_version: "0.1.0".to_string(),
            target: Some("linux-x86_64".to_string()),
            host_sdk: "python".to_string(),
            host_sdk_version: None,
            plushie_rust_version: EXPECTED_PLUSHIE_RUST_VERSION.to_string(),
            protocol_version: EXPECTED_PROTOCOL_VERSION,
            start: StartManifest {
                working_dir: ".".to_string(),
                command: vec!["bin/notes".to_string()],
                forward_env: Vec::new(),
            },
            renderer: RendererManifest {
                path: "bin/plushie-renderer".to_string(),
                kind: "stock".to_string(),
                source: None,
            },
            platform: None,
            updates: None,
            signing: None,
            payload: PayloadManifest {
                archive: "payload.tar.zst".to_string(),
                hash: format!("sha256:{:x}", Sha256::digest(payload)),
                size: Some(payload.len() as u64),
            },
        }
    }

    fn payload_archive_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
        payload_archive_with_dirs(entries, &[])
    }

    fn payload_archive_with_dirs(entries: &[(&str, &[u8])], dirs: &[&str]) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for path in dirs {
                append_dir(&mut builder, path);
            }
            for (path, bytes) in entries {
                append_file(&mut builder, path, bytes);
            }
            builder.finish().unwrap();
        }
        zstd::stream::encode_all(tar_bytes.as_slice(), 0).unwrap()
    }

    fn malicious_payload_archive() -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        append_raw_tar_entry(&mut tar_bytes, "../escape", b"bad", b'0');
        tar_bytes.extend_from_slice(&[0; 1024]);
        zstd::stream::encode_all(tar_bytes.as_slice(), 0).unwrap()
    }

    fn append_file(builder: &mut tar::Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append_data(&mut header, path, bytes).unwrap();
    }

    fn append_dir(builder: &mut tar::Builder<&mut Vec<u8>>, path: &str) {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append_data(&mut header, path, &[][..]).unwrap();
    }

    fn append_raw_tar_entry(out: &mut Vec<u8>, path: &str, bytes: &[u8], entry_type: u8) {
        let mut header = [0u8; 512];
        header[..path.len()].copy_from_slice(path.as_bytes());
        write_octal(&mut header[100..108], 0o755);
        write_octal(&mut header[108..116], 0);
        write_octal(&mut header[116..124], 0);
        write_octal(&mut header[124..136], bytes.len() as u64);
        write_octal(&mut header[136..148], 0);
        header[148..156].fill(b' ');
        header[156] = entry_type;
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        let checksum: u32 = header.iter().map(|byte| u32::from(*byte)).sum();
        write_octal(&mut header[148..156], u64::from(checksum));
        out.extend_from_slice(&header);
        out.extend_from_slice(bytes);
        let padding = (512 - (bytes.len() % 512)) % 512;
        out.extend(std::iter::repeat_n(0, padding));
    }

    fn write_octal(field: &mut [u8], value: u64) {
        field.fill(0);
        let text = format!("{value:0width$o}", width = field.len() - 1);
        field[..text.len()].copy_from_slice(text.as_bytes());
    }
}
