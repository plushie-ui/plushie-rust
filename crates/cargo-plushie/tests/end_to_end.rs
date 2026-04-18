//! End-to-end scaffold + build integration test.
//!
//! Runs the full authoring sequence that a new widget author follows:
//!
//! 1. `cargo plushie new-widget` produces a widget crate.
//! 2. `cargo plushie init` produces an app crate that depends on it.
//! 3. `cargo plushie build` generates the custom renderer workspace
//!    and compiles it against both crates.
//! 4. The resulting binary is exec'd with `--version` and must print
//!    a version line.
//!
//! This test is `#[ignore]` by default: it shells out to `cargo` in a
//! tempdir and compiles a fresh renderer, which takes minutes on a
//! cold target cache. It is also gated on `PLUSHIE_RUST_SOURCE_PATH`
//! pointing at the plushie-rust checkout so the scaffolder emits
//! path deps instead of hitting crates.io. Run explicitly with:
//!
//! ```sh
//! PLUSHIE_RUST_SOURCE_PATH=$(pwd) cargo test -p cargo-plushie --test end_to_end -- --ignored
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

/// Path to the cargo-plushie binary that the test runner produced.
/// Cargo integration tests put it one directory up from the test exe.
fn cargo_plushie_binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("current_exe");
    path.pop();
    path.pop();
    path.push(if cfg!(windows) {
        "cargo-plushie.exe"
    } else {
        "cargo-plushie"
    });
    path
}

/// Run a cargo-plushie subcommand inside `cwd` with the given env.
/// Panics on failure so the test output carries the subcommand's
/// stderr.
fn run_plushie(bin: &Path, cwd: &Path, args: &[&str], source_path: &std::ffi::OsStr) {
    let output = Command::new(bin)
        .current_dir(cwd)
        .args(args)
        .env("PLUSHIE_RUST_SOURCE_PATH", source_path)
        .output()
        .unwrap_or_else(|e| panic!("spawn cargo-plushie {args:?}: {e}"));
    if !output.status.success() {
        panic!(
            "cargo-plushie {args:?} failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
#[ignore = "slow: compiles a full renderer binary; run with --ignored"]
fn scaffold_init_and_build_produces_versioned_binary() {
    let Some(source_path) = std::env::var_os("PLUSHIE_RUST_SOURCE_PATH") else {
        eprintln!(
            "skipping: PLUSHIE_RUST_SOURCE_PATH is not set. Point it at a plushie-rust \
             checkout so the scaffolder emits path deps."
        );
        return;
    };

    let bin = cargo_plushie_binary();
    if !bin.is_file() {
        panic!("cargo-plushie binary not found at {}", bin.display());
    }

    let workspace = tempdir().expect("tempdir");
    let root = workspace.path();

    // Step 1: scaffold a widget crate at native/gauge-demo.
    run_plushie(
        &bin,
        root,
        &["plushie", "new-widget", "gauge-demo"],
        &source_path,
    );
    let widget_dir = root.join("native/gauge-demo");
    assert!(widget_dir.join("Cargo.toml").is_file());
    assert!(widget_dir.join("src/lib.rs").is_file());

    // Step 2: scaffold an app crate alongside it.
    run_plushie(
        &bin,
        root,
        &["plushie", "init", "end-to-end-app"],
        &source_path,
    );
    let app_dir = root.join("end-to-end-app");
    assert!(app_dir.join("Cargo.toml").is_file());
    assert!(app_dir.join("src/main.rs").is_file());

    // Wire the widget into the app's Cargo.toml. The scaffolders are
    // deliberately independent so the test stitches them together the
    // same way a human would.
    let app_cargo = app_dir.join("Cargo.toml");
    let mut contents = std::fs::read_to_string(&app_cargo).expect("read app Cargo.toml");
    let widget_path = widget_dir.canonicalize().expect("canonicalize widget");
    contents.push_str(&format!(
        "\ngauge-demo = {{ path = {:?} }}\n",
        widget_path.display().to_string()
    ));
    std::fs::write(&app_cargo, contents).expect("write app Cargo.toml");

    // Step 3: build the custom renderer. The scaffolder picked up
    // PLUSHIE_RUST_SOURCE_PATH during new-widget/init, so the generated
    // workspace already resolves against the local plushie-rust
    // checkout instead of crates.io.
    run_plushie(&bin, &app_dir, &["plushie", "build"], &source_path);

    // Step 4: the renderer binary lives under the generated
    // workspace's target/debug directory with the derived name.
    let bin_name = if cfg!(windows) {
        "end-to-end-app-renderer.exe"
    } else {
        "end-to-end-app-renderer"
    };
    let renderer = app_dir
        .join("target/plushie-renderer/target/debug")
        .join(bin_name);
    assert!(
        renderer.is_file(),
        "renderer binary not produced at {}",
        renderer.display(),
    );

    // Step 5: exec with --version and require a version line. Some
    // SDKs probe this before spawning the wire renderer; a regression
    // that silently drops the flag would mask a handshake mismatch.
    let out = Command::new(&renderer)
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("exec renderer --version: {e}"));
    assert!(
        out.status.success(),
        "renderer --version exited with {}: stderr=\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    let reported = String::from_utf8_lossy(&out.stdout);
    assert!(
        reported.trim().len() >= 3,
        "renderer --version output looked empty: {reported:?}"
    );
}
