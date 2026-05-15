use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=PLUSHIE_TOOL_SOURCE_KIND");

    let source_kind = std::env::var("PLUSHIE_TOOL_SOURCE_KIND").ok();
    if let Some(kind) = source_kind {
        println!("cargo:rustc-env=PLUSHIE_TOOL_SOURCE_KIND={kind}");
    } else if git_commit().is_some() {
        println!("cargo:rustc-env=PLUSHIE_TOOL_SOURCE_KIND=source");
    } else {
        println!("cargo:rustc-env=PLUSHIE_TOOL_SOURCE_KIND=crate");
    }

    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=PLUSHIE_BUILD_TARGET={target}");
    }
    if let Ok(profile) = std::env::var("PROFILE") {
        println!("cargo:rustc-env=PLUSHIE_BUILD_PROFILE={profile}");
    }
    if let Some(commit) = git_commit() {
        println!("cargo:rustc-env=PLUSHIE_GIT_COMMIT={commit}");
    }
    if let Some(dirty) = git_dirty() {
        println!("cargo:rustc-env=PLUSHIE_GIT_DIRTY={dirty}");
    }
}

fn git_commit() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn git_dirty() -> Option<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(!output.stdout.is_empty())
}
