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
//! SDK (Elixir, Gleam, Python, Ruby, TypeScript): Elixir's 21 exact
//! names + 8 prefix classes, plus a dedicated `PLUSHIE_*` prefix for
//! plushie-reserved debug toggles. Anything not on the list is
//! stripped.
//!
//! The `PLUSHIE_*` prefix covers runtime debug toggles the renderer
//! reads (currently `PLUSHIE_NO_CATCH_UNWIND`, `PLUSHIE_TOKEN`,
//! `PLUSHIE_SOCKET`, `PLUSHIE_UPDATE_SNAPSHOTS`). Adding new toggles
//! no longer requires teaching every SDK about the individual name;
//! the prefix is reserved for plushie internals, so there is no
//! plausible secret stored under it.

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
];

/// Prefix patterns. Any variable whose name starts with one of these
/// prefixes passes through. The prefixes cover locale (LC_*), the
/// Mesa / GLX / Vulkan / Gallium graphics stack, accessibility bridge
/// settings, fontconfig, and the plushie-reserved PLUSHIE_* namespace
/// for renderer debug toggles.
const PREFIXES: &[&str] = &[
    "LC_",
    "MESA_",
    "LIBGL_",
    "__GLX_",
    "VK_",
    "GALLIUM_",
    "AT_SPI_",
    "FONTCONFIG_",
    "PLUSHIE_",
];

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
    EXACT.contains(&name) || PREFIXES.iter().any(|p| name.starts_with(p))
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
    fn plushie_prefix_allowed() {
        assert!(is_allowed("PLUSHIE_NO_CATCH_UNWIND"));
        assert!(is_allowed("PLUSHIE_UPDATE_SNAPSHOTS"));
        assert!(is_allowed("PLUSHIE_TOKEN"));
        assert!(is_allowed("PLUSHIE_SOCKET"));
        // Future toggles join without a whitelist edit.
        assert!(is_allowed("PLUSHIE_FUTURE_DEBUG_KNOB"));
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
