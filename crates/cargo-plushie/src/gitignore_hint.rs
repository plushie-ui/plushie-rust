//! Friendly warning when generated artifact directories live inside a
//! git work tree but are not gitignored.
//!
//! Used by commands that write outputs the user almost certainly does
//! not want committed:
//! - `cargo plushie tools sync` -> `bin/`
//! - `cargo plushie package portable` -> `target/plushie/`
//! - `cargo plushie package bundle` -> `target/plushie/`
//!
//! The helper is best-effort: any failure (git not installed, path
//! oddities) results in silence rather than noise.

use std::path::Path;
use std::process::Command;

/// Emit a one-paragraph stderr warning if `path` is inside a git work
/// tree and is not git-ignored.
///
/// No-ops if:
/// - `git` is not available
/// - the path is not inside a git work tree
/// - the path is already ignored
/// - git check-ignore returns any error condition (kept quiet so
///   transient failures don't spam users)
pub fn warn_if_not_gitignored(path: &Path) {
    if !is_inside_git_work_tree(path) {
        return;
    }
    if path_is_ignored(path) != Some(false) {
        // Either ignored, or undetermined: stay quiet.
        return;
    }
    let display = path.display();
    eprintln!("warning: {display}/ is not in .gitignore.");
    eprintln!(
        "  Recommended: add the following line so generated artifacts don't end\n  \
         up committed:\n\n      /{display}/"
    );
}

fn is_inside_git_work_tree(path: &Path) -> bool {
    let dir = nearest_existing_dir(path);
    let Some(dir) = dir else {
        return false;
    };
    let Ok(output) = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&dir)
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim() == "true"
}

/// Returns `Some(true)` if ignored, `Some(false)` if not, or `None` if
/// the answer could not be determined (treated as "stay quiet").
fn path_is_ignored(path: &Path) -> Option<bool> {
    let dir = nearest_existing_dir(path)?;
    let path_arg = path.as_os_str();
    let output = Command::new("git")
        .args(["check-ignore", "-q", "--"])
        .arg(path_arg)
        .current_dir(&dir)
        .output()
        .ok()?;
    match output.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        // 128 = fatal (e.g. not a repo), other codes are unexpected.
        _ => None,
    }
}

fn nearest_existing_dir(path: &Path) -> Option<std::path::PathBuf> {
    let mut p: &Path = path;
    loop {
        if p.is_dir() {
            return Some(p.to_path_buf());
        }
        match p.parent() {
            Some(parent) => p = parent,
            None => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;
    use tempfile::tempdir;

    fn git_available() -> bool {
        Command::new("git")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn git_init(dir: &Path) {
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git init");
        assert!(status.success());
    }

    #[test]
    fn not_inside_repo_is_quiet() {
        if !git_available() {
            return;
        }
        let dir = tempdir().unwrap();
        // Plain tempdir, no `git init`.
        let target = dir.path().join("bin");
        std::fs::create_dir_all(&target).unwrap();
        // Sanity check: helper should return false.
        assert!(!is_inside_git_work_tree(&target));
    }

    #[test]
    fn detects_unignored_path() {
        if !git_available() {
            return;
        }
        let dir = tempdir().unwrap();
        git_init(dir.path());
        let target = dir.path().join("bin");
        std::fs::create_dir_all(&target).unwrap();

        assert!(is_inside_git_work_tree(&target));
        assert_eq!(path_is_ignored(&target), Some(false));
    }

    #[test]
    fn detects_ignored_path() {
        if !git_available() {
            return;
        }
        let dir = tempdir().unwrap();
        git_init(dir.path());
        std::fs::write(dir.path().join(".gitignore"), "/bin/\n").unwrap();
        let target = dir.path().join("bin");
        std::fs::create_dir_all(&target).unwrap();

        assert_eq!(path_is_ignored(&target), Some(true));
    }

    #[test]
    fn ignores_nonexistent_path_inside_repo() {
        if !git_available() {
            return;
        }
        let dir = tempdir().unwrap();
        git_init(dir.path());
        let target = dir.path().join("target/plushie");
        // Path does not exist yet; helper should still locate the repo
        // via the nearest existing ancestor and detect the path as not
        // ignored.
        assert!(is_inside_git_work_tree(&target));
        assert_eq!(path_is_ignored(&target), Some(false));
    }
}
