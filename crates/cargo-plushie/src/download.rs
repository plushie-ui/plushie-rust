//! Download subcommand: fetch a stock renderer from GitHub releases.
//!
//! Ported shape from Elixir's `mix plushie.download` with the URL
//! scheme adapted to the plushie-rust repository. The binary lives
//! under `bin/` alongside its `.sha256` sidecar so
//! `wire_discovery` can pick it up.

use crate::{Error, Result, platform};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Canonical base URL for Plushie renderer releases.
pub const RELEASE_BASE_URL: &str = "https://github.com/plushie-ui/plushie-rust/releases/download";
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

/// Resolved paths for the download target.
#[derive(Debug)]
pub struct DownloadTarget {
    /// Absolute path the renderer binary will live at.
    pub binary_path: PathBuf,
    /// Absolute path to the `.sha256` sidecar.
    pub sha256_path: PathBuf,
    /// GitHub releases URL for the binary.
    pub binary_url: String,
    /// GitHub releases URL for the `.sha256` sidecar.
    pub sha256_url: String,
}

impl DownloadTarget {
    /// Compute the target paths + URLs without doing any I/O.
    ///
    /// `project_dir` is the app directory. The release artifact name
    /// follows the `{os}-{arch}` convention, while the local installed
    /// filename is stable.
    #[must_use]
    pub fn new(project_dir: &Path, version: &str) -> Self {
        let download_name = platform::download_name();
        let bin_dir = project_dir.join("bin");
        let binary_path = bin_dir.join(platform::renderer_name());
        let sha256_path = bin_dir.join(format!("{}.sha256", platform::renderer_name()));
        let binary_url = format!("{RELEASE_BASE_URL}/v{version}/{download_name}");
        let sha256_url = format!("{binary_url}.sha256");
        Self {
            binary_path,
            sha256_path,
            binary_url,
            sha256_url,
        }
    }
}

/// Fetch `url` into memory, returning the raw bytes.
///
/// # Errors
///
/// Wraps transport + HTTP errors in [`Error::Other`].
pub fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("GET {url} failed: {e}"))?;
    let mut reader = response.into_reader().take(MAX_DOWNLOAD_BYTES + 1);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err(Error::Other(anyhow::anyhow!(
            "download from {url} exceeded {} bytes",
            MAX_DOWNLOAD_BYTES
        )));
    }
    Ok(bytes)
}

/// Verify that `binary` hashes to the SHA-256 recorded in
/// `expected_sidecar`.
///
/// `expected_sidecar` is the raw content of a `.sha256` file of the
/// form `"<hex>  filename\n"`. The filename is ignored; only the hex
/// prefix participates in the comparison.
///
/// # Errors
///
/// Returns [`Error::Other`] with context when the sidecar is
/// malformed or the digest doesn't match.
pub fn verify_sha256(binary: &[u8], expected_sidecar: &str) -> Result<()> {
    let expected_hex = expected_sidecar
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("sha256 sidecar is empty"))?
        .to_ascii_lowercase();
    if expected_hex.len() != 64 || !expected_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Other(anyhow::anyhow!(
            "sha256 sidecar digest must be 64 hex characters"
        )));
    }

    let mut hasher = Sha256::new();
    hasher.update(binary);
    let actual_hex = format!("{:x}", hasher.finalize());

    if actual_hex != expected_hex {
        return Err(Error::Other(anyhow::anyhow!(
            "sha256 mismatch: expected {expected_hex}, got {actual_hex}"
        )));
    }
    Ok(())
}

/// Install the binary at `target.binary_path`, marking it executable
/// on Unix.
///
/// # Errors
///
/// Returns [`Error::Io`] when file I/O fails.
pub fn install_binary(target: &DownloadTarget, bytes: &[u8], sidecar: &str) -> Result<()> {
    if let Some(parent) = target.binary_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target.binary_path, bytes)?;
    std::fs::write(&target.sha256_path, sidecar)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&target.binary_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&target.binary_path, perms)?;
    }
    Ok(())
}

/// Refuse to download if native widgets are present.
///
/// Mirrors Elixir's safety gate at `plushie.download.ex:63`.
///
/// # Errors
///
/// Returns [`Error::DownloadWithNativeWidgets`] with the offending
/// crate names if any widget metadata was discovered.
pub fn refuse_if_native_widgets(widgets: &[crate::WidgetMetadata]) -> Result<()> {
    if !widgets.is_empty() {
        let names: Vec<String> = widgets.iter().map(|w| w.crate_name.clone()).collect();
        return Err(Error::DownloadWithNativeWidgets {
            widgets: names.join(", "),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_resolves_paths_and_urls() {
        let target = DownloadTarget::new(Path::new("/project"), "0.6.1");
        assert!(target.binary_url.contains("v0.6.1"));
        assert!(target.sha256_url.ends_with(".sha256"));
        assert!(target.binary_path.starts_with("/project/bin"));
    }

    #[test]
    fn verifies_matching_sha256() {
        let bytes = b"hello";
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let hex = format!("{:x}", hasher.finalize());
        let sidecar = format!("{hex}  bin\n");
        assert!(verify_sha256(bytes, &sidecar).is_ok());
    }

    #[test]
    fn rejects_mismatching_sha256() {
        let err = verify_sha256(
            b"hello",
            "0000000000000000000000000000000000000000000000000000000000000000  bin\n",
        )
        .unwrap_err();
        assert!(matches!(err, Error::Other(_)));
    }

    #[test]
    fn rejects_malformed_sha256_sidecar() {
        let err = verify_sha256(b"hello", "deadbeef  bin\n").unwrap_err();
        assert!(matches!(err, Error::Other(_)));
        assert!(err.to_string().contains("64 hex characters"));
    }

    #[test]
    fn refuses_download_with_widgets() {
        let widgets = vec![crate::WidgetMetadata {
            crate_name: "my-gauge".to_string(),
            crate_path: PathBuf::new(),
            type_name: "my_gauge".to_string(),
            constructor: "x::y()".to_string(),
        }];
        let err = refuse_if_native_widgets(&widgets).unwrap_err();
        assert!(matches!(err, Error::DownloadWithNativeWidgets { .. }));
    }

    #[test]
    fn allows_download_without_widgets() {
        assert!(refuse_if_native_widgets(&[]).is_ok());
    }
}
