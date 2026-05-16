//! Bundled default Plushie icon assets.

use crate::{Error, Result};
use std::path::{Path, PathBuf};

/// One built-in icon asset shipped with `cargo-plushie`.
pub struct DefaultIcon {
    /// File name to write when exporting the icon.
    pub name: &'static str,
    /// PNG bytes for the icon.
    pub bytes: &'static [u8],
}

const DEFAULT_ICONS: &[DefaultIcon] = &[
    DefaultIcon {
        name: "default-app-icon-16.png",
        bytes: include_bytes!("../assets/default-icons/default-app-icon-16.png"),
    },
    DefaultIcon {
        name: "default-app-icon-32.png",
        bytes: include_bytes!("../assets/default-icons/default-app-icon-32.png"),
    },
    DefaultIcon {
        name: "default-app-icon-180.png",
        bytes: include_bytes!("../assets/default-icons/default-app-icon-180.png"),
    },
    DefaultIcon {
        name: "default-app-icon-192.png",
        bytes: include_bytes!("../assets/default-icons/default-app-icon-192.png"),
    },
    DefaultIcon {
        name: "default-app-icon-512.png",
        bytes: include_bytes!("../assets/default-icons/default-app-icon-512.png"),
    },
];

/// Return the default icon assets bundled into this tool.
#[must_use]
pub fn default_icons() -> &'static [DefaultIcon] {
    DEFAULT_ICONS
}

/// Write the bundled default icons into `out_dir`.
///
/// # Errors
///
/// Returns an error if the output directory cannot be created, an icon
/// cannot be written, or a bundled icon has an invalid file name.
pub fn write_default_icons(out_dir: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(out_dir)?;
    let mut written = Vec::new();

    for icon in default_icons() {
        validate_icon_name(icon.name)?;
        let path = out_dir.join(icon.name);
        std::fs::write(&path, icon.bytes)?;
        written.push(path);
    }

    Ok(written)
}

fn validate_icon_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    if path.components().count() != 1 || path.file_name().and_then(|v| v.to_str()) != Some(name) {
        return Err(Error::Other(anyhow::anyhow!(
            "default icon name must be a plain file name: {name}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{default_icons, write_default_icons};
    use tempfile::tempdir;

    #[test]
    fn bundled_default_icons_have_png_payloads() {
        for icon in default_icons() {
            assert!(icon.name.ends_with(".png"));
            assert!(icon.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        }
    }

    #[test]
    fn writes_bundled_default_icons() {
        let dir = tempdir().unwrap();

        let written = write_default_icons(dir.path()).unwrap();

        let names: Vec<_> = default_icons().iter().map(|icon| icon.name).collect();
        for name in names {
            assert!(written.iter().any(|path| path.file_name().unwrap() == name));
            let bytes = std::fs::read(dir.path().join(name)).unwrap();
            let expected = default_icons()
                .iter()
                .find(|icon| icon.name == name)
                .unwrap()
                .bytes;
            assert_eq!(bytes, expected);
        }
    }
}
