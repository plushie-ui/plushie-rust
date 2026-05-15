use cargo_plushie::package::{PackageOpts, build_launcher};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

#[cfg(unix)]
#[test]
fn real_payload_launcher_starts_host_first() {
    let dir = tempdir().unwrap();
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).unwrap();

    let manifest = write_package(&package_dir, "A");
    let launcher = dir.path().join("bin").join("launcher");
    let built = build_launcher(&PackageOpts {
        manifest_path: &manifest,
        out_path: Some(&launcher),
        launcher_path: Some(launcher_template()),
        run_signing_hooks: false,
        verbose: false,
    })
    .unwrap();

    let launch_cache = dir.path().join("launch-cache");
    let marker = dir.path().join("marker.txt");
    let package_root = dir.path().join("package-root.txt");
    let args_file = dir.path().join("host-args.txt");
    let cwd_file = dir.path().join("host-cwd.txt");
    let renderer_path = dir.path().join("renderer-path.txt");
    let actual = run_launcher(
        &built.binary_path,
        &launch_cache,
        Some(RuntimeProbe {
            marker: &marker,
            package_root: &package_root,
            args_file: &args_file,
            cwd_file: &cwd_file,
            renderer_path: &renderer_path,
        }),
    );
    assert_success(&actual);
    assert!(actual.stderr.contains("cache_status=extracted"));
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "A\n");
    let package_root = std::fs::read_to_string(&package_root).unwrap();
    assert!(Path::new(package_root.trim()).starts_with(&launch_cache));
    assert!(
        std::fs::read_to_string(&cwd_file)
            .unwrap()
            .starts_with(&package_root)
    );
    assert_host_launch(&args_file, &renderer_path, &package_root, "A");

    let relative_marker = dir.path().join("relative-marker.txt");
    let relative_package_root = dir.path().join("relative-package-root.txt");
    let relative_args_file = dir.path().join("relative-host-args.txt");
    let relative_cwd_file = dir.path().join("relative-host-cwd.txt");
    let relative_renderer_path = dir.path().join("relative-renderer-path.txt");
    let relative_cache = Path::new("relative-launch-cache");
    let actual = run_launcher_from(
        &built.binary_path,
        dir.path(),
        relative_cache,
        Some(RuntimeProbe {
            marker: &relative_marker,
            package_root: &relative_package_root,
            args_file: &relative_args_file,
            cwd_file: &relative_cwd_file,
            renderer_path: &relative_renderer_path,
        }),
    );
    assert_success(&actual);
    let package_root = std::fs::read_to_string(&relative_package_root).unwrap();
    assert!(
        Path::new(package_root.trim()).starts_with(dir.path().join(relative_cache)),
        "relative PLUSHIE_CACHE_DIR produced package root `{package_root}`"
    );
    assert_host_launch(
        &relative_args_file,
        &relative_renderer_path,
        &package_root,
        "A",
    );
}

#[cfg(unix)]
#[test]
fn real_payload_launcher_postcheck_and_replacement_use_embedded_payload() {
    let dir = tempdir().unwrap();
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).unwrap();

    let manifest = write_package(&package_dir, "A");
    let launcher_a = dir.path().join("bin").join("launcher-a");
    let built_a = build_launcher(&PackageOpts {
        manifest_path: &manifest,
        out_path: Some(&launcher_a),
        launcher_path: Some(launcher_template()),
        run_signing_hooks: false,
        verbose: false,
    })
    .unwrap();

    let postcheck_cache = dir.path().join("postcheck-cache");
    let postcheck_first = run_launcher(&built_a.binary_path, &postcheck_cache, None);
    assert_success(&postcheck_first);
    assert!(postcheck_first.stdout.trim().is_empty());
    assert!(postcheck_first.stderr.contains("cache_status=extracted"));
    assert!(
        postcheck_first
            .stderr
            .contains("plushie launcher: postcheck ok")
    );

    let postcheck_second = run_launcher(&built_a.binary_path, &postcheck_cache, None);
    assert_success(&postcheck_second);
    assert!(postcheck_second.stderr.contains("cache_status=reused"));

    let launch_cache = dir.path().join("launch-cache");
    let marker = dir.path().join("marker.txt");
    let package_root = dir.path().join("package-root.txt");
    let args_file = dir.path().join("host-args.txt");
    let cwd_file = dir.path().join("host-cwd.txt");
    let renderer_path = dir.path().join("renderer-path.txt");
    let actual_a = run_launcher(
        &built_a.binary_path,
        &launch_cache,
        Some(RuntimeProbe {
            marker: &marker,
            package_root: &package_root,
            args_file: &args_file,
            cwd_file: &cwd_file,
            renderer_path: &renderer_path,
        }),
    );
    assert_success(&actual_a);
    assert!(actual_a.stderr.contains("cache_status=extracted"));
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "A\n");
    let package_root_a = std::fs::read_to_string(&package_root).unwrap();
    assert!(Path::new(package_root_a.trim()).starts_with(&launch_cache));
    assert_host_launch(&args_file, &renderer_path, &package_root_a, "A");

    let manifest = write_package(&package_dir, "B");
    let launcher_b = dir.path().join("bin").join("launcher-b");
    let built_b = build_launcher(&PackageOpts {
        manifest_path: &manifest,
        out_path: Some(&launcher_b),
        launcher_path: Some(launcher_template()),
        run_signing_hooks: false,
        verbose: false,
    })
    .unwrap();

    let actual_b = run_launcher(
        &built_b.binary_path,
        &launch_cache,
        Some(RuntimeProbe {
            marker: &marker,
            package_root: &package_root,
            args_file: &args_file,
            cwd_file: &cwd_file,
            renderer_path: &renderer_path,
        }),
    );
    assert_success(&actual_b);
    assert!(actual_b.stderr.contains("cache_status=extracted"));
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "B\n");
    let package_root_b = std::fs::read_to_string(&package_root).unwrap();
    assert_ne!(package_root_a, package_root_b);
    assert!(Path::new(package_root_b.trim()).starts_with(&launch_cache));
    assert!(
        std::fs::read_to_string(&cwd_file)
            .unwrap()
            .starts_with(&package_root_b)
    );
    assert_host_launch(&args_file, &renderer_path, &package_root_b, "B");

    let actual_b_reused = run_launcher(
        &built_b.binary_path,
        &launch_cache,
        Some(RuntimeProbe {
            marker: &marker,
            package_root: &package_root,
            args_file: &args_file,
            cwd_file: &cwd_file,
            renderer_path: &renderer_path,
        }),
    );
    assert_success(&actual_b_reused);
    assert!(actual_b_reused.stderr.contains("cache_status=reused"));
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "B\n");
}

fn launcher_template() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_plushie-launcher"))
}

struct RuntimeProbe<'a> {
    marker: &'a Path,
    package_root: &'a Path,
    args_file: &'a Path,
    cwd_file: &'a Path,
    renderer_path: &'a Path,
}

struct LauncherOutput {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn run_launcher(binary: &Path, cache: &Path, probe: Option<RuntimeProbe<'_>>) -> LauncherOutput {
    run_launcher_from(binary, Path::new("."), cache, probe)
}

fn run_launcher_from(
    binary: &Path,
    cwd: &Path,
    cache: &Path,
    probe: Option<RuntimeProbe<'_>>,
) -> LauncherOutput {
    let mut command = Command::new(binary);
    command
        .current_dir(cwd)
        .env("PLUSHIE_CACHE_DIR", cache)
        .env("PLUSHIE_BINARY_PATH", "/tmp/ambient-renderer")
        .env("PLUSHIE_PACKAGE_DIR", "/tmp/ambient-package");
    if let Some(probe) = probe {
        command
            .env("PLUSHIE_TEST_MARKER", probe.marker)
            .env("PLUSHIE_TEST_PACKAGE_DIR", probe.package_root)
            .env("PLUSHIE_TEST_ARGS", probe.args_file)
            .env("PLUSHIE_TEST_CWD", probe.cwd_file)
            .env("PLUSHIE_TEST_RENDERER_PATH", probe.renderer_path);
    } else {
        command.env("PLUSHIE_PACKAGE_POSTCHECK", "1");
    }

    let output = command.output().unwrap();
    LauncherOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn assert_success(output: &LauncherOutput) {
    assert!(
        output.status.success(),
        "launcher failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        output.stdout,
        output.stderr
    );
}

fn assert_host_launch(
    args_file: &Path,
    renderer_path: &Path,
    package_root: &str,
    payload_label: &str,
) {
    let args = std::fs::read_to_string(args_file).unwrap();
    let renderer = std::fs::read_to_string(renderer_path).unwrap();
    let root = package_root.trim();
    let expected_renderer = Path::new(root).join("bin/plushie-renderer");

    assert_eq!(args.trim(), format!("--payload {payload_label}"));
    assert_eq!(renderer.trim(), expected_renderer.display().to_string());
}

fn write_package(dir: &Path, payload_label: &str) -> PathBuf {
    let payload = payload_archive(payload_label);
    let archive = dir.join("payload.tar.zst");
    std::fs::write(&archive, &payload).unwrap();
    let hash = format!("sha256:{:x}", Sha256::digest(&payload));
    let manifest = dir.join("plushie-package.toml");
    std::fs::write(
        &manifest,
        format!(
            r#"
schema_version = 1
app_id = "com.example.package-test"
app_version = "0.1.0"
target = "{}"
host_sdk = "test"
plushie_rust_version = "{}"
protocol_version = {}

[start]
working_dir = "."
command = ["bin/host", "--payload", "{payload_label}"]
forward_env = [
  "PLUSHIE_TEST_MARKER",
  "PLUSHIE_TEST_PACKAGE_DIR",
  "PLUSHIE_TEST_ARGS",
  "PLUSHIE_TEST_CWD",
  "PLUSHIE_TEST_RENDERER_PATH",
]

[renderer]
path = "bin/plushie-renderer"
kind = "stock"

[payload]
archive = "payload.tar.zst"
hash = "{hash}"
size = {}
"#,
            package_target(),
            env!("CARGO_PKG_VERSION"),
            plushie_core::protocol::PROTOCOL_VERSION,
            payload.len()
        ),
    )
    .unwrap();
    manifest
}

fn payload_archive(payload_label: &str) -> Vec<u8> {
    let renderer = r#"#!/bin/sh
printf 'renderer should not start\n' >&2
exit 9
"#;
    let host = format!(
        r#"#!/bin/sh
set -eu
printf '{}\n' > "$PLUSHIE_TEST_MARKER"
printf '%s\n' "$PLUSHIE_PACKAGE_DIR" > "$PLUSHIE_TEST_PACKAGE_DIR"
printf '%s\n' "$*" > "$PLUSHIE_TEST_ARGS"
printf '%s\n' "$PWD" > "$PLUSHIE_TEST_CWD"
printf '%s\n' "$PLUSHIE_BINARY_PATH" > "$PLUSHIE_TEST_RENDERER_PATH"
"#,
        payload_label
    );

    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        append_file(&mut builder, "bin/plushie-renderer", renderer.as_bytes());
        append_file(&mut builder, "bin/host", host.as_bytes());
        builder.finish().unwrap();
    }
    zstd::stream::encode_all(tar_bytes.as_slice(), 0).unwrap()
}

fn append_file(builder: &mut tar::Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    builder.append_data(&mut header, path, bytes).unwrap();
}

fn package_target() -> &'static str {
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
