//! Bidirectional window ID mapping with associated per-window state.
//!
//! Wraps the window ID <-> iced window::Id relationship and any
//! per-window state (decoration, theme cache) in a single type.
//! Insertions and removals are atomic: it's impossible to update
//! one side without the other.

use iced::{Theme, window};
use plushie_widget_sdk::runtime::ThemeChrome;
use std::collections::HashMap;

/// Per-window state beyond the ID mapping.
struct WindowState {
    /// Current decoration state. iced only exposes toggle_decorations(),
    /// so we track the boolean to avoid toggling when already correct.
    decorated: bool,
    /// Resolved theme for this window, if set via the tree's theme prop.
    /// None means "use app theme" unless theme_follows_system is set.
    theme: Option<Theme>,
    theme_follows_system: bool,
    theme_chrome: ThemeChrome,
    /// Per-window scale factor override. None means "use global default".
    scale_factor: Option<f32>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            decorated: true,
            theme: None,
            theme_follows_system: false,
            theme_chrome: ThemeChrome::default(),
            scale_factor: None,
        }
    }
}

/// Bidirectional window ID <-> iced window::Id mapping with per-window
/// state. All mutations keep both maps in sync; callers cannot
/// accidentally desync the forward and reverse maps.
pub struct WindowMap {
    /// Window ID -> (iced window ID, per-window state).
    forward: HashMap<String, (window::Id, WindowState)>,
    /// Iced window ID -> window ID.
    reverse: HashMap<window::Id, String>,
}

impl Default for WindowMap {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowMap {
    pub fn new() -> Self {
        Self {
            forward: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    /// Insert a new window mapping. If the window_id already exists,
    /// the old iced_id is removed from the reverse map to prevent
    /// dangling entries.
    pub fn insert(&mut self, window_id: String, iced_id: window::Id) {
        if let Some((old_iced_id, _)) = self.forward.get(&window_id) {
            self.reverse.remove(old_iced_id);
        }
        self.forward
            .insert(window_id.clone(), (iced_id, WindowState::default()));
        self.reverse.insert(iced_id, window_id);
    }

    pub fn remove_by_iced(&mut self, iced_id: &window::Id) -> Option<String> {
        if let Some(window_id) = self.reverse.remove(iced_id) {
            self.forward.remove(&window_id);
            Some(window_id)
        } else {
            None
        }
    }

    pub fn remove_by_window(&mut self, window_id: &str) -> Option<window::Id> {
        if let Some((iced_id, _)) = self.forward.remove(window_id) {
            self.reverse.remove(&iced_id);
            Some(iced_id)
        } else {
            None
        }
    }

    pub fn contains_window(&self, window_id: &str) -> bool {
        self.forward.contains_key(window_id)
    }

    pub fn get_iced(&self, window_id: &str) -> Option<&window::Id> {
        self.forward.get(window_id).map(|(id, _)| id)
    }

    /// Borrow the host-facing window ID for an iced window. Returns
    /// `None` when the iced ID isn't tracked (e.g. late events after
    /// the window has closed).
    pub fn get_window_id(&self, iced_id: &window::Id) -> Option<&str> {
        self.reverse.get(iced_id).map(String::as_str)
    }

    pub fn iced_ids(&self) -> impl Iterator<Item = &window::Id> {
        self.reverse.keys()
    }

    pub fn window_ids(&self) -> impl Iterator<Item = &String> {
        self.forward.keys()
    }

    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &window::Id)> {
        self.forward.iter().map(|(jid, (iid, _))| (jid, iid))
    }

    pub fn clear(&mut self) {
        self.forward.clear();
        self.reverse.clear();
    }

    // -- Per-window decoration state --

    pub fn is_decorated(&self, window_id: &str) -> bool {
        self.forward.get(window_id).is_none_or(|(_, s)| s.decorated)
    }

    pub fn set_decorated(&mut self, window_id: &str, decorated: bool) {
        if let Some((_, state)) = self.forward.get_mut(window_id) {
            state.decorated = decorated;
        }
    }

    // -- Per-window theme cache --

    pub fn cached_theme(&self, window_id: &str) -> Option<&Theme> {
        self.forward
            .get(window_id)
            .and_then(|(_, s)| s.theme.as_ref())
    }

    pub fn theme_follows_system(&self, window_id: &str) -> bool {
        self.forward
            .get(window_id)
            .is_some_and(|(_, s)| s.theme_follows_system)
    }

    pub fn any_theme_follows_system(&self) -> bool {
        self.forward
            .values()
            .any(|(_, state)| state.theme_follows_system)
    }

    pub fn cached_theme_chrome(&self, window_id: &str) -> Option<ThemeChrome> {
        self.forward
            .get(window_id)
            .and_then(|(_, s)| s.theme.as_ref().map(|_| s.theme_chrome))
    }

    pub fn set_theme(&mut self, window_id: &str, theme: Theme, chrome: ThemeChrome) {
        if let Some((_, state)) = self.forward.get_mut(window_id) {
            state.theme = Some(theme);
            state.theme_follows_system = false;
            state.theme_chrome = chrome;
        }
    }

    pub fn set_theme_follows_system(&mut self, window_id: &str) {
        if let Some((_, state)) = self.forward.get_mut(window_id) {
            state.theme = None;
            state.theme_follows_system = true;
            state.theme_chrome = ThemeChrome::default();
        }
    }

    pub fn clear_theme_cache(&mut self) {
        for (_, state) in self.forward.values_mut() {
            state.theme = None;
            state.theme_follows_system = false;
            state.theme_chrome = ThemeChrome::default();
        }
    }

    // -- Per-window scale factor --

    pub fn scale_factor(&self, window_id: &str) -> Option<f32> {
        self.forward
            .get(window_id)
            .and_then(|(_, s)| s.scale_factor)
    }

    pub fn set_scale_factor(&mut self, window_id: &str, scale_factor: Option<f32>) {
        if let Some((_, state)) = self.forward.get_mut(window_id) {
            state.scale_factor = scale_factor;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_theme_is_distinct_from_missing_and_cached_theme() {
        let mut map = WindowMap::new();
        map.insert("main".to_string(), window::Id::unique());

        assert!(!map.theme_follows_system("main"));
        assert!(!map.any_theme_follows_system());
        assert!(map.cached_theme("main").is_none());

        map.set_theme("main", Theme::Light, ThemeChrome::default());
        assert!(!map.theme_follows_system("main"));
        assert!(!map.any_theme_follows_system());
        assert!(matches!(map.cached_theme("main"), Some(Theme::Light)));

        map.set_theme_follows_system("main");
        assert!(map.theme_follows_system("main"));
        assert!(map.any_theme_follows_system());
        assert!(map.cached_theme("main").is_none());

        map.clear_theme_cache();
        assert!(!map.theme_follows_system("main"));
        assert!(!map.any_theme_follows_system());
        assert!(map.cached_theme("main").is_none());
    }
}
