//! File-system watcher that rebuilds the custom renderer on widget
//! source changes and surfaces progress through a [`DevOverlayHandle`].
//!
//! `watch_renderer` discovers widget crates via the caller's cargo
//! metadata (widgets are packages declaring a
//! `[package.metadata.plushie.widget]` table), watches each crate's
//! `src/` directory plus `Cargo.toml`, and re-runs `cargo plushie
//! build` after a debounce window. Output streams to stderr and, when
//! a [`DevOverlayHandle`] is registered, into the in-tree rebuild
//! overlay so the app itself can surface build status.
//!
//! # Scope
//!
//! This is the MVP: the loop rebuilds the custom renderer binary and
//! pushes status into the overlay handle. In-process Bridge restart
//! (to swap renderers on a running app without losing Model state)
//! lands in a follow-on commit once the wire runner grows a graceful
//! reload hook. Today a successful rebuild is visible on the next
//! app launch (or to a future Bridge restart wired into run_wire).

use crate::dev::overlay::{DevOverlayHandle, Status};
use crate::{App, Error, Result};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Configuration for [`watch_renderer`].
#[derive(Debug, Clone)]
pub struct WatchOpts {
    /// Debounce window after which a burst of file events triggers a
    /// single rebuild. 250 ms matches the Elixir dev server's Rust
    /// watcher defaults.
    pub debounce: Duration,
    /// Optional handle the watcher pushes rebuild status to. When set,
    /// the matching [`dev::overlay::inject`](crate::dev::overlay::inject)
    /// call surfaces rebuild state in-tree.
    pub overlay: Option<DevOverlayHandle>,
    /// Build with the `--release` profile (slower rebuilds, faster
    /// renderer). Defaults to debug.
    pub release: bool,
}

impl Default for WatchOpts {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(250),
            overlay: None,
            release: false,
        }
    }
}

/// Entry point: watch widget crates, rebuild on change, then hand off
/// to [`crate::run`] once the initial build succeeds.
///
/// When no widget crates are registered in cargo metadata, no watcher
/// is started and this returns the result of [`crate::run`] directly
/// (no dev-only overhead when there's nothing to rebuild).
///
/// # Errors
///
/// - [`Error::InvalidSettings`] on cargo metadata failures.
/// - Whatever [`crate::run`] returns once the initial rebuild is done
///   and the app hands off to the runner.
pub fn watch_renderer<A: App>() -> Result {
    watch_renderer_with_opts::<A>(WatchOpts::default())
}

/// Variant of [`watch_renderer`] that accepts custom options (debounce,
/// overlay handle, release profile). Use this when wiring the in-tree
/// overlay into the runtime view tree.
///
/// # Errors
///
/// Same as [`watch_renderer`].
pub fn watch_renderer_with_opts<A: App>(opts: WatchOpts) -> Result {
    let crates = discover_widget_crates()?;
    if crates.is_empty() {
        log::info!("plushie dev: no widget crates declared; running without watcher");
        return crate::run::<A>();
    }

    log::info!("plushie dev: watching {} widget crate(s)", crates.len());
    for c in &crates {
        log::info!("  - {} at {}", c.name, c.root.display());
    }

    // Initial build so the renderer is up to date before the app starts.
    run_build(&opts);

    spawn_watch_thread(crates, opts.clone());
    crate::run::<A>()
}

/// Widget crate metadata extracted from `cargo metadata`.
#[derive(Debug, Clone)]
struct WidgetCrate {
    /// Package name.
    name: String,
    /// Absolute path to the crate's root directory (the one
    /// containing `Cargo.toml`).
    root: PathBuf,
}

/// Walk the current cargo metadata and return every package that
/// declares `[package.metadata.plushie.widget]`.
fn discover_widget_crates() -> std::result::Result<Vec<WidgetCrate>, Error> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .exec()
        .map_err(|e| Error::InvalidSettings(format!("cargo metadata failed: {e}")))?;

    let mut out = Vec::new();
    for pkg in &metadata.packages {
        if pkg
            .metadata
            .get("plushie")
            .and_then(|v| v.get("widget"))
            .is_none()
        {
            continue;
        }
        let manifest = PathBuf::from(pkg.manifest_path.clone());
        let Some(root) = manifest.parent().map(Path::to_path_buf) else {
            continue;
        };
        out.push(WidgetCrate {
            name: pkg.name.to_string(),
            root,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Spawn a background OS thread that owns the notify watcher and
/// debounce state. The main thread continues into `crate::run`; the
/// watcher lives for the lifetime of the process.
fn spawn_watch_thread(crates: Vec<WidgetCrate>, opts: WatchOpts) {
    std::thread::Builder::new()
        .name("plushie-dev-watch".to_string())
        .spawn(move || {
            if let Err(e) = watch_loop(&crates, &opts) {
                log::warn!("plushie dev: watcher stopped: {e}");
            }
        })
        .expect("failed to spawn plushie-dev-watch thread");
}

/// Block on the notify channel, debounce events into rebuild windows,
/// and invoke `cargo plushie build` once a window elapses.
fn watch_loop(
    crates: &[WidgetCrate],
    opts: &WatchOpts,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(tx)?;

    for c in crates {
        let src = c.root.join("src");
        if src.is_dir() {
            watcher.watch(&src, RecursiveMode::Recursive)?;
        }
        let manifest = c.root.join("Cargo.toml");
        if manifest.is_file() {
            // File-watchers don't love watching individual files on
            // every backend; watching the manifest's parent with
            // NonRecursive is the robust fallback. But the parent is
            // already covered by the src/ watch above... we want the
            // root dir to see manifest edits.
            watcher.watch(&c.root, RecursiveMode::NonRecursive)?;
        }
    }

    let mut pending_since: Option<Instant> = None;
    loop {
        // Block for the next event with a timeout so we can fire the
        // rebuild when the debounce window expires even if no new
        // events arrive.
        let deadline = pending_since.map(|t| {
            let elapsed = t.elapsed();
            if elapsed >= opts.debounce {
                Duration::ZERO
            } else {
                opts.debounce - elapsed
            }
        });
        let recv = match deadline {
            Some(d) => rx.recv_timeout(d),
            None => rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };

        match recv {
            Ok(Ok(event)) => {
                if is_rebuild_trigger(&event) {
                    pending_since = Some(Instant::now());
                }
            }
            Ok(Err(e)) => {
                log::warn!("plushie dev: watcher error: {e}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Debounce elapsed; fire rebuild.
                pending_since = None;
                run_build(opts);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Ok(());
            }
        }
    }
}

/// True for events the watcher should treat as "rebuild needed":
/// create/modify/remove of `.rs` files or `Cargo.toml` edits inside
/// a watched crate. Filters out noisy access/attribute changes.
fn is_rebuild_trigger(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && event.paths.iter().any(|p| is_rust_source_path(p))
}

fn is_rust_source_path(path: &Path) -> bool {
    // Skip anything under `target/` so build output doesn't trigger
    // another build.
    if path.components().any(|c| c.as_os_str() == "target") {
        return false;
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => true,
        _ => path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "Cargo.toml"),
    }
}

/// Run `cargo plushie build` once, streaming stdout/stderr to the
/// caller's terminal and updating the overlay handle (if any) with
/// the result.
fn run_build(opts: &WatchOpts) {
    if let Some(h) = &opts.overlay {
        h.publish(Status::Rebuilding, "");
    }

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = std::process::Command::new(cargo);
    cmd.arg("plushie").arg("build");
    if opts.release {
        cmd.arg("--release");
    }

    log::info!("plushie dev: running cargo plushie build");
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            let msg = format!("cargo plushie build failed to spawn: {e}");
            log::warn!("plushie dev: {msg}");
            if let Some(h) = &opts.overlay {
                h.publish(Status::Failed, msg);
            }
            return;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let combined = if stderr.is_empty() {
        stdout.clone()
    } else if stdout.is_empty() {
        stderr.clone()
    } else {
        format!("{stdout}\n{stderr}")
    };

    // Always tee to the terminal so tail-f-stderr workflows keep working.
    eprint!("{stderr}");
    if !stdout.is_empty() {
        eprintln!("{stdout}");
    }

    if output.status.success() {
        log::info!("plushie dev: rebuild succeeded");
        if let Some(h) = &opts.overlay {
            h.publish(Status::Success, combined);
        }
    } else {
        log::warn!("plushie dev: rebuild failed (status {:?})", output.status);
        if let Some(h) = &opts.overlay {
            h.publish(Status::Failed, combined);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_source_path_detects_rs_and_cargo_toml() {
        assert!(is_rust_source_path(Path::new("/crate/src/lib.rs")));
        assert!(is_rust_source_path(Path::new("/crate/Cargo.toml")));
        assert!(!is_rust_source_path(Path::new("/crate/README.md")));
    }

    #[test]
    fn rust_source_path_skips_target_dir() {
        assert!(!is_rust_source_path(Path::new(
            "/crate/target/debug/foo.rs"
        )));
        assert!(!is_rust_source_path(Path::new(
            "/crate/target/plushie-renderer/src/main.rs"
        )));
    }

    #[test]
    fn default_debounce_is_250ms() {
        let o = WatchOpts::default();
        assert_eq!(o.debounce, Duration::from_millis(250));
        assert!(o.overlay.is_none());
        assert!(!o.release);
    }
}
