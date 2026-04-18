//! Loaded-font registry.
//!
//! Tracks font family names that have been registered with iced via
//! runtime `LoadFont` commands. `default_font` resolution in
//! [`crate::engine`] and custom font family lookups in
//! [`crate::iced_convert`] consult this registry before emitting a
//! `font_family_not_found` diagnostic so app-loaded fonts resolve
//! without round-tripping through font-file metadata parsing.
//!
//! Names are stored as `&'static str` via the existing
//! [`crate::widget::helpers`] interner so repeated lookups share one
//! allocation and iced's `Family::Name(&'static str)` requirement is
//! satisfied.

use std::collections::HashSet;
use std::sync::{LazyLock, RwLock};

static LOADED_FAMILIES: LazyLock<RwLock<HashSet<&'static str>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

/// Record that a font family has been registered with iced.
///
/// Called by the renderer when a `LoadFont` command completes. Idempotent:
/// calling with the same family twice is a no-op.
pub fn register_loaded_family(family: &str) {
    if family.is_empty() {
        return;
    }
    let interned = crate::widget::helpers::intern_font_family_public(family);
    let mut guard = LOADED_FAMILIES.write().unwrap_or_else(|e| e.into_inner());
    guard.insert(interned);
}

/// Returns true if `family` has been registered via
/// [`register_loaded_family`].
pub fn is_loaded(family: &str) -> bool {
    let guard = LOADED_FAMILIES.read().unwrap_or_else(|e| e.into_inner());
    guard.contains(family)
}

/// Clear the registry. Used in tests that need to run with isolated
/// font state.
#[doc(hidden)]
pub fn reset_for_tests() {
    let mut guard = LOADED_FAMILIES.write().unwrap_or_else(|e| e.into_inner());
    guard.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn registering_family_makes_it_loaded() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_for_tests();
        register_loaded_family("Inter");
        assert!(is_loaded("Inter"));
    }

    #[test]
    fn empty_name_is_not_stored() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_for_tests();
        register_loaded_family("");
        assert!(!is_loaded(""));
    }

    #[test]
    fn repeated_registration_is_idempotent() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_for_tests();
        register_loaded_family("Roboto");
        register_loaded_family("Roboto");
        assert!(is_loaded("Roboto"));
    }
}
