use anyhow::{Context, Result};
use plushie_core::tool_identity::{ToolBuildIdentity, ToolIdentity, ToolSourceIdentity};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub(crate) fn current_tool_identity(tool: &str) -> ToolIdentity {
    ToolIdentity::new(
        tool,
        env!("CARGO_PKG_VERSION"),
        option_env!("PLUSHIE_BUILD_TARGET").unwrap_or("unknown"),
        ToolSourceIdentity::new(
            option_env!("PLUSHIE_TOOL_SOURCE_KIND").unwrap_or("unknown"),
            option_env!("PLUSHIE_GIT_COMMIT"),
            option_env!("PLUSHIE_GIT_DIRTY"),
        ),
        ToolBuildIdentity::new(option_env!("PLUSHIE_BUILD_PROFILE").unwrap_or("unknown")),
    )
}

pub(crate) fn print_current_version(tool: &str, json: bool) -> Result<()> {
    let identity = current_tool_identity(tool);
    if json {
        println!("{}", serde_json::to_string_pretty(&identity)?);
    } else {
        println!("{}", identity.human_version());
    }
    Ok(())
}

pub(crate) fn probe_tool_identity(path: &Path, timeout: Duration) -> Result<ToolIdentity> {
    let mut child = Command::new(path)
        .args(["--version", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("start `{}` for version probe", path.display()))?;

    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            let output = child
                .wait_with_output()
                .with_context(|| format!("read `{}` version probe", path.display()))?;
            if !output.status.success() {
                anyhow::bail!(
                    "`{}` version probe exited with status {}",
                    path.display(),
                    output.status
                );
            }
            return serde_json::from_slice(&output.stdout)
                .with_context(|| format!("parse `{}` version identity", path.display()));
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("`{}` version probe timed out", path.display());
        }

        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn is_downloaded_release(identity: &ToolIdentity) -> bool {
    identity.source.kind == "release"
}
