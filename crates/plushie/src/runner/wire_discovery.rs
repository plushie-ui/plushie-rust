//! Locate a renderer binary for wire mode.
//!
//! Discovery order (first hit wins):
//!
//! 1. `PLUSHIE_BINARY_PATH` environment variable. Treated as an
//!    absolute or relative path to the renderer binary.
//! 2. `PATH` search for `plushie-renderer` (Unix) or
//!    `plushie-renderer.exe` (Windows).
//!
//! Apps that ship a custom renderer build (for example, one with
//! additional `PlushieWidget` implementations registered) can bypass
//! discovery entirely by calling [`crate::run_with_renderer`] with an
//! explicit path.

use crate::Error;

/// Executable name cargo installs for the stock renderer.
#[cfg(not(target_os = "windows"))]
const RENDERER_BIN: &str = "plushie-renderer";

#[cfg(target_os = "windows")]
const RENDERER_BIN: &str = "plushie-renderer.exe";

/// Find a renderer binary path using env, then `PATH`.
///
/// Returns the first executable match. Returns [`Error::BinaryNotFound`]
/// with guidance text if neither mechanism locates the binary.
pub(crate) fn discover_renderer() -> Result<String, Error> {
    if let Ok(path) = std::env::var("PLUSHIE_BINARY_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            if std::path::Path::new(trimmed).is_file() {
                return Ok(trimmed.to_string());
            }
            return Err(Error::BinaryNotFound {
                hint: format!("PLUSHIE_BINARY_PATH is set to `{trimmed}` but no file exists there"),
            });
        }
    }

    if let Some(path) = find_on_path(RENDERER_BIN) {
        return Ok(path);
    }

    Err(Error::BinaryNotFound {
        hint: format!(
            "install the stock renderer with `cargo install plushie-renderer`, \
             set PLUSHIE_BINARY_PATH to a custom renderer, or call \
             `plushie::run_with_renderer(path)` directly; \
             `{RENDERER_BIN}` was not found on PATH"
        ),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    // The real discover_renderer() reads PATH and PLUSHIE_BINARY_PATH
    // from the process environment, which is workspace-unsafe to mutate
    // (`unsafe_code = "deny"`). The path-search helpers below are pure
    // functions and easy to exercise without touching process state.

    #[test]
    fn is_executable_says_no_for_missing_file() {
        assert!(!is_executable(std::path::Path::new(
            "/nonexistent/plushie-renderer-xyz"
        )));
    }

    #[test]
    fn find_on_path_returns_none_for_missing_binary() {
        // Anything sufficiently silly so PATH is guaranteed not to
        // contain it.
        let needle = "plushie-renderer-definitely-not-installed-xyz-12345";
        assert!(find_on_path(needle).is_none());
    }
}
