//! Scaffolder for `cargo plushie new-widget`.
//!
//! Produces a fresh widget crate on disk with the conventional
//! `[package.metadata.plushie.widget]` layout and an optional `impl`
//! feature that pulls in `plushie-widget-sdk` for the renderer-side
//! implementation.
//!
//! The scaffolders are intentionally template-based rather than
//! shelling out to `cargo new`: the generated files are tiny, and
//! keeping the file generation in-process sidesteps Cargo's
//! interactive prompts and VCS behaviour (init-git, ignored files,
//! etc.) that would otherwise need to be unwound.

use crate::{Error, Result};
use std::path::{Path, PathBuf};

/// Produce the snake_case form of a kebab-case identifier.
fn to_snake_case(name: &str) -> String {
    name.replace('-', "_")
}

/// Produce the PascalCase form of a kebab- or snake-case identifier.
fn to_pascal_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper_next = true;
    for ch in name.chars() {
        if ch == '-' || ch == '_' {
            upper_next = true;
            continue;
        }
        if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Validate that `name` is a well-formed kebab-case identifier
/// suitable for both a Cargo crate name and a widget type name.
///
/// Rules (matching Cargo's own package-name rules, minus the
/// underscore-allowed form to keep the snake_case -> kebab-case
/// conversion deterministic):
///
/// - Non-empty.
/// - ASCII letters, digits, and `-` only.
/// - First character must be an ASCII letter.
/// - No consecutive `-`.
/// - Does not start or end with `-`.
fn validate_kebab_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::Other(anyhow::anyhow!("name must not be empty")));
    }
    let first = name.as_bytes()[0];
    if !first.is_ascii_alphabetic() {
        return Err(Error::Other(anyhow::anyhow!(
            "name `{name}` must start with an ASCII letter"
        )));
    }
    let mut prev_dash = false;
    for (i, b) in name.bytes().enumerate() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => prev_dash = false,
            b'-' => {
                if prev_dash {
                    return Err(Error::Other(anyhow::anyhow!(
                        "name `{name}` contains consecutive `-`"
                    )));
                }
                if i + 1 == name.len() {
                    return Err(Error::Other(anyhow::anyhow!(
                        "name `{name}` must not end with `-`"
                    )));
                }
                prev_dash = true;
            }
            _ => {
                return Err(Error::Other(anyhow::anyhow!(
                    "name `{name}` contains invalid character `{}`",
                    b as char
                )));
            }
        }
    }
    Ok(())
}

/// Resolve an optional `PLUSHIE_SOURCE_PATH` override to an absolute
/// plushie-rust checkout. Returns `None` if the env var is unset or
/// the path does not exist.
fn source_path_override() -> Option<PathBuf> {
    let path = std::env::var_os("PLUSHIE_SOURCE_PATH")?;
    let buf = PathBuf::from(path);
    std::fs::canonicalize(&buf).ok()
}

/// Input for `cargo plushie new-widget`.
pub struct NewWidgetOpts<'a> {
    /// Kebab-case widget crate name (e.g. `my-gauge`).
    pub name: &'a str,
    /// Destination directory; defaults to `./native/<name>`.
    pub path: Option<&'a Path>,
    /// Reserved widget type names the scaffold must not conflict with.
    pub builtin_type_names: &'a [&'a str],
}

/// Outcome returned from [`scaffold_widget`]. The caller is expected
/// to print a summary to stdout using the paths it carries.
#[derive(Debug)]
pub struct ScaffoldResult {
    /// Absolute path to the new crate root.
    pub crate_root: PathBuf,
}

/// Create a new widget crate rooted at `opts.path` (or `./native/<name>`).
///
/// # Errors
///
/// - `name` must be kebab-case (see [`validate_kebab_name`]).
/// - `name` must not shadow a built-in widget type name.
/// - The destination must not already exist.
pub fn scaffold_widget(opts: &NewWidgetOpts<'_>) -> Result<ScaffoldResult> {
    validate_kebab_name(opts.name)?;
    let type_name = to_snake_case(opts.name);
    if opts.builtin_type_names.contains(&type_name.as_str()) {
        return Err(Error::BuiltinCollision {
            crate_name: opts.name.to_string(),
            type_name,
        });
    }

    let default_path = PathBuf::from("native").join(opts.name);
    let target = opts.path.map(Path::to_path_buf).unwrap_or(default_path);
    if target.exists() {
        return Err(Error::Other(anyhow::anyhow!(
            "destination `{}` already exists; pick another path",
            target.display()
        )));
    }

    std::fs::create_dir_all(target.join("src"))?;

    let struct_name = to_pascal_case(opts.name);
    let module_name = to_snake_case(opts.name);
    let factory_name = format!("{struct_name}Factory");
    let cargo_toml = render_widget_cargo_toml(opts.name, &type_name, &module_name, &factory_name);
    let lib_rs = render_widget_lib_rs(&type_name, &struct_name, &factory_name);

    std::fs::write(target.join("Cargo.toml"), cargo_toml)?;
    std::fs::write(target.join("src").join("lib.rs"), lib_rs)?;
    maybe_write_iced_paths_override(&target)?;

    let crate_root = std::fs::canonicalize(&target).unwrap_or(target);
    Ok(ScaffoldResult { crate_root })
}

/// When `PLUSHIE_SOURCE_PATH` is set and a sibling `plushie-iced`
/// checkout exists, scaffold a `.cargo/config.toml` that forwards the
/// `paths = [".../plushie-iced"]` override so the scaffolded crate
/// can compile against the fork the same way the source workspace
/// does. Silently skipped when either piece is missing; apps that
/// publish against crates.io pick up the registry version.
fn maybe_write_iced_paths_override(target: &Path) -> Result<()> {
    let Some(source) = source_path_override() else {
        return Ok(());
    };
    let iced = source
        .parent()
        .map(|p| p.join("plushie-iced"))
        .filter(|p| p.is_dir());
    let Some(iced) = iced else {
        return Ok(());
    };
    let cargo_dir = target.join(".cargo");
    std::fs::create_dir_all(&cargo_dir)?;
    let body = format!(
        "# Forwards the PLUSHIE_SOURCE_PATH workspace's plushie-iced\n\
         # override so the scaffold compiles against the fork locally.\n\
         # Delete this file before publishing to crates.io.\n\
         paths = [{path:?}]\n",
        path = iced.display().to_string(),
    );
    std::fs::write(cargo_dir.join("config.toml"), body)?;
    Ok(())
}

/// Render the widget crate's `Cargo.toml`.
///
/// When `PLUSHIE_SOURCE_PATH` is set the scaffold emits path
/// dependencies pointing at the local checkout so the generated
/// crate compiles against workspace crates that haven't been
/// published yet. Without the env var, the scaffold targets
/// crates.io.
///
/// The `constructor` field names the factory type (not the builder)
/// so the custom renderer can register it with a zero-arg call.
fn render_widget_cargo_toml(
    name: &str,
    type_name: &str,
    module_name: &str,
    factory_name: &str,
) -> String {
    let mut out = String::new();
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{name}\"\n"));
    out.push_str("version = \"0.1.0\"\n");
    out.push_str("edition = \"2024\"\n\n");
    out.push_str("[package.metadata.plushie.widget]\n");
    out.push_str(&format!("type_name = \"{type_name}\"\n"));
    out.push_str(&format!(
        "constructor = \"{module_name}::factory::{factory_name}::new()\"\n\n"
    ));
    out.push_str("[features]\n");
    out.push_str("impl = [\"dep:plushie-widget-sdk\"]\n\n");
    out.push_str("[dependencies]\n");
    if let Some(source) = source_path_override() {
        let core = source.join("crates/plushie-core");
        let macros = source.join("crates/plushie-core-macros");
        let sdk = source.join("crates/plushie-widget-sdk");
        out.push_str(&format!(
            "plushie-core = {{ path = {:?} }}\n",
            core.display().to_string()
        ));
        out.push_str(&format!(
            "plushie-core-macros = {{ path = {:?} }}\n",
            macros.display().to_string()
        ));
        out.push_str(&format!(
            "plushie-widget-sdk = {{ path = {:?}, optional = true }}\n",
            sdk.display().to_string()
        ));
    } else {
        out.push_str("plushie-core = \"0.6\"\n");
        out.push_str("plushie-core-macros = \"0.6\"\n");
        out.push_str("plushie-widget-sdk = { version = \"0.6\", optional = true }\n");
    }
    out
}

/// Render the widget crate's `src/lib.rs`.
///
/// The stub (default features) is iced-free: the `widget!` builder
/// emits a `TreeNode` and the metadata constant the build tool reads.
/// Enabling the `impl` feature pulls in `plushie-widget-sdk` and
/// exposes a paired factory type that the custom renderer registers.
/// The factory's zero-arg `new()` is what `cargo plushie build`
/// injects into `PlushieAppBuilder::widget(...)`.
fn render_widget_lib_rs(type_name: &str, struct_name: &str, factory_name: &str) -> String {
    format!(
        r#"//! {type_name} - a custom plushie widget.
//!
//! The stub (default features) is iced-free and can be used by any
//! plushie app in wire mode. The `impl` feature adds the iced-based
//! renderer implementation used by the custom renderer binary.

use plushie_core::widget;

widget! {{
    /// {struct_name} widget builder. Used in `App::view` to declare
    /// a {type_name} node in the view tree.
    #[widget(type_name = "{type_name}", constructor = "{struct_name}Factory::new()")]
    pub struct {struct_name} {{
        /// Current value; clamped to `[0.0, max]` at render time.
        pub value: f32,
        /// Full-scale value for the gauge.
        pub max: f32,
    }}
}}

#[cfg(feature = "impl")]
pub mod factory {{
    //! Renderer-side factory for the {type_name} widget.
    //!
    //! Compiled only under the `impl` feature so the stub crate
    //! stays iced-free. The custom renderer generated by
    //! `cargo plushie build` imports this module and calls
    //! [`{factory_name}::new`] to register the widget.

    use plushie_widget_sdk::prelude::*;

    /// Zero-sized factory marker.
    #[derive(PlushieWidget, ::core::default::Default)]
    #[plushie_widget(type_name = "{type_name}")]
    pub struct {factory_name};

    impl {factory_name} {{
        /// Fresh factory instance. Invoked by the custom renderer's
        /// generated `main.rs` at startup.
        #[must_use]
        pub const fn new() -> Self {{
            Self
        }}
    }}

    impl<R: PlushieRenderer> PlushieWidgetRender<R> for {factory_name} {{
        fn render<'a>(
            &'a self,
            _node: &'a TreeNode,
            _ctx: &RenderCtx<'a, R>,
        ) -> PlushieElement<'a, R> {{
            // TODO: implement renderer-side drawing with the iced API.
            todo!("renderer impl for {factory_name}")
        }}
    }}
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_snake_case_replaces_dashes() {
        assert_eq!(to_snake_case("my-gauge"), "my_gauge");
        assert_eq!(to_snake_case("plain"), "plain");
    }

    #[test]
    fn to_pascal_case_capitalises_parts() {
        assert_eq!(to_pascal_case("my-gauge"), "MyGauge");
        assert_eq!(to_pascal_case("my_gauge"), "MyGauge");
        assert_eq!(to_pascal_case("gauge"), "Gauge");
        assert_eq!(to_pascal_case("my-colour-picker"), "MyColourPicker");
    }

    #[test]
    fn validate_kebab_name_accepts_standard_forms() {
        assert!(validate_kebab_name("gauge").is_ok());
        assert!(validate_kebab_name("my-gauge").is_ok());
        assert!(validate_kebab_name("gauge-v2").is_ok());
    }

    #[test]
    fn validate_kebab_name_rejects_invalid_forms() {
        assert!(validate_kebab_name("").is_err());
        assert!(validate_kebab_name("-leading").is_err());
        assert!(validate_kebab_name("trailing-").is_err());
        assert!(validate_kebab_name("double--dash").is_err());
        assert!(validate_kebab_name("1bad").is_err());
        assert!(validate_kebab_name("has_underscore").is_err());
        assert!(validate_kebab_name("has space").is_err());
    }

    #[test]
    fn scaffold_widget_refuses_builtin_shadow() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("button");
        let opts = NewWidgetOpts {
            name: "button",
            path: Some(&target),
            builtin_type_names: &["button", "text"],
        };
        let err = scaffold_widget(&opts).unwrap_err();
        assert!(matches!(err, Error::BuiltinCollision { .. }));
    }

    #[test]
    fn scaffold_widget_refuses_existing_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("gauge");
        std::fs::create_dir_all(&target).unwrap();
        let opts = NewWidgetOpts {
            name: "gauge",
            path: Some(&target),
            builtin_type_names: &[],
        };
        assert!(scaffold_widget(&opts).is_err());
    }

    #[test]
    fn scaffold_widget_writes_expected_files() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("my-gauge");
        let opts = NewWidgetOpts {
            name: "my-gauge",
            path: Some(&target),
            builtin_type_names: &[],
        };
        let result = scaffold_widget(&opts).unwrap();
        assert!(result.crate_root.join("Cargo.toml").is_file());
        assert!(result.crate_root.join("src/lib.rs").is_file());
        let cargo = std::fs::read_to_string(result.crate_root.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"my-gauge\""));
        assert!(cargo.contains("type_name = \"my_gauge\""));
        assert!(cargo.contains("my_gauge::factory::MyGaugeFactory::new()"));
        assert!(cargo.contains("impl = [\"dep:plushie-widget-sdk\"]"));
        let lib = std::fs::read_to_string(result.crate_root.join("src/lib.rs")).unwrap();
        assert!(lib.contains("pub struct MyGauge"));
        assert!(lib.contains("pub struct MyGaugeFactory"));
        assert!(lib.contains("#[cfg(feature = \"impl\")]"));
    }
}
