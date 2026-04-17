//! Property-based round-trip test for Selector wire encoding.
//!
//! Generates arbitrary `Selector` values, encodes them to JSON via
//! `to_wire`, and checks that `Selector::from_wire(...)` recovers
//! the original. The wire format is the single source of truth for
//! cross-SDK automation; drift between serialization and parsing
//! would break every host SDK at once.

use plushie_core::Selector;
use proptest::prelude::*;

fn arb_selector() -> impl Strategy<Value = Selector> {
    prop_oneof![
        // Id selectors: plain id, scoped path, and window-qualified.
        ("[a-z][a-z0-9_]{0,8}").prop_map(|id| Selector::id(&id)),
        ("[a-z]{1,4}", "[a-z]{1,6}").prop_map(|(win, id)| Selector::id(&format!("{win}#{id}"))),
        ("[a-z]{1,4}", "[a-z]{1,6}", "[a-z]{1,6}")
            .prop_map(|(win, scope, id)| Selector::id(&format!("{win}#{scope}/{id}"))),
        ("[a-zA-Z0-9 ]{1,10}").prop_map(|s| Selector::text(&s)),
        ("[a-z]{1,10}").prop_map(|s| Selector::role(&s)),
        ("[a-zA-Z0-9 ]{1,10}").prop_map(|s| Selector::label(&s)),
        Just(Selector::focused()),
    ]
}

proptest! {
    #[test]
    fn selector_to_wire_from_wire_round_trip(sel in arb_selector()) {
        let encoded = sel.to_wire();
        let decoded = Selector::from_wire(&encoded).unwrap_or_else(|| {
            panic!("Selector::from_wire returned None for encoded {encoded}");
        });
        prop_assert_eq!(decoded, sel);
    }
}
