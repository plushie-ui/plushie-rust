//! Platform detection helpers for build + download paths.
//!
//! Mirrors Elixir's `Plushie.Binary.os_name/0` and `arch_name/0`.

/// Returns the `{os}` fragment used in download file names
/// (`linux`, `darwin`, `windows`).
#[must_use]
pub const fn os_name() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "macos")]
    {
        "darwin"
    }
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        "unknown"
    }
}

/// Returns the `{arch}` fragment used in download file names
/// (`x86_64`, `aarch64`).
#[must_use]
pub const fn arch_name() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }
    #[cfg(target_arch = "aarch64")]
    {
        "aarch64"
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        "unknown"
    }
}

/// Returns the extension for executables on this platform
/// (`.exe` on Windows, `""` elsewhere).
#[must_use]
pub const fn exe_suffix() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        ".exe"
    }
    #[cfg(not(target_os = "windows"))]
    {
        ""
    }
}

/// Returns the download file name for the stock renderer on this
/// platform. Format: `plushie-renderer-{os}-{arch}[.exe]`.
#[must_use]
pub fn download_name() -> String {
    format!(
        "plushie-renderer-{}-{}{}",
        os_name(),
        arch_name(),
        exe_suffix()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_name_well_formed() {
        let name = download_name();
        assert!(name.starts_with("plushie-renderer-"));
    }
}
