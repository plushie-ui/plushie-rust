//! Cargo subcommand entry point.
//!
//! Invoked as either `cargo plushie <sub>` (the Cargo subcommand
//! convention: Cargo rewrites the argv to `cargo-plushie plushie
//! <sub>`) or `cargo-plushie <sub>` directly. Both shapes dispatch
//! through the same clap parser below.

use anyhow::{Context, Result};
use cargo_plushie::{discover, download, generator, platform};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Built-in renderer widget type names. Populated at compile time by
/// the plushie-widget-sdk const so the build tool and the renderer
/// share a single source of truth.
const BUILTIN_TYPE_NAMES: &[&str] = plushie_widget_sdk_builtin_names::LIST;

/// Private module providing the constant indirectly so we avoid a
/// direct `plushie-widget-sdk` dependency in the build tool (which
/// would re-pull iced). The list is duplicated by the drift test in
/// `plushie-widget-sdk/tests/builtin_type_names.rs`.
mod plushie_widget_sdk_builtin_names {
    /// Sorted list of built-in widget type names registered by the
    /// stock renderer's iced widget set.
    pub const LIST: &[&str] = &[
        "button",
        "canvas",
        "checkbox",
        "column",
        "combo_box",
        "container",
        "float",
        "grid",
        "image",
        "keyed_column",
        "markdown",
        "overlay",
        "pane_grid",
        "pick_list",
        "pin",
        "pointer_area",
        "progress_bar",
        "qr_code",
        "radio",
        "responsive",
        "rich",
        "rich_text",
        "row",
        "rule",
        "scrollable",
        "sensor",
        "slider",
        "space",
        "stack",
        "svg",
        "table",
        "text",
        "text_editor",
        "text_input",
        "themer",
        "toggler",
        "tooltip",
        "vertical_slider",
        "window",
    ];
}

#[derive(Parser, Debug)]
#[command(
    name = "cargo-plushie",
    bin_name = "cargo",
    about = "Cargo subcommand for Plushie renderer build + download",
    version
)]
enum Cli {
    /// The `plushie` subcommand wrapper (matches the argv shape that
    /// Cargo passes when the user runs `cargo plushie <sub>`).
    Plushie(PlushieArgs),
}

#[derive(Args, Debug)]
struct PlushieArgs {
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
}

#[derive(Args, Debug)]
struct DownloadArgs {
    /// Force overwrite of an existing binary.
    #[arg(long)]
    force: bool,
    /// Path to the app crate manifest (defaults to `./Cargo.toml`).
    #[arg(long)]
    manifest_path: Option<PathBuf>,
}

fn main() -> Result<()> {
    // The first argv element after the binary name is the subcommand
    // shape Cargo hands us (`plushie`). Accept both shapes: when run
    // directly as `cargo-plushie build` we don't have the extra
    // `plushie` word; rewrite argv to make clap's parsing uniform.
    let mut argv: Vec<String> = std::env::args().collect();
    if argv.len() >= 2 && argv[1] != "plushie" {
        argv.insert(1, "plushie".to_string());
    }
    let cli = Cli::parse_from(argv);
    let Cli::Plushie(args) = cli;
    match args.command {
        PlushieSubcommand::Build(b) => cmd_build(&b),
        PlushieSubcommand::Download(d) => cmd_download(&d),
    }
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
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target"))
}

fn cmd_build(args: &BuildArgs) -> Result<()> {
    let manifest_dir = resolve_manifest_dir(args.manifest_path.as_ref())?;
    let target = target_dir(&manifest_dir);
    let output_dir = target.join("plushie-renderer");
    std::fs::create_dir_all(&output_dir)?;

    let widgets = discover::discover_widgets(&manifest_dir)?;
    discover::check_all_collisions(&widgets, BUILTIN_TYPE_NAMES)?;

    // Resolve app package metadata (name + version + optional
    // [package.metadata.plushie] overrides) from the caller's manifest.
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .no_deps()
        .exec()
        .with_context(|| "cargo metadata (no-deps) failed")?;
    let root_id = metadata
        .resolve
        .as_ref()
        .and_then(|r| r.root.as_ref())
        .cloned();
    let app_pkg = match root_id {
        Some(id) => metadata.packages.iter().find(|p| p.id == id).cloned(),
        None => metadata.packages.first().cloned(),
    };
    let app_pkg = app_pkg.ok_or_else(|| anyhow::anyhow!("no root package in cargo metadata"))?;

    let binary_name_override = app_pkg
        .metadata
        .get("plushie")
        .and_then(|v| v.get("binary_name"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // PLUSHIE_SOURCE_PATH env wins over any per-package override; both
    // resolve to the absolute path to the plushie-rust checkout root.
    let source_path_env = std::env::var_os("PLUSHIE_SOURCE_PATH").map(PathBuf::from);
    let source_path_meta = app_pkg
        .metadata
        .get("plushie")
        .and_then(|v| v.get("source_path"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let source_path = source_path_env.or(source_path_meta).and_then(|p| {
        std::fs::canonicalize(&p).ok().or_else(|| {
            eprintln!(
                "warning: plushie source_path `{}` does not exist; ignoring",
                p.display()
            );
            None
        })
    });

    let workspace_version = metadata.workspace_metadata.to_string();
    // cargo_metadata doesn't give us [workspace.package].version; fall
    // back to the app package version, which equals the workspace
    // version when the app is inside a workspace using `.workspace = true`.
    let _ = workspace_version;
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
    if args.release {
        cmd.arg("--release");
    }
    if args.verbose {
        cmd.arg("--verbose");
        eprintln!(
            "running: cargo build{}",
            if args.release { " --release" } else { "" }
        );
    }
    let status = cmd.status().with_context(|| "failed to run cargo build")?;
    if !status.success() {
        return Err(cargo_plushie::Error::CargoBuildFailed(status).into());
    }
    println!(
        "plushie: generated renderer workspace at {} ({} widgets registered)",
        output_dir.display(),
        widgets.len()
    );
    Ok(())
}

fn cmd_download(args: &DownloadArgs) -> Result<()> {
    let manifest_dir = resolve_manifest_dir(args.manifest_path.as_ref())?;
    let target = target_dir(&manifest_dir);

    // Correctness gate: refuse if custom widgets are present.
    let widgets = discover::discover_widgets(&manifest_dir)?;
    download::refuse_if_native_widgets(&widgets)?;

    // RENDERER_VERSION: the plushie-renderer-lib crate version from the
    // app's dep graph. Required so the download pins to the exact
    // version the SDK negotiates against at handshake time.
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .exec()
        .with_context(|| "cargo metadata failed")?;
    let version = metadata
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
        })?;

    let dl_target = download::DownloadTarget::new(&target, &version);
    println!(
        "plushie: resolved download platform as {}-{}",
        platform::os_name(),
        platform::arch_name()
    );

    if dl_target.binary_path.exists() && !args.force {
        println!(
            "plushie: binary already present at {}; pass --force to re-download",
            dl_target.binary_path.display()
        );
        return Ok(());
    }

    println!("plushie: downloading {}", dl_target.binary_url);
    let bytes = download::fetch_bytes(&dl_target.binary_url)?;
    let sidecar_bytes = download::fetch_bytes(&dl_target.sha256_url)?;
    let sidecar_str =
        std::str::from_utf8(&sidecar_bytes).context("sha256 sidecar was not UTF-8")?;
    download::verify_sha256(&bytes, sidecar_str)?;
    download::install_binary(&dl_target, &bytes, sidecar_str)?;
    println!(
        "plushie: installed renderer at {}",
        dl_target.binary_path.display()
    );
    Ok(())
}
