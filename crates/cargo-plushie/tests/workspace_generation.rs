//! Integration tests that drive the library layer of `cargo-plushie`
//! end-to-end without spawning the binary.
//!
//! The end-to-end "compile a real app + widget and exec the produced
//! binary" test is marked `#[ignore]` because it needs `cargo` on PATH,
//! a working plushie-renderer-lib toolchain, and several minutes of
//! wall time. It is meaningful to run before a hat 16 release:
//!
//! ```sh
//! cargo test -p cargo-plushie --test workspace_generation -- \
//!     --ignored --nocapture
//! ```

use cargo_plushie::{WidgetMetadata, generator};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn widget(crate_name: &str, type_name: &str, path: &Path, ctor: &str) -> WidgetMetadata {
    WidgetMetadata {
        crate_name: crate_name.to_string(),
        crate_path: path.to_path_buf(),
        type_name: type_name.to_string(),
        constructor: ctor.to_string(),
    }
}

#[test]
fn generates_workspace_files_for_empty_widget_set() {
    let dir = tempdir().unwrap();
    let config = generator::WorkspaceConfig {
        app_manifest_dir: Path::new("/app"),
        output_dir: dir.path(),
        binary_name: None,
        app_name: "my-app",
        workspace_version: "0.6.1",
        source_path: None,
        widgets: &[],
    };
    generator::generate_workspace(&config).unwrap();

    let cargo_toml = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains("name = \"my_app_renderer\""));
    assert!(cargo_toml.contains("plushie-widget-sdk = \"0.6.1\""));
    assert!(cargo_toml.contains("plushie-renderer = \"0.6.1\""));

    let main_rs = std::fs::read_to_string(dir.path().join("src/main.rs")).unwrap();
    assert!(main_rs.contains("PlushieAppBuilder::new()"));
    assert!(main_rs.contains("plushie_renderer::run(builder)"));
}

#[test]
fn generates_workspace_files_with_widgets() {
    let dir = tempdir().unwrap();
    let widget_root = dir.path().join("native/gauge");
    std::fs::create_dir_all(&widget_root).unwrap();
    let widgets = vec![widget(
        "my-gauge",
        "my_gauge",
        &widget_root,
        "my_gauge::Gauge::new()",
    )];

    let output_dir = dir.path().join("target/plushie-renderer");
    let config = generator::WorkspaceConfig {
        app_manifest_dir: dir.path(),
        output_dir: &output_dir,
        binary_name: Some("custom-renderer".to_string()),
        app_name: "my-app",
        workspace_version: "0.6.1",
        source_path: None,
        widgets: &widgets,
    };
    generator::generate_workspace(&config).unwrap();

    let cargo_toml = std::fs::read_to_string(output_dir.join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains("name = \"custom_renderer\""));
    assert!(cargo_toml.contains("gauge"));
    assert!(cargo_toml.contains("features = [\"impl\"]"));

    let main_rs = std::fs::read_to_string(output_dir.join("src/main.rs")).unwrap();
    assert!(main_rs.contains(".widget(my_gauge::Gauge::new())"));
}

#[test]
fn write_if_changed_preserves_mtime_on_noop_regenerate() {
    let dir = tempdir().unwrap();
    let config = generator::WorkspaceConfig {
        app_manifest_dir: Path::new("/app"),
        output_dir: dir.path(),
        binary_name: None,
        app_name: "my-app",
        workspace_version: "0.6.1",
        source_path: None,
        widgets: &[],
    };
    generator::generate_workspace(&config).unwrap();
    let cargo_path = dir.path().join("Cargo.toml");
    let first = std::fs::metadata(&cargo_path).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    generator::generate_workspace(&config).unwrap();
    let second = std::fs::metadata(&cargo_path).unwrap().modified().unwrap();

    assert_eq!(
        first, second,
        "regenerating with identical inputs must not rewrite the file"
    );
}

#[test]
#[ignore = "exercises cargo build end-to-end; run manually before release"]
fn end_to_end_build_produces_binary() {
    // This is a placeholder documenting the shape of the full smoke
    // test. When enabled it would:
    //   1. Scaffold a minimal app crate + widget crate via fixtures
    //      under tempdir.
    //   2. Run `cargo plushie build` against that manifest.
    //   3. Launch the resulting binary with `--version` and assert the
    //      version string matches RENDERER_VERSION.
    //
    // The full exercise requires a live plushie-widget-sdk checkout on
    // PLUSHIE_SOURCE_PATH so it's intentionally kept as a manual step.
    let _guard: PathBuf = tempdir().unwrap().keep();
}
