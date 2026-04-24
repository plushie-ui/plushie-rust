//! Widget event type classification.
//!
//! [`EventType`] is the canonical mapping from wire family strings
//! to typed event kinds. Shared between the SDK (event parsing) and
//! renderer (event construction).
//!
//! The variant list and the variant to family-string mapping are
//! expressed once via the [`event_types!`] macro; adding a variant
//! means adding one line, and the enum definition, `from_family`,
//! and `as_family` stay in lock-step.
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
struct BuiltinEventType {
    event_type: EventType,
    family: &'static str,
}

#[derive(Debug)]
struct EventTypeMap {
    builtin: Vec<EventType>,
    by_family: HashMap<&'static str, EventType>,
}

impl EventTypeMap {
    fn new(entries: Vec<BuiltinEventType>) -> Self {
        if let Err(duplicate) = validate_builtin_event_types(&entries) {
            panic!(
                "duplicate built-in event family {:?}: {:?} and {:?}",
                duplicate.family, duplicate.first, duplicate.second
            );
        }

        let builtin = entries
            .iter()
            .map(|entry| entry.event_type.clone())
            .collect();
        let by_family = entries
            .iter()
            .map(|entry| (entry.family, entry.event_type.clone()))
            .collect();

        Self { builtin, by_family }
    }

    fn event_for_family(&self, family: &str) -> EventType {
        self.by_family
            .get(family)
            .cloned()
            .unwrap_or_else(|| EventType::Custom(family.to_string()))
    }

    fn is_builtin_family(&self, family: &str) -> bool {
        self.by_family.contains_key(family)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct DuplicateEventFamily {
    family: &'static str,
    first: EventType,
    second: EventType,
}

fn validate_builtin_event_types(entries: &[BuiltinEventType]) -> Result<(), DuplicateEventFamily> {
    let mut seen: HashMap<&'static str, EventType> = HashMap::new();

    for entry in entries {
        if let Some(first) = seen.get(entry.family) {
            return Err(DuplicateEventFamily {
                family: entry.family,
                first: (*first).clone(),
                second: entry.event_type.clone(),
            });
        }

        seen.insert(entry.family, entry.event_type.clone());
    }

    Ok(())
}

/// Declare the full set of built-in event types in one place.
///
/// Each entry is `Variant <=> "family_string"`. The macro expands to
/// the [`EventType`] enum definition plus the bidirectional
/// `from_family` / `as_family` mappings so drift between the three is
/// impossible.
macro_rules! event_types {
    ( $( $( #[$attr:meta] )* $variant:ident <=> $family:literal ),* $(,)? ) => {
        /// The kind of widget interaction that occurred.
        ///
        /// Each variant corresponds to a wire protocol event family
        /// string. Use [`EventType::from_family`] for the canonical
        /// string-to-enum conversion.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum EventType {
            $(
                $( #[$attr] )*
                $variant,
            )*
            /// A custom event family (e.g. `"star_rating:select"`) that
            /// does not match any built-in variant.
            Custom(String),
        }

        impl EventType {
            fn map() -> &'static EventTypeMap {
                static MAP: OnceLock<EventTypeMap> = OnceLock::new();
                MAP.get_or_init(|| {
                    EventTypeMap::new(vec![
                        $(
                            BuiltinEventType {
                                event_type: EventType::$variant,
                                family: $family,
                            },
                        )*
                    ])
                })
            }

            /// Convert a wire protocol family string to an EventType.
            ///
            /// This is the single source of truth for the family-to-type
            /// mapping. All event parsing paths (direct mode, wire mode,
            /// event bridge) should call this instead of duplicating the
            /// match.
            pub fn from_family(family: &str) -> Self {
                Self::map().event_for_family(family)
            }

            /// Whether `family` is reserved by a built-in event type.
            pub fn is_builtin_family(family: &str) -> bool {
                Self::map().is_builtin_family(family)
            }

            /// Panic if `family` collides with a built-in event type.
            ///
            /// Custom widget events must use their own family names so
            /// built-in events can keep round-tripping canonically.
            pub fn assert_custom_family(family: &str) {
                assert!(
                    !Self::is_builtin_family(family),
                    "custom event family {family:?} collides with a built-in event family"
                );
            }

            /// The wire protocol family string for this event type.
            pub fn as_family(&self) -> &str {
                let _ = Self::map();
                match self {
                    $( Self::$variant => $family, )*
                    Self::Custom(family) => {
                        Self::assert_custom_family(family);
                        family
                    }
                }
            }

            /// Every built-in variant, useful for exhaustive tests and
            /// documentation.
            ///
            /// Excludes [`Custom`](Self::Custom) by design: custom
            /// families are open-ended and not part of the fixed set.
            pub fn builtin() -> &'static [EventType] {
                Self::map().builtin.as_slice()
            }
        }
    };
}

event_types! {
    /// Pointer click on a focusable widget.
    Click <=> "click",
    /// Rapid pointer press sequence interpreted as a double click.
    DoubleClick <=> "double_click",
    /// Text input changed.
    Input <=> "input",
    /// Input submitted (Enter key or equivalent).
    Submit <=> "submit",
    /// Paste into an input from the system clipboard.
    Paste <=> "paste",
    /// Boolean widget flipped on or off.
    Toggle <=> "toggle",
    /// Selection chosen from a list of options.
    Select <=> "select",
    /// Slider value changed while dragging.
    Slide <=> "slide",
    /// Slider drag released.
    SlideRelease <=> "slide_release",
    /// Pointer pressed (mouse button down, finger down, etc.).
    Press <=> "press",
    /// Pointer released.
    Release <=> "release",
    /// Pointer moved without a button transition.
    Move <=> "move",
    /// Scroll gesture delta.
    Scroll <=> "scroll",
    /// Scroll position changed (scrollable widgets).
    Scrolled <=> "scrolled",
    /// Pointer entered a hit region.
    Enter <=> "enter",
    /// Pointer exited a hit region.
    Exit <=> "exit",
    /// Widget or container resized.
    Resize <=> "resize",
    /// Widget gained keyboard focus.
    Focused <=> "focused",
    /// Widget lost keyboard focus.
    Blurred <=> "blurred",
    /// Drag gesture in progress.
    Drag <=> "drag",
    /// Drag gesture ended.
    DragEnd <=> "drag_end",
    /// Keyboard key pressed.
    KeyPress <=> "key_press",
    /// Keyboard key released.
    KeyRelease <=> "key_release",
    /// Column sort changed.
    Sort <=> "sort",
    /// Arbitrary status update.
    Status <=> "status",
    /// Dropdown option hovered.
    OptionHovered <=> "option_hovered",
    /// Pane grid focus cycled to the next pane.
    PaneFocusCycle <=> "pane_focus_cycle",
    /// Pane grid split resized.
    PaneResized <=> "pane_resized",
    /// Pane grid pane dragged.
    PaneDragged <=> "pane_dragged",
    /// Pane grid pane clicked.
    PaneClicked <=> "pane_clicked",
    /// Declarative animation transition reached its end.
    TransitionComplete <=> "transition_complete",
    /// Opening / expansion event (overlays, menus, disclosure widgets).
    Open <=> "open",
    /// Closing / collapse event.
    Close <=> "close",
    /// Keyboard binding fired.
    KeyBinding <=> "key_binding",
    /// A link in a link-capable widget (rich_text, markdown, etc.) was clicked.
    LinkClick <=> "link_click",
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_family())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_family_known_types() {
        assert_eq!(EventType::from_family("click"), EventType::Click);
        assert_eq!(EventType::from_family("toggle"), EventType::Toggle);
        assert_eq!(EventType::from_family("key_press"), EventType::KeyPress);
        assert_eq!(
            EventType::from_family("pane_clicked"),
            EventType::PaneClicked
        );
    }

    #[test]
    fn from_family_unknown_is_custom() {
        assert_eq!(
            EventType::from_family("star_rating:select"),
            EventType::Custom("star_rating:select".to_string())
        );
    }

    #[test]
    fn every_builtin_round_trips() {
        for variant in EventType::builtin() {
            let family = variant.as_family();
            assert_eq!(
                EventType::from_family(family),
                *variant,
                "round-trip failed for family {family:?}"
            );
        }
    }

    #[test]
    fn custom_variant_round_trips() {
        let custom = EventType::Custom("my:event".into());
        assert_eq!(EventType::from_family(custom.as_family()), custom);
    }

    #[test]
    fn builtin_family_detection_matches_canonical_mapping() {
        assert!(EventType::is_builtin_family("click"));
        assert!(EventType::is_builtin_family("key_press"));
        assert!(!EventType::is_builtin_family("star_rating:select"));
    }

    #[test]
    #[should_panic(
        expected = "custom event family \"click\" collides with a built-in event family"
    )]
    fn custom_variant_panics_when_family_collides_with_builtin() {
        let custom = EventType::Custom("click".into());
        let _ = custom.as_family();
    }

    #[test]
    fn builtin_family_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for variant in EventType::builtin() {
            let family = variant.as_family();
            assert!(
                seen.insert(family),
                "duplicate family string {family:?} across built-in variants"
            );
        }
    }

    #[test]
    fn validation_rejects_duplicate_builtin_family_strings() {
        let entries = vec![
            BuiltinEventType {
                event_type: EventType::Click,
                family: "click",
            },
            BuiltinEventType {
                event_type: EventType::DoubleClick,
                family: "click",
            },
        ];

        assert_eq!(
            validate_builtin_event_types(&entries),
            Err(DuplicateEventFamily {
                family: "click",
                first: EventType::Click,
                second: EventType::DoubleClick,
            })
        );
    }

    #[test]
    #[should_panic(expected = "duplicate built-in event family")]
    fn map_initialization_panics_on_duplicate_builtin_family_strings() {
        let _ = EventTypeMap::new(vec![
            BuiltinEventType {
                event_type: EventType::Click,
                family: "click",
            },
            BuiltinEventType {
                event_type: EventType::DoubleClick,
                family: "click",
            },
        ]);
    }
}
