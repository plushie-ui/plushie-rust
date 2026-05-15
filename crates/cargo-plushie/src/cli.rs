//! Cargo subcommand entry point.
//!
//! Invoked as either `cargo plushie <sub>` (the Cargo subcommand
//! convention: Cargo rewrites the argv to `cargo-plushie plushie
//! <sub>`) or `cargo-plushie <sub>` directly. Both shapes dispatch
//! through the same clap parser below.

use crate::{
    default_icons, discover, doctor, download, generator, package, package_rust, platform,
    scaffold, tool_identity,
};
use anyhow::{Context, Result};
use cargo_metadata::CargoOpt;
use clap::{Args, Parser, Subcommand};
use plushie_core::tool_identity::ToolIdentity;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Built-in renderer widget type names from `plushie-core`, shared
/// with the widget SDK without pulling iced into this build tool.
const BUILTIN_TYPE_NAMES: &[&str] = plushie_core::BUILTIN_TYPE_NAMES;

#[derive(Parser, Debug)]
#[command(
    name = "plushie",
    about = "Cargo subcommand for Plushie renderer build + download",
    version
)]
struct Cli {
    /// Nested plushie subcommand.
    #[command(subcommand)]
    command: PlushieSubcommand,
}

#[derive(Subcommand, Debug)]
enum PlushieSubcommand {
    /// Build a custom renderer binary wired to all native widgets
    /// found in the dep graph.
    Build(BuildArgs),
    /// Download a precompiled stock renderer binary.
    Download(DownloadArgs),
    /// Check or sync Plushie native tools under the project bin directory.
    Tools(ToolsArgs),
    /// Build the custom renderer and then run the app binary. The
    /// SDK's wire discovery picks the freshly built renderer up from
    /// `target/plushie-renderer/` without any extra wiring.
    Run(RunArgs),
    /// Assemble, bundle, check, or build portable Plushie packages.
    Package(PackageArgs),
    /// Scaffold a new native widget crate with the conventional
    /// `[package.metadata.plushie.widget]` layout.
    NewWidget(NewWidgetArgs),
    /// Scaffold a new plushie app crate with a wired-up main.rs,
    /// automation-script example, and a sample `.plushie` script.
    Init(InitArgs),
    /// Print a diagnostic report (toolchain, env, renderer discovery,
    /// widgets, version skew). Exits non-zero if any critical issue
    /// is detected.
    Doctor(DoctorArgs),
    /// Write Plushie's bundled default app icons to a directory.
    DefaultIcons(DefaultIconsArgs),
}

#[derive(Args, Debug)]
struct BuildArgs {
    /// Build with the `release` Cargo profile.
    #[arg(long)]
    release: bool,
    /// Print the underlying cargo command and stream its output.
    #[arg(long)]
    verbose: bool,
    /// Path to the app crate manifest (defaults to `./Cargo.toml`).
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    /// Build the `plushie-renderer-wasm` bundle via wasm-pack
    /// instead of producing a native custom renderer.
    #[arg(long)]
    wasm: bool,
    /// Output directory for the wasm-pack bundle. Defaults to
    /// `target/plushie/pkg/`.
    #[arg(long)]
    wasm_dir: Option<PathBuf>,
    /// Cargo features to enable while resolving the app graph.
    #[arg(long = "features")]
    features: Vec<String>,
    /// Disable default features while resolving the app graph.
    #[arg(long)]
    no_default_features: bool,
    /// Enable all features while resolving the app graph.
    #[arg(long)]
    all_features: bool,
}

#[derive(Args, Debug)]
struct DownloadArgs {
    /// Force overwrite of an existing binary.
    #[arg(long)]
    force: bool,
    /// Exact plushie-rust version to download without reading Cargo metadata.
    #[arg(long)]
    required_version: Option<String>,
    /// Path to the app crate manifest (defaults to `./Cargo.toml`).
    #[arg(long)]
    manifest_path: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct ToolsArgs {
    /// Native tool workflow command.
    #[command(subcommand)]
    command: ToolsSubcommand,
}

#[derive(Subcommand, Debug)]
enum ToolsSubcommand {
    /// Check local Plushie native tool versions.
    Check(ToolsCheckArgs),
    /// Sync local Plushie native tools to the required version.
    Sync(ToolsSyncArgs),
}

#[derive(Args, Debug)]
struct ToolsCheckArgs {
    /// Exact plushie-rust version expected by the SDK.
    #[arg(long)]
    required_version: Option<String>,
    /// Path to the app crate manifest when using Cargo metadata.
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    /// Treat dirty or mixed-source tools as failures.
    #[arg(long)]
    strict: bool,
    /// Emit JSON instead of human-readable output.
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct ToolsSyncArgs {
    /// Exact plushie-rust version expected by the SDK.
    #[arg(long)]
    required_version: Option<String>,
    /// Path to the app crate manifest when using Cargo metadata.
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    /// Allow replacing source-built, custom, or identity-less tools.
    #[arg(long)]
    force: bool,
}

#[derive(Args, Debug)]
struct NewWidgetArgs {
    /// Kebab-case widget name (e.g. `my-gauge`). Becomes the
    /// Cargo package name, the `type_name` (snake-cased), and
    /// a PascalCase builder struct.
    name: String,
    /// Destination path for the new crate. Defaults to
    /// `./native/<name>`.
    #[arg(long)]
    path: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct DoctorArgs {
    /// Path to the app crate manifest (defaults to `./Cargo.toml`).
    #[arg(long)]
    manifest_path: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct InitArgs {
    /// Kebab-case app name (e.g. `my-app`). Becomes the Cargo
    /// package name and a PascalCase App struct.
    name: String,
    /// Destination path for the new crate. Defaults to `./<name>`.
    #[arg(long)]
    path: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct RunArgs {
    /// Watch the app's src/ for changes and restart on edit.
    /// Delegates to `cargo-watch` if it's installed, otherwise
    /// falls back to a single `cargo run` invocation.
    #[arg(long)]
    watch: bool,
    /// Build with the `release` Cargo profile.
    #[arg(long)]
    release: bool,
    /// Print the underlying cargo commands.
    #[arg(long)]
    verbose: bool,
    /// Path to the app crate manifest (defaults to `./Cargo.toml`).
    #[arg(long)]
    manifest_path: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct PackageArgs {
    /// Package workflow command.
    #[command(subcommand)]
    command: PackageSubcommand,
}

#[derive(Subcommand, Debug)]
enum PackageSubcommand {
    /// Build a wire-mode Rust app payload directory and manifest.
    Assemble(PackageRustArgs),
    /// Build a self-extracting portable launcher from a package manifest.
    Portable(PackagePortableArgs),
    /// Check a package manifest, payload, or portable launcher.
    Check(PackageCheckArgs),
    /// Create a platform bundle through cargo-packager.
    Bundle(PackageBundleArgs),
}

#[derive(Args, Debug)]
struct PackagePortableArgs {
    /// Path to the Plushie package manifest.
    #[arg(long)]
    manifest: PathBuf,
    /// Fail when managed native tools are missing, dirty, mixed, or version-mismatched.
    #[arg(long)]
    strict_tools: bool,
    /// Final launcher output path. Defaults under target/plushie/package/.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Reusable plushie-launcher binary to use for the portable artifact.
    #[arg(long)]
    launcher: Option<PathBuf>,
    /// Run signing hooks declared by the package manifest.
    #[arg(long)]
    run_signing_hooks: bool,
    /// Print launcher template resolution.
    #[arg(long)]
    verbose: bool,
}

#[derive(Args, Debug)]
struct PackageCheckArgs {
    /// Path to the Plushie package manifest.
    #[arg(long)]
    manifest: PathBuf,
    /// Fail when managed native tools are missing, dirty, mixed, or version-mismatched.
    #[arg(long)]
    strict_tools: bool,
    /// Also build the portable launcher and run its extraction/cache check.
    #[arg(long)]
    postcheck: bool,
    /// Postcheck timeout in seconds.
    #[arg(long, default_value_t = 10)]
    postcheck_timeout: u64,
    /// Final launcher output path for --postcheck.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Reusable plushie-launcher binary to use for --postcheck.
    #[arg(long)]
    launcher: Option<PathBuf>,
    /// Run signing hooks declared by the package manifest.
    #[arg(long)]
    run_signing_hooks: bool,
    /// Print launcher template resolution.
    #[arg(long)]
    verbose: bool,
}

#[derive(Args, Debug)]
struct PackageBundleArgs {
    /// Assembled app directory to bundle.
    #[arg(long)]
    app: PathBuf,
}

#[derive(Args, Debug)]
struct PackageRustArgs {
    /// Path to the Rust app crate manifest (defaults to `./Cargo.toml`).
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    /// Cargo binary target to build when the package has multiple bins.
    #[arg(long)]
    bin: Option<String>,
    /// Package application ID. Defaults to package metadata or package name.
    #[arg(long)]
    app_id: Option<String>,
    /// Human-readable app name written as optional package metadata.
    #[arg(long)]
    app_name: Option<String>,
    /// App icon to copy into the payload. Defaults to bundled Plushie icons.
    #[arg(long)]
    icon: Option<PathBuf>,
    /// Output directory for generated manifest and payload archive.
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Developer-owned package config. Defaults to plushie-package.config.toml
    /// next to the app manifest when present.
    #[arg(long)]
    package_config: Option<PathBuf>,
    /// Write a package config template and exit before building.
    #[arg(long)]
    write_package_config: bool,
    /// Final launcher output path. Defaults under target/plushie/package/.
    #[arg(long)]
    launcher_out: Option<PathBuf>,
    /// Build host, renderer, and launcher with Cargo's release profile.
    #[arg(long)]
    release: bool,
    /// Print underlying cargo commands.
    #[arg(long)]
    verbose: bool,
    /// Additional Cargo features for the host build.
    #[arg(long = "features")]
    features: Vec<String>,
    /// Disable default features for the host build.
    #[arg(long)]
    no_default_features: bool,
    /// Enable all features for the host build.
    #[arg(long)]
    all_features: bool,
    /// Stop after writing plushie-package.toml and payload.tar.zst.
    #[arg(long)]
    no_launcher: bool,
    /// Run signing hooks declared by the generated package manifest.
    #[arg(long)]
    run_signing_hooks: bool,
    /// Reusable plushie-launcher binary to use for the portable artifact.
    #[arg(long)]
    launcher: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct DefaultIconsArgs {
    /// Output directory for the bundled icon files.
    #[arg(long)]
    out: PathBuf,
}

/// Parse CLI arguments and dispatch the selected Plushie command.
pub fn run() -> Result<()> {
    // Cargo invokes subcommands as `cargo-plushie plushie <sub>`.
    // Drop the extra word so the standalone `plushie` binary and the
    // Cargo entry point share one parser.
    let mut argv: Vec<String> = std::env::args().collect();
    if argv.len() >= 2 && argv[1] == "plushie" {
        argv.remove(1);
        argv[0] = "cargo plushie".to_string();
    } else {
        argv[0] = "plushie".to_string();
    }
    if argv.iter().any(|arg| arg == "--version") {
        let json = argv.iter().any(|arg| arg == "--json");
        let tool = if argv[0] == "cargo plushie" {
            "cargo-plushie"
        } else {
            "plushie"
        };
        return tool_identity::print_current_version(tool, json);
    }
    let cli = Cli::parse_from(argv);
    match cli.command {
        PlushieSubcommand::Build(b) => cmd_build(&b),
        PlushieSubcommand::Download(d) => cmd_download(&d),
        PlushieSubcommand::Tools(t) => cmd_tools(&t),
        PlushieSubcommand::Run(r) => cmd_run(&r),
        PlushieSubcommand::Package(p) => cmd_package(&p),
        PlushieSubcommand::NewWidget(n) => cmd_new_widget(&n),
        PlushieSubcommand::Init(i) => cmd_init(&i),
        PlushieSubcommand::Doctor(d) => cmd_doctor(&d),
        PlushieSubcommand::DefaultIcons(i) => cmd_default_icons(&i),
    }
}

fn cmd_package_rust(args: &PackageRustArgs, build_portable: bool) -> Result<()> {
    let manifest_path = args
        .manifest_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("Cargo.toml"));
    let manifest_path = std::fs::canonicalize(&manifest_path)
        .with_context(|| format!("manifest path `{}` not found", manifest_path.display()))?;
    let manifest_dir = manifest_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("manifest path has no parent directory"))?
        .to_path_buf();
    if args.write_package_config {
        let out_dir = args
            .out_dir
            .clone()
            .unwrap_or_else(|| target_dir(&manifest_dir).join("plushie/rust-package"));
        let path =
            package_rust::write_rust_package_config(&package_rust::RustPackageAssembleOpts {
                manifest_path: &manifest_path,
                renderer_path: Path::new(""),
                source_path: None,
                out_dir: &out_dir,
                package_config: args.package_config.as_deref(),
                bin: args.bin.as_deref(),
                app_id: args.app_id.as_deref(),
                app_name: args.app_name.as_deref(),
                icon: args.icon.as_deref(),
                features: &args.features,
                no_default_features: args.no_default_features,
                all_features: args.all_features,
                release: args.release,
                verbose: args.verbose,
            })?;
        println!(
            "plushie: wrote package config template at {}",
            path.display()
        );
        return Ok(());
    }

    let build = BuildArgs {
        release: args.release,
        verbose: args.verbose,
        manifest_path: Some(manifest_path.clone()),
        wasm: false,
        wasm_dir: None,
        features: package_rust_features(args),
        no_default_features: args.no_default_features,
        all_features: args.all_features,
    };
    package_rust::ensure_current_host_target()?;
    cmd_build(&build)?;
    let app_pkg = load_app_package_no_deps(&manifest_dir)?;
    let source_path = resolve_source_path(&manifest_dir, &app_pkg)?;
    let renderer_path = resolve_built_binary(
        &manifest_dir,
        &RunArgs {
            watch: false,
            release: args.release,
            verbose: args.verbose,
            manifest_path: Some(manifest_path.clone()),
        },
    )?;
    if !renderer_path.is_file() {
        return Err(anyhow::anyhow!(
            "expected renderer at `{}` but it was not found",
            renderer_path.display()
        ));
    }

    let out_dir = args
        .out_dir
        .clone()
        .unwrap_or_else(|| target_dir(&manifest_dir).join("plushie/rust-package"));
    let assembled = package_rust::assemble_rust_package(&package_rust::RustPackageAssembleOpts {
        manifest_path: &manifest_path,
        renderer_path: &renderer_path,
        source_path: source_path.as_deref(),
        out_dir: &out_dir,
        package_config: args.package_config.as_deref(),
        bin: args.bin.as_deref(),
        app_id: args.app_id.as_deref(),
        app_name: args.app_name.as_deref(),
        icon: args.icon.as_deref(),
        features: &args.features,
        no_default_features: args.no_default_features,
        all_features: args.all_features,
        release: args.release,
        verbose: args.verbose,
    })?;

    println!(
        "plushie: assembled Rust package manifest at {}",
        assembled.manifest_path.display()
    );
    println!(
        "plushie: assembled Rust payload at {}",
        assembled.payload_archive_path.display()
    );
    println!(
        "plushie: assembled Rust package icon at {}",
        assembled.icon_payload_path.display()
    );
    println!(
        "plushie: assembled Rust package host at {}",
        assembled.host_payload_path.display()
    );
    println!(
        "plushie: assembled Rust package renderer at {}",
        assembled.renderer_payload_path.display()
    );
    println!(
        "plushie: assembled Rust package payload root at {}",
        assembled.payload_dir.display()
    );

    if args.no_launcher || !build_portable {
        println!(
            "plushie: hand off with `cargo plushie package portable --manifest {}`",
            assembled.manifest_path.display()
        );
        return Ok(());
    }

    let result = package::build_launcher(&package::PackageOpts {
        manifest_path: &assembled.manifest_path,
        out_path: args.launcher_out.as_deref(),
        launcher_path: args.launcher.as_deref(),
        run_signing_hooks: args.run_signing_hooks,
        verbose: args.verbose,
    })?;
    println!(
        "plushie: wrote portable launcher at {}",
        result.binary_path.display()
    );
    println!(
        "plushie: used launcher template {}",
        result.launcher_template_path.display()
    );
    Ok(())
}

fn package_rust_features(args: &PackageRustArgs) -> Vec<String> {
    let mut features = args.features.clone();
    if !features.iter().any(|feature| feature == "plushie/wire") {
        features.push("plushie/wire".to_string());
    }
    features
}

fn cmd_default_icons(args: &DefaultIconsArgs) -> Result<()> {
    let written = default_icons::write_default_icons(&args.out)?;
    for path in written {
        println!("plushie: wrote default icon {}", path.display());
    }
    Ok(())
}

fn cmd_package(args: &PackageArgs) -> Result<()> {
    match &args.command {
        PackageSubcommand::Assemble(a) => cmd_package_rust(a, true),
        PackageSubcommand::Portable(p) => cmd_package_portable(p),
        PackageSubcommand::Check(c) => cmd_package_check(c),
        PackageSubcommand::Bundle(b) => {
            anyhow::bail!(
                "package bundle is reserved for cargo-packager integration; app directory was `{}`",
                b.app.display()
            )
        }
    }
}

fn cmd_tools(args: &ToolsArgs) -> Result<()> {
    match &args.command {
        ToolsSubcommand::Check(check) => cmd_tools_check(check),
        ToolsSubcommand::Sync(sync) => cmd_tools_sync(sync),
    }
}

#[derive(Debug, Serialize)]
struct ToolCheckReport {
    required_version: String,
    ok: bool,
    tools: Vec<ToolCheckEntry>,
    issues: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ToolCheckEntry {
    tool: String,
    path: Option<String>,
    identity: Option<ToolIdentity>,
    status: String,
}

fn cmd_tools_check(args: &ToolsCheckArgs) -> Result<()> {
    let required_version = resolve_required_version(
        args.required_version.as_deref(),
        args.manifest_path.as_ref(),
    )?;
    let project_dir = std::env::current_dir().with_context(|| "resolve current directory")?;
    let report = check_native_tools(&project_dir, &required_version, args.strict);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_tool_check_report(&report);
    }

    if report.ok {
        Ok(())
    } else {
        anyhow::bail!("Plushie native tool check failed")
    }
}

fn cmd_tools_sync(args: &ToolsSyncArgs) -> Result<()> {
    let required_version = resolve_required_version(
        args.required_version.as_deref(),
        args.manifest_path.as_ref(),
    )?;
    let self_identity = tool_identity::current_tool_identity("plushie");
    if self_identity.plushie_rust_version != required_version {
        anyhow::bail!(
            "plushie is version {} but this project requires {}; update bin/plushie first",
            self_identity.plushie_rust_version,
            required_version
        );
    }

    let project_dir = std::env::current_dir().with_context(|| "resolve current directory")?;
    if let Some(manifest_path) = &args.manifest_path {
        check_native_tool_manifest(manifest_path)?;
    }
    if self_identity.source.kind == "source" {
        return sync_source_native_tools(&project_dir);
    }

    let download_args = DownloadArgs {
        force: args.force,
        required_version: Some(required_version.clone()),
        manifest_path: args.manifest_path.clone(),
    };
    let source_configured = if args.manifest_path.is_some() {
        check_download_manifest(&download_args, None)?
    } else {
        false
    };
    let release_base_url = download::release_base_url()?;
    download_renderer_with_base_url(
        &project_dir,
        &required_version,
        args.force,
        source_configured,
        &release_base_url,
    )?;
    download_launcher_with_base_url(
        &project_dir,
        &required_version,
        args.force,
        source_configured,
        &release_base_url,
    )?;
    download_plushie_with_base_url(
        &project_dir,
        &required_version,
        args.force,
        source_configured,
        &release_base_url,
    )
}

fn sync_source_native_tools(project_dir: &Path) -> Result<()> {
    let workspace_root = plushie_rust_workspace_root()?;
    let target_dir = source_build_target_dir(&workspace_root)?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    build_source_tool(
        &cargo,
        &workspace_root,
        &target_dir,
        &["build", "--release", "-p", "plushie-renderer"],
    )?;
    build_source_tool(
        &cargo,
        &workspace_root,
        &target_dir,
        &[
            "build",
            "--release",
            "-p",
            "cargo-plushie",
            "--bin",
            "plushie",
            "--bin",
            "plushie-launcher",
        ],
    )?;

    install_source_tool(
        &target_dir.join("release").join(platform::plushie_name()),
        &project_dir.join("bin").join(platform::plushie_name()),
        "plushie",
    )?;
    install_source_tool(
        &target_dir.join("release").join(platform::renderer_name()),
        &project_dir.join("bin").join(platform::renderer_name()),
        "renderer",
    )?;
    install_source_tool(
        &target_dir.join("release").join(platform::launcher_name()),
        &project_dir.join("bin").join(platform::launcher_name()),
        "launcher",
    )?;
    Ok(())
}

fn build_source_tool(
    cargo: &str,
    workspace_root: &Path,
    target_dir: &Path,
    args: &[&str],
) -> Result<()> {
    println!("plushie: running {} {}", cargo, args.join(" "));
    let mut command = std::process::Command::new(cargo);
    command.current_dir(workspace_root).args(args);
    if std::env::var_os("CARGO_TARGET_DIR").is_some() {
        command.env("CARGO_TARGET_DIR", target_dir);
    }
    let status = command
        .status()
        .with_context(|| format!("failed to run `{}`", cargo))?;
    if !status.success() {
        anyhow::bail!("source native tool build failed with status {status}");
    }
    Ok(())
}

fn install_source_tool(source: &Path, dest: &Path, label: &str) -> Result<()> {
    if !source.is_file() {
        anyhow::bail!("source-built {label} was not found at {}", source.display());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, dest)
        .with_context(|| format!("copy source-built {label} to `{}`", dest.display()))?;
    make_executable(dest)?;
    println!(
        "plushie: installed source-built {label} at {}",
        dest.display()
    );
    Ok(())
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

fn source_build_target_dir(workspace_root: &Path) -> Result<PathBuf> {
    Ok(target_dir_from(
        std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        &std::env::current_dir().with_context(|| "resolve current directory")?,
        workspace_root,
    ))
}

fn plushie_rust_workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow::anyhow!("unable to resolve plushie-rust workspace root"))
}

fn check_native_tool_manifest(manifest_path: &PathBuf) -> Result<()> {
    let manifest_dir = resolve_manifest_dir(Some(manifest_path))?;
    let widgets = discover::discover_widgets(&manifest_dir)?;
    download::refuse_if_native_widgets(&widgets)?;
    Ok(())
}

fn resolve_required_version(
    explicit: Option<&str>,
    manifest_path: Option<&PathBuf>,
) -> Result<String> {
    if let Some(version) = explicit {
        let version = version.trim();
        if version.is_empty() {
            anyhow::bail!("--required-version must not be empty");
        }
        return Ok(version.to_string());
    }

    let manifest_dir = resolve_manifest_dir(manifest_path)?;
    resolve_renderer_version(&manifest_dir)
}

fn check_native_tools(project_dir: &Path, required_version: &str, strict: bool) -> ToolCheckReport {
    let self_identity = tool_identity::current_tool_identity("plushie");
    let mut tools = Vec::new();
    let mut issues = Vec::new();
    let mut warnings = Vec::new();

    if self_identity.plushie_rust_version != required_version {
        issues.push(format!(
            "plushie is version {} but project requires {}; update bin/plushie",
            self_identity.plushie_rust_version, required_version
        ));
    }
    collect_identity_warnings(
        &self_identity,
        "plushie",
        strict,
        &mut issues,
        &mut warnings,
    );
    tools.push(ToolCheckEntry {
        tool: "plushie-current".to_string(),
        path: None,
        identity: Some(self_identity.clone()),
        status: "present".to_string(),
    });

    check_managed_tool(
        project_dir,
        "plushie",
        &platform::plushie_name(),
        required_version,
        strict,
        &self_identity,
        &mut tools,
        &mut issues,
        &mut warnings,
    );
    check_managed_tool(
        project_dir,
        "plushie-renderer",
        &platform::renderer_name(),
        required_version,
        strict,
        &self_identity,
        &mut tools,
        &mut issues,
        &mut warnings,
    );
    check_managed_tool(
        project_dir,
        "plushie-launcher",
        &platform::launcher_name(),
        required_version,
        strict,
        &self_identity,
        &mut tools,
        &mut issues,
        &mut warnings,
    );

    let ok = issues.is_empty();
    ToolCheckReport {
        required_version: required_version.to_string(),
        ok,
        tools,
        issues,
        warnings,
    }
}

fn check_managed_tool(
    project_dir: &Path,
    label: &str,
    local_name: &str,
    required_version: &str,
    strict: bool,
    self_identity: &ToolIdentity,
    tools: &mut Vec<ToolCheckEntry>,
    issues: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let path = project_dir.join("bin").join(local_name);
    match tool_identity::probe_tool_identity(&path, Duration::from_secs(2)) {
        Ok(identity) => {
            if identity.plushie_rust_version != required_version {
                issues.push(format!(
                    "{} is version {} but project requires {}; run `bin/plushie tools sync --required-version {}`",
                    label, identity.plushie_rust_version, required_version, required_version
                ));
            }
            if identity.target != self_identity.target {
                issues.push(format!(
                    "{} target {} does not match plushie target {}; run `bin/plushie tools sync --required-version {}`",
                    label, identity.target, self_identity.target, required_version
                ));
            }
            if identity.source.kind != self_identity.source.kind {
                let message = format!(
                    "plushie source kind {} does not match {} source kind {}",
                    self_identity.source.kind, label, identity.source.kind
                );
                if strict {
                    issues.push(message);
                } else {
                    warnings.push(message);
                }
            }
            collect_identity_warnings(&identity, label, strict, issues, warnings);
            tools.push(ToolCheckEntry {
                tool: label.to_string(),
                path: Some(path.display().to_string()),
                identity: Some(identity),
                status: "present".to_string(),
            });
        }
        Err(error) if path.exists() => {
            issues.push(format!(
                "{} at {} did not report Plushie identity ({error}); run `bin/plushie tools sync --required-version {} --force`",
                label,
                path.display(),
                required_version
            ));
            tools.push(ToolCheckEntry {
                tool: label.to_string(),
                path: Some(path.display().to_string()),
                identity: None,
                status: "unreadable".to_string(),
            });
        }
        Err(_) => {
            issues.push(format!(
                "{} is missing at {}; run `bin/plushie tools sync --required-version {}`",
                label,
                path.display(),
                required_version
            ));
            tools.push(ToolCheckEntry {
                tool: label.to_string(),
                path: Some(path.display().to_string()),
                identity: None,
                status: "missing".to_string(),
            });
        }
    }
}

fn collect_identity_warnings(
    identity: &ToolIdentity,
    label: &str,
    strict: bool,
    issues: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if identity.source.git_dirty == Some(true) {
        let message = format!("{label} was built from a dirty checkout");
        if strict {
            issues.push(message);
        } else {
            warnings.push(message);
        }
    }
}

fn print_tool_check_report(report: &ToolCheckReport) {
    for tool in &report.tools {
        match &tool.identity {
            Some(identity) => {
                println!(
                    "plushie: {} {} ({}, {})",
                    tool.tool, identity.plushie_rust_version, identity.target, identity.source.kind
                );
            }
            None => {
                println!("plushie: {} {}", tool.tool, tool.status);
            }
        }
    }
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    for issue in &report.issues {
        eprintln!("error: {issue}");
    }
}

fn cmd_package_check(args: &PackageCheckArgs) -> Result<()> {
    let precheck = package::precheck_package(&args.manifest)?;
    print_package_warnings(&precheck);
    if args.strict_tools {
        ensure_strict_package_tools(&precheck.plushie_rust_version)?;
    }

    if !args.postcheck {
        println!(
            "plushie: checked package {} {} ({})",
            precheck.app_id, precheck.app_version, precheck.payload_hash
        );
        return Ok(());
    }

    let result = package::postcheck_package(&package::PackagePostcheckOpts {
        package: package::PackageOpts {
            manifest_path: &args.manifest,
            out_path: args.out.as_deref(),
            launcher_path: args.launcher.as_deref(),
            run_signing_hooks: args.run_signing_hooks,
            verbose: args.verbose,
        },
        timeout: Duration::from_secs(args.postcheck_timeout),
    })?;
    println!(
        "plushie: postchecked portable launcher at {}",
        result.binary_path.display()
    );
    println!("plushie: postcheck cache at {}", result.cache_dir.display());
    Ok(())
}

fn cmd_package_portable(args: &PackagePortableArgs) -> Result<()> {
    let precheck = package::precheck_package(&args.manifest)?;
    print_package_warnings(&precheck);
    if args.strict_tools {
        ensure_strict_package_tools(&precheck.plushie_rust_version)?;
    }
    let result = package::build_launcher(&package::PackageOpts {
        manifest_path: &args.manifest,
        out_path: args.out.as_deref(),
        launcher_path: args.launcher.as_deref(),
        run_signing_hooks: args.run_signing_hooks,
        verbose: args.verbose,
    })?;
    println!(
        "plushie: wrote portable launcher at {}",
        result.binary_path.display()
    );
    println!(
        "plushie: used launcher template {}",
        result.launcher_template_path.display()
    );
    Ok(())
}

fn ensure_strict_package_tools(required_version: &str) -> Result<()> {
    let project_dir = std::env::current_dir().with_context(|| "resolve current directory")?;
    let report = check_native_tools(&project_dir, required_version, true);
    if report.ok {
        return Ok(());
    }

    print_tool_check_report(&report);
    anyhow::bail!("Plushie native tool check failed")
}

fn print_package_warnings(precheck: &package::PackagePrecheckResult) {
    for warning in &precheck.warnings {
        eprintln!("warning: {}", warning.message());
    }
}

fn cmd_doctor(args: &DoctorArgs) -> Result<()> {
    let manifest_dir = resolve_manifest_dir(args.manifest_path.as_ref())?;
    let opts = doctor::DoctorOpts {
        manifest_dir: &manifest_dir,
        min_rustc_version: "1.92",
    };
    let report = doctor::run_doctor(&opts)?;
    let mut stdout = std::io::stdout().lock();
    doctor::write_report(&report, &mut stdout)?;
    if report.critical {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_init(args: &InitArgs) -> Result<()> {
    let opts = scaffold::InitOpts {
        name: &args.name,
        path: args.path.as_deref(),
    };
    let result = scaffold::scaffold_app(&opts)?;
    let shown = result
        .crate_root
        .strip_prefix(std::env::current_dir().unwrap_or_default())
        .unwrap_or(&result.crate_root)
        .display()
        .to_string();
    let shown = if shown.is_empty() {
        result.crate_root.display().to_string()
    } else {
        shown
    };
    println!(
        "Scaffolded {name} at {shown}.\n\nNext steps:\n  \
         cd {name}\n  cargo run                 # direct mode\n  \
         cargo plushie run --watch # custom renderer + dev loop",
        name = args.name,
    );
    Ok(())
}

fn cmd_new_widget(args: &NewWidgetArgs) -> Result<()> {
    let opts = scaffold::NewWidgetOpts {
        name: &args.name,
        path: args.path.as_deref(),
        builtin_type_names: BUILTIN_TYPE_NAMES,
    };
    let result = scaffold::scaffold_widget(&opts)?;
    let relative = result
        .crate_root
        .strip_prefix(std::env::current_dir().unwrap_or_default())
        .unwrap_or(&result.crate_root)
        .display()
        .to_string();
    let shown = if relative.is_empty() {
        result.crate_root.display().to_string()
    } else {
        relative
    };
    println!(
        "Scaffolded {name} at {shown}. Add it to your app's \
         [package.metadata.plushie].native_widgets or let auto-discovery \
         pick it up via cargo plushie build.",
        name = args.name,
    );
    Ok(())
}

fn resolve_manifest_dir(manifest_path: Option<&PathBuf>) -> Result<PathBuf> {
    let path = match manifest_path {
        Some(p) => p.clone(),
        None => PathBuf::from("Cargo.toml"),
    };
    let abs = std::fs::canonicalize(&path)
        .with_context(|| format!("manifest path `{}` not found", path.display()))?;
    Ok(abs.parent().map(PathBuf::from).unwrap_or(abs))
}

fn target_dir(manifest_dir: &std::path::Path) -> PathBuf {
    target_dir_from(
        std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        &std::env::current_dir().unwrap_or_else(|_| manifest_dir.to_path_buf()),
        manifest_dir,
    )
}

fn target_dir_from(
    cargo_target_dir: Option<PathBuf>,
    invocation_dir: &std::path::Path,
    manifest_dir: &std::path::Path,
) -> PathBuf {
    match cargo_target_dir {
        Some(path) if path.is_absolute() => path,
        Some(path) => invocation_dir.join(path),
        None => manifest_dir.join("target"),
    }
}

/// Narrow `discovered` to the crates named in the app's explicit
/// `[package.metadata.plushie].native_widgets` allowlist.
///
/// Returns an error if any named crate is not a direct dep of the app
/// or is not declared as a plushie widget (no
/// `[package.metadata.plushie.widget]` table). The latter surfaces
/// either because the crate predates the metadata convention or
/// because the user typo'd a name; either way, failing loud is
/// friendlier than silently omitting the widget from the build.
fn filter_native_widgets(
    app_pkg: &cargo_metadata::Package,
    discovered: &[crate::WidgetMetadata],
    allowlist: &[String],
) -> Result<Vec<crate::WidgetMetadata>> {
    use std::collections::HashSet;

    let direct_deps: HashSet<&str> = app_pkg
        .dependencies
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    let discovered_by_name: std::collections::HashMap<&str, &crate::WidgetMetadata> = discovered
        .iter()
        .map(|w| (w.crate_name.as_str(), w))
        .collect();

    let mut out = Vec::with_capacity(allowlist.len());
    for name in allowlist {
        if !direct_deps.contains(name.as_str()) {
            return Err(anyhow::anyhow!(
                "[package.metadata.plushie].native_widgets lists `{name}`, but `{name}` \
                 is not a direct dependency of `{app}`. Add it to [dependencies] or remove \
                 it from the allowlist.",
                app = app_pkg.name,
            ));
        }
        match discovered_by_name.get(name.as_str()) {
            Some(widget) => out.push((*widget).clone()),
            None => {
                return Err(anyhow::anyhow!(
                    "[package.metadata.plushie].native_widgets lists `{name}`, but that \
                     crate does not declare `[package.metadata.plushie.widget]`. Either \
                     remove it from the allowlist or add the widget metadata table to \
                     the crate's Cargo.toml."
                ));
            }
        }
    }
    // Deterministic order for reproducible output, same as discover_widgets.
    out.sort_by(|a, b| a.crate_name.cmp(&b.crate_name));
    Ok(out)
}

fn cmd_build(args: &BuildArgs) -> Result<()> {
    let manifest_dir = resolve_manifest_dir(args.manifest_path.as_ref())?;

    if args.wasm {
        return cmd_build_wasm(&manifest_dir, args);
    }

    let target = target_dir(&manifest_dir);
    let output_dir = target.join("plushie-renderer");
    std::fs::create_dir_all(&output_dir)?;

    // Resolve app package metadata first (name + version + optional
    // [package.metadata.plushie] overrides) using `--no-deps` so we
    // don't need the full dep graph to resolve cleanly. Host SDKs
    // generate "spec" manifests whose widget crates depend on
    // unpublished plushie-rust versions; the full-graph discovery call
    // below fails on those until we drop a `.cargo/config.toml` with
    // patch overrides alongside the spec manifest.
    let mut metadata_cmd = cargo_metadata::MetadataCommand::new();
    metadata_cmd
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .no_deps();
    apply_feature_args(
        &mut metadata_cmd,
        &args.features,
        args.no_default_features,
        args.all_features,
    );
    let metadata = metadata_cmd
        .exec()
        .with_context(|| "cargo metadata (no-deps) failed")?;
    let app_pkg = app_package_from_metadata(&metadata, &manifest_dir.join("Cargo.toml"))?;

    let binary_name_override = app_pkg
        .metadata
        .get("plushie")
        .and_then(|v| v.get("binary_name"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // Optional explicit allowlist of widget crates to register. When
    // set (non-empty), we filter discovery down to the named crates
    // and validate that each one is a direct dep of the app crate
    // declaring a `[package.metadata.plushie.widget]` table. When
    // unset, full auto-discovery stands.
    let native_widgets_override: Vec<String> = app_pkg
        .metadata
        .get("plushie")
        .and_then(|v| v.get("native_widgets"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let source_path = resolve_source_path(&manifest_dir, &app_pkg)?;

    // When the caller points at a local plushie-rust checkout, drop a
    // `.cargo/config.toml` alongside the manifest so subsequent cargo
    // invocations (starting with `discover_widgets` below) can resolve
    // unpublished workspace deps via `[patch.crates-io]` redirects.
    // Cargo's config walk starts from the current working directory,
    // so `discover_widgets` runs `cargo metadata` with
    // `current_dir(manifest_dir)` to pick this file up.
    if let Some(source) = &source_path {
        crate::patch_config::write_scratch_cargo_config(&manifest_dir, source)?;
    }

    let discovered = discover::discover_widgets_with_options(
        &manifest_dir,
        &discover::DiscoverOpts {
            features: &args.features,
            no_default_features: args.no_default_features,
            all_features: args.all_features,
        },
    )?;
    let widgets = if native_widgets_override.is_empty() {
        discovered
    } else {
        filter_native_widgets(&app_pkg, &discovered, &native_widgets_override)?
    };
    discover::check_all_collisions(&widgets, BUILTIN_TYPE_NAMES)?;

    // cargo_metadata doesn't surface `[workspace.package].version`
    // separately; the app package version already resolves to it when
    // the app uses `version.workspace = true`, so we use that directly.
    let effective_version = app_pkg.version.to_string();

    let config = generator::WorkspaceConfig {
        app_manifest_dir: &manifest_dir,
        output_dir: &output_dir,
        binary_name: binary_name_override,
        app_name: &app_pkg.name,
        workspace_version: &effective_version,
        source_path,
        widgets: &widgets,
    };
    generator::generate_workspace(&config)?;

    // Invoke cargo build in the generated workspace.
    let mut cmd =
        std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cmd.current_dir(&output_dir).arg("build");
    cmd.env("CARGO_TARGET_DIR", output_dir.join("target"));
    if args.release {
        cmd.arg("--release");
    }
    if args.verbose {
        cmd.arg("--verbose");
        eprintln!(
            "running: CARGO_TARGET_DIR={} cargo build{}",
            output_dir.join("target").display(),
            if args.release { " --release" } else { "" }
        );
    }
    let status = cmd.status().with_context(|| "failed to run cargo build")?;
    if !status.success() {
        return Err(crate::Error::CargoBuildFailed(status).into());
    }
    println!(
        "plushie: generated renderer workspace at {} ({} widgets registered)",
        output_dir.display(),
        widgets.len()
    );
    let installed = install_built_renderer(&manifest_dir, args)?;
    println!("plushie: installed renderer at {}", installed.display());
    Ok(())
}

fn install_built_renderer(manifest_dir: &Path, args: &BuildArgs) -> Result<PathBuf> {
    let cwd = std::env::current_dir().with_context(|| "resolve current directory")?;
    let built = resolve_built_binary(
        manifest_dir,
        &RunArgs {
            watch: false,
            release: args.release,
            verbose: args.verbose,
            manifest_path: Some(manifest_dir.join("Cargo.toml")),
        },
    )?;
    if !built.is_file() {
        return Err(anyhow::anyhow!(
            "cargo build succeeded but renderer binary is missing at `{}`",
            built.display()
        ));
    }
    let dest = cwd.join("bin").join(platform::renderer_name());
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&built, &dest).with_context(|| {
        format!(
            "copy built renderer `{}` to `{}`",
            built.display(),
            dest.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)?;
    }
    Ok(dest)
}

/// WASM build path: delegate to `wasm-pack` against the
/// `plushie-renderer-wasm` crate. The source comes from
/// `PLUSHIE_RUST_SOURCE_PATH`, the app's `[package.metadata.plushie]
/// source_path` key, a workspace sibling (`..`), or the crates.io
/// registry (downloaded on demand), in that order.
fn cmd_build_wasm(manifest_dir: &Path, args: &BuildArgs) -> Result<()> {
    // Verify wasm-pack up front with a clear message; the command
    // will fail later otherwise with a less obvious IO error.
    if std::process::Command::new("wasm-pack")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| !s.success())
        .unwrap_or(true)
    {
        return Err(anyhow::anyhow!(
            "`wasm-pack` not found on PATH. Install it with \
             `cargo install wasm-pack` or `curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh`."
        ));
    }

    let crate_dir = resolve_wasm_crate_dir(manifest_dir)?;

    let target = target_dir(manifest_dir);
    let output_dir = args
        .wasm_dir
        .clone()
        .unwrap_or_else(|| target.join("plushie/pkg"));
    std::fs::create_dir_all(&output_dir)?;

    // Redirect cargo's artifact output to a writable location under the
    // app's target dir. Required when crate_dir points into the cargo
    // registry cache, which is read-only by design.
    let wasm_target_dir = target.join("plushie/wasm-target");

    let mut cmd = std::process::Command::new("wasm-pack");
    cmd.arg("build")
        .arg(&crate_dir)
        .args(["--target", "web"])
        .args(["--out-dir", &output_dir.display().to_string()])
        .env("CARGO_TARGET_DIR", &wasm_target_dir);
    if args.release {
        cmd.arg("--release");
    } else {
        cmd.arg("--dev");
    }
    if args.verbose {
        eprintln!(
            "running: wasm-pack build {crate_dir} --target web --out-dir {out}{profile}",
            crate_dir = crate_dir.display(),
            out = output_dir.display(),
            profile = if args.release { " --release" } else { " --dev" },
        );
    }

    let status = cmd.status().with_context(|| "failed to run wasm-pack")?;
    if !status.success() {
        return Err(crate::Error::CargoBuildFailed(status).into());
    }
    println!("plushie: wasm bundle generated at {}", output_dir.display());
    Ok(())
}

/// Resolve the `plushie-renderer-wasm` crate directory.
///
/// Priority:
///
/// 1. `PLUSHIE_RUST_SOURCE_PATH` env var (pointing at a plushie-rust
///    checkout root).
/// 2. `[package.metadata.plushie].source_path` on the caller's
///    manifest.
/// 3. A sibling workspace at `..` (the convention for developing
///    multiple plushie-* repos in parallel).
/// 4. Download `plushie-renderer-wasm` from crates.io and return the
///    extracted source from the cargo registry cache.
fn resolve_wasm_crate_dir(manifest_dir: &Path) -> Result<PathBuf> {
    if let Some(env) = std::env::var_os("PLUSHIE_RUST_SOURCE_PATH") {
        let path = PathBuf::from(env);
        let root = std::fs::canonicalize(&path).with_context(|| {
            format!(
                "PLUSHIE_RUST_SOURCE_PATH `{}` does not exist",
                path.display()
            )
        })?;
        return Ok(root.join("crates/plushie-renderer-wasm"));
    }

    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .no_deps()
        .exec()
        .with_context(|| "cargo metadata (no-deps) failed")?;
    let app_pkg = app_package_from_metadata(&metadata, &manifest_dir.join("Cargo.toml")).ok();
    if let Some(pkg) = app_pkg
        && let Some(meta_path) = pkg
            .metadata
            .get("plushie")
            .and_then(|v| v.get("source_path"))
            .and_then(|v| v.as_str())
    {
        let resolved = manifest_dir.join(meta_path);
        if let Ok(abs) = std::fs::canonicalize(&resolved) {
            return Ok(abs.join("crates/plushie-renderer-wasm"));
        }
    }

    let sibling = manifest_dir.join("..");
    if sibling.join("crates/plushie-renderer-wasm").is_dir() {
        let abs = std::fs::canonicalize(&sibling).unwrap_or(sibling);
        return Ok(abs.join("crates/plushie-renderer-wasm"));
    }

    let version = resolve_renderer_version(manifest_dir)?;
    let scratch_dir = target_dir(manifest_dir).join("plushie-wasm-scratch");
    fetch_wasm_crate_from_registry(&scratch_dir, &version)
}

/// Download `plushie-renderer-wasm` from crates.io into the cargo
/// registry cache and return the extracted source directory.
///
/// Creates a minimal scratch workspace at `scratch_dir`, runs
/// `cargo metadata` to trigger download and extraction, then returns
/// the source path from the registry cache. Subsequent calls hit the
/// cache without re-downloading.
fn fetch_wasm_crate_from_registry(scratch_dir: &Path, version: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(scratch_dir)?;
    generator::write_if_changed(
        &scratch_dir.join("Cargo.toml"),
        &format!(
            "# Auto-generated by cargo-plushie. Do not edit.\n\
             [package]\nname = \"plushie-wasm-scratch\"\nversion = \"0.0.1\"\nedition = \"2024\"\n\n\
             [dependencies]\nplushie-renderer-wasm = \"{version}\"\n"
        ),
    )?;

    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(scratch_dir.join("Cargo.toml"))
        .exec()
        .with_context(|| {
            format!("failed to fetch plushie-renderer-wasm {version} from crates.io")
        })?;

    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name == "plushie-renderer-wasm" && p.version.to_string() == version)
        .ok_or_else(|| {
            anyhow::anyhow!("plushie-renderer-wasm {version} not found in registry after fetch")
        })?;

    let crate_dir = pkg
        .manifest_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("unexpected manifest_path for plushie-renderer-wasm"))?
        .to_path_buf()
        .into_std_path_buf();

    Ok(crate_dir)
}

fn cmd_download(args: &DownloadArgs) -> Result<()> {
    let project_dir = std::env::current_dir().with_context(|| "resolve current directory")?;
    let (version, source_configured) =
        resolve_download_version_and_manifest_state(args.required_version.as_deref(), args)?;

    download_renderer(&project_dir, &version, args.force, source_configured)
}

fn resolve_download_version_and_manifest_state(
    required_version: Option<&str>,
    args: &DownloadArgs,
) -> Result<(String, bool)> {
    if let Some(version) = required_version {
        let version = version.trim();
        if version.is_empty() {
            anyhow::bail!("--required-version must not be empty");
        }
        let source_configured = if args.manifest_path.is_some() {
            check_download_manifest(args, None)?
        } else {
            false
        };
        return Ok((version.to_string(), source_configured));
    }

    let manifest_dir = resolve_manifest_dir(args.manifest_path.as_ref())?;
    let version = resolve_renderer_version(&manifest_dir)?;
    let source_configured = check_download_manifest(args, Some(&manifest_dir))?;
    Ok((version, source_configured))
}

fn check_download_manifest(args: &DownloadArgs, manifest_dir: Option<&Path>) -> Result<bool> {
    let manifest_dir = match manifest_dir {
        Some(path) => path.to_path_buf(),
        None => resolve_manifest_dir(args.manifest_path.as_ref())?,
    };
    let app_pkg = load_app_package_no_deps(&manifest_dir)?;
    let source_path = resolve_source_path(&manifest_dir, &app_pkg)?;
    if source_path.is_some() && !args.force {
        anyhow::bail!(
            "refusing to replace a source-configured renderer with a downloaded release; \
             unset the source path or pass --force if this project should use release binaries"
        );
    }
    if let Some(source) = &source_path {
        crate::patch_config::write_scratch_cargo_config(&manifest_dir, source)?;
    }

    let widgets = discover::discover_widgets(&manifest_dir)?;
    download::refuse_if_native_widgets(&widgets)?;
    Ok(source_path.is_some())
}

fn download_renderer(
    project_dir: &Path,
    version: &str,
    force: bool,
    source_configured: bool,
) -> Result<()> {
    let release_base_url = download::release_base_url()?;
    download_renderer_with_base_url(
        project_dir,
        version,
        force,
        source_configured,
        &release_base_url,
    )
}

fn download_renderer_with_base_url(
    project_dir: &Path,
    version: &str,
    force: bool,
    source_configured: bool,
    release_base_url: &str,
) -> Result<()> {
    let dl_target =
        download::DownloadTarget::new_with_base_url(project_dir, version, release_base_url)?;
    download_tool_with_base_url(&dl_target, "renderer", version, force, source_configured)
}

fn download_launcher_with_base_url(
    project_dir: &Path,
    version: &str,
    force: bool,
    source_configured: bool,
    release_base_url: &str,
) -> Result<()> {
    let dl_target =
        download::DownloadTarget::launcher_with_base_url(project_dir, version, release_base_url)?;
    download_tool_with_base_url(&dl_target, "launcher", version, force, source_configured)
}

fn download_plushie_with_base_url(
    project_dir: &Path,
    version: &str,
    force: bool,
    source_configured: bool,
    release_base_url: &str,
) -> Result<()> {
    let dl_target =
        download::DownloadTarget::plushie_with_base_url(project_dir, version, release_base_url)?;
    if current_exe_matches_path(&dl_target.binary_path) {
        println!(
            "plushie: keeping currently running plushie tool at {}",
            dl_target.binary_path.display()
        );
        return Ok(());
    }
    download_tool_with_base_url(&dl_target, "plushie", version, force, source_configured)
}

fn current_exe_matches_path(path: &Path) -> bool {
    let Ok(current) = std::env::current_exe() else {
        return false;
    };
    let Ok(current) = current.canonicalize() else {
        return false;
    };
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    current == path
}

fn download_tool_with_base_url(
    dl_target: &download::DownloadTarget,
    label: &str,
    version: &str,
    force: bool,
    source_configured: bool,
) -> Result<()> {
    println!(
        "plushie: resolved download platform as {}-{}",
        platform::os_name(),
        platform::arch_name()
    );

    if dl_target.binary_path.exists() {
        if source_configured && !force {
            anyhow::bail!(
                "refusing to replace a source-configured {label} at {}; \
                 unset the source path or pass --force if this project should use release binaries",
                dl_target.binary_path.display()
            );
        }
        match tool_identity::probe_tool_identity(&dl_target.binary_path, Duration::from_secs(2)) {
            Ok(identity) if tool_identity::is_downloaded_release(&identity) => {
                if identity.plushie_rust_version == version {
                    println!(
                        "plushie: replacing existing downloaded {label} {} at {}",
                        identity.plushie_rust_version,
                        dl_target.binary_path.display()
                    );
                } else {
                    println!(
                        "plushie: replacing stale downloaded {label} {} with {} at {}",
                        identity.plushie_rust_version,
                        version,
                        dl_target.binary_path.display()
                    );
                }
            }
            Ok(identity) if force => {
                println!(
                    "plushie: replacing existing {} {label} {} at {}",
                    identity.source.kind,
                    identity.plushie_rust_version,
                    dl_target.binary_path.display()
                );
            }
            Ok(identity) => {
                anyhow::bail!(
                    "refusing to replace existing {} {label} {} at {}; \
                     pass --force if this project should use release binaries",
                    identity.source.kind,
                    identity.plushie_rust_version,
                    dl_target.binary_path.display()
                );
            }
            Err(error) if force => {
                println!(
                    "plushie: replacing existing {label} with unreadable identity at {} ({error})",
                    dl_target.binary_path.display()
                );
            }
            Err(error) => {
                anyhow::bail!(
                    "refusing to replace existing {label} at {} because its Plushie identity \
                     could not be read ({error}); pass --force if this project should use release binaries",
                    dl_target.binary_path.display()
                );
            }
        }
    }

    println!("plushie: downloading {}", dl_target.binary_url);
    let bytes = download::fetch_bytes(&dl_target.binary_url)?;
    let sidecar_bytes = download::fetch_bytes(&dl_target.sha256_url)?;
    let sidecar_str =
        std::str::from_utf8(&sidecar_bytes).context("sha256 sidecar was not UTF-8")?;
    download::verify_sha256(&bytes, sidecar_str)?;
    download::install_binary(&dl_target, &bytes, sidecar_str)?;
    println!(
        "plushie: installed {label} at {}",
        dl_target.binary_path.display()
    );
    Ok(())
}

fn resolve_renderer_version(manifest_dir: &Path) -> Result<String> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .current_dir(manifest_dir)
        .exec()
        .with_context(|| "cargo metadata failed")?;
    metadata
        .packages
        .iter()
        .find(|p| p.name == "plushie-renderer-lib")
        .map(|p| p.version.to_string())
        .or_else(|| {
            metadata
                .packages
                .iter()
                .find(|p| p.name == "plushie")
                .map(|p| p.version.to_string())
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "unable to determine renderer version: neither `plushie-renderer-lib` \
                 nor `plushie` appears in the dep graph"
            )
        })
}

fn load_app_package_no_deps(manifest_dir: &Path) -> Result<cargo_metadata::Package> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .no_deps()
        .exec()
        .with_context(|| "cargo metadata (no-deps) failed")?;
    app_package_from_metadata(&metadata, &manifest_dir.join("Cargo.toml"))
}

fn app_package_from_metadata(
    metadata: &cargo_metadata::Metadata,
    manifest_path: &Path,
) -> Result<cargo_metadata::Package> {
    let expected = std::fs::canonicalize(manifest_path)
        .with_context(|| format!("manifest path `{}` not found", manifest_path.display()))?;
    for package in &metadata.packages {
        let candidate = package.manifest_path.as_std_path();
        if std::fs::canonicalize(candidate)
            .map(|path| path == expected)
            .unwrap_or(false)
        {
            return Ok(package.clone());
        }
    }

    Err(anyhow::anyhow!(
        "`{}` is not a package manifest; pass the Cargo.toml for the Rust app package",
        manifest_path.display()
    ))
}

fn apply_feature_args(
    cmd: &mut cargo_metadata::MetadataCommand,
    features: &[String],
    no_default_features: bool,
    all_features: bool,
) {
    if !features.is_empty() {
        cmd.features(CargoOpt::SomeFeatures(features.to_vec()));
    }
    if no_default_features {
        cmd.features(CargoOpt::NoDefaultFeatures);
    }
    if all_features {
        cmd.features(CargoOpt::AllFeatures);
    }
}

fn resolve_source_path(
    manifest_dir: &Path,
    app_pkg: &cargo_metadata::Package,
) -> Result<Option<PathBuf>> {
    let source_path_env = std::env::var_os("PLUSHIE_RUST_SOURCE_PATH").map(PathBuf::from);
    let source_path_meta = app_pkg
        .metadata
        .get("plushie")
        .and_then(|v| v.get("source_path"))
        .and_then(|v| v.as_str())
        .map(|path| manifest_dir.join(path));

    let Some(path) = source_path_env.or(source_path_meta) else {
        return Ok(None);
    };
    let source = std::fs::canonicalize(&path)
        .with_context(|| format!("plushie source_path `{}` does not exist", path.display()))?;
    Ok(Some(source))
}

fn cmd_run(args: &RunArgs) -> Result<()> {
    let manifest_dir = resolve_manifest_dir(args.manifest_path.as_ref())?;

    // Step 1: build the custom renderer. Reuse the full build flow so
    // widget discovery + collision checks happen in one place.
    let build = BuildArgs {
        release: args.release,
        verbose: args.verbose,
        manifest_path: args.manifest_path.clone(),
        wasm: false,
        wasm_dir: None,
        features: Vec::new(),
        no_default_features: false,
        all_features: false,
    };
    cmd_build(&build)?;

    // Pin PLUSHIE_BINARY_PATH to the binary we just built for the
    // profile the user asked for. Without this, the SDK's wire-mode
    // discovery probes `release/` before `debug/` regardless of which
    // profile `cargo run` is using, so a stale `release/` binary plus
    // `cargo plushie run` (debug) would silently launch the release
    // renderer. Passing the exact path removes the ambiguity.
    //
    let pinned = resolve_built_binary(&manifest_dir, args)?;
    if !pinned.is_file() {
        return Err(anyhow::anyhow!(
            "expected freshly built renderer at `{}` but it was not found",
            pinned.display()
        ));
    }

    // Step 2: hand off to either cargo-watch (preferred when installed;
    // it handles restart-on-change cleanly) or a single cargo run.
    if args.watch && cargo_watch_available() {
        run_with_cargo_watch(&manifest_dir, args, &pinned)
    } else if args.watch {
        eprintln!(
            "plushie: `cargo-watch` not found; install with `cargo install cargo-watch` \
             for --watch, falling back to single `cargo run`"
        );
        run_cargo_run(&manifest_dir, args, &pinned)
    } else {
        run_cargo_run(&manifest_dir, args, &pinned)
    }
}

/// Resolve the freshly-built renderer's binary path for the profile
/// specified on `cargo plushie run`.
///
/// Uses the same logic `cmd_build` uses to derive the binary name so
/// the two stay in sync.
fn resolve_built_binary(manifest_dir: &Path, args: &RunArgs) -> Result<PathBuf> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .no_deps()
        .exec()
        .with_context(|| "cargo metadata (no-deps) failed")?;
    let app_pkg = app_package_from_metadata(&metadata, &manifest_dir.join("Cargo.toml"))?;

    let binary_name_override = app_pkg
        .metadata
        .get("plushie")
        .and_then(|v| v.get("binary_name"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let bin_name = binary_name_override
        .unwrap_or_else(|| format!("{}-renderer", app_pkg.name.replace('_', "-")));

    let profile_dir = if args.release { "release" } else { "debug" };
    let target = target_dir(manifest_dir);
    let binary = target
        .join("plushie-renderer/target")
        .join(profile_dir)
        .join(if cfg!(windows) {
            format!("{bin_name}.exe")
        } else {
            bin_name
        });
    Ok(binary)
}

/// Check whether `cargo-watch` (invoked via `cargo watch`) is
/// installed. A missing binary maps to `status != 0` from
/// `cargo --list`; we do a simple `cargo watch --version` probe.
fn cargo_watch_available() -> bool {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    std::process::Command::new(cargo)
        .args(["watch", "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Single-shot `cargo run` against the app crate.
fn run_cargo_run(manifest_dir: &std::path::Path, args: &RunArgs, pinned: &Path) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = std::process::Command::new(cargo);
    cmd.current_dir(manifest_dir).arg("run");
    if args.release {
        cmd.arg("--release");
    }
    if args.verbose {
        cmd.arg("--verbose");
        eprintln!(
            "running: cargo run{}",
            if args.release { " --release" } else { "" }
        );
    }
    cmd.env("PLUSHIE_BINARY_PATH", pinned);
    let status = cmd.status().with_context(|| "failed to run cargo run")?;
    if !status.success() {
        return Err(crate::Error::CargoBuildFailed(status).into());
    }
    Ok(())
}

/// Loop-forever `cargo watch` invocation that rebuilds the renderer
/// workspace and re-runs the app on app-src change.
///
/// The watch command chain (`-s 'cargo plushie build && cargo run'`)
/// keeps the renderer binary in sync with any app-side widget changes
/// that slip into the app crate itself, then restarts the app so
/// `PLUSHIE_BINARY_PATH` discovery picks up the fresh binary.
fn run_with_cargo_watch(
    manifest_dir: &std::path::Path,
    args: &RunArgs,
    pinned: &Path,
) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    // `cargo watch -w src -s '<cmd>'` reruns <cmd> on src/ changes.
    // We chain `cargo plushie build` before each `cargo run` so widget
    // rebuilds happen in-band.
    let profile = if args.release { " --release" } else { "" };
    let verbose = if args.verbose { " --verbose" } else { "" };
    let shell_cmd = format!("cargo plushie build{profile}{verbose} && cargo run{profile}{verbose}");
    let mut cmd = std::process::Command::new(cargo);
    cmd.current_dir(manifest_dir)
        .args(["watch", "-w", "src", "-s", &shell_cmd]);
    if args.verbose {
        eprintln!("running: cargo watch -w src -s '{shell_cmd}'");
    }
    cmd.env("PLUSHIE_BINARY_PATH", pinned);
    let status = cmd.status().with_context(|| "failed to run cargo watch")?;
    if !status.success() {
        return Err(crate::Error::CargoBuildFailed(status).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BUILTIN_TYPE_NAMES, check_native_tools, download_launcher_with_base_url,
        download_plushie_with_base_url, download_renderer_with_base_url, resolve_required_version,
        target_dir_from,
    };
    use crate::platform;
    use sha2::{Digest, Sha256};
    use std::path::{Path, PathBuf};

    #[test]
    fn builtin_type_names_come_from_core() {
        assert!(std::ptr::eq(
            BUILTIN_TYPE_NAMES.as_ptr(),
            plushie_core::BUILTIN_TYPE_NAMES.as_ptr()
        ));
    }

    #[test]
    fn download_renderer_installs_from_file_release() {
        let release = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let version = "0.0.0-test";
        let version_dir = release.path().join(format!("v{version}"));
        std::fs::create_dir_all(&version_dir).unwrap();

        let artifact = version_dir.join(platform::download_name());
        let bytes = b"fake renderer";
        std::fs::write(&artifact, bytes).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let sidecar = format!("{:x}  {}\n", hasher.finalize(), platform::download_name());
        std::fs::write(format!("{}.sha256", artifact.display()), sidecar).unwrap();

        download_renderer_with_base_url(
            project.path(),
            version,
            false,
            false,
            &format!("file://{}", release.path().display()),
        )
        .unwrap();

        let installed = project.path().join("bin").join(platform::renderer_name());
        assert_eq!(std::fs::read(installed).unwrap(), bytes);
    }

    #[test]
    fn download_launcher_installs_from_file_release() {
        let release = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let version = "0.0.0-test";
        let version_dir = release.path().join(format!("v{version}"));
        std::fs::create_dir_all(&version_dir).unwrap();

        let artifact = version_dir.join(platform::launcher_download_name());
        let bytes = b"fake launcher";
        std::fs::write(&artifact, bytes).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let sidecar = format!(
            "{:x}  {}\n",
            hasher.finalize(),
            platform::launcher_download_name()
        );
        std::fs::write(format!("{}.sha256", artifact.display()), sidecar).unwrap();

        download_launcher_with_base_url(
            project.path(),
            version,
            false,
            false,
            &format!("file://{}", release.path().display()),
        )
        .unwrap();

        let installed = project.path().join("bin").join(platform::launcher_name());
        assert_eq!(std::fs::read(installed).unwrap(), bytes);
    }

    #[test]
    fn download_plushie_installs_from_file_release() {
        let release = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let version = "0.0.0-test";
        let version_dir = release.path().join(format!("v{version}"));
        std::fs::create_dir_all(&version_dir).unwrap();

        let artifact = version_dir.join(platform::plushie_download_name());
        let bytes = b"fake plushie";
        std::fs::write(&artifact, bytes).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let sidecar = format!(
            "{:x}  {}\n",
            hasher.finalize(),
            platform::plushie_download_name()
        );
        std::fs::write(format!("{}.sha256", artifact.display()), sidecar).unwrap();

        download_plushie_with_base_url(
            project.path(),
            version,
            false,
            false,
            &format!("file://{}", release.path().display()),
        )
        .unwrap();

        let installed = project.path().join("bin").join(platform::plushie_name());
        assert_eq!(std::fs::read(installed).unwrap(), bytes);
    }

    #[test]
    fn target_dir_normalizes_relative_cargo_target_dir_from_invocation_dir() {
        assert_eq!(
            target_dir_from(
                Some(PathBuf::from("relative-target")),
                Path::new("/caller"),
                Path::new("/app")
            ),
            Path::new("/caller").join("relative-target")
        );
        assert_eq!(
            target_dir_from(None, Path::new("/caller"), Path::new("/app")),
            Path::new("/app/target")
        );
    }

    #[test]
    fn explicit_required_version_is_used_without_cargo_metadata() {
        assert_eq!(
            resolve_required_version(Some(" 0.7.1 "), None).unwrap(),
            "0.7.1"
        );
    }

    #[test]
    fn tool_check_reports_missing_native_tools_with_sync_hint() {
        let dir = tempfile::tempdir().unwrap();
        let report = check_native_tools(dir.path(), env!("CARGO_PKG_VERSION"), false);

        assert!(!report.ok);
        assert!(report.issues.iter().any(|issue| {
            issue.contains("plushie is missing") && issue.contains("bin/plushie tools sync")
        }));
        assert!(report.issues.iter().any(|issue| {
            issue.contains("renderer is missing") && issue.contains("bin/plushie tools sync")
        }));
        assert!(report.issues.iter().any(|issue| {
            issue.contains("launcher is missing") && issue.contains("bin/plushie tools sync")
        }));
    }
}
