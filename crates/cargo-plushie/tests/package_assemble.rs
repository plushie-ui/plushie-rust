use cargo_plushie::package::PackageOpts;
use cargo_plushie::package::build_launcher;
use cargo_plushie::package_assemble::{AssembleOpts, assemble_package};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn current_target() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "linux-x86_64"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "linux-aarch64"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "darwin-x86_64"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "darwin-aarch64"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "windows-x86_64"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "windows-aarch64"
    }
}

fn launcher_template() -> &'static Path {
    // Reuse the same launcher binary the package_launcher tests use.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/plushie/bin/plushie-launcher")
        .leak()
}

fn write_payload_dir(dir: &Path, label: &str) {
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let renderer = "#!/bin/sh\nprintf 'should not start\\n' >&2\nexit 9\n";
    let host = format!("#!/bin/sh\nset -eu\nprintf '{label}\\n' > \"$PLUSHIE_TEST_MARKER\"\n");
    std::fs::write(dir.join("bin/plushie-renderer"), renderer).unwrap();
    std::fs::write(dir.join("bin/host"), &host).unwrap();

    // Make them executable on unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in ["bin/plushie-renderer", "bin/host"] {
            let path = dir.join(name);
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
    }
}

fn write_partial_manifest(dir: &Path, start: Option<&str>, platform: Option<&str>) -> PathBuf {
    let start_section = start.unwrap_or(
        r#"[start]
working_dir = "."
command = ["bin/host"]
forward_env = []
"#,
    );
    let platform_section = platform.unwrap_or("");
    let text = format!(
        r#"schema_version = 1
app_id = "com.example.assemble_test"
app_version = "1.0.0"
target = "{target}"
host_sdk = "elixir"
host_sdk_version = "0.2.0"
plushie_rust_version = "{version}"
protocol_version = {proto}

{start_section}

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

{platform_section}
"#,
        target = current_target(),
        version = env!("CARGO_PKG_VERSION"),
        proto = plushie_core::protocol::PROTOCOL_VERSION,
    );
    let path = dir.join("plushie-package.toml");
    std::fs::write(&path, text).unwrap();
    path
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn assemble_produces_complete_manifest_and_archive() {
    let dir = tempdir().unwrap();
    let payload_dir = dir.path().join("payload");
    write_payload_dir(&payload_dir, "hello");
    let manifest_path = write_partial_manifest(dir.path(), None, None);

    let result = assemble_package(&AssembleOpts {
        manifest_path: &manifest_path,
        payload_dir: &payload_dir,
        package_config: None,
    })
    .unwrap();

    // Archive exists next to the manifest.
    let archive_path = dir.path().join("payload.tar.zst");
    assert!(archive_path.is_file(), "payload archive created");
    assert_eq!(result.payload_archive_path, archive_path);

    // Manifest is complete.
    let text = std::fs::read_to_string(&result.manifest_path).unwrap();
    assert!(text.contains("[payload]"), "manifest has [payload]");
    assert!(text.contains("archive = \"payload.tar.zst\""));
    assert!(text.contains("hash = \"sha256:"));
    assert!(text.contains("[start]"));
    assert!(text.contains("host_sdk = \"elixir\""));
}

#[test]
fn assemble_hash_matches_archive_bytes() {
    let dir = tempdir().unwrap();
    let payload_dir = dir.path().join("payload");
    write_payload_dir(&payload_dir, "hash-check");
    let manifest_path = write_partial_manifest(dir.path(), None, None);

    let result = assemble_package(&AssembleOpts {
        manifest_path: &manifest_path,
        payload_dir: &payload_dir,
        package_config: None,
    })
    .unwrap();

    let archive_bytes = std::fs::read(&result.payload_archive_path).unwrap();
    let expected_hash = format!("sha256:{:x}", Sha256::digest(&archive_bytes));
    let manifest_text = std::fs::read_to_string(&result.manifest_path).unwrap();
    assert!(
        manifest_text.contains(&expected_hash),
        "manifest hash matches archive"
    );
}

#[test]
fn assemble_materializes_default_icon() {
    let dir = tempdir().unwrap();
    let payload_dir = dir.path().join("payload");
    write_payload_dir(&payload_dir, "icon-test");
    let manifest_path = write_partial_manifest(dir.path(), None, None);

    assemble_package(&AssembleOpts {
        manifest_path: &manifest_path,
        payload_dir: &payload_dir,
        package_config: None,
    })
    .unwrap();

    let icon_path = payload_dir.join("assets/default-app-icon-512.png");
    assert!(icon_path.is_file(), "default icon written into payload dir");
    let manifest_text = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(
        manifest_text.contains("icon = \"assets/default-app-icon-512.png\""),
        "manifest records default icon"
    );
}

#[test]
fn assemble_does_not_overwrite_declared_icon() {
    let dir = tempdir().unwrap();
    let payload_dir = dir.path().join("payload");
    write_payload_dir(&payload_dir, "custom-icon");
    std::fs::create_dir_all(payload_dir.join("assets")).unwrap();
    std::fs::write(payload_dir.join("assets/custom.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    let manifest_path = write_partial_manifest(
        dir.path(),
        None,
        Some("[platform]\nicon = \"assets/custom.png\""),
    );

    assemble_package(&AssembleOpts {
        manifest_path: &manifest_path,
        payload_dir: &payload_dir,
        package_config: None,
    })
    .unwrap();

    let text = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(
        text.contains("icon = \"assets/custom.png\""),
        "custom icon kept"
    );
    assert!(!text.contains("default-app-icon"), "default not inserted");
}

#[test]
fn assemble_source_config_provides_start() {
    let dir = tempdir().unwrap();
    let payload_dir = dir.path().join("payload");
    write_payload_dir(&payload_dir, "src-config");
    // Partial manifest has NO [start] section.
    let text = format!(
        r#"schema_version = 1
app_id = "com.example.assemble_test"
app_version = "1.0.0"
target = "{target}"
host_sdk = "elixir"
plushie_rust_version = "{version}"
protocol_version = {proto}

[renderer]
path = "bin/plushie-renderer"
kind = "stock"
"#,
        target = current_target(),
        version = env!("CARGO_PKG_VERSION"),
        proto = plushie_core::protocol::PROTOCOL_VERSION,
    );
    let manifest_path = dir.path().join("plushie-package.toml");
    std::fs::write(&manifest_path, text).unwrap();

    let config_path = dir.path().join("plushie-package.config.toml");
    std::fs::write(
        &config_path,
        "config_version = 1\n\n[start]\nworking_dir = \".\"\ncommand = [\"bin/host\"]\nforward_env = []\n",
    )
    .unwrap();

    assemble_package(&AssembleOpts {
        manifest_path: &manifest_path,
        payload_dir: &payload_dir,
        package_config: Some(&config_path),
    })
    .unwrap();

    let out = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(
        out.contains("[start]"),
        "start section injected from source config"
    );
}

#[test]
fn assemble_rejects_missing_start_no_config() {
    let dir = tempdir().unwrap();
    let payload_dir = dir.path().join("payload");
    write_payload_dir(&payload_dir, "no-start");
    // No [start] and no source config.
    let text = format!(
        r#"schema_version = 1
app_id = "com.example.assemble_test"
app_version = "1.0.0"
target = "{target}"
host_sdk = "elixir"
plushie_rust_version = "{version}"
protocol_version = {proto}

[renderer]
path = "bin/plushie-renderer"
kind = "stock"
"#,
        target = current_target(),
        version = env!("CARGO_PKG_VERSION"),
        proto = plushie_core::protocol::PROTOCOL_VERSION,
    );
    let manifest_path = dir.path().join("plushie-package.toml");
    std::fs::write(&manifest_path, text).unwrap();

    let err = assemble_package(&AssembleOpts {
        manifest_path: &manifest_path,
        payload_dir: &payload_dir,
        package_config: None,
    })
    .unwrap_err();
    assert!(
        err.to_string().contains("[start]") || err.to_string().contains("start"),
        "error mentions missing start: {err}"
    );
}

#[test]
fn assemble_rejects_invalid_app_id() {
    let dir = tempdir().unwrap();
    let payload_dir = dir.path().join("payload");
    write_payload_dir(&payload_dir, "bad-id");
    let text = format!(
        r#"schema_version = 1
app_id = "INVALID-ID"
app_version = "1.0.0"
target = "{target}"
host_sdk = "elixir"
plushie_rust_version = "{version}"
protocol_version = {proto}

[start]
working_dir = "."
command = ["bin/host"]
forward_env = []

[renderer]
path = "bin/plushie-renderer"
kind = "stock"
"#,
        target = current_target(),
        version = env!("CARGO_PKG_VERSION"),
        proto = plushie_core::protocol::PROTOCOL_VERSION,
    );
    let manifest_path = dir.path().join("plushie-package.toml");
    std::fs::write(&manifest_path, text).unwrap();

    let err = assemble_package(&AssembleOpts {
        manifest_path: &manifest_path,
        payload_dir: &payload_dir,
        package_config: None,
    })
    .unwrap_err();
    assert!(
        err.to_string().contains("reverse-DNS") || err.to_string().contains("app_id"),
        "error mentions app_id format: {err}"
    );
}

#[cfg(unix)]
#[test]
fn assemble_rejects_payload_with_symlink() {
    let dir = tempdir().unwrap();
    let payload_dir = dir.path().join("payload");
    write_payload_dir(&payload_dir, "symlink-reject");
    std::os::unix::fs::symlink("/etc/passwd", payload_dir.join("bin/evil")).unwrap();
    let manifest_path = write_partial_manifest(dir.path(), None, None);

    let err = assemble_package(&AssembleOpts {
        manifest_path: &manifest_path,
        payload_dir: &payload_dir,
        package_config: None,
    })
    .unwrap_err();
    assert!(
        err.to_string().contains("symlink"),
        "error mentions symlink: {err}"
    );
}

/// End-to-end: assemble a manifest + payload, build a portable launcher from
/// the resulting manifest, and verify the launcher binary is produced.
#[cfg(unix)]
#[test]
fn assemble_then_portable_produces_launcher() {
    let launcher_template = launcher_template();
    if !launcher_template.is_file() {
        eprintln!(
            "skipping: launcher template not present at {}",
            launcher_template.display()
        );
        return;
    }

    let dir = tempdir().unwrap();
    let payload_dir = dir.path().join("payload");
    write_payload_dir(&payload_dir, "e2e");
    let manifest_path = write_partial_manifest(dir.path(), None, None);

    let assemble_result = assemble_package(&AssembleOpts {
        manifest_path: &manifest_path,
        payload_dir: &payload_dir,
        package_config: None,
    })
    .unwrap();

    let launcher_out = dir.path().join("launcher");
    let portable_result = build_launcher(&PackageOpts {
        manifest_path: &assemble_result.manifest_path,
        out_path: Some(&launcher_out),
        launcher_path: Some(launcher_template),
        run_signing_hooks: false,
        verbose: false,
    })
    .unwrap();

    assert!(portable_result.binary_path.is_file(), "launcher built");
}
