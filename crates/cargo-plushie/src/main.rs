//! Cargo subcommand entry point.
//!
//! Invoked as either `cargo plushie <sub>` (the Cargo subcommand
//! convention: Cargo rewrites the argv to `cargo-plushie plushie
//! <sub>`) or `cargo-plushie <sub>` directly. Both shapes dispatch
//! through the same clap parser below.

use anyhow::{Context, Result};
use cargo_plushie::{discover, doctor, download, generator, package, platform, scaffold};
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};

/// Built-in renderer widget type names from `plushie-core`, shared
/// with the widget SDK without pulling iced into this build tool.
const BUILTIN_TYPE_NAMES: &[&str] = plushie_core::BUILTIN_TYPE_NAMES;

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
    /// Build the custom renderer and then run the app binary. The
    /// SDK's wire discovery picks the freshly built renderer up from
    /// `target/plushie-renderer/` without any extra wiring.
    Run(RunArgs),
    /// Build a standalone launcher from a Plushie package manifest.
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
    /// Path to the Plushie package manifest.
    #[arg(long)]
    manifest: PathBuf,
    /// Validate the manifest and payload without building a launcher.
    #[arg(long)]
    validate: bool,
    /// Final launcher output path. Defaults under target/plushie/package/.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Build the generated launcher with the release Cargo profile.
    #[arg(long)]
    release: bool,
    /// Print the underlying cargo command.
    #[arg(long)]
    verbose: bool,
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
        PlushieSubcommand::Run(r) => cmd_run(&r),
        PlushieSubcommand::Package(p) => cmd_package(&p),
        PlushieSubcommand::NewWidget(n) => cmd_new_widget(&n),
        PlushieSubcommand::Init(i) => cmd_init(&i),
        PlushieSubcommand::Doctor(d) => cmd_doctor(&d),
    }
}

fn cmd_package(args: &PackageArgs) -> Result<()> {
    if args.validate {
        let validation = package::validate_package(&args.manifest)?;
        println!(
            "plushie: validated standalone package {} {} ({})",
            validation.app_id, validation.app_version, validation.payload_hash
        );
        return Ok(());
    }

    let result = package::build_launcher(&package::PackageOpts {
        manifest_path: &args.manifest,
        out_path: args.out.as_deref(),
        release: args.release,
        verbose: args.verbose,
    })?;
    println!(
        "plushie: generated standalone launcher at {}",
        result.binary_path.display()
    );
    println!(
        "plushie: launcher crate retained at {}",
        result.launcher_crate_dir.display()
    );
    Ok(())
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
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target"))
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
    discovered: &[cargo_plushie::WidgetMetadata],
    allowlist: &[String],
) -> Result<Vec<cargo_plushie::WidgetMetadata>> {
    use std::collections::HashSet;

    let direct_deps: HashSet<&str> = app_pkg
        .dependencies
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    let discovered_by_name: std::collections::HashMap<&str, &cargo_plushie::WidgetMetadata> =
        discovered
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
        cargo_plushie::patch_config::write_scratch_cargo_config(&manifest_dir, source)?;
    }

    let discovered = discover::discover_widgets(&manifest_dir)?;
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
        return Err(cargo_plushie::Error::CargoBuildFailed(status).into());
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
    let app_pkg = metadata
        .resolve
        .as_ref()
        .and_then(|r| r.root.as_ref())
        .and_then(|id| metadata.packages.iter().find(|p| &p.id == id))
        .or_else(|| metadata.packages.first());
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
    let manifest_dir = resolve_manifest_dir(args.manifest_path.as_ref())?;
    let target = target_dir(&manifest_dir);
    let app_pkg = load_app_package_no_deps(&manifest_dir)?;
    let source_path = resolve_source_path(&manifest_dir, &app_pkg)?;
    if let Some(source) = &source_path {
        cargo_plushie::patch_config::write_scratch_cargo_config(&manifest_dir, source)?;
    }

    // Correctness gate: refuse if custom widgets are present.
    let widgets = discover::discover_widgets(&manifest_dir)?;
    download::refuse_if_native_widgets(&widgets)?;

    // RENDERER_VERSION: the plushie-renderer-lib crate version from the
    // app's dep graph. Required so the download pins to the exact
    // version the SDK negotiates against at handshake time.
    let version = resolve_renderer_version(&manifest_dir)?;

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
    let root_id = metadata
        .resolve
        .as_ref()
        .and_then(|r| r.root.as_ref())
        .cloned();
    let app_pkg = match root_id {
        Some(id) => metadata.packages.iter().find(|p| p.id == id).cloned(),
        None => metadata.packages.first().cloned(),
    };
    app_pkg.ok_or_else(|| anyhow::anyhow!("no root package in cargo metadata"))
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
        return Err(cargo_plushie::Error::CargoBuildFailed(status).into());
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
        return Err(cargo_plushie::Error::CargoBuildFailed(status).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::BUILTIN_TYPE_NAMES;

    #[test]
    fn builtin_type_names_come_from_core() {
        assert!(std::ptr::eq(
            BUILTIN_TYPE_NAMES.as_ptr(),
            plushie_core::BUILTIN_TYPE_NAMES.as_ptr()
        ));
    }
}
