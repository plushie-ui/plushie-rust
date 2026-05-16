//! Diagnostic report for `cargo plushie doctor`.
//!
//! Gathers the checks a user typically runs by hand when a wire-mode
//! setup misbehaves: Rust toolchain, cargo-plushie version, mode
//! environment variables, renderer discovery, binary architecture,
//! detected native widgets, and version skew between the app's
//! `plushie-renderer-lib` and the discovered binary.
//!
//! The command is read-only: it never starts the host app, never
//! modifies files, and never spawns the renderer in a way that could
//! affect live sessions. The version probe talks to the binary over
//! `--mock --json`, which is the protocol-only stub path.

use crate::{Result, discover, platform};
use anyhow::Context;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Input for [`run_doctor`].
pub struct DoctorOpts<'a> {
    /// Directory containing the app's Cargo.toml.
    pub manifest_dir: &'a Path,
    /// Minimum supported Rust toolchain version.
    pub min_rustc_version: &'a str,
    /// When true, show full values of environment variables in the report.
    /// By default only `set` / `unset` is shown to avoid leaking paths.
    pub show_env_values: bool,
}

/// Diagnostic outcome. The `critical` field is the gate on exit code:
/// any critical finding makes `cargo plushie doctor` exit non-zero so
/// CI setups can treat it as a hard failure.
#[derive(Debug, Default)]
pub struct DoctorReport {
    /// Ordered list of (label, value, severity) rows.
    pub rows: Vec<Row>,
    /// True when at least one critical issue was detected.
    pub critical: bool,
}

/// A single row in the diagnostic report.
#[derive(Debug)]
pub struct Row {
    /// Short label shown on the left.
    pub label: String,
    /// Rendered value (may span multiple lines).
    pub value: String,
    /// Severity (drives the leading symbol and exit-code gate).
    pub severity: Severity,
}

/// Row severity classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Normal informational row.
    Ok,
    /// Worth mentioning but not broken.
    Warn,
    /// Broken: will fail at handshake or run time.
    Critical,
}

impl Severity {
    /// Leading symbol printed in the report.
    fn symbol(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warn => "WARN",
            Self::Critical => "FAIL",
        }
    }
}

/// Build the environment-variable rows for the doctor report.
///
/// Each tracked variable contributes one row. When `show_values` is
/// false, set variables are reported as `"set"` rather than revealing
/// their contents (which may include home paths or socket paths).
fn env_rows(show_values: bool) -> Vec<Row> {
    env_rows_with_lookup(show_values, |var| std::env::var(var).ok())
}

/// Inner implementation of [`env_rows`] with an injectable lookup, for
/// unit testing without touching the process environment.
fn env_rows_with_lookup<F>(show_values: bool, lookup: F) -> Vec<Row>
where
    F: Fn(&str) -> Option<String>,
{
    const TRACKED: &[&str] = &[
        "PLUSHIE_BINARY_PATH",
        "PLUSHIE_RUST_SOURCE_PATH",
        "PLUSHIE_MODE",
        "PLUSHIE_SOCKET",
    ];
    TRACKED
        .iter()
        .map(|&var| {
            let value = match lookup(var) {
                Some(v) => {
                    if show_values {
                        v
                    } else {
                        "set".to_string()
                    }
                }
                None => "unset".to_string(),
            };
            Row {
                label: var.to_string(),
                value,
                severity: Severity::Ok,
            }
        })
        .collect()
}

/// Gather the diagnostic rows and return a populated [`DoctorReport`].
///
/// # Errors
///
/// Propagates [`cargo_metadata`] failures when the workspace dep
/// graph cannot be resolved. Missing binaries, missing env vars,
/// and missing tools are reported as rows, not errors.
pub fn run_doctor(opts: &DoctorOpts<'_>) -> Result<DoctorReport> {
    let mut report = DoctorReport::default();

    // -- Toolchain --
    push_rustc_row(&mut report, opts.min_rustc_version);
    report.rows.push(Row {
        label: "cargo-plushie".to_string(),
        value: env!("CARGO_PKG_VERSION").to_string(),
        severity: Severity::Ok,
    });

    // -- Host --
    report.rows.push(Row {
        label: "host".to_string(),
        value: format!("{}-{}", platform::os_name(), platform::arch_name()),
        severity: Severity::Ok,
    });

    // -- Environment --
    report.rows.extend(env_rows(opts.show_env_values));

    // -- Renderer discovery --
    let discovered = discover_renderer(opts.manifest_dir);
    match &discovered {
        Some(path) => report.rows.push(Row {
            label: "renderer".to_string(),
            value: path.display().to_string(),
            severity: Severity::Ok,
        }),
        None => {
            report.critical = true;
            report.rows.push(Row {
                label: "renderer".to_string(),
                value: renderer_not_found_hint(),
                severity: Severity::Critical,
            });
        }
    }

    // -- Architecture --
    if let Some(path) = discovered.as_deref() {
        push_arch_row(&mut report, path);
    }

    // -- Metadata-driven checks --
    push_metadata_rows(&mut report, opts.manifest_dir)?;

    // -- Version skew --
    if let Some(path) = discovered.as_deref() {
        push_version_skew_row(&mut report, path, opts.manifest_dir);
    }

    Ok(report)
}

/// Write a textual report to `writer` using aligned columns.
///
/// # Errors
///
/// Propagates the writer's errors.
pub fn write_report<W: Write>(report: &DoctorReport, writer: &mut W) -> std::io::Result<()> {
    let max_label = report.rows.iter().map(|r| r.label.len()).max().unwrap_or(0);
    for row in &report.rows {
        let pad = " ".repeat(max_label.saturating_sub(row.label.len()));
        let symbol = row.severity.symbol();
        // Multi-line values indent subsequent lines under the value column.
        let mut lines = row.value.lines();
        let first = lines.next().unwrap_or("");
        writeln!(
            writer,
            "  [{symbol:^4}] {label}{pad}  {first}",
            label = row.label
        )?;
        // Prefix width: 2 leading spaces + `[XXXX]` (6) + ` ` (1) +
        // padded label + `  ` (2) = 11 + max_label. Continuation
        // lines align under the first line's value column.
        let indent = " ".repeat(11 + max_label);
        for line in lines {
            writeln!(writer, "{indent}{line}")?;
        }
    }
    if report.critical {
        writeln!(writer)?;
        writeln!(writer, "Critical issues detected; see entries marked FAIL.")?;
    }
    Ok(())
}

fn push_rustc_row(report: &mut DoctorReport, min_version: &str) {
    let output = Command::new("rustc").arg("--version").output();
    let Ok(output) = output else {
        report.critical = true;
        report.rows.push(Row {
            label: "rustc".to_string(),
            value: "rustc not found on PATH".to_string(),
            severity: Severity::Critical,
        });
        return;
    };
    if !output.status.success() {
        report.critical = true;
        report.rows.push(Row {
            label: "rustc".to_string(),
            value: "rustc --version returned a non-zero status".to_string(),
            severity: Severity::Critical,
        });
        return;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let version = parse_rustc_version(&stdout);
    let severity = match &version {
        Some(v) if !version_at_least(v, min_version) => Severity::Critical,
        _ => Severity::Ok,
    };
    let value = match (&version, severity) {
        (Some(v), Severity::Critical) => {
            format!("{stdout} (below supported {min_version}; host rustc reports {v})")
        }
        _ => stdout,
    };
    if severity == Severity::Critical {
        report.critical = true;
    }
    report.rows.push(Row {
        label: "rustc".to_string(),
        value,
        severity,
    });
}

/// Extract the dotted version (e.g. `1.92.0`) from a rustc
/// `--version` line. Returns `None` if the format is unexpected.
fn parse_rustc_version(line: &str) -> Option<String> {
    // Typical shape: `rustc 1.92.0 (abcdef 2025-10-31)`.
    let rest = line.strip_prefix("rustc ")?;
    let version = rest.split_whitespace().next()?;
    Some(version.to_string())
}

/// Numeric comparison over dotted version strings. Missing
/// components are treated as 0; non-numeric components short-circuit
/// to `false` so unparseable values don't falsely clear the gate.
fn version_at_least(actual: &str, min: &str) -> bool {
    fn parts(s: &str) -> Option<Vec<u64>> {
        s.split('.').map(|p| p.parse::<u64>().ok()).collect()
    }
    let Some(a) = parts(actual) else { return false };
    let Some(m) = parts(min) else { return false };
    for i in 0..a.len().max(m.len()) {
        let av = a.get(i).copied().unwrap_or(0);
        let mv = m.get(i).copied().unwrap_or(0);
        if av > mv {
            return true;
        }
        if av < mv {
            return false;
        }
    }
    true
}

fn push_arch_row(report: &mut DoctorReport, binary: &Path) {
    let host = platform::arch_name();
    match detect_binary_arch(binary) {
        Some(arch) if arch == host => report.rows.push(Row {
            label: "arch".to_string(),
            value: format!("{arch} (matches host)"),
            severity: Severity::Ok,
        }),
        Some(arch) => {
            report.critical = true;
            report.rows.push(Row {
                label: "arch".to_string(),
                value: format!("{arch} (host is {host}; runtime will mis-spawn)"),
                severity: Severity::Critical,
            });
        }
        None => report.rows.push(Row {
            label: "arch".to_string(),
            value: "unknown (file(1) unavailable or unrecognised output)".to_string(),
            severity: Severity::Warn,
        }),
    }
}

/// Invoke `file(1)` on Unix to classify the binary's architecture.
/// Windows and unknown platforms return `None`.
fn detect_binary_arch(path: &Path) -> Option<String> {
    if cfg!(not(unix)) {
        return None;
    }
    let output = Command::new("file").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let lower = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    if lower.contains("x86-64") || lower.contains("x86_64") || lower.contains("amd64") {
        Some("x86_64".to_string())
    } else if lower.contains("aarch64") || lower.contains("arm64") {
        Some("aarch64".to_string())
    } else {
        None
    }
}

fn push_metadata_rows(report: &mut DoctorReport, manifest_dir: &Path) -> Result<()> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .exec()
        .with_context(|| "cargo metadata failed")?;

    // Widget discovery from the dep graph.
    let widgets = discover::discover_widgets(manifest_dir)?;
    let widgets_row = if widgets.is_empty() {
        "(none)".to_string()
    } else {
        widgets
            .iter()
            .map(|w| format!("{} ({})", w.crate_name, w.type_name))
            .collect::<Vec<_>>()
            .join("\n")
    };
    report.rows.push(Row {
        label: "native widgets".to_string(),
        value: widgets_row,
        severity: Severity::Ok,
    });

    // Declared renderer-lib version from the dep graph.
    let renderer_version = metadata
        .packages
        .iter()
        .find(|p| p.name == "plushie-renderer-lib")
        .map(|p| p.version.to_string());
    let value = renderer_version.unwrap_or_else(|| "(not in dep graph)".to_string());
    report.rows.push(Row {
        label: "renderer-lib".to_string(),
        value,
        severity: Severity::Ok,
    });

    Ok(())
}

fn push_version_skew_row(report: &mut DoctorReport, binary: &Path, manifest_dir: &Path) {
    let Ok(metadata) = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .exec()
    else {
        return;
    };
    let expected = metadata
        .packages
        .iter()
        .find(|p| p.name == "plushie-renderer-lib")
        .map(|p| p.version.to_string());
    let Some(expected) = expected else {
        return;
    };

    match probe_renderer_version(binary) {
        Some(actual) if actual == expected => report.rows.push(Row {
            label: "version skew".to_string(),
            value: format!("matched ({actual})"),
            severity: Severity::Ok,
        }),
        Some(actual) => {
            report.critical = true;
            report.rows.push(Row {
                label: "version skew".to_string(),
                value: format!(
                    "app expects {expected} but binary reports {actual}; \
                     handshake will reject incompatible protocol versions"
                ),
                severity: Severity::Critical,
            });
        }
        None => report.rows.push(Row {
            label: "version skew".to_string(),
            value: "could not probe binary (mock handshake failed)".to_string(),
            severity: Severity::Warn,
        }),
    }
}

/// Spawn the renderer with `--mock --json`, feed a minimal Settings
/// message, and parse the `version` field from the hello response.
///
/// The probe is bounded: the child's stdin is closed immediately,
/// and we only read the first line from stdout. A hung or
/// incompatible binary will not block the doctor run forever.
fn probe_renderer_version(binary: &Path) -> Option<String> {
    probe_renderer_version_with_timeout(binary, Duration::from_secs(5))
}

fn probe_renderer_version_with_timeout(binary: &Path, timeout: Duration) -> Option<String> {
    let mut child = Command::new(binary)
        .args(["--mock", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let settings = format!(
        r#"{{"type":"settings","session":"","protocol_version":{},"codec":"json"}}{}"#,
        plushie_core::protocol::PROTOCOL_VERSION,
        "\n"
    );
    {
        let mut stdin = child.stdin.take()?;
        let _ = stdin.write_all(settings.as_bytes());
        let _ = stdin.flush();
    }

    let mut buf = Vec::with_capacity(1024);
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // Bound the read to the first newline. 4KB is generous for a
        // hello JSON payload, with one extra byte to detect overlong
        // output without reading indefinitely.
        let result = BufReader::new(&mut stdout)
            .take(4097)
            .read_until(b'\n', &mut buf)
            .map(|_| buf);
        let _ = tx.send(result);
    });

    let buf = match rx.recv_timeout(timeout) {
        Ok(Ok(buf)) => buf,
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    let line = String::from_utf8(buf).ok()?;
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    value.get("version")?.as_str().map(str::to_string)
}

/// Four-step discovery mirroring the SDK's `wire_discovery` chain.
/// Returns the first hit or `None` if everything falls through.
fn discover_renderer(manifest_dir: &Path) -> Option<PathBuf> {
    if let Some(env) = std::env::var_os("PLUSHIE_BINARY_PATH") {
        let p = PathBuf::from(env);
        if p.is_file() {
            return Some(p);
        }
    }
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target"));

    // Custom build output.
    for profile in ["release", "debug"] {
        let profile_dir = target_dir.join("plushie-renderer/target").join(profile);
        if let Ok(entries) = std::fs::read_dir(&profile_dir) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if is_executable_file(&path) && has_executable_extension(&path) {
                    return Some(path);
                }
            }
        }
    }

    // Project-local installed renderer.
    let download = manifest_dir.join("bin").join(platform::renderer_name());
    if is_executable_file(&download) {
        return Some(download);
    }
    None
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && (meta.permissions().mode() & 0o111) != 0,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(unix)]
fn has_executable_extension(path: &Path) -> bool {
    path.extension().is_none_or(|e| e != "d" && e != "rlib")
}

#[cfg(windows)]
fn has_executable_extension(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "exe")
}

#[cfg(all(not(unix), not(windows)))]
fn has_executable_extension(path: &Path) -> bool {
    path.extension().is_none()
}

fn renderer_not_found_hint() -> String {
    "not found. Try one of:\n  \
     cargo plushie build      (widget-aware custom build)\n  \
     cargo plushie download   (precompiled stock binary)\n  \
     or set PLUSHIE_BINARY_PATH to an existing binary."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rustc_version_standard_shape() {
        let v = parse_rustc_version("rustc 1.92.0 (abcdef0 2025-10-31)");
        assert_eq!(v.as_deref(), Some("1.92.0"));
    }

    #[test]
    fn parse_rustc_version_rejects_garbage() {
        assert!(parse_rustc_version("").is_none());
        assert!(parse_rustc_version("gcc 14.1.0").is_none());
    }

    #[test]
    fn version_at_least_compares_numerically() {
        assert!(version_at_least("1.92.0", "1.92"));
        assert!(version_at_least("1.92.0", "1.92.0"));
        assert!(version_at_least("2.0.0", "1.92.0"));
        assert!(!version_at_least("1.91.9", "1.92.0"));
        assert!(!version_at_least("1.91", "1.92.0"));
    }

    #[test]
    fn write_report_renders_aligned_columns() {
        let mut report = DoctorReport::default();
        report.rows.push(Row {
            label: "short".to_string(),
            value: "value1".to_string(),
            severity: Severity::Ok,
        });
        report.rows.push(Row {
            label: "longer-label".to_string(),
            value: "value2\ncontinuation".to_string(),
            severity: Severity::Warn,
        });
        let mut buf = Vec::new();
        write_report(&report, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("OK"));
        assert!(out.contains("WARN"));
        assert!(out.contains("continuation"));
    }

    #[test]
    fn write_report_mentions_critical_when_flagged() {
        let mut report = DoctorReport {
            critical: true,
            ..Default::default()
        };
        report.rows.push(Row {
            label: "renderer".to_string(),
            value: "missing".to_string(),
            severity: Severity::Critical,
        });
        let mut buf = Vec::new();
        write_report(&report, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("Critical issues detected"));
    }

    #[cfg(unix)]
    #[test]
    fn probe_renderer_version_times_out_when_renderer_never_writes() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::Instant;

        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("quiet-renderer");
        std::fs::write(&binary, "#!/bin/sh\nsleep 30\n").unwrap();

        let mut permissions = std::fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&binary, permissions).unwrap();

        let started = Instant::now();
        let version = probe_renderer_version_with_timeout(&binary, Duration::from_millis(100));

        assert!(version.is_none());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    fn lookup_with_binary_path(value: &str) -> impl Fn(&str) -> Option<String> + '_ {
        move |var| {
            if var == "PLUSHIE_BINARY_PATH" {
                Some(value.to_string())
            } else {
                None
            }
        }
    }

    #[test]
    fn env_rows_redacts_values_by_default() {
        let sentinel = "/home/testuser/plushie/bin/renderer-abc123";
        let rows = env_rows_with_lookup(false, lookup_with_binary_path(sentinel));
        let binary_row = rows
            .iter()
            .find(|r| r.label == "PLUSHIE_BINARY_PATH")
            .expect("PLUSHIE_BINARY_PATH row must be present");

        assert_eq!(
            binary_row.value, "set",
            "default mode must not expose the value"
        );
        assert!(
            !binary_row.value.contains("testuser"),
            "path must not leak into default output"
        );
    }

    #[test]
    fn env_rows_shows_values_in_verbose_mode() {
        let sentinel = "/home/testuser/plushie/bin/renderer-verbose456";
        let rows = env_rows_with_lookup(true, lookup_with_binary_path(sentinel));
        let binary_row = rows
            .iter()
            .find(|r| r.label == "PLUSHIE_BINARY_PATH")
            .expect("PLUSHIE_BINARY_PATH row must be present");

        assert_eq!(
            binary_row.value, sentinel,
            "verbose mode must show the full value"
        );
    }

    #[test]
    fn env_rows_reports_unset_for_missing_vars() {
        let rows = env_rows_with_lookup(false, |_var| None);
        let binary_row = rows
            .iter()
            .find(|r| r.label == "PLUSHIE_BINARY_PATH")
            .expect("PLUSHIE_BINARY_PATH row must be present");

        assert_eq!(binary_row.value, "unset");
    }
}
