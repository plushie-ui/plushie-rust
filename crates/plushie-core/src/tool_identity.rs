//! Machine-readable identity reported by Plushie native tools.
//!
//! The shape is intentionally small and avoids local paths, environment
//! variables, hostnames, and other machine-specific details.

use serde::{Deserialize, Serialize};

/// Identity payload emitted by native Plushie tools.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolIdentity {
    /// Tool name, such as `plushie`, `cargo-plushie`, or
    /// `plushie-renderer`.
    pub tool: String,
    /// plushie-rust version the tool was built from.
    pub plushie_rust_version: String,
    /// Rust compilation target, for example `x86_64-unknown-linux-gnu`.
    pub target: String,
    /// Source information known at build time.
    pub source: ToolSourceIdentity,
    /// Build profile information.
    pub build: ToolBuildIdentity,
}

impl ToolIdentity {
    /// Build a new identity payload.
    #[must_use]
    pub fn new(
        tool: impl Into<String>,
        plushie_rust_version: impl Into<String>,
        target: impl Into<String>,
        source: ToolSourceIdentity,
        build: ToolBuildIdentity,
    ) -> Self {
        Self {
            tool: tool.into(),
            plushie_rust_version: plushie_rust_version.into(),
            target: target.into(),
            source,
            build,
        }
    }

    /// Render a compact human-readable version line.
    #[must_use]
    pub fn human_version(&self) -> String {
        format!("{} {}", self.tool, self.plushie_rust_version)
    }
}

/// Source information known when a native tool was built.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolSourceIdentity {
    /// Source kind, such as `release`, `source`, `crate`, or `unknown`.
    pub kind: String,
    /// Git commit when the build had access to a checkout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    /// Whether the checkout had local changes when built.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_dirty: Option<bool>,
}

impl ToolSourceIdentity {
    /// Build source identity from compile-time values.
    #[must_use]
    pub fn new(
        kind: impl Into<String>,
        git_commit: Option<&'static str>,
        git_dirty: Option<&'static str>,
    ) -> Self {
        Self {
            kind: kind.into(),
            git_commit: git_commit
                .map(str::to_string)
                .filter(|value| !value.is_empty()),
            git_dirty: git_dirty.and_then(parse_bool),
        }
    }
}

/// Build profile information for a native tool.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolBuildIdentity {
    /// Cargo profile used for the build.
    pub profile: String,
}

impl ToolBuildIdentity {
    /// Build profile identity from a compile-time Cargo profile value.
    #[must_use]
    pub fn new(profile: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
        }
    }
}

fn parse_bool(value: &'static str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_version_includes_tool_and_version() {
        let identity = ToolIdentity::new(
            "plushie-renderer",
            "0.7.0",
            "x86_64-unknown-linux-gnu",
            ToolSourceIdentity::new("release", None, None),
            ToolBuildIdentity::new("release"),
        );

        assert_eq!(identity.human_version(), "plushie-renderer 0.7.0");
    }

    #[test]
    fn source_identity_drops_empty_commit() {
        let source = ToolSourceIdentity::new("source", Some(""), Some("true"));

        assert_eq!(source.git_commit, None);
        assert_eq!(source.git_dirty, Some(true));
    }
}
