//! Loaded-font registry.
//!
//! Tracks font family names that have been registered with iced via
//! runtime `LoadFont` commands. `default_font` resolution in
//! [`crate::runtime::Core`] consults this registry before emitting a
//! `font_family_not_found` diagnostic so app-loaded fonts resolve
//! without round-tripping through font-file metadata parsing.
//!
//! Names are stored as `&'static str` via the widget SDK's font family
//! interner so repeated lookups share one allocation and iced's
//! `Family::Name(&'static str)` requirement is satisfied.

use std::collections::HashSet;
use std::sync::LazyLock;

use parking_lot::RwLock;

static LOADED_FAMILIES: LazyLock<RwLock<HashSet<&'static str>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

/// Record that a font family has been registered with iced.
///
/// Called by the renderer when a `LoadFont` command completes.
/// Idempotent: calling with the same family twice is a no-op. If the
/// family-name intern cache is full, the family is not recorded; the
/// font load itself still succeeds with iced but resolution falls back
/// to the default family.
pub fn register_loaded_family(family: &str) {
    if family.is_empty() {
        return;
    }
    let Some(interned) = crate::widget::helpers::intern_font_family_public(family) else {
        return;
    };
    LOADED_FAMILIES.write().insert(interned);
}

/// Returns true if `family` has been registered via
/// [`register_loaded_family`].
pub fn is_loaded(family: &str) -> bool {
    LOADED_FAMILIES.read().contains(family)
}

/// Clear the registry. Used in tests that need to run with isolated
/// font state.
#[doc(hidden)]
pub fn reset_for_tests() {
    LOADED_FAMILIES.write().clear();
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
