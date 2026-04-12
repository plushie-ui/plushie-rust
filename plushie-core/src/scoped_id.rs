//! Structured representation of a scoped widget ID.
//!
//! Wire IDs use the canonical format `window#scope/path/id`:
//!
//! - `"main#form/email"` - widget in window
//! - `"main#users/u1"` - table row
//! - `"main"` - the window itself
//!
//! `ScopedId` parses this format into its components for programmatic
//! manipulation. Event structs use flat fields (`id`, `scope`, `window`)
//! for ergonomic pattern matching.

/// A parsed scoped widget ID with its components.
///
/// The `full` field holds the canonical wire ID. The other fields
/// are the parsed components for programmatic access.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopedId {
    /// The local widget name (last segment of the path).
    pub id: String,
    /// Reversed ancestor chain (nearest parent first), excluding window.
    pub scope: Vec<String>,
    /// The window this ID belongs to, if any.
    pub window_id: Option<String>,
    /// The canonical wire ID (`window#scope/path/id`).
    pub full: String,
}

impl ScopedId {
    /// Parse a canonical wire ID into its components.
    ///
    /// ```
    /// use plushie_core::ScopedId;
    ///
    /// let sid = ScopedId::parse("main#sidebar/form/email");
    /// assert_eq!(sid.id, "email");
    /// assert_eq!(sid.scope, vec!["form", "sidebar"]);
    /// assert_eq!(sid.window_id, Some("main".to_string()));
    ///
    /// let sid = ScopedId::parse("form/email");
    /// assert_eq!(sid.id, "email");
    /// assert_eq!(sid.scope, vec!["form"]);
    /// assert_eq!(sid.window_id, None);
    ///
    /// let sid = ScopedId::parse("email");
    /// assert_eq!(sid.id, "email");
    /// assert!(sid.scope.is_empty());
    /// assert_eq!(sid.window_id, None);
    /// ```
    pub fn parse(canonical: &str) -> Self {
        let (window, path) = match canonical.split_once('#') {
            Some((win, rest)) if !win.is_empty() => (Some(win.to_string()), rest),
            _ => (None, canonical),
        };

        let (id, scope) = match path.rsplit_once('/') {
            Some((prefix, local)) => {
                let scope: Vec<String> = prefix.split('/').rev().map(String::from).collect();
                (local.to_string(), scope)
            }
            None => (path.to_string(), Vec::new()),
        };

        Self {
            id,
            scope,
            window_id: window,
            full: canonical.to_string(),
        }
    }

    /// Build a ScopedId from its components.
    ///
    /// `scope` should be in reversed order (nearest ancestor first).
    pub fn new(id: impl Into<String>, scope: Vec<String>, window_id: Option<String>) -> Self {
        let id = id.into();
        let full = Self::build_full(window_id.as_deref(), &scope, &id);
        Self {
            id,
            scope,
            window_id,
            full,
        }
    }

    /// True if the local ID matches the given name.
    pub fn matches_local(&self, name: &str) -> bool {
        self.id == name
    }

    /// True if the ancestor appears anywhere in the scope chain.
    pub fn matches_scope(&self, ancestor: &str) -> bool {
        self.scope.iter().any(|s| s == ancestor)
    }

    /// True if the ID is in the given window.
    pub fn in_window(&self, window: &str) -> bool {
        self.window_id.as_deref() == Some(window)
    }

    /// Returns the immediate parent (nearest ancestor), or None.
    pub fn parent(&self) -> Option<&str> {
        self.scope.first().map(|s| s.as_str())
    }

    fn build_full(window: Option<&str>, scope: &[String], id: &str) -> String {
        match (window, scope.is_empty()) {
            (None, true) => id.to_string(),
            (None, false) => {
                let path: Vec<&str> = scope.iter().rev().map(|s| s.as_str()).collect();
                format!("{}/{}", path.join("/"), id)
            }
            (Some(win), true) => format!("{win}#{id}"),
            (Some(win), false) => {
                let path: Vec<&str> = scope.iter().rev().map(|s| s.as_str()).collect();
                format!("{win}#{}/{id}", path.join("/"))
            }
        }
    }
}

impl std::fmt::Display for ScopedId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_path() {
        let sid = ScopedId::parse("main#sidebar/form/email");
        assert_eq!(sid.id, "email");
        assert_eq!(sid.scope, vec!["form", "sidebar"]);
        assert_eq!(sid.window_id, Some("main".to_string()));
        assert_eq!(sid.full, "main#sidebar/form/email");
    }

    #[test]
    fn parse_window_and_id() {
        let sid = ScopedId::parse("main#email");
        assert_eq!(sid.id, "email");
        assert!(sid.scope.is_empty());
        assert_eq!(sid.window_id, Some("main".to_string()));
    }

    #[test]
    fn parse_scoped_no_window() {
        let sid = ScopedId::parse("form/email");
        assert_eq!(sid.id, "email");
        assert_eq!(sid.scope, vec!["form"]);
        assert_eq!(sid.window_id, None);
    }

    #[test]
    fn parse_bare_id() {
        let sid = ScopedId::parse("email");
        assert_eq!(sid.id, "email");
        assert!(sid.scope.is_empty());
        assert_eq!(sid.window_id, None);
    }

    #[test]
    fn parse_window_only() {
        let sid = ScopedId::parse("main");
        assert_eq!(sid.id, "main");
        assert!(sid.scope.is_empty());
        assert_eq!(sid.window_id, None);
    }

    #[test]
    fn matches_local() {
        let sid = ScopedId::parse("main#form/email");
        assert!(sid.matches_local("email"));
        assert!(!sid.matches_local("form"));
    }

    #[test]
    fn matches_scope() {
        let sid = ScopedId::parse("main#sidebar/form/email");
        assert!(sid.matches_scope("form"));
        assert!(sid.matches_scope("sidebar"));
        assert!(!sid.matches_scope("email"));
        assert!(!sid.matches_scope("main"));
    }

    #[test]
    fn in_window() {
        let sid = ScopedId::parse("main#form/email");
        assert!(sid.in_window("main"));
        assert!(!sid.in_window("settings"));
    }

    #[test]
    fn parent() {
        let sid = ScopedId::parse("main#sidebar/form/email");
        assert_eq!(sid.parent(), Some("form"));

        let sid = ScopedId::parse("main#email");
        assert_eq!(sid.parent(), None);
    }

    #[test]
    fn new_builds_full() {
        let sid = ScopedId::new(
            "email",
            vec!["form".into(), "sidebar".into()],
            Some("main".into()),
        );
        assert_eq!(sid.full, "main#sidebar/form/email");
    }

    #[test]
    fn new_no_window() {
        let sid = ScopedId::new("email", vec!["form".into()], None);
        assert_eq!(sid.full, "form/email");
    }

    #[test]
    fn new_bare() {
        let sid = ScopedId::new("email", vec![], None);
        assert_eq!(sid.full, "email");
    }

    #[test]
    fn display() {
        let sid = ScopedId::parse("main#form/email");
        assert_eq!(format!("{sid}"), "main#form/email");
    }
}
