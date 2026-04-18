//! Integration tests that drive the library layer of `cargo-plushie`
//! without spawning the binary. Exercises workspace generation against
//! a real tempdir so the on-disk shape of the generated files is
//! verified end-to-end from `WorkspaceConfig` through to
//! `Cargo.toml` + `src/main.rs`.

use cargo_plushie::{WidgetMetadata, generator};
use std::path::Path;
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

// End-to-end smoke test (scaffold app + widget, run `cargo plushie
// build`, launch the resulting binary and assert its `--version`) is
// tracked separately. It requires cargo on PATH, a live
// plushie-widget-sdk checkout on PLUSHIE_SOURCE_PATH, and several
// minutes of wall time, so it doesn't fit in this test module.
