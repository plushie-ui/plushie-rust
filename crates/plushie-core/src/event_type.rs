//! Widget event type classification.
//!
//! [`EventType`] is the canonical mapping from wire family strings
//! to typed event kinds. Shared between the SDK (event parsing) and
//! renderer (event construction).

/// The kind of widget interaction that occurred.
///
/// Each variant corresponds to a wire protocol event family string.
/// Use [`EventType::from_family`] for the canonical string-to-enum
/// conversion.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Click.
    Click,
    /// Double Click.
    DoubleClick,
    /// Input.
    Input,
    /// Submit.
    Submit,
    /// Paste.
    Paste,
    /// Toggle.
    Toggle,
    /// Select.
    Select,
    /// Slide.
    Slide,
    /// Slide Release.
    SlideRelease,
    /// Press.
    Press,
    /// Release.
    Release,
    /// Move.
    Move,
    /// Scroll.
    Scroll,
    /// Scrolled.
    Scrolled,
    /// Enter.
    Enter,
    /// Exit.
    Exit,
    /// Resize.
    Resize,
    /// Focused.
    Focused,
    /// Blurred.
    Blurred,
    /// Drag.
    Drag,
    /// Drag End.
    DragEnd,
    /// Key Press.
    KeyPress,
    /// Key Release.
    KeyRelease,
    /// Sort.
    Sort,
    /// Status.
    Status,
    /// Option Hovered.
    OptionHovered,
    /// Pane Focus Cycle.
    PaneFocusCycle,
    /// Pane Resized.
    PaneResized,
    /// Pane Dragged.
    PaneDragged,
    /// Pane Clicked.
    PaneClicked,
    /// Transition Complete.
    TransitionComplete,
    /// Open.
    Open,
    /// Close.
    Close,
    /// Key Binding.
    KeyBinding,
    /// A custom event family (e.g., "star_rating:select").
    Custom(String),
}

impl EventType {
    /// Convert a wire protocol family string to an EventType.
    ///
    /// This is the single source of truth for the family-to-type
    /// mapping. All event parsing paths (direct mode, wire mode,
    /// event bridge) should call this instead of duplicating the
    /// match.
    pub fn from_family(family: &str) -> Self {
        match family {
            "click" => Self::Click,
            "double_click" => Self::DoubleClick,
            "input" => Self::Input,
            "submit" => Self::Submit,
            "toggle" => Self::Toggle,
            "select" => Self::Select,
            "slide" => Self::Slide,
            "slide_release" => Self::SlideRelease,
            "paste" => Self::Paste,
            "press" => Self::Press,
            "release" => Self::Release,
            "move" => Self::Move,
            "scroll" => Self::Scroll,
            "scrolled" => Self::Scrolled,
            "enter" => Self::Enter,
            "exit" => Self::Exit,
            "resize" => Self::Resize,
            "focused" => Self::Focused,
            "blurred" => Self::Blurred,
            "drag" => Self::Drag,
            "drag_end" => Self::DragEnd,
            "sort" => Self::Sort,
            "status" => Self::Status,
            "transition_complete" => Self::TransitionComplete,
            "open" => Self::Open,
            "close" => Self::Close,
            "option_hovered" => Self::OptionHovered,
            "key_binding" => Self::KeyBinding,
            "key_press" => Self::KeyPress,
            "key_release" => Self::KeyRelease,
            "pane_focus_cycle" => Self::PaneFocusCycle,
            "pane_resized" => Self::PaneResized,
            "pane_dragged" => Self::PaneDragged,
            "pane_clicked" => Self::PaneClicked,
            _ => Self::Custom(family.to_string()),
        }
    }

    /// The wire protocol family string for this event type.
    pub fn as_family(&self) -> &str {
        match self {
            Self::Click => "click",
            Self::DoubleClick => "double_click",
            Self::Input => "input",
            Self::Submit => "submit",
            Self::Toggle => "toggle",
            Self::Select => "select",
            Self::Slide => "slide",
            Self::SlideRelease => "slide_release",
            Self::Paste => "paste",
            Self::Press => "press",
            Self::Release => "release",
            Self::Move => "move",
            Self::Scroll => "scroll",
            Self::Scrolled => "scrolled",
            Self::Enter => "enter",
            Self::Exit => "exit",
            Self::Resize => "resize",
            Self::Focused => "focused",
            Self::Blurred => "blurred",
            Self::Drag => "drag",
            Self::DragEnd => "drag_end",
            Self::Sort => "sort",
            Self::Status => "status",
            Self::TransitionComplete => "transition_complete",
            Self::Open => "open",
            Self::Close => "close",
            Self::OptionHovered => "option_hovered",
            Self::KeyBinding => "key_binding",
            Self::KeyPress => "key_press",
            Self::KeyRelease => "key_release",
            Self::PaneFocusCycle => "pane_focus_cycle",
            Self::PaneResized => "pane_resized",
            Self::PaneDragged => "pane_dragged",
            Self::PaneClicked => "pane_clicked",
            Self::Custom(family) => family,
        }
    }
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
    fn as_family_round_trips() {
        let types = [
            EventType::Click,
            EventType::Toggle,
            EventType::KeyPress,
            EventType::DragEnd,
            EventType::Custom("my:event".into()),
        ];
        for t in &types {
            assert_eq!(EventType::from_family(t.as_family()), *t);
        }
    }
}
