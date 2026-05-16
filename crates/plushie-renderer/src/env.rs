//! Environment whitelist for renderer-spawned host commands.
//!
//! `plushie-renderer --exec-bin` starts a host command from inside
//! the renderer process. Without filtering, that child inherits the
//! full renderer environment. The renderer always sets `PLUSHIE_SOCKET`,
//! `PLUSHIE_TOKEN`, and `PLUSHIE_TOKEN_SHA256` explicitly on the child
//! after `env_clear`. No `PLUSHIE_*` prefix forwarding is needed: what
//! the host needs is set deliberately, not inherited from ambient env.
//! The user can forward additional names via `--exec-env NAME[,NAME]`.
//!
//! The renderer subprocess in spawn (host-parent) mode reads at most
//! `PLUSHIE_NO_CATCH_UNWIND` from its inherited env. Other `PLUSHIE_*`
//! names are host-side, launcher-set, or secrets that must not leak
//! across the process boundary. Adding a new `PLUSHIE_*` var to the
//! list below is a deliberate review decision.

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
];

/// Build the filtered env for a renderer-spawned child process.
pub(crate) fn child_env(extra_names: &[String]) -> Vec<(String, String)> {
    child_env_from(std::env::vars(), extra_names)
}

fn child_env_from<I, K, V>(vars: I, extra_names: &[String]) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    vars.into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .filter(|(k, _)| is_allowed_with_extra(k, extra_names))
        .collect()
}

/// Return true if `name` is allowed through to exec children.
pub(crate) fn is_allowed(name: &str) -> bool {
    EXACT.contains(&name) || PREFIXES.iter().any(|p| name.starts_with(p))
}

fn is_allowed_with_extra(name: &str, extra_names: &[String]) -> bool {
    is_allowed(name) || extra_names.iter().any(|extra| extra == name)
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
    fn rejects_plushie_prefix_from_ambient_env() {
        // PLUSHIE_SOCKET, PLUSHIE_TOKEN, and PLUSHIE_TOKEN_SHA256 are set
        // explicitly on the child command after env_clear; they must not
        // pass through from ambient renderer env, which would allow
        // unintended PLUSHIE_* names to leak. Adding a name here is a
        // deliberate review decision.
        let blocked = [
            "PLUSHIE_SOCKET",
            "PLUSHIE_TOKEN",
            "PLUSHIE_TOKEN_SHA256",
            "PLUSHIE_UPDATE_SNAPSHOTS",
            "PLUSHIE_BINARY_PATH",
            "PLUSHIE_PACKAGE_DIR",
            "PLUSHIE_PACKAGE_READY_FILE",
            "PLUSHIE_TRANSPORT",
            "PLUSHIE_FORMAT",
            "PLUSHIE_RUST_SOURCE_PATH",
            "PLUSHIE_RELEASE_BASE_URL",
            "PLUSHIE_CACHE_DIR",
            "PLUSHIE_LAUNCHER_PATH",
            "PLUSHIE_TOOL_SOURCE_KIND",
            "PLUSHIE_LAUNCHER_QUIET",
            "PLUSHIE_NO_CATCH_UNWIND",
        ];
        for name in blocked {
            assert!(
                !is_allowed(name),
                "PLUSHIE_* var {name} must not forward from ambient renderer env to exec child"
            );
        }
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
        // PLUSHIE_SOCKET and PLUSHIE_TOKEN are set explicitly on the child
        // command by the caller; they must not pass through from ambient env.
        let env = child_env_from(
            [
                ("PATH", "/bin"),
                ("PLUSHIE_SOCKET", "/tmp/plushie.sock"),
                ("PLUSHIE_TOKEN", "token"),
                ("AWS_SECRET_ACCESS_KEY", "secret"),
                ("DATABASE_URL", "postgres://secret"),
            ],
            &[],
        );

        assert_eq!(env, vec![("PATH".to_string(), "/bin".to_string()),]);
    }

    #[test]
    fn preserves_explicit_extra_env_names() {
        let extra_names = vec!["MIX_HOME".to_string(), "HEX_HOME".to_string()];
        let env = child_env_from(
            [
                ("PATH", "/bin"),
                ("MIX_HOME", "/tmp/mix"),
                ("HEX_HOME", "/tmp/hex"),
                ("DATABASE_URL", "postgres://secret"),
            ],
            &extra_names,
        );

        assert_eq!(
            env,
            vec![
                ("PATH".to_string(), "/bin".to_string()),
                ("MIX_HOME".to_string(), "/tmp/mix".to_string()),
                ("HEX_HOME".to_string(), "/tmp/hex".to_string()),
            ]
        );
    }
}
