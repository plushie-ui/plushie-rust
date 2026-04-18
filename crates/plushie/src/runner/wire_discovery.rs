//! Locate a renderer binary for wire mode.
//!
//! Discovery order (first hit wins), mirroring Elixir's
//! `Plushie.Binary.path!`:
//!
//! 1. `PLUSHIE_BINARY_PATH` env. Treated as an absolute or relative
//!    path to the renderer binary. If set but pointing at a missing
//!    file, the explicit intent is respected and we fail immediately
//!    rather than falling through.
//! 2. Custom build output: `target/plushie-renderer/target/<profile>/
//!    <bin-name>` where `cargo plushie build` deposits a widget-aware
//!    binary. Both `release` and `debug` profiles are checked, in that
//!    order.
//! 3. Downloaded stock binary: `target/plushie/bin/<download-name>`
//!    where `cargo plushie download` places a precompiled binary.
//! 4. `plushie-renderer` on `PATH` (catch-all for users who ran
//!    `cargo install plushie-renderer`).
//!
//! Apps that ship a custom renderer build (for example, one with
//! additional `PlushieWidget` implementations registered) can bypass
//! discovery entirely by calling [`crate::run_with_renderer`] with an
//! explicit path.

use crate::Error;
use std::path::{Path, PathBuf};

/// Executable name cargo installs for the stock renderer.
#[cfg(not(target_os = "windows"))]
const RENDERER_BIN: &str = "plushie-renderer";

#[cfg(target_os = "windows")]
const RENDERER_BIN: &str = "plushie-renderer.exe";

/// Resolve the Cargo target directory the same way Cargo itself does.
///
/// Prefers `CARGO_TARGET_DIR`, falls back to `{cwd}/target`.
fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("target")
        })
}

/// Platform download file name used by `cargo plushie download`
/// (`plushie-renderer-{os}-{arch}[.exe]`).
fn download_name() -> String {
    let ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    let os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };
    format!("plushie-renderer-{os}-{arch}{ext}")
}

/// Find a renderer binary using the four-step discovery chain.
///
/// # Errors
///
/// Returns [`Error::BinaryNotFound`] with guidance text if none of the
/// steps resolve, and propagates the explicit-intent failure when
/// `PLUSHIE_BINARY_PATH` is set but the file is missing.
pub(crate) fn discover_renderer() -> Result<String, Error> {
    let resolved = resolve_candidate()?;
    validate_architecture(Path::new(&resolved));
    Ok(resolved)
}

fn resolve_candidate() -> Result<String, Error> {
    // Step 1: PLUSHIE_BINARY_PATH env (explicit; fail-fast if the file
    // doesn't exist).
    if let Ok(path) = std::env::var("PLUSHIE_BINARY_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            if Path::new(trimmed).is_file() {
                return Ok(trimmed.to_string());
            }
            return Err(Error::BinaryNotFound {
                hint: format!("PLUSHIE_BINARY_PATH is set to `{trimmed}` but no file exists there"),
            });
        }
    }

    let target = target_dir();

    // Step 2: custom build output from `cargo plushie build`. Try the
    // release profile first, then debug.
    for profile in ["release", "debug"] {
        if let Some(path) = find_custom_build_binary(&target, profile) {
            return Ok(path);
        }
    }

    // Step 3: downloaded stock binary from `cargo plushie download`.
    let download_path = target.join("plushie/bin").join(download_name());
    if is_executable(&download_path) {
        return Ok(download_path.to_string_lossy().into_owned());
    }

    // Step 4: plushie-renderer on PATH.
    if let Some(path) = find_on_path(RENDERER_BIN) {
        return Ok(path);
    }

    Err(Error::BinaryNotFound {
        hint: not_found_message(),
    })
}

/// Advisory architecture check. Shell out to `file(1)` on Unix,
/// parse out the arch, and warn on mismatch. Never fails: a missing
/// `file` tool, an unparseable output, or a platform without a shell
/// call is silently ignored so discovery stays functional.
///
/// Mirrors `Plushie.Binary.validate_architecture!` in the Elixir SDK,
/// minus the raise: the binary may still run, so we warn instead of
/// aborting.
#[cfg(unix)]
fn validate_architecture(path: &Path) {
    use std::process::Command;
    let output = match Command::new("file").arg(path).output() {
        Ok(o) if o.status.success() => o,
        _ => return,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detected = detect_arch(&stdout);
    let expected = std::env::consts::ARCH;
    let expected_canonical = canonicalize_arch(expected);
    if let (Some(got), Some(expected)) = (detected, expected_canonical)
        && got != expected
    {
        log::warn!(
            "architecture mismatch: binary `{}` is {got}, host is {expected}. \
             Rebuild for the correct architecture or set PLUSHIE_BINARY_PATH \
             to the matching binary.",
            path.display()
        );
    }
}

#[cfg(not(unix))]
fn validate_architecture(_path: &Path) {}

/// Parse `file(1)` output for a recognised architecture token.
///
/// `file` output varies per platform, so we match a handful of common
/// spellings. Unknown output returns `None` so the advisory warning
/// stays silent.
fn detect_arch(output: &str) -> Option<&'static str> {
    // Lowercase once; the tokens we match against are all lowercase.
    let lower = output.to_ascii_lowercase();
    if lower.contains("x86-64") || lower.contains("x86_64") || lower.contains("amd64") {
        Some("x86_64")
    } else if lower.contains("aarch64") || lower.contains("arm64") {
        Some("aarch64")
    } else {
        None
    }
}

/// Normalise `std::env::consts::ARCH` into the same token set that
/// [`detect_arch`] produces. Returns `None` on platforms we don't
/// know how to match (e.g. riscv64, powerpc).
fn canonicalize_arch(arch: &str) -> Option<&'static str> {
    match arch {
        "x86_64" => Some("x86_64"),
        "aarch64" => Some("aarch64"),
        _ => None,
    }
}

/// Look for `target/plushie-renderer/target/<profile>/<bin>` where
/// `<bin>` is any executable file under that profile directory. The
/// binary name is app-specific (`{app}-renderer`) so we enumerate the
/// directory rather than requiring the caller to know the name.
fn find_custom_build_binary(target: &Path, profile: &str) -> Option<String> {
    let profile_dir = target.join("plushie-renderer/target").join(profile);
    if !profile_dir.is_dir() {
        return None;
    }
    let ext = if cfg!(target_os = "windows") {
        "exe"
    } else {
        ""
    };
    let entries = std::fs::read_dir(&profile_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_executable(&path) {
            continue;
        }
        // On Unix executables rarely have extensions; on Windows we
        // insist on .exe so we don't pick up .rlib or .d output.
        let has_ext = path.extension().map(|e| e.to_string_lossy().into_owned());
        let matches_ext = if ext.is_empty() {
            has_ext.as_deref() != Some("rlib") && has_ext.as_deref() != Some("d")
        } else {
            has_ext.as_deref() == Some(ext)
        };
        if matches_ext {
            return Some(path.to_string_lossy().into_owned());
        }
    }
    None
}

/// Search the `PATH` environment for `name`, returning the first
/// executable match (absolute path).
fn find_on_path(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && (meta.permissions().mode() & 0o111) != 0,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.is_file()
}

/// Guidance text shown when every discovery step fails.
///
/// Points at all three install paths the SDK supports: the
/// crates.io-hosted stock binary (`cargo install plushie-renderer`),
/// the prebuilt download flow (`cargo plushie download`), and the
/// custom build flow (`cargo plushie build`).
pub(crate) fn not_found_message() -> String {
    format!(
        "renderer binary not found. Try one of:\n  \
         cargo plushie build      (widget-aware custom build)\n  \
         cargo plushie download   (precompiled stock binary)\n  \
         cargo install plushie-renderer   (build stock from source)\n\
         or set PLUSHIE_BINARY_PATH to an existing binary. Searched \
         PLUSHIE_BINARY_PATH, target/plushie-renderer/target/{{release,debug}}/, \
         target/plushie/bin/{download_name}, and PATH for `{RENDERER_BIN}`.",
        download_name = download_name()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_executable_says_no_for_missing_file() {
        assert!(!is_executable(std::path::Path::new(
            "/nonexistent/plushie-renderer-xyz"
        )));
    }

    #[test]
    fn find_on_path_returns_none_for_missing_binary() {
        let needle = "plushie-renderer-definitely-not-installed-xyz-12345";
        assert!(find_on_path(needle).is_none());
    }

    #[test]
    fn not_found_message_mentions_all_install_paths() {
        let msg = not_found_message();
        assert!(msg.contains("cargo plushie build"));
        assert!(msg.contains("cargo plushie download"));
        assert!(msg.contains("cargo install plushie-renderer"));
        assert!(msg.contains("PLUSHIE_BINARY_PATH"));
    }

    #[test]
    fn download_name_is_well_formed() {
        let name = download_name();
        assert!(name.starts_with("plushie-renderer-"));
    }

    #[test]
    fn detect_arch_recognises_x86_64_variants() {
        let samples = [
            "ELF 64-bit LSB pie executable, x86-64, version 1",
            "ELF 64-bit LSB executable, x86_64, version 1 (GNU/Linux)",
            "Mach-O 64-bit executable x86_64",
            "Mach-O universal binary with 2 architectures: [x86_64] [arm64]",
            "PE32+ executable (console) x86-64, for MS Windows",
        ];
        for sample in samples {
            // Each sample mentions x86-64 / x86_64 first; detect_arch
            // returns the first-seen arch but several of these include
            // arm64 too. Assert the dominant token maps to x86_64 by
            // stripping the arm64 mention where present.
            let trimmed = sample.replace("arm64", "");
            assert_eq!(detect_arch(&trimmed), Some("x86_64"), "sample: {sample}");
        }
    }

    #[test]
    fn detect_arch_recognises_aarch64_variants() {
        let samples = [
            "ELF 64-bit LSB executable, ARM aarch64, version 1 (GNU/Linux)",
            "Mach-O 64-bit executable arm64",
        ];
        for sample in samples {
            assert_eq!(detect_arch(sample), Some("aarch64"), "sample: {sample}");
        }
    }

    #[test]
    fn detect_arch_returns_none_for_unknown_output() {
        assert_eq!(detect_arch("this is not an executable"), None);
        assert_eq!(detect_arch(""), None);
    }

    #[test]
    fn canonicalize_arch_known_values() {
        assert_eq!(canonicalize_arch("x86_64"), Some("x86_64"));
        assert_eq!(canonicalize_arch("aarch64"), Some("aarch64"));
        assert_eq!(canonicalize_arch("riscv64"), None);
    }
}
