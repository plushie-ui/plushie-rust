//! Scaffolders for `cargo plushie new-widget` and `cargo plushie init`.
//!
//! `new-widget` produces a widget crate with the conventional
//! `[package.metadata.plushie.widget]` layout and an optional `impl`
//! feature that pulls in `plushie-widget-sdk` for the renderer-side
//! implementation. `init` produces a plushie app crate with the
//! `plushie::cli::run` easy-path main, an automation-script example,
//! and a sample `.plushie` script under `scripts/`.
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

/// Resolve an optional `PLUSHIE_RUST_SOURCE_PATH` override to an absolute
/// plushie-rust checkout. Returns `None` if the env var is unset or
/// the path does not exist.
fn source_path_override() -> Option<PathBuf> {
    let path = std::env::var_os("PLUSHIE_RUST_SOURCE_PATH")?;
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

/// Input for `cargo plushie init`.
pub struct InitOpts<'a> {
    /// Kebab-case app crate name (e.g. `my-app`).
    pub name: &'a str,
    /// Destination directory; defaults to `./<name>`.
    pub path: Option<&'a Path>,
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
/// - `name` must be kebab-case (ASCII letters, digits, and `-`; must
///   start with a letter; no consecutive or trailing `-`).
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

/// When `PLUSHIE_RUST_SOURCE_PATH` is set and a sibling `plushie-iced`
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
        "# Forwards the PLUSHIE_RUST_SOURCE_PATH workspace's plushie-iced\n\
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
/// When `PLUSHIE_RUST_SOURCE_PATH` is set the scaffold emits path
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
/// injects into `PlushieAppBuilder::widget(...)`. The constructor
/// string lives in the crate's Cargo.toml as the single source of
/// truth; the `widget!` attribute carries only the wire type name.
///
/// The scaffolded `render` body does real work: it reads the typed
/// `value` / `max` props, computes a ratio, and renders a padded
/// container around a formatted label. Authors see the prop
/// extraction + iced composition pattern from the first run instead
/// of staring at a `todo!()`. A comment points at canvas drawing
/// for custom visuals.
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
    #[widget(type_name = "{type_name}")]
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
    //!
    //! For custom 2D drawing, reach for the canvas path via
    //! `plushie_widget_sdk::iced::widget::canvas` plus a
    //! `canvas::Program` impl. See `docs/custom-widgets.md` in the
    //! plushie-rust repository for a canvas-based walk-through.

    use plushie_widget_sdk::iced;
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
            node: &'a TreeNode,
            _ctx: &RenderCtx<'a, R>,
        ) -> PlushieElement<'a, R> {{
            // Pull typed props the app set via the builder in
            // App::view. `prop_f32` handles JSON's loose number
            // shapes for us.
            let value = node.prop_f32("value").unwrap_or(0.0);
            let max = node.prop_f32("max").unwrap_or(1.0).max(0.0001);
            let ratio = (value / max).clamp(0.0, 1.0);

            // Built-in widgets from the prelude. Swap this for a
            // canvas widget when custom drawing is needed.
            let label = text(format!("{{:.0}}%", ratio * 100.0));
            container(label)
                .padding(iced::Padding::from(8))
                .into()
        }}
    }}
}}
"#
    )
}

/// Create a new plushie app crate rooted at `opts.path` (or `./<name>`).
///
/// The scaffolded crate's `main.rs` wires the `plushie::cli::run`
/// easy path, so the user gets `--plushie-script`,
/// `--plushie-replay`, `--plushie-inspect`, and mode selection for
/// free. A sample automation script and a thin example entry point
/// are scaffolded alongside.
///
/// # Errors
///
/// - `name` must be kebab-case (ASCII letters, digits, and `-`; must
///   start with a letter; no consecutive or trailing `-`).
/// - The destination must not already exist.
pub fn scaffold_app(opts: &InitOpts<'_>) -> Result<ScaffoldResult> {
    validate_kebab_name(opts.name)?;

    let default_path = PathBuf::from(opts.name);
    let target = opts.path.map(Path::to_path_buf).unwrap_or(default_path);
    if target.exists() {
        return Err(Error::Other(anyhow::anyhow!(
            "destination `{}` already exists; pick another path",
            target.display()
        )));
    }

    std::fs::create_dir_all(target.join("src"))?;
    std::fs::create_dir_all(target.join("examples"))?;
    std::fs::create_dir_all(target.join("scripts"))?;

    let struct_name = to_pascal_case(opts.name);
    let cargo_toml = render_app_cargo_toml(opts.name);
    let main_rs = render_app_main_rs(&struct_name);
    let script_example = render_script_example_rs(&struct_name);
    let sample_script = render_sample_script(&struct_name);

    std::fs::write(target.join("Cargo.toml"), cargo_toml)?;
    std::fs::write(target.join("src").join("main.rs"), main_rs)?;
    std::fs::write(
        target.join("examples").join("plushie_script.rs"),
        script_example,
    )?;
    std::fs::write(target.join("scripts").join("smoke.plushie"), sample_script)?;
    maybe_write_iced_paths_override(&target)?;

    let crate_root = std::fs::canonicalize(&target).unwrap_or(target);
    Ok(ScaffoldResult { crate_root })
}

/// Render the app crate's `Cargo.toml`.
fn render_app_cargo_toml(name: &str) -> String {
    let mut out = String::new();
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{name}\"\n"));
    out.push_str("version = \"0.1.0\"\n");
    out.push_str("edition = \"2024\"\n\n");
    out.push_str("[package.metadata.plushie]\n");
    out.push_str("# App marker; cargo-plushie uses the presence of this\n");
    out.push_str("# section to detect plushie apps in the workspace.\n");
    out.push_str("app = true\n\n");
    out.push_str("[dependencies]\n");
    if let Some(source) = source_path_override() {
        let plushie = source.join("crates/plushie");
        out.push_str(&format!(
            "plushie = {{ path = {:?} }}\n",
            plushie.display().to_string()
        ));
    } else {
        out.push_str("plushie = \"0.6\"\n");
    }
    out
}

/// Render the app crate's `src/main.rs`.
///
/// Uses the `plushie::cli::run` easy path so `--plushie-script`,
/// `--plushie-replay`, `--plushie-inspect`, and mode / socket
/// selection all work out of the box. The seeded app wires a
/// counter-style state transition so new users can run the binary,
/// click a button, and see the model update without writing any
/// code first.
fn render_app_main_rs(struct_name: &str) -> String {
    format!(
        r#"use plushie::prelude::*;

/// Application state. Replace these fields with whatever your app
/// actually needs to track.
#[derive(Default)]
pub struct {struct_name} {{
    count: i32,
}}

impl App for {struct_name} {{
    type Model = Self;

    fn init() -> (Self, Command) {{
        (Self::default(), Command::none())
    }}

    fn update(model: &mut Self, event: Event) -> Command {{
        match event.widget_match() {{
            Some(Click("inc")) => model.count += 1,
            Some(Click("dec")) => model.count -= 1,
            Some(Click("reset")) => model.count = 0,
            _ => {{}}
        }}
        Command::none()
    }}

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {{
        window("main")
            .title("{struct_name}")
            .child(
                column()
                    .padding(16)
                    .spacing(8.0)
                    .child(text(&format!("Count: {{}}", model.count)).id("count"))
                    .child(
                        row()
                            .spacing(8.0)
                            .children([
                                button("inc", "+"),
                                button("dec", "-"),
                                button("reset", "reset"),
                            ]),
                    ),
            )
            .into()
    }}
}}

fn main() -> plushie::Result {{
    plushie::cli::run::<{struct_name}>()
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use plushie::test::TestSession;

    #[test]
    fn starts_at_zero() {{
        let session = TestSession::<{struct_name}>::start();
        session.assert_text("count", "Count: 0");
    }}

    #[test]
    fn increment_then_reset() {{
        let mut session = TestSession::<{struct_name}>::start();
        session.click("inc");
        session.click("inc");
        assert_eq!(session.model().count, 2);
        session.click("reset");
        assert_eq!(session.model().count, 0);
    }}
}}
"#
    )
}

/// Render the scaffolded automation-script example.
///
/// Cargo examples can't reach the binary's private items, so the
/// example redeclares the app type. Keeping it self-contained means
/// `cargo run --example plushie_script -- --plushie-script ...`
/// works in a fresh checkout without any wiring.
fn render_script_example_rs(struct_name: &str) -> String {
    format!(
        r#"//! Entry point that exposes the app to `--plushie-script`,
//! `--plushie-replay`, and `--plushie-inspect` without invoking
//! the main binary. Useful when iterating on automation stubs.
//!
//! Example usage:
//!
//! ```sh
//! cargo run --example plushie_script -- --plushie-script scripts/smoke.plushie
//! ```

use plushie::prelude::*;

#[derive(Default)]
struct {struct_name} {{
    count: i32,
}}

impl App for {struct_name} {{
    type Model = Self;

    fn init() -> (Self, Command) {{
        (Self::default(), Command::none())
    }}

    fn update(model: &mut Self, event: Event) -> Command {{
        match event.widget_match() {{
            Some(Click("inc")) => model.count += 1,
            Some(Click("dec")) => model.count -= 1,
            Some(Click("reset")) => model.count = 0,
            _ => {{}}
        }}
        Command::none()
    }}

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> View {{
        window("main")
            .title("{struct_name} automation")
            .child(
                column()
                    .padding(16)
                    .spacing(8.0)
                    .child(text(&format!("Count: {{}}", model.count)).id("count"))
                    .child(
                        row()
                            .spacing(8.0)
                            .children([
                                button("inc", "+"),
                                button("dec", "-"),
                                button("reset", "reset"),
                            ]),
                    ),
            )
            .into()
    }}
}}

fn main() -> plushie::Result {{
    plushie::cli::run::<{struct_name}>()
}}
"#
    )
}

/// Render a sample `.plushie` automation script.
fn render_sample_script(struct_name: &str) -> String {
    format!(
        "# Sample plushie automation script for {struct_name}. Run with:\n\
         #\n\
         #     cargo run --example plushie_script -- --plushie-script scripts/smoke.plushie\n\
         #\n\
         # Actions are one per line. Blank lines and lines starting\n\
         # with `#` are ignored.\n\
         \n\
         assert_text count \"Count: 0\"\n\
         click inc\n\
         click inc\n\
         click inc\n\
         assert_text count \"Count: 3\"\n\
         click reset\n\
         assert_text count \"Count: 0\"\n"
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
    fn scaffold_app_writes_expected_files() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("my-app");
        let opts = InitOpts {
            name: "my-app",
            path: Some(&target),
        };
        let result = scaffold_app(&opts).unwrap();
        assert!(result.crate_root.join("Cargo.toml").is_file());
        assert!(result.crate_root.join("src/main.rs").is_file());
        assert!(
            result
                .crate_root
                .join("examples/plushie_script.rs")
                .is_file()
        );
        assert!(result.crate_root.join("scripts/smoke.plushie").is_file());
        let cargo = std::fs::read_to_string(result.crate_root.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"my-app\""));
        assert!(cargo.contains("[package.metadata.plushie]"));
        let main = std::fs::read_to_string(result.crate_root.join("src/main.rs")).unwrap();
        assert!(main.contains("impl App for MyApp"));
        assert!(main.contains("plushie::cli::run::<MyApp>"));
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
