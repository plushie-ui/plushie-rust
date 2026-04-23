//! Drift check for [`plushie_core::BUILTIN_TYPE_NAMES`].
//!
//! The core const is what `cargo plushie build` consults to flag
//! native widgets that shadow a built-in. The iced widget set is what
//! the renderer actually registers. If a widget is added, removed, or
//! renamed in `IcedWidgetSet` without updating the const, this test
//! fails and points at the discrepancy.

use plushie_core::BUILTIN_TYPE_NAMES as CORE_BUILTIN_TYPE_NAMES;
use plushie_widget_sdk::BUILTIN_TYPE_NAMES;
use plushie_widget_sdk::runtime::IcedWidgetSet;

#[test]
fn builtin_type_names_matches_iced_widget_set() {
    let mut from_set = IcedWidgetSet::type_names();
    from_set.sort();
    from_set.dedup();

    let mut from_const: Vec<String> = BUILTIN_TYPE_NAMES.iter().map(|s| s.to_string()).collect();
    from_const.sort();
    from_const.dedup();

    assert_eq!(
        from_set, from_const,
        "BUILTIN_TYPE_NAMES has drifted from IcedWidgetSet::type_names().\n\
         Expected (from the iced widget set): {from_set:#?}\n\
         Got (from the const): {from_const:#?}"
    );
}

#[test]
fn builtin_type_names_is_reexported_from_core() {
    assert_eq!(BUILTIN_TYPE_NAMES, CORE_BUILTIN_TYPE_NAMES);
}

#[test]
fn builtin_type_names_is_sorted_and_deduplicated() {
    let mut sorted = BUILTIN_TYPE_NAMES.to_vec();
    sorted.sort();
    sorted.dedup();
    let original: Vec<&str> = BUILTIN_TYPE_NAMES.to_vec();
    assert_eq!(
        sorted, original,
        "BUILTIN_TYPE_NAMES must be sorted and deduplicated"
    );
}
