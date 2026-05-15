//! Standalone package command support.
//!
//! The SDKs own host-language packaging. This module owns the shared
//! Plushie wrapper step: validate a package manifest, embed its payload
//! archive in the reusable launcher, and write the portable artifact.

use crate::{Error, Result, package_runtime, platform, tool_identity};
use anyhow::Context;
use cargo_packager::{
    Config as CargoPackagerConfig, PackageFormat, config::Binary as CargoPackagerBinary,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

/// Conventional developer-owned source package config filename.
pub const SOURCE_CONFIG: &str = "plushie-package.config.toml";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const EXPECTED_PLUSHIE_RUST_VERSION: &str = env!("CARGO_PKG_VERSION");
const EXPECTED_PROTOCOL_VERSION: u32 = plushie_core::protocol::PROTOCOL_VERSION;
const SOURCE_CONFIG_VERSION: u32 = 1;
const PACKAGE_READY_FILE_ENV: &str = "PLUSHIE_PACKAGE_READY_FILE";

/// Options for building a standalone launcher from a package manifest.
#[derive(Debug)]
pub struct PackageOpts<'a> {
    /// Path to the Plushie package manifest.
    pub manifest_path: &'a Path,
    /// Optional final launcher output path.
    pub out_path: Option<&'a Path>,
    /// Optional reusable launcher binary to embed package data into.
    pub launcher_path: Option<&'a Path>,
    /// Run signing hooks declared in the package manifest.
    pub run_signing_hooks: bool,
    /// Print the launcher template resolution.
    pub verbose: bool,
}

/// Options for postchecking a portable launcher.
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
    /// Final launcher executable path.
    pub binary_path: PathBuf,
    /// Reusable launcher binary used as the artifact runtime.
    pub launcher_template_path: PathBuf,
}

/// Result of running the portable launcher's postcheck path.
#[derive(Debug)]
pub struct PackagePostcheckResult {
    /// Final launcher executable path.
    pub binary_path: PathBuf,
    /// Isolated cache directory used by the postcheck run.
    pub cache_dir: PathBuf,
    /// Captured launcher stderr.
    pub stderr: String,
}

/// Options for creating a platform bundle through cargo-packager.
#[derive(Debug)]
pub struct PackageBundleOpts<'a> {
    /// Path to the Plushie package manifest.
    pub manifest_path: &'a Path,
    /// Optional already-built portable executable.
    pub portable_path: Option<&'a Path>,
    /// Optional final bundle output directory.
    pub out_dir: Option<&'a Path>,
    /// Package formats passed to cargo-packager.
    pub formats: &'a [String],
    /// Optional cargo-packager config path. When absent, Plushie writes one.
    pub config_path: Option<&'a Path>,
    /// Portable launcher options used when `portable_path` is absent.
    pub package: PackageOpts<'a>,
}

/// Result of invoking cargo-packager for a platform bundle.
#[derive(Debug)]
pub struct PackageBundleResult {
    /// Portable executable consumed by cargo-packager.
    pub portable_path: PathBuf,
    /// Source cargo-packager config path. Generated when the user does not
    /// supply one.
    pub config_path: PathBuf,
    /// Fully merged cargo-packager config written for debugging.
    pub effective_config_path: PathBuf,
    /// Directory where cargo-packager writes outputs.
    pub out_dir: PathBuf,
    /// Platform package artifacts written by cargo-packager.
    pub outputs: Vec<PathBuf>,
}

/// Result of prechecking a standalone package manifest and payload.
#[derive(Debug)]
pub struct PackagePrecheckResult {
    /// Package application ID.
    pub app_id: String,
    /// Package application version.
    pub app_version: String,
    /// plushie-rust version recorded in the package manifest.
    pub plushie_rust_version: String,
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
                 --out <payload>/assets` during assembly."
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
    phase: String,
    command: Vec<String>,
}

struct PreparedPortableLauncher {
    manifest_dir: PathBuf,
    launcher_template_path: PathBuf,
    output_path: PathBuf,
    signing_hooks: Vec<SigningHookManifest>,
    manifest_text: String,
    payload: Vec<u8>,
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
        plushie_rust_version: loaded.manifest.plushie_rust_version,
        payload_hash: loaded.manifest.payload.hash,
        warnings,
    })
}

/// Create a platform bundle by delegating to cargo-packager.
///
/// # Errors
///
/// Returns an error when the package manifest is invalid, the portable
/// executable cannot be prepared, or cargo-packager cannot create the
/// requested artifacts.
pub fn bundle_package(opts: &PackageBundleOpts<'_>) -> Result<PackageBundleResult> {
    let loaded = load_package(opts.manifest_path)?;
    let portable_path = match opts.portable_path {
        Some(path) => absolute_from_invocation(path.to_path_buf())?,
        None => build_launcher(&opts.package)?.binary_path,
    };
    if !portable_path.is_file() {
        return Err(Error::Other(anyhow::anyhow!(
            "portable executable `{}` is not a file",
            portable_path.display()
        )));
    }

    let target_root = package_target_root(&loaded.manifest_dir);
    let work_dir = target_root
        .join("plushie/packager")
        .join(app_cache_name(&loaded.manifest.app_id));
    let out_dir =
        absolute_from_invocation(opts.out_dir.map(Path::to_path_buf).unwrap_or_else(|| {
            target_root
                .join("plushie/bundles")
                .join(app_cache_name(&loaded.manifest.app_id))
        }))?;

    let prepared = prepare_packager_config(
        &loaded,
        &portable_path,
        &work_dir,
        &out_dir,
        opts.formats,
        opts.config_path,
        opts.out_dir.is_some(),
    )?;
    let outputs = with_current_dir(&prepared.working_dir, || {
        cargo_packager::package(&prepared.config).map_err(|err| Error::Other(anyhow::anyhow!(err)))
    })
    .with_context(|| "cargo-packager failed")?
    .into_iter()
    .flat_map(|output| output.paths)
    .collect();

    Ok(PackageBundleResult {
        portable_path,
        config_path: prepared.config_path,
        effective_config_path: prepared.effective_config_path,
        out_dir: prepared.out_dir,
        outputs,
    })
}

#[derive(Debug)]
struct PreparedPackagerConfig {
    config: CargoPackagerConfig,
    config_path: PathBuf,
    effective_config_path: PathBuf,
    out_dir: PathBuf,
    working_dir: PathBuf,
}

fn prepare_packager_config(
    loaded: &LoadedPackage,
    portable_path: &Path,
    work_dir: &Path,
    out_dir: &Path,
    formats: &[String],
    config_path: Option<&Path>,
    out_dir_explicit: bool,
) -> Result<PreparedPackagerConfig> {
    prepare_packager_input(loaded, portable_path, work_dir, out_dir)?;
    let bin_dir = work_dir.join("bin");
    let binary_stem = safe_name(&loaded.manifest.app_id);
    let icon = materialize_packager_icon(loaded, work_dir)?;
    let parsed_formats = parse_packager_formats(formats)?;

    if let Some(config_path) = config_path {
        let config_path = absolute_from_invocation(config_path.to_path_buf())?;
        let mut config = read_packager_config(&config_path)?;
        apply_packager_defaults(
            &mut config,
            loaded,
            &binary_stem,
            &bin_dir,
            out_dir,
            icon,
            parsed_formats,
            !formats.is_empty(),
            out_dir_explicit,
        );
        validate_packager_config(&config)?;
        let effective_config_path = work_dir.join("Packager.effective.toml");
        write_packager_config(&effective_config_path, &config)?;
        let working_dir = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let resolved_out_dir = resolve_packager_out_dir(&config, &working_dir);
        Ok(PreparedPackagerConfig {
            config,
            config_path,
            effective_config_path,
            out_dir: resolved_out_dir,
            working_dir,
        })
    } else {
        let mut config = CargoPackagerConfig::default();
        apply_packager_defaults(
            &mut config,
            loaded,
            &binary_stem,
            &bin_dir,
            out_dir,
            icon,
            parsed_formats,
            false,
            true,
        );
        validate_packager_config(&config)?;
        let config_path = work_dir.join("Packager.toml");
        write_packager_config(&config_path, &config)?;
        let resolved_out_dir = resolve_packager_out_dir(&config, work_dir);
        Ok(PreparedPackagerConfig {
            config,
            config_path: config_path.clone(),
            effective_config_path: config_path,
            out_dir: resolved_out_dir,
            working_dir: work_dir.to_path_buf(),
        })
    }
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

/// Build the portable launcher and copy it to the requested output.
///
/// # Errors
///
/// Returns an error when manifest validation fails, the reusable
/// launcher cannot be found, or the final binary cannot be written.
pub fn build_launcher(opts: &PackageOpts<'_>) -> Result<PackageResult> {
    let prepared = prepare_portable_launcher(opts)?;
    validate_launcher_template_identity(&prepared.launcher_template_path)?;
    if opts.verbose {
        eprintln!(
            "plushie: embedding package data into launcher template {}",
            prepared.launcher_template_path.display()
        );
    }
    if let Some(parent) = prepared.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut output = std::fs::File::create(&prepared.output_path)
        .with_context(|| format!("create launcher `{}`", prepared.output_path.display()))?;
    let mut template =
        std::fs::File::open(&prepared.launcher_template_path).with_context(|| {
            format!(
                "open launcher template `{}`",
                prepared.launcher_template_path.display()
            )
        })?;
    std::io::copy(&mut template, &mut output).with_context(|| {
        format!(
            "copy launcher template `{}` to `{}`",
            prepared.launcher_template_path.display(),
            prepared.output_path.display()
        )
    })?;
    package_runtime::append_embedded_package(
        &mut output,
        &prepared.manifest_text,
        &prepared.payload,
    )?;
    output.flush()?;
    make_executable(&prepared.output_path)?;
    if opts.run_signing_hooks {
        run_signing_hooks(
            &prepared.manifest_dir,
            &prepared.output_path,
            &prepared.signing_hooks,
        )?;
    }

    Ok(PackageResult {
        binary_path: prepared.output_path,
        launcher_template_path: prepared.launcher_template_path,
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
        .arg("--postcheck")
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

fn prepare_portable_launcher(opts: &PackageOpts<'_>) -> Result<PreparedPortableLauncher> {
    let loaded = load_package(opts.manifest_path)?;

    let target_root = package_target_root(&loaded.manifest_dir);
    let output_path =
        absolute_from_invocation(opts.out_path.map(Path::to_path_buf).unwrap_or_else(|| {
            target_root
                .join("plushie/package")
                .join(executable_name(&app_cache_name(&loaded.manifest.app_id)))
        }))?;

    let launcher_template_path = resolve_launcher_template(opts.launcher_path)?;
    validate_distinct_launcher_output(&launcher_template_path, &output_path)?;

    Ok(PreparedPortableLauncher {
        manifest_dir: loaded.manifest_dir,
        launcher_template_path,
        output_path,
        signing_hooks: loaded
            .manifest
            .signing
            .map(|signing| signing.hooks)
            .unwrap_or_default(),
        manifest_text: loaded.manifest_text,
        payload: loaded.payload,
    })
}

fn prepare_packager_input(
    loaded: &LoadedPackage,
    portable_path: &Path,
    work_dir: &Path,
    out_dir: &Path,
) -> Result<()> {
    if work_dir.exists() {
        std::fs::remove_dir_all(work_dir)
            .with_context(|| format!("remove packager work directory `{}`", work_dir.display()))?;
    }
    let bin_dir = work_dir.join("bin");
    std::fs::create_dir_all(&bin_dir)
        .with_context(|| format!("create packager bin directory `{}`", bin_dir.display()))?;
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("create package bundle output `{}`", out_dir.display()))?;

    let binary_stem = safe_name(&loaded.manifest.app_id);
    let binary_name = executable_name(&binary_stem);
    let binary_path = bin_dir.join(&binary_name);
    std::fs::copy(portable_path, &binary_path).with_context(|| {
        format!(
            "copy portable executable `{}` to packager input `{}`",
            portable_path.display(),
            binary_path.display()
        )
    })?;
    make_executable(&binary_path)?;
    Ok(())
}

fn read_packager_config(path: &Path) -> Result<CargoPackagerConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read cargo-packager config `{}`", path.display()))?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => serde_json::from_str(&text)
            .with_context(|| format!("parse cargo-packager config `{}`", path.display())),
        _ => toml::from_str(&text)
            .with_context(|| format!("parse cargo-packager config `{}`", path.display())),
    }
    .map_err(Error::Other)
}

fn write_packager_config(path: &Path, config: &CargoPackagerConfig) -> Result<()> {
    let config_text =
        toml::to_string_pretty(config).with_context(|| "serialize cargo-packager configuration")?;
    std::fs::write(path, config_text)
        .with_context(|| format!("write cargo-packager config `{}`", path.display()))?;
    Ok(())
}

fn validate_packager_config(config: &CargoPackagerConfig) -> Result<()> {
    let formats = resolved_packager_formats(config);
    validate_packager_formats(&formats)?;
    validate_bundle_environment(&formats)?;
    Ok(())
}

fn apply_packager_defaults(
    config: &mut CargoPackagerConfig,
    loaded: &LoadedPackage,
    binary_stem: &str,
    bin_dir: &Path,
    out_dir: &Path,
    icon: Option<PathBuf>,
    formats: Option<Vec<PackageFormat>>,
    formats_explicit: bool,
    out_dir_explicit: bool,
) {
    if config.product_name.is_empty() {
        config.product_name = loaded
            .manifest
            .app_name
            .clone()
            .unwrap_or_else(|| loaded.manifest.app_id.clone());
    }
    if config.version.is_empty() {
        config.version = loaded.manifest.app_version.clone();
    }
    if config.binaries.is_empty() {
        config.binaries = vec![CargoPackagerBinary::new(binary_stem).main(true)];
    }
    if config.identifier.is_none() {
        config.identifier = loaded
            .manifest
            .platform
            .as_ref()
            .and_then(|platform| platform.bundle_id.clone())
            .or_else(|| Some(loaded.manifest.app_id.clone()));
    }
    if config.publisher.is_none() {
        config.publisher = loaded
            .manifest
            .platform
            .as_ref()
            .and_then(|platform| platform.publisher.clone());
    }
    if out_dir_explicit || config.out_dir.as_os_str().is_empty() {
        config.out_dir = out_dir.to_path_buf();
    }
    if config.binaries_dir.is_none() {
        config.binaries_dir = Some(bin_dir.to_path_buf());
    }
    if config.icons.is_none() {
        config.icons = icon.map(|icon| vec![icon.to_string_lossy().into_owned()]);
    }
    if formats_explicit || config.formats.is_none() {
        config.formats = formats;
    }
}

fn resolve_packager_out_dir(config: &CargoPackagerConfig, working_dir: &Path) -> PathBuf {
    if config.out_dir.as_os_str().is_empty() {
        working_dir.to_path_buf()
    } else if config.out_dir.is_absolute() {
        config.out_dir.clone()
    } else {
        working_dir.join(&config.out_dir)
    }
}

fn parse_packager_formats(formats: &[String]) -> Result<Option<Vec<PackageFormat>>> {
    if formats.is_empty() {
        return Ok(None);
    }

    let mut parsed = Vec::new();
    for format in formats {
        match format.as_str() {
            "default" => parsed.extend_from_slice(PackageFormat::platform_default()),
            "all" => parsed.extend_from_slice(PackageFormat::platform_all()),
            "pacman" => parsed.push(PackageFormat::Pacman),
            other => parsed.push(PackageFormat::from_short_name(other).ok_or_else(|| {
                Error::Other(anyhow::anyhow!("unsupported package format `{other}`"))
            })?),
        }
    }
    parsed.sort_by_key(|format| format.short_name());
    parsed.dedup();
    Ok(Some(parsed))
}

fn validate_packager_formats(formats: &[PackageFormat]) -> Result<()> {
    let unsupported: Vec<&str> = formats
        .iter()
        .filter(|format| !is_supported_packager_format(format))
        .map(PackageFormat::short_name)
        .collect();
    if unsupported.is_empty() {
        return Ok(());
    }
    Err(Error::Other(anyhow::anyhow!(
        "package formats not supported on this platform ({}): {}",
        std::env::consts::OS,
        unsupported.join(", ")
    )))
}

fn validate_bundle_environment(formats: &[PackageFormat]) -> Result<()> {
    if formats.contains(&PackageFormat::AppImage) {
        ensure_commands_available(&[
            "desktop-file-validate",
            "file",
            "mksquashfs",
            "objdump",
            "patchelf",
            "strip",
        ])?;
    }
    Ok(())
}

fn resolved_packager_formats(config: &CargoPackagerConfig) -> Vec<PackageFormat> {
    config
        .formats
        .clone()
        .unwrap_or_else(|| PackageFormat::platform_default().to_vec())
}

fn ensure_commands_available(commands: &[&str]) -> Result<()> {
    let missing: Vec<&str> = commands
        .iter()
        .copied()
        .filter(|command| !command_available(command))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(Error::Other(anyhow::anyhow!(
        "missing system commands required for packaging: {}",
        missing.join(", ")
    )))
}

fn command_available(command: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(command);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

fn is_supported_packager_format(format: &PackageFormat) -> bool {
    #[cfg(target_os = "linux")]
    {
        matches!(
            format,
            PackageFormat::AppImage | PackageFormat::Deb | PackageFormat::Pacman
        )
    }
    #[cfg(target_os = "macos")]
    {
        matches!(format, PackageFormat::App | PackageFormat::Dmg)
    }
    #[cfg(target_os = "windows")]
    {
        matches!(format, PackageFormat::Nsis | PackageFormat::Wix)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = format;
        false
    }
}

fn materialize_packager_icon(loaded: &LoadedPackage, work_dir: &Path) -> Result<Option<PathBuf>> {
    let Some(icon) = loaded
        .manifest
        .platform
        .as_ref()
        .and_then(|platform| platform.icon.as_deref())
    else {
        return Ok(None);
    };
    let icon_path = clean_payload_relative_path("platform.icon", icon)?;
    let icons_dir = work_dir.join("icons");
    std::fs::create_dir_all(&icons_dir)
        .with_context(|| format!("create packager icons directory `{}`", icons_dir.display()))?;
    let icon_name = icon_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(safe_name)
        .unwrap_or_else(|| "app-icon.png".to_string());
    let output_path = icons_dir.join(icon_name);

    let decoder = zstd::stream::read::Decoder::new(std::io::Cursor::new(&loaded.payload[..]))
        .with_context(|| "failed to open payload archive as zstd")?;
    let mut archive = tar::Archive::new(decoder);
    for entry in archive
        .entries()
        .with_context(|| "failed to read payload archive entries")?
    {
        let mut entry = entry.with_context(|| "failed to read payload archive entry")?;
        let path = entry
            .path()
            .with_context(|| "failed to read payload archive entry path")?;
        let entry_path =
            clean_payload_relative_path("payload archive entry", &path.to_string_lossy())?;
        if entry_path == icon_path {
            let mut output = std::fs::File::create(&output_path)
                .with_context(|| format!("create packager icon `{}`", output_path.display()))?;
            std::io::copy(&mut entry, &mut output)
                .with_context(|| format!("write packager icon `{}`", output_path.display()))?;
            return Ok(Some(output_path));
        }
    }

    Err(Error::Other(anyhow::anyhow!(
        "payload archive does not contain platform.icon `{icon}`"
    )))
}

fn with_current_dir<T>(dir: &Path, f: impl FnOnce() -> Result<T>) -> Result<T> {
    struct CurrentDirGuard(PathBuf);

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    let previous = std::env::current_dir().with_context(|| "resolve current directory")?;
    std::env::set_current_dir(dir)
        .with_context(|| format!("enter cargo-packager directory `{}`", dir.display()))?;
    let _guard = CurrentDirGuard(previous);
    f()
}

fn resolve_launcher_template(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return canonical_launcher_template(path);
    }
    if let Some(path) = std::env::var_os("PLUSHIE_LAUNCHER_PATH") {
        return canonical_launcher_template(&PathBuf::from(path));
    }

    let name = platform::launcher_name();
    let mut candidates = vec![std::env::current_dir()?.join("bin").join(&name)];
    if let Ok(current_exe) = std::env::current_exe()
        && let Some(parent) = current_exe.parent()
    {
        candidates.push(parent.join(&name));
    }
    for candidate in candidates {
        if candidate.is_file() {
            return canonical_launcher_template(&candidate);
        }
    }

    Err(Error::Other(anyhow::anyhow!(
        "could not find reusable `{}`. Run `bin/plushie tools sync --required-version {}` or pass --launcher PATH.",
        name,
        EXPECTED_PLUSHIE_RUST_VERSION
    )))
}

fn canonical_launcher_template(path: &Path) -> Result<PathBuf> {
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("launcher template `{}` not found", path.display()))?;
    if !path.is_file() {
        return Err(Error::Other(anyhow::anyhow!(
            "launcher template `{}` is not a file",
            path.display()
        )));
    }
    Ok(path)
}

fn validate_launcher_template_identity(path: &Path) -> Result<()> {
    if package_runtime::has_embedded_package(path)? {
        return Err(Error::Other(anyhow::anyhow!(
            "launcher template `{}` already contains an embedded Plushie package; use a pristine `plushie-launcher` from `bin/plushie tools sync --required-version {}` or pass --launcher PATH",
            path.display(),
            EXPECTED_PLUSHIE_RUST_VERSION
        )));
    }

    let identity = tool_identity::probe_tool_identity(path, Duration::from_secs(2))
        .with_context(|| format!("probe launcher template `{}`", path.display()))?;
    if identity.tool != "plushie-launcher" {
        return Err(Error::Other(anyhow::anyhow!(
            "launcher template `{}` identifies as {}, expected plushie-launcher",
            path.display(),
            identity.tool
        )));
    }
    if identity.plushie_rust_version != EXPECTED_PLUSHIE_RUST_VERSION {
        return Err(Error::Other(anyhow::anyhow!(
            "launcher template `{}` is version {} but package tool is {}; run `bin/plushie tools sync --required-version {}`",
            path.display(),
            identity.plushie_rust_version,
            EXPECTED_PLUSHIE_RUST_VERSION,
            EXPECTED_PLUSHIE_RUST_VERSION
        )));
    }

    let current = tool_identity::current_tool_identity("plushie");
    if identity.target != current.target {
        return Err(Error::Other(anyhow::anyhow!(
            "launcher template `{}` target {} does not match package tool target {}; run `bin/plushie tools sync --required-version {}`",
            path.display(),
            identity.target,
            current.target,
            EXPECTED_PLUSHIE_RUST_VERSION
        )));
    }

    Ok(())
}

fn validate_distinct_launcher_output(template_path: &Path, output_path: &Path) -> Result<()> {
    let same_path = template_path == output_path
        || std::fs::canonicalize(output_path)
            .map(|existing| existing == template_path)
            .unwrap_or(false);
    if same_path {
        return Err(Error::Other(anyhow::anyhow!(
            "portable launcher output must not overwrite reusable launcher template `{}`",
            template_path.display()
        )));
    }
    Ok(())
}

fn absolute_from_invocation(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn run_signing_hooks(
    manifest_dir: &Path,
    output_path: &Path,
    signing_hooks: &[SigningHookManifest],
) -> Result<()> {
    for hook in signing_hooks {
        let argv: Vec<String> = hook
            .command
            .iter()
            .map(|arg| expand_signing_hook_arg(arg, output_path))
            .collect();
        let program = argv.first().expect("validated signing hook argv");
        let status = std::process::Command::new(program)
            .args(&argv[1..])
            .current_dir(manifest_dir)
            .status()
            .with_context(|| {
                format!(
                    "failed to run signing hook `{}` for phase `{}`",
                    program, hook.phase
                )
            })?;
        if !status.success() {
            return Err(Error::Other(anyhow::anyhow!(
                "signing hook `{}` for phase `{}` failed with status {}",
                program,
                hook.phase,
                status
            )));
        }
    }
    Ok(())
}

fn expand_signing_hook_arg(arg: &str, launcher_path: &Path) -> String {
    arg.replace("{launcher}", &launcher_path.display().to_string())
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
    if start.forward_env.iter().any(|name| {
        name == "PLUSHIE_BINARY_PATH"
            || name == "PLUSHIE_PACKAGE_DIR"
            || name == PACKAGE_READY_FILE_ENV
    }) {
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
        require_nonempty("signing.hooks.phase", &hook.phase)?;
        match hook.phase.as_str() {
            "after-launcher-build" => {}
            value => {
                return Err(Error::Other(anyhow::anyhow!(
                    "signing hook phase must be `after-launcher-build`, got `{value}`"
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

fn validate_current_package_target(value: &str) -> Result<()> {
    let current = current_package_target();
    if value != current {
        return Err(Error::Other(anyhow::anyhow!(
            "target `{value}` does not match current build host `{current}`; cross-target package manifests are not supported yet"
        )));
    }
    Ok(())
}

fn current_package_target() -> String {
    format!("{}-{}", platform::os_name(), platform::arch_name())
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
            if !working_dir.as_os_str().is_empty() {
                found_working_dir |= entry_path.starts_with(&working_dir);
            }
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

fn app_cache_name(app_id: &str) -> String {
    let hash = Sha256::digest(app_id.as_bytes());
    format!(
        "{}-{:016x}",
        safe_name(app_id),
        u64::from_be_bytes(hash[..8].try_into().expect("sha256 digest is long enough"))
    )
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
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
    fn parses_valid_manifest() {
        let payload = b"payload";
        let hash = format!("sha256:{:x}", Sha256::digest(payload));
        let text = format!(
            r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
target = "{}"
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
size = 7
"#,
            current_package_target()
        );

        let manifest = parse_manifest(&text).unwrap();
        assert_eq!(manifest.app_id, "com.example.notes");
        assert_eq!(manifest.start.command, ["bin/notes"]);
    }

    #[test]
    fn precheck_reports_manifest_plushie_rust_version() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let result = precheck_package(&manifest).unwrap();

        assert_eq!(result.plushie_rust_version, EXPECTED_PLUSHIE_RUST_VERSION);
    }

    #[test]
    fn prepare_packager_config_uses_library_config_shape() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let loaded = load_package(&manifest).unwrap();
        let portable = dir.path().join("portable-app");
        std::fs::write(&portable, b"portable").unwrap();

        let prepared = prepare_packager_config(
            &loaded,
            &portable,
            &dir.path().join("packager"),
            &dir.path().join("bundles"),
            &["deb".to_string()],
            None,
            true,
        )
        .unwrap();

        let config = std::fs::read_to_string(&prepared.config_path).unwrap();
        assert!(config.contains("productName = \"com.example.notes\""));
        assert!(config.contains("version = \"0.1.0\""));
        assert!(config.contains("identifier = \"com.example.notes\""));
        assert!(config.contains("formats = [\"deb\"]"));
        assert_eq!(prepared.config.product_name, "com.example.notes");
        assert_eq!(
            prepared.config.binaries[0].path,
            PathBuf::from("com.example.notes")
        );
        assert!(prepared.config.binaries[0].main);
        assert_eq!(prepared.config.formats, Some(vec![PackageFormat::Deb]));
        assert!(prepared.out_dir.ends_with("bundles"));
        assert_eq!(prepared.config_path, prepared.effective_config_path);
    }

    #[test]
    fn prepare_packager_config_writes_effective_config_for_custom_input() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let loaded = load_package(&manifest).unwrap();
        let portable = dir.path().join("portable-app");
        let config_path = dir.path().join("Packager.toml");
        std::fs::write(&portable, b"portable").unwrap();
        std::fs::write(
            &config_path,
            r#"
productName = "Notes"
outDir = "release"
"#,
        )
        .unwrap();

        let prepared = prepare_packager_config(
            &loaded,
            &portable,
            &dir.path().join("packager"),
            &dir.path().join("bundles"),
            &["deb".to_string()],
            Some(&config_path),
            true,
        )
        .unwrap();

        assert_eq!(prepared.config_path, config_path);
        assert!(
            prepared
                .effective_config_path
                .ends_with("packager/Packager.effective.toml")
        );
        let effective = std::fs::read_to_string(&prepared.effective_config_path).unwrap();
        assert!(effective.contains("productName = \"Notes\""));
        assert!(effective.contains("formats = [\"deb\"]"));
    }

    #[test]
    fn parse_packager_formats_accepts_platform_names() {
        let formats = parse_packager_formats(&[
            "app".to_string(),
            "appimage".to_string(),
            "deb".to_string(),
            "dmg".to_string(),
            "nsis".to_string(),
            "pacman".to_string(),
            "wix".to_string(),
        ])
        .unwrap()
        .unwrap();

        assert!(formats.contains(&PackageFormat::App));
        assert!(formats.contains(&PackageFormat::AppImage));
        assert!(formats.contains(&PackageFormat::Deb));
        assert!(formats.contains(&PackageFormat::Dmg));
        assert!(formats.contains(&PackageFormat::Nsis));
        assert!(formats.contains(&PackageFormat::Pacman));
        assert!(formats.contains(&PackageFormat::Wix));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepare_packager_config_rejects_non_linux_bundle_formats() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let loaded = load_package(&manifest).unwrap();
        let portable = dir.path().join("portable-app");
        std::fs::write(&portable, b"portable").unwrap();

        let err = prepare_packager_config(
            &loaded,
            &portable,
            &dir.path().join("packager"),
            &dir.path().join("bundles"),
            &["dmg".to_string()],
            None,
            true,
        )
        .unwrap_err();

        assert!(err.to_string().contains("not supported on this platform"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepare_packager_config_rejects_non_linux_formats_from_custom_config() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let loaded = load_package(&manifest).unwrap();
        let portable = dir.path().join("portable-app");
        let config_path = dir.path().join("Packager.toml");
        std::fs::write(&portable, b"portable").unwrap();
        std::fs::write(
            &config_path,
            r#"
formats = ["dmg"]
"#,
        )
        .unwrap();

        let err = prepare_packager_config(
            &loaded,
            &portable,
            &dir.path().join("packager"),
            &dir.path().join("bundles"),
            &[],
            Some(&config_path),
            true,
        )
        .unwrap_err();

        assert!(err.to_string().contains("not supported on this platform"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bundle_package_writes_deb_through_packager_library() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package_with_icon(dir.path());
        let portable = dir.path().join("portable-app");
        std::fs::copy("/usr/bin/true", &portable).unwrap();
        make_executable(&portable).unwrap();

        let result = bundle_package(&PackageBundleOpts {
            manifest_path: &manifest,
            portable_path: Some(&portable),
            out_dir: Some(&dir.path().join("bundles")),
            formats: &["deb".to_string()],
            config_path: None,
            package: PackageOpts {
                manifest_path: &manifest,
                out_path: None,
                launcher_path: None,
                run_signing_hooks: false,
                verbose: false,
            },
        })
        .unwrap();

        assert_eq!(result.outputs.len(), 1);
        assert_eq!(
            result.outputs[0]
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("deb")
        );
        assert!(result.outputs[0].is_file());
    }

    #[test]
    fn rejects_empty_start_command() {
        let text = format!(
            r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
target = "{}"
host_sdk = "python"
plushie_rust_version = "{EXPECTED_PLUSHIE_RUST_VERSION}"
protocol_version = {EXPECTED_PROTOCOL_VERSION}

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
"#,
            current_package_target()
        );

        let err = parse_manifest(&text).unwrap_err();
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
                &format!(r#"target = "{}""#, current_package_target()),
                &format!(r#"target = "{target}""#),
            );

            let err = parse_manifest(&text).unwrap_err();
            assert!(err.to_string().contains("target"));
        }
    }

    #[test]
    fn rejects_missing_package_target() {
        let text = valid_manifest_text("")
            .replace(&format!("target = \"{}\"\n", current_package_target()), "");

        let err = parse_manifest(&text).unwrap_err();
        assert!(err.to_string().contains("target"));
    }

    #[test]
    fn rejects_package_target_for_a_different_host() {
        let other_target = if current_package_target() == "linux-x86_64" {
            "darwin-x86_64"
        } else {
            "linux-x86_64"
        };
        let text = valid_manifest_text("").replace(
            &format!(r#"target = "{}""#, current_package_target()),
            &format!(r#"target = "{other_target}""#),
        );

        let err = parse_manifest(&text).unwrap_err();
        assert!(err.to_string().contains("current build host"));
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
            r#"forward_env = ["PLUSHIE_PACKAGE_READY_FILE"]"#,
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
            r#"
config_version = 1

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = ["PLUSHIE_PACKAGE_READY_FILE"]
"#,
        ] {
            assert!(parse_source_config(text).is_err());
        }
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
phase = "after-launcher-build"
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
phase = "before-launcher-build"
command = ["codesign"]
"#,
            r#"
[[signing.hooks]]
phase = "after-launcher-build"
command = []
"#,
            r#"
[[signing.hooks]]
phase = "after-launcher-build"
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
phase = "after-launcher-build"
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
            target: Some(current_package_target()),
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
    fn prepares_portable_launcher_from_template() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let launcher_template = write_launcher_template(dir.path());

        let opts = PackageOpts {
            manifest_path: &manifest,
            out_path: None,
            launcher_path: Some(&launcher_template),
            run_signing_hooks: false,
            verbose: false,
        };
        let prepared = prepare_portable_launcher(&opts).unwrap();
        assert_eq!(prepared.launcher_template_path, launcher_template);
        assert_eq!(
            prepared.manifest_text,
            std::fs::read_to_string(&manifest).unwrap()
        );
        assert!(!prepared.payload.is_empty());
    }

    #[test]
    fn prepares_relative_launcher_output_as_absolute_path() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let launcher_template = write_launcher_template(dir.path());

        let opts = PackageOpts {
            manifest_path: &manifest,
            out_path: Some(Path::new("dist/notes")),
            launcher_path: Some(&launcher_template),
            run_signing_hooks: false,
            verbose: false,
        };
        let prepared = prepare_portable_launcher(&opts).unwrap();

        assert!(prepared.output_path.is_absolute());
        assert!(prepared.output_path.ends_with("dist/notes"));
    }

    #[test]
    fn rejects_portable_output_that_overwrites_launcher_template() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let launcher_template = write_launcher_template(dir.path());

        let opts = PackageOpts {
            manifest_path: &manifest,
            out_path: Some(&launcher_template),
            launcher_path: Some(&launcher_template),
            run_signing_hooks: false,
            verbose: false,
        };

        let err = match prepare_portable_launcher(&opts) {
            Ok(_) => panic!("expected launcher overwrite rejection"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("must not overwrite"));
    }

    #[test]
    fn rejects_launcher_template_without_identity() {
        let dir = tempdir().unwrap();
        let manifest = write_sample_package(dir.path());
        let launcher_template = write_launcher_template(dir.path());

        let opts = PackageOpts {
            manifest_path: &manifest,
            out_path: None,
            launcher_path: Some(&launcher_template),
            run_signing_hooks: false,
            verbose: false,
        };

        let err = match build_launcher(&opts) {
            Ok(_) => panic!("expected launcher identity rejection"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("probe launcher template"));
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
        let signing_hooks = vec![SigningHookManifest {
            phase: "after-launcher-build".to_string(),
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf '%s\n%s\n' \"$1\" \"$PWD\" > \"$2\"".to_string(),
                "signing-test".to_string(),
                "{launcher}".to_string(),
                marker.display().to_string(),
            ],
        }];

        run_signing_hooks(dir.path(), &launcher, &signing_hooks).unwrap();

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
        let output_path = dir.path().join("dist/notes");
        let signing_hooks = vec![SigningHookManifest {
            phase: "after-launcher-build".to_string(),
            command: vec!["sh".to_string(), "-c".to_string(), "exit 9".to_string()],
        }];

        let err = run_signing_hooks(dir.path(), &output_path, &signing_hooks).unwrap_err();

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
    fn app_cache_names_include_hash_to_avoid_safe_name_collisions() {
        assert_ne!(
            app_cache_name("com.example/a"),
            app_cache_name("com.example_a")
        );
        assert_eq!(
            app_cache_name("com.example/a"),
            app_cache_name("com.example/a")
        );
    }

    #[test]
    fn rejects_global_host_program_paths() {
        let hash = format!("sha256:{:x}", Sha256::digest(b"payload"));
        let text = format!(
            r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
target = "{}"
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
            current_package_target(),
            EXPECTED_PLUSHIE_RUST_VERSION,
            EXPECTED_PROTOCOL_VERSION
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
    fn accepts_payload_without_explicit_non_root_working_dir_entry() {
        let payload = payload_archive_with_entries(&[
            ("bin/plushie-renderer", b"renderer".as_slice()),
            ("app/bin/notes", b"host".as_slice()),
        ]);
        let mut manifest = package_manifest_for_payload(&payload);
        manifest.start.command = vec!["app/bin/notes".to_string()];
        manifest.start.working_dir = "app".to_string();

        validate_payload_archive(&manifest, &payload).unwrap();
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
target = "{}"
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
            current_package_target(),
            EXPECTED_PLUSHIE_RUST_VERSION,
            EXPECTED_PROTOCOL_VERSION
        );

        let err = parse_manifest(&text).unwrap_err();
        assert!(err.to_string().contains("app_id"));
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
        write_package_for_payload(dir, &payload, "")
    }

    fn write_sample_package_with_icon(dir: &Path) -> PathBuf {
        let icon = crate::default_icons::default_icons()
            .iter()
            .find(|icon| icon.name == "plushie-checkbox-512x512.png")
            .expect("default app icon is bundled");
        let payload = payload_archive_with_dirs(
            &[
                ("bin/plushie-renderer", b"renderer".as_slice()),
                ("bin/notes", b"host".as_slice()),
                ("assets/plushie-checkbox-512x512.png", icon.bytes),
            ],
            &[],
        );
        write_package_for_payload(
            dir,
            &payload,
            r#"
[platform]
icon = "assets/plushie-checkbox-512x512.png"
"#,
        )
    }

    fn write_package_for_payload(dir: &Path, payload: &[u8], extra: &str) -> PathBuf {
        let archive = dir.join("payload.tar.zst");
        std::fs::write(&archive, payload).unwrap();
        let hash = format!("sha256:{:x}", Sha256::digest(payload));
        let manifest = dir.join("plushie-package.toml");
        std::fs::write(
            &manifest,
            format!(
                r#"
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
target = "{}"
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
hash = "{hash}"
"#,
                current_package_target()
            ),
        )
        .unwrap();
        manifest
    }

    fn write_launcher_template(dir: &Path) -> PathBuf {
        let path = dir.join("bin").join(platform::launcher_name());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"launcher").unwrap();
        std::fs::canonicalize(path).unwrap()
    }

    fn valid_manifest_text(extra: &str) -> String {
        let payload_hash = format!("sha256:{:x}", Sha256::digest(b"payload"));
        format!(
            r#"
schema_version = {MANIFEST_SCHEMA_VERSION}
app_id = "com.example.notes"
app_version = "0.1.0"
target = "{}"
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
"#,
            current_package_target()
        )
    }

    fn package_manifest_for_payload(payload: &[u8]) -> PackageManifest {
        PackageManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            app_id: "com.example.notes".to_string(),
            app_name: None,
            app_version: "0.1.0".to_string(),
            target: Some(current_package_target()),
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
target = "{}"
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
"#,
                    current_package_target()
                )
            }
            _ => unreachable!("unknown path field"),
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
