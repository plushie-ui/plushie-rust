//! Environment whitelist for the wire-mode renderer subprocess.
//!
//! `Bridge::spawn` launches the renderer as a child process. Without
//! filtering, the renderer inherits the host's entire environment:
//! AWS keys, database URLs, SSH agent sockets, tokens, everything.
//! A compromised renderer (e.g. via a font or SVG parser CVE) would
//! then have immediate access to those secrets, and stderr logs from
//! the renderer can leak env values into operator dashboards.
//!
//! [`renderer_env`] returns only the variables the renderer actually
//! needs. The list is the canonical whitelist shared across every host
//! SDK (Elixir, Gleam, Python, Ruby, TypeScript): Elixir's exact names
//! and prefix classes, plus a closed set of `PLUSHIE_*` names the
//! renderer subprocess actually reads. Anything not on the list is
//! stripped.
//!
//! The renderer subprocess in spawn (host-parent) mode reads at most
//! `PLUSHIE_NO_CATCH_UNWIND` from its inherited env. Other `PLUSHIE_*`
//! names are host-side, launcher-set, or secrets (e.g. `PLUSHIE_TOKEN`)
//! that must not leak across the process boundary. Adding a new
//! `PLUSHIE_*` var to the list below is a deliberate review decision.

/// Variables passed through to the renderer subprocess unchanged when
/// they are set in the host's environment. Matches the canonical list
/// every host SDK applies.
const EXACT: &[&str] = &[
    // Display servers
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "WAYLAND_SOCKET",
    "WINIT_UNIX_BACKEND",
    "XDG_RUNTIME_DIR",
    "XDG_DATA_DIRS",
    "XDG_DATA_HOME",
    // PATH / shared library resolution
    "PATH",
    "LD_LIBRARY_PATH",
    "DYLD_LIBRARY_PATH",
    "DYLD_FALLBACK_LIBRARY_PATH",
    // Locale
    "LANG",
    "LANGUAGE",
    // Desktop integration
    "DBUS_SESSION_BUS_ADDRESS",
    "GTK_MODULES",
    "NO_AT_BRIDGE",
    // Renderer + log controls
    "WGPU_BACKEND",
    "RUST_LOG",
    "RUST_BACKTRACE",
    // Identity
    "HOME",
    "USER",
    // Windows: required for DLL loader, child process resolution, and tempdir.
    // Harmless on other platforms (just absent from the host env).
    "SystemRoot",
    "WINDIR",
    "PATHEXT",
    "TEMP",
    "TMP",
];

/// Prefix patterns. Any variable whose name starts with one of these
/// prefixes passes through. The prefixes cover locale (LC_*), the
/// Mesa / GLX / Vulkan / Gallium graphics stack, accessibility bridge
/// settings, and fontconfig.
const PREFIXES: &[&str] = &[
    "LC_",
    "MESA_",
    "LIBGL_",
    "__GLX_",
    "VK_",
    "GALLIUM_",
    "AT_SPI_",
    "FONTCONFIG_",
];

/// The closed set of `PLUSHIE_*` names forwarded to the renderer subprocess.
///
/// The renderer in spawn mode reads only `PLUSHIE_NO_CATCH_UNWIND` from
/// inherited env. `PLUSHIE_FORMAT` and `PLUSHIE_TRANSPORT` are host-side.
/// All other `PLUSHIE_*` names are either host-only, launcher-set, or
/// secrets (e.g. `PLUSHIE_TOKEN`) that the renderer subprocess must not
/// receive.
const PLUSHIE_EXACT: &[&str] = &["PLUSHIE_NO_CATCH_UNWIND"];

/// Build the filtered env for a renderer child process.
///
/// Iterates the current process env and keeps only variables that
/// match an exact name in [`EXACT`] or start with a prefix in
/// [`PREFIXES`].
pub(crate) fn renderer_env() -> Vec<(String, String)> {
    std::env::vars().filter(|(k, _)| is_allowed(k)).collect()
}

/// Return true if `name` is allowed through to the renderer.
pub(crate) fn is_allowed(name: &str) -> bool {
    EXACT.contains(&name)
        || PLUSHIE_EXACT.contains(&name)
        || PREFIXES.iter().any(|p| name.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_servers_allowed() {
        assert!(is_allowed("DISPLAY"));
        assert!(is_allowed("WAYLAND_DISPLAY"));
        assert!(is_allowed("XDG_RUNTIME_DIR"));
    }

    #[test]
    fn secrets_rejected() {
        assert!(!is_allowed("AWS_ACCESS_KEY_ID"));
        assert!(!is_allowed("AWS_SECRET_ACCESS_KEY"));
        assert!(!is_allowed("GITHUB_TOKEN"));
        assert!(!is_allowed("DATABASE_URL"));
        assert!(!is_allowed("SSH_AUTH_SOCK"));
        assert!(!is_allowed("HTTP_COOKIE"));
        assert!(!is_allowed("API_KEY"));
    }

    #[test]
    fn locale_prefix_allowed() {
        assert!(is_allowed("LC_ALL"));
        assert!(is_allowed("LC_CTYPE"));
        assert!(is_allowed("LC_MESSAGES"));
    }

    #[test]
    fn graphics_prefixes_allowed() {
        assert!(is_allowed("MESA_DEBUG"));
        assert!(is_allowed("LIBGL_ALWAYS_SOFTWARE"));
        assert!(is_allowed("__GLX_VENDOR_LIBRARY_NAME"));
        assert!(is_allowed("VK_LAYER_PATH"));
        assert!(is_allowed("GALLIUM_DRIVER"));
    }

    #[test]
    fn plushie_no_catch_unwind_allowed() {
        assert!(is_allowed("PLUSHIE_NO_CATCH_UNWIND"));
    }

    #[test]
    fn plushie_closed_list_rejects_all_others() {
        // Every known PLUSHIE_* name except PLUSHIE_NO_CATCH_UNWIND must be
        // blocked. These are host-side, launcher-set, or secrets that the
        // renderer subprocess must not receive. If you need to add a name,
        // extend PLUSHIE_EXACT deliberately after review.
        let blocked = [
            "PLUSHIE_BINARY_PATH",
            "PLUSHIE_PACKAGE_DIR",
            "PLUSHIE_PACKAGE_READY_FILE",
            "PLUSHIE_SOCKET",
            "PLUSHIE_TOKEN",
            "PLUSHIE_TRANSPORT",
            "PLUSHIE_FORMAT",
            "PLUSHIE_RUST_SOURCE_PATH",
            "PLUSHIE_RELEASE_BASE_URL",
            "PLUSHIE_CACHE_DIR",
            "PLUSHIE_LAUNCHER_PATH",
            "PLUSHIE_TOOL_SOURCE_KIND",
            "PLUSHIE_LAUNCHER_QUIET",
            "PLUSHIE_UPDATE_SNAPSHOTS",
        ];
        for name in blocked {
            assert!(
                !is_allowed(name),
                "PLUSHIE_* var {name} must not forward to the renderer subprocess"
            );
        }
    }

    #[test]
    fn windows_critical_vars_allowed() {
        // The DLL loader, child process PATHEXT lookups, and the temp dir
        // resolver all rely on these. The whitelist is the same on
        // every platform; on non-Windows hosts they just won't appear
        // in the env to be passed through.
        assert!(is_allowed("SystemRoot"));
        assert!(is_allowed("WINDIR"));
        assert!(is_allowed("PATHEXT"));
        assert!(is_allowed("TEMP"));
        assert!(is_allowed("TMP"));
    }

    #[test]
    fn home_and_user_allowed_but_nothing_sneakily_alike() {
        assert!(is_allowed("HOME"));
        assert!(is_allowed("USER"));
        assert!(!is_allowed("HOMEBREW_PREFIX"));
        assert!(!is_allowed("USERDATA"));
    }

    // Naked `is_allowed` check against plausibly-leaky names covers
    // the whitelist invariant without touching the global process
    // env (std::env::set_var is unsafe workspace-wide).
    #[test]
    fn typical_secret_names_are_rejected() {
        let leaky = [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "GITHUB_TOKEN",
            "GITLAB_TOKEN",
            "DATABASE_URL",
            "SSH_AUTH_SOCK",
            "HTTP_COOKIE",
            "BEARER_TOKEN",
            "OAUTH_CLIENT_SECRET",
            "MY_CUSTOM_SECRET",
        ];
        for name in leaky {
            assert!(
                !is_allowed(name),
                "secret-like env var {name} must not pass the renderer whitelist"
            );
        }
    }
}
