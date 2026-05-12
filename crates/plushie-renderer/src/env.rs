//! Environment whitelist for renderer-spawned host commands.
//!
//! `plushie-renderer --exec` starts a host command from inside the
//! renderer process. Without filtering, that child inherits the full
//! renderer environment. This mirrors the Rust SDK renderer subprocess
//! whitelist so exec children get display, locale, graphics, and
//! plushie control variables without inheriting unrelated secrets.

/// Variables passed through to exec children unchanged when set in
/// the renderer environment.
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
    // Windows: required for DLL loader, exec resolution, and tempdir.
    "SystemRoot",
    "WINDIR",
    "PATHEXT",
    "TEMP",
    "TMP",
];

/// Prefix patterns. Any variable whose name starts with one of these
/// prefixes passes through.
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

/// Build the filtered env for a renderer-spawned child process.
pub(crate) fn child_env() -> Vec<(String, String)> {
    child_env_from(std::env::vars())
}

fn child_env_from<I, K, V>(vars: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    vars.into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .filter(|(k, _)| is_allowed(k))
        .collect()
}

/// Return true if `name` is allowed through to exec children.
pub(crate) fn is_allowed(name: &str) -> bool {
    EXACT.contains(&name) || PREFIXES.iter().any(|p| name.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_required_runtime_env() {
        assert!(is_allowed("DISPLAY"));
        assert!(is_allowed("WAYLAND_DISPLAY"));
        assert!(is_allowed("XDG_RUNTIME_DIR"));
        assert!(is_allowed("PATH"));
        assert!(is_allowed("LANG"));
        assert!(is_allowed("RUST_LOG"));
        assert!(is_allowed("HOME"));
        assert!(is_allowed("USER"));
    }

    #[test]
    fn preserves_plushie_listen_env() {
        assert!(is_allowed("PLUSHIE_SOCKET"));
        assert!(is_allowed("PLUSHIE_TOKEN"));
        assert!(is_allowed("PLUSHIE_UPDATE_SNAPSHOTS"));
    }

    #[test]
    fn rejects_common_secret_env() {
        let leaky = [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "GITHUB_TOKEN",
            "DATABASE_URL",
            "SSH_AUTH_SOCK",
            "HTTP_COOKIE",
            "API_KEY",
            "MY_CUSTOM_SECRET",
        ];

        for name in leaky {
            assert!(
                !is_allowed(name),
                "secret-like env var {name} must not pass the exec whitelist"
            );
        }
    }

    #[test]
    fn filters_env_without_touching_process_state() {
        let env = child_env_from([
            ("PATH", "/bin"),
            ("PLUSHIE_SOCKET", "/tmp/plushie.sock"),
            ("PLUSHIE_TOKEN", "token"),
            ("AWS_SECRET_ACCESS_KEY", "secret"),
            ("DATABASE_URL", "postgres://secret"),
        ]);

        assert_eq!(
            env,
            vec![
                ("PATH".to_string(), "/bin".to_string()),
                (
                    "PLUSHIE_SOCKET".to_string(),
                    "/tmp/plushie.sock".to_string()
                ),
                ("PLUSHIE_TOKEN".to_string(), "token".to_string()),
            ]
        );
    }
}
