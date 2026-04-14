//! Tests for event types and matching.

use plushie::event::*;
use plushie_core::ScopedId;
use serde_json::json;

// ---------------------------------------------------------------------------
// WidgetEvent construction and value accessors
// ---------------------------------------------------------------------------

fn click_event(id: &str) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Click,
        scoped_id: ScopedId::new(id, vec![], Some("main".to_string())),
        value: serde_json::Value::Null,
    })
}

fn input_event(id: &str, text: &str) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Input,
        scoped_id: ScopedId::new(id, vec![], Some("main".to_string())),
        value: json!(text),
    })
}

fn toggle_event(id: &str, checked: bool) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Toggle,
        scoped_id: ScopedId::new(id, vec![], Some("main".to_string())),
        value: json!(checked),
    })
}

fn slide_event(id: &str, value: f64) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Slide,
        scoped_id: ScopedId::new(id, vec![], Some("main".to_string())),
        value: json!(value),
    })
}

fn scoped_click(id: &str, scope: Vec<&str>) -> Event {
    Event::Widget(WidgetEvent {
        event_type: EventType::Click,
        scoped_id: ScopedId::new(
            id,
            scope.into_iter().map(String::from).collect(),
            Some("main".to_string()),
        ),
        value: serde_json::Value::Null,
    })
}

// ---------------------------------------------------------------------------
// Event accessor methods
// ---------------------------------------------------------------------------

#[test]
fn as_widget_returns_some_for_widget_event() {
    let event = click_event("btn");
    assert!(event.as_widget().is_some());
    assert_eq!(event.as_widget().unwrap().scoped_id.id, "btn");
}

#[test]
fn as_widget_returns_none_for_non_widget_event() {
    let event = Event::Timer(TimerEvent {
        tag: "tick".into(),
        timestamp: 0,
    });
    assert!(event.as_widget().is_none());
}

#[test]
fn as_timer_returns_some_for_timer_event() {
    let event = Event::Timer(TimerEvent {
        tag: "tick".into(),
        timestamp: 42,
    });
    let t = event.as_timer().unwrap();
    assert_eq!(t.tag, "tick");
    assert_eq!(t.timestamp, 42);
}

#[test]
fn as_async_returns_some_for_async_event() {
    let event = Event::Async(AsyncEvent {
        tag: "fetch".into(),
        result: Ok(json!({"data": "hello"})),
    });
    let a = event.as_async().unwrap();
    assert_eq!(a.tag, "fetch");
    assert!(a.result.is_ok());
}

// ---------------------------------------------------------------------------
// WidgetEvent value accessors
// ---------------------------------------------------------------------------

#[test]
fn value_string_extracts_text() {
    let event = input_event("name", "Alice");
    let w = event.as_widget().unwrap();
    assert_eq!(w.value_string(), Some("Alice".to_string()));
}

#[test]
fn value_bool_extracts_checked() {
    let event = toggle_event("dark", true);
    let w = event.as_widget().unwrap();
    assert_eq!(w.value_bool(), Some(true));
}

#[test]
fn value_f64_extracts_number() {
    let event = slide_event("vol", 0.75);
    let w = event.as_widget().unwrap();
    assert_eq!(w.value_f64(), Some(0.75));
}

// ---------------------------------------------------------------------------
// WidgetEvent target reconstruction
// ---------------------------------------------------------------------------

#[test]
fn target_without_scope_is_canonical() {
    let event = click_event("save");
    let w = event.as_widget().unwrap();
    assert_eq!(w.target(), "main#save");
}

#[test]
fn target_with_scope_joins_path() {
    let event = scoped_click("save", vec!["form"]);
    let w = event.as_widget().unwrap();
    assert_eq!(w.target(), "main#form/save");
}

#[test]
fn target_with_nested_scope() {
    let event = scoped_click("field", vec!["row", "section"]);
    let w = event.as_widget().unwrap();
    assert_eq!(w.target(), "main#section/row/field");
}

// ---------------------------------------------------------------------------
// Event.scope() convenience accessor
// ---------------------------------------------------------------------------

#[test]
fn scope_returns_slice_for_widget_events() {
    let event = scoped_click("btn", vec!["form"]);
    assert_eq!(event.scope(), Some(vec!["form".to_string()].as_slice()));
}

#[test]
fn scope_returns_none_for_non_widget_events() {
    let event = Event::Timer(TimerEvent {
        tag: "tick".into(),
        timestamp: 0,
    });
    assert!(event.scope().is_none());
}

// ---------------------------------------------------------------------------
// WidgetMatch pattern matching
// ---------------------------------------------------------------------------

#[test]
fn widget_match_click_carries_id() {
    let event = click_event("inc");
    match event.widget_match() {
        Some(WidgetMatch::Click("inc")) => {}
        other => panic!("expected Click(\"inc\"), got {other:?}"),
    }
}

#[test]
fn widget_match_input_carries_text() {
    let event = input_event("email", "test@example.com");
    match event.widget_match() {
        Some(WidgetMatch::Input("email", text)) => {
            assert_eq!(text, "test@example.com");
        }
        other => panic!("expected Input, got {other:?}"),
    }
}

#[test]
fn widget_match_toggle_carries_bool() {
    let event = toggle_event("notifications", true);
    match event.widget_match() {
        Some(WidgetMatch::Toggle("notifications", on)) => {
            assert!(on);
        }
        other => panic!("expected Toggle, got {other:?}"),
    }
}

#[test]
fn widget_match_slide_carries_f64() {
    let event = slide_event("volume", 0.5);
    match event.widget_match() {
        Some(WidgetMatch::Slide("volume", vol)) => {
            assert!((vol - 0.5).abs() < f64::EPSILON);
        }
        other => panic!("expected Slide, got {other:?}"),
    }
}

#[test]
fn widget_match_handles_timer_events() {
    let event = Event::Timer(TimerEvent {
        tag: "tick".into(),
        timestamp: 0,
    });
    match event.widget_match() {
        Some(WidgetMatch::Timer("tick")) => {}
        other => panic!("expected Timer(\"tick\"), got {other:?}"),
    }
}

#[test]
fn widget_match_returns_none_for_non_matchable_events() {
    let event = Event::System(plushie::event::SystemEvent {
        event_type: plushie::event::SystemEventType::ThemeChanged,
        tag: None,
        value: None,
        id: None,
        window_id: None,
    });
    assert!(event.widget_match().is_none());
}

// ---------------------------------------------------------------------------
// Typed pointer events in WidgetMatch
// ---------------------------------------------------------------------------

#[test]
fn widget_match_press_parses_pointer_data() {
    let event = Event::Widget(WidgetEvent {
        event_type: EventType::Press,
        scoped_id: ScopedId::parse("main#canvas"),
        value: json!({"x": 10.5, "y": 20.0, "button": "right", "pointer": "mouse"}),
    });
    match event.widget_match() {
        Some(WidgetMatch::Press("canvas", ptr)) => {
            assert!((ptr.x - 10.5).abs() < 0.01);
            assert!((ptr.y - 20.0).abs() < 0.01);
            assert_eq!(ptr.button, plushie_core::key::MouseButton::Right);
            assert_eq!(ptr.pointer, plushie_core::key::PointerKind::Mouse);
            assert_eq!(ptr.finger, None);
        }
        other => panic!("expected Press, got {other:?}"),
    }
}

#[test]
fn widget_match_press_touch_with_finger() {
    let event = Event::Widget(WidgetEvent {
        event_type: EventType::Press,
        scoped_id: ScopedId::parse("main#canvas"),
        value: json!({"x": 5.0, "y": 15.0, "button": "left", "pointer": "touch", "finger": 2}),
    });
    match event.widget_match() {
        Some(WidgetMatch::Press("canvas", ptr)) => {
            assert_eq!(ptr.pointer, plushie_core::key::PointerKind::Touch);
            assert_eq!(ptr.finger, Some(2));
        }
        other => panic!("expected Press, got {other:?}"),
    }
}

#[test]
fn widget_match_move_parses_coordinates() {
    let event = Event::Widget(WidgetEvent {
        event_type: EventType::Move,
        scoped_id: ScopedId::parse("main#canvas"),
        value: json!({"x": 50.0, "y": 75.0, "pointer": "pen"}),
    });
    match event.widget_match() {
        Some(WidgetMatch::Move("canvas", ptr)) => {
            assert!((ptr.x - 50.0).abs() < 0.01);
            assert!((ptr.y - 75.0).abs() < 0.01);
            assert_eq!(ptr.pointer, plushie_core::key::PointerKind::Pen);
        }
        other => panic!("expected Move, got {other:?}"),
    }
}

#[test]
fn widget_match_scroll_parses_deltas() {
    let event = Event::Widget(WidgetEvent {
        event_type: EventType::Scroll,
        scoped_id: ScopedId::parse("main#area"),
        value: json!({"x": 0.0, "y": 0.0, "delta_x": 0.0, "delta_y": -3.0}),
    });
    match event.widget_match() {
        Some(WidgetMatch::Scroll("area", ptr)) => {
            assert!((ptr.delta_y - (-3.0)).abs() < 0.01);
        }
        other => panic!("expected Scroll, got {other:?}"),
    }
}

#[test]
fn widget_match_press_handles_missing_fields() {
    // Graceful defaults when JSON has minimal data.
    let event = Event::Widget(WidgetEvent {
        event_type: EventType::Press,
        scoped_id: ScopedId::parse("main#canvas"),
        value: json!({}),
    });
    match event.widget_match() {
        Some(WidgetMatch::Press("canvas", ptr)) => {
            assert_eq!(ptr.x, 0.0);
            assert_eq!(ptr.y, 0.0);
            assert_eq!(ptr.button, plushie_core::key::MouseButton::Left);
            assert_eq!(ptr.pointer, plushie_core::key::PointerKind::Mouse);
            assert_eq!(ptr.finger, None);
        }
        other => panic!("expected Press, got {other:?}"),
    }
}

#[test]
fn widget_match_key_press_parses_typed_key() {
    let event = Event::Widget(WidgetEvent {
        event_type: EventType::KeyPress,
        scoped_id: ScopedId::parse("main#editor"),
        value: json!({"key": "Enter", "text": null, "repeat": false}),
    });
    match event.widget_match() {
        Some(WidgetMatch::KeyPress("editor", data)) => {
            assert_eq!(data.key, plushie_core::Key::Enter);
            assert!(!data.repeat);
        }
        other => panic!("expected KeyPress, got {other:?}"),
    }
}

#[test]
fn widget_match_key_press_char_key() {
    let event = Event::Widget(WidgetEvent {
        event_type: EventType::KeyPress,
        scoped_id: ScopedId::parse("main#editor"),
        value: json!({"key": "a", "modified_key": "A", "text": "A", "repeat": false,
                       "modifiers": {"shift": true}}),
    });
    match event.widget_match() {
        Some(WidgetMatch::KeyPress("editor", data)) => {
            assert_eq!(data.key, plushie_core::Key::Char('a'));
            assert_eq!(data.modified_key, Some(plushie_core::Key::Char('A')));
            assert_eq!(data.text.as_deref(), Some("A"));
            assert!(data.modifiers.shift);
        }
        other => panic!("expected KeyPress, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// WidgetMatch used in a realistic update pattern
// ---------------------------------------------------------------------------

#[test]
fn counter_update_pattern() {
    let mut count = 0i32;

    for event in [click_event("inc"), click_event("dec"), click_event("inc")] {
        match event.widget_match() {
            Some(WidgetMatch::Click("inc")) => count += 1,
            Some(WidgetMatch::Click("dec")) => count -= 1,
            _ => {}
        }
    }

    assert_eq!(count, 1);
}

#[test]
fn form_update_pattern() {
    let mut name = String::new();
    let mut agreed = false;

    let events = vec![input_event("name", "Alice"), toggle_event("agree", true)];

    for event in events {
        match event.widget_match() {
            Some(WidgetMatch::Input("name", text)) => name = text.to_string(),
            Some(WidgetMatch::Toggle("agree", on)) => agreed = on,
            _ => {}
        }
    }

    assert_eq!(name, "Alice");
    assert!(agreed);
}

// ---------------------------------------------------------------------------
// KeyEvent helpers
// ---------------------------------------------------------------------------

#[test]
fn key_event_is_press() {
    let event = Event::Key(KeyEvent {
        event_type: KeyEventType::Press,
        key: "Escape".to_string(),
        modified_key: None,
        physical_key: None,
        location: KeyLocation::Standard,
        modifiers: Default::default(),
        text: None,
        repeat: false,
        captured: false,
        window_id: Some("main".to_string()),
    });
    let k = event.as_key_press().unwrap();
    assert!(k.is_press());
    assert!(!k.is_release());
    assert_eq!(k.key, "Escape");
}

#[test]
fn as_key_press_returns_none_for_release() {
    let event = Event::Key(KeyEvent {
        event_type: KeyEventType::Release,
        key: "a".to_string(),
        modified_key: None,
        physical_key: None,
        location: KeyLocation::Standard,
        modifiers: Default::default(),
        text: None,
        repeat: false,
        captured: false,
        window_id: None,
    });
    assert!(event.as_key_press().is_none());
    assert!(event.as_key_release().is_some());
}

// ---------------------------------------------------------------------------
// family_to_event_type
// ---------------------------------------------------------------------------

#[test]
fn family_to_event_type_maps_all_known_families() {
    use plushie::event::family_to_event_type;

    let cases: &[(&str, EventType)] = &[
        ("click", EventType::Click),
        ("double_click", EventType::DoubleClick),
        ("input", EventType::Input),
        ("submit", EventType::Submit),
        ("toggle", EventType::Toggle),
        ("select", EventType::Select),
        ("slide", EventType::Slide),
        ("slide_release", EventType::SlideRelease),
        ("paste", EventType::Paste),
        ("press", EventType::Press),
        ("release", EventType::Release),
        ("move", EventType::Move),
        ("scroll", EventType::Scroll),
        ("scrolled", EventType::Scrolled),
        ("enter", EventType::Enter),
        ("exit", EventType::Exit),
        ("resize", EventType::Resize),
        ("focused", EventType::Focused),
        ("blurred", EventType::Blurred),
        ("drag", EventType::Drag),
        ("drag_end", EventType::DragEnd),
        ("sort", EventType::Sort),
        ("status", EventType::Status),
        ("transition_complete", EventType::TransitionComplete),
        ("open", EventType::Open),
        ("close", EventType::Close),
        ("option_hovered", EventType::OptionHovered),
        ("key_binding", EventType::KeyBinding),
        ("key_press", EventType::KeyPress),
        ("key_release", EventType::KeyRelease),
        ("pane_focus_cycle", EventType::PaneFocusCycle),
        ("pane_resized", EventType::PaneResized),
        ("pane_dragged", EventType::PaneDragged),
        ("pane_clicked", EventType::PaneClicked),
    ];

    for (family, expected) in cases {
        assert_eq!(
            family_to_event_type(family),
            *expected,
            "family_to_event_type({family:?}) returned wrong variant"
        );
    }
}

#[test]
fn family_to_event_type_returns_other_for_unknown() {
    use plushie::event::family_to_event_type;
    assert!(matches!(
        family_to_event_type("nonsense"),
        EventType::Custom(_)
    ));
}
