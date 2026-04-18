//! Renderer workspace generator.
//!
//! Produces `target/plushie-renderer/{Cargo.toml, src/main.rs}` with
//! every native widget wired into a `PlushieAppBuilder`. Ported from
//! Elixir's `generate_workspace/4` in `plushie.build.ex`.
//!
//! [`write_if_changed`] preserves Cargo's mtime-based rebuild detection
//! by skipping the write when the content is identical to what's
//! already on disk.

use crate::{Error, Result, WidgetMetadata};
use std::path::{Path, PathBuf};

/// Configuration for a workspace generation pass.
pub struct WorkspaceConfig<'a> {
    /// Absolute path to the app crate's manifest directory.
    pub app_manifest_dir: &'a Path,
    /// Absolute path to the generated workspace root (normally
    /// `{target_dir}/plushie-renderer/`).
    pub output_dir: &'a Path,
    /// Binary name for the generated renderer. If `None`, derive as
    /// `{app_name}-renderer` from the app manifest.
    pub binary_name: Option<String>,
    /// Name of the app crate (used to derive `binary_name` when the
    /// caller didn't supply one).
    pub app_name: &'a str,
    /// Workspace version string (written into the generated
    /// Cargo.toml's `[package].version`).
    pub workspace_version: &'a str,
    /// Optional `PLUSHIE_RUST_SOURCE_PATH` pointing at a local
    /// plushie-rust checkout; when set, the generated Cargo.toml
    /// emits `[patch.crates-io]` forwarding entries for the
    /// plushie crates (and any patches declared at the source
    /// workspace root).
    pub source_path: Option<PathBuf>,
    /// Widgets to register (already validated by the collision
    /// checks in `discover`).
    pub widgets: &'a [WidgetMetadata],
}

impl WorkspaceConfig<'_> {
    /// Resolved binary name. Defaults to `{app_name}-renderer` with
    /// dashes preserved.
    #[must_use]
    pub fn resolved_binary_name(&self) -> String {
        self.binary_name
            .clone()
            .unwrap_or_else(|| format!("{}-renderer", self.app_name.replace('_', "-")))
    }
}

/// Write `content` to `path` only if the on-disk contents differ.
///
/// Mirrors Elixir's `write_if_changed/2` (nine lines). The optimisation
/// is load-bearing: identical content at the same mtime keeps Cargo
/// from rebuilding, so a no-op invocation of `cargo plushie build` is
/// genuinely instant.
///
/// # Errors
///
/// Propagates the read or write failure from [`std::fs`].
pub fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Ok(existing) = std::fs::read_to_string(path)
        && existing == content
    {
        return Ok(());
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// Matches identifiers, paths, and simple constructor invocations.
///
/// Mirrors Elixir's `@rust_constructor_pattern` so we accept the same
/// set of expressions.
fn constructor_is_valid(expr: &str) -> bool {
    // Must start with a letter or underscore, followed by characters
    // valid in a path (letters, digits, underscores, colons, angle
    // brackets, commas, spaces). Optionally end in a `()` call.
    if expr.is_empty() {
        return false;
    }
    let bytes = expr.as_bytes();
    let first = bytes[0];
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    let mut chars = expr.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | ':' | '<' | '>' | ',' | ' ' => {}
            '(' => {
                // Balance: consume until matching ')'.
                let mut depth = 1;
                for nc in chars.by_ref() {
                    match nc {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if depth != 0 {
                    return false;
                }
                // Only trailing whitespace allowed after the closing paren.
                return chars.all(|c| c.is_whitespace());
            }
            _ => return false,
        }
    }
    // No parens means the expression must be a pure path.
    true
}

/// Generate `Cargo.toml` and `src/main.rs` under the output directory.
///
/// # Errors
///
/// Returns [`Error::InvalidConstructor`] if any widget's declared
/// `constructor` doesn't look like a valid Rust path, and propagates
/// [`std::io::Error`] from writes.
pub fn generate_workspace(config: &WorkspaceConfig<'_>) -> Result<()> {
    for w in config.widgets {
        if !constructor_is_valid(&w.constructor) {
            return Err(Error::InvalidConstructor {
                crate_name: w.crate_name.clone(),
                constructor: w.constructor.clone(),
            });
        }
    }

    let cargo_toml = render_cargo_toml(config);
    write_if_changed(&config.output_dir.join("Cargo.toml"), &cargo_toml)?;

    let main_rs = render_main_rs(config);
    write_if_changed(&config.output_dir.join("src/main.rs"), &main_rs)?;

    Ok(())
}

fn render_cargo_toml(config: &WorkspaceConfig<'_>) -> String {
    let bin_name = config.resolved_binary_name();
    let package_name = bin_name.replace('-', "_");

    let mut out = String::new();
    out.push_str("# Auto-generated by `cargo plushie build`. Do not edit.\n\n");
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{package_name}\"\n"));
    out.push_str(&format!("version = \"{}\"\n", config.workspace_version));
    out.push_str("edition = \"2024\"\n\n");
    out.push_str("[[bin]]\n");
    out.push_str(&format!("name = \"{bin_name}\"\n"));
    out.push_str("path = \"src/main.rs\"\n\n");
    out.push_str("[dependencies]\n");

    let (sdk_line, renderer_line) = if let Some(source) = &config.source_path {
        let sdk_abs = source.join("crates/plushie-widget-sdk");
        let ren_abs = source.join("crates/plushie-renderer");
        (
            format!(
                "plushie-widget-sdk = {{ path = {:?} }}",
                sdk_abs.display().to_string()
            ),
            format!(
                "plushie-renderer = {{ path = {:?} }}",
                ren_abs.display().to_string()
            ),
        )
    } else {
        (
            format!("plushie-widget-sdk = \"{}\"", config.workspace_version),
            format!("plushie-renderer = \"{}\"", config.workspace_version),
        )
    };
    out.push_str(&format!("{sdk_line}\n"));
    out.push_str(&format!("{renderer_line}\n"));

    for w in config.widgets {
        let crate_basename = w
            .crate_path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| w.crate_name.clone());
        out.push_str(&format!(
            "{} = {{ path = {:?}, features = [\"impl\"] }}\n",
            crate_basename,
            w.crate_path.display().to_string()
        ));
    }

    if let Some(source) = &config.source_path {
        out.push('\n');
        // Emit patches for every plushie-rust crate plus any
        // forwarded non-plushie patches (plushie-iced overrides, etc.)
        // declared at the source workspace root. The scratch
        // `.cargo/config.toml` written by `patch_config` uses the same
        // entries so both files stay in sync.
        let entries = crate::patch_config::all_patches(source);
        out.push_str(&crate::patch_config::render_patch_block(&entries));
    }

    out
}

fn render_main_rs(config: &WorkspaceConfig<'_>) -> String {
    let mut body = String::new();
    body.push_str(
        "// Auto-generated by `cargo plushie build`. Do not edit.\n\n\
         use plushie_widget_sdk::app::PlushieAppBuilder;\n\n\
         fn main() -> plushie_widget_sdk::iced::Result {\n    \
         let builder = PlushieAppBuilder::new()",
    );
    for w in config.widgets {
        body.push_str(&format!("\n        .widget({})", w.constructor));
    }
    body.push_str(";\n    plushie_renderer::run(builder)\n}\n");
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn sample_widgets() -> Vec<WidgetMetadata> {
        vec![WidgetMetadata {
            crate_name: "my-gauge".to_string(),
            crate_path: PathBuf::from("/abs/native/gauge"),
            type_name: "my_gauge".to_string(),
            constructor: "my_gauge::Gauge::new()".to_string(),
        }]
    }

    #[test]
    fn write_if_changed_skips_identical_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");
        write_if_changed(&path, "hello").unwrap();
        let mtime1 = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        write_if_changed(&path, "hello").unwrap();
        let mtime2 = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2);
    }

    #[test]
    fn write_if_changed_writes_new_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");
        write_if_changed(&path, "hello").unwrap();
        write_if_changed(&path, "goodbye").unwrap();
        let read = std::fs::read_to_string(&path).unwrap();
        assert_eq!(read, "goodbye");
    }

    #[test]
    fn constructor_pattern_accepts_paths_and_calls() {
        assert!(constructor_is_valid("my_gauge::Gauge::new()"));
        assert!(constructor_is_valid("MyType::new()"));
        assert!(constructor_is_valid("create_widget()"));
        assert!(constructor_is_valid("my::path::Type<T>::new()"));
        assert!(constructor_is_valid("MyType::<i32>::new()"));
        assert!(constructor_is_valid("bare_path"));
    }

    #[test]
    fn constructor_pattern_rejects_garbage() {
        assert!(!constructor_is_valid(""));
        assert!(!constructor_is_valid("1abc::fn()"));
        assert!(!constructor_is_valid("rm -rf /"));
        assert!(!constructor_is_valid("my_gauge::Gauge::new(x)trailing"));
        assert!(!constructor_is_valid("`injected`"));
    }

    #[test]
    fn generator_validates_constructor() {
        let dir = tempdir().unwrap();
        let widget = WidgetMetadata {
            crate_name: "bad-widget".to_string(),
            crate_path: PathBuf::from("/abs/native/bad"),
            type_name: "bad".to_string(),
            constructor: "rm -rf /".to_string(),
        };
        let widgets = vec![widget];
        let config = WorkspaceConfig {
            app_manifest_dir: Path::new("/app"),
            output_dir: dir.path(),
            binary_name: None,
            app_name: "app",
            workspace_version: "0.6.1",
            source_path: None,
            widgets: &widgets,
        };
        let err = generate_workspace(&config).unwrap_err();
        assert!(matches!(err, Error::InvalidConstructor { .. }));
    }

    #[test]
    fn renders_cargo_toml_with_widget() {
        let widgets = sample_widgets();
        let config = WorkspaceConfig {
            app_manifest_dir: Path::new("/app"),
            output_dir: Path::new("/app/target/plushie-renderer"),
            binary_name: None,
            app_name: "my-app",
            workspace_version: "0.6.1",
            source_path: None,
            widgets: &widgets,
        };
        let cargo = render_cargo_toml(&config);
        assert!(cargo.contains("name = \"my_app_renderer\""));
        assert!(cargo.contains("plushie-widget-sdk = \"0.6.1\""));
        assert!(cargo.contains("features = [\"impl\"]"));
        assert!(cargo.contains("gauge"));
    }

    #[test]
    fn renders_cargo_toml_with_source_path() {
        let widgets = sample_widgets();
        let config = WorkspaceConfig {
            app_manifest_dir: Path::new("/app"),
            output_dir: Path::new("/app/target/plushie-renderer"),
            binary_name: Some("custom-renderer".to_string()),
            app_name: "my-app",
            workspace_version: "0.6.1",
            source_path: Some(PathBuf::from("/src/plushie-rust")),
            widgets: &widgets,
        };
        let cargo = render_cargo_toml(&config);
        assert!(cargo.contains("name = \"custom_renderer\""));
        assert!(cargo.contains("[patch.crates-io]"));
        assert!(cargo.contains("crates/plushie-widget-sdk"));
        assert!(cargo.contains("crates/plushie-renderer"));
    }

    #[test]
    fn renders_main_rs_registers_every_widget() {
        let widgets = sample_widgets();
        let config = WorkspaceConfig {
            app_manifest_dir: Path::new("/app"),
            output_dir: Path::new("/app/target/plushie-renderer"),
            binary_name: None,
            app_name: "my-app",
            workspace_version: "0.6.1",
            source_path: None,
            widgets: &widgets,
        };
        let main_rs = render_main_rs(&config);
        assert!(main_rs.contains("PlushieAppBuilder::new()"));
        assert!(main_rs.contains(".widget(my_gauge::Gauge::new())"));
        assert!(main_rs.contains("plushie_renderer::run(builder)"));
    }
}
