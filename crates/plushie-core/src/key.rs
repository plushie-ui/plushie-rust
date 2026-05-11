//! Keyboard key types with forgiving string parsing.
//!
//! The [`Key`] enum represents keyboard keys with typed variants for
//! common keys and a [`Named`](Key::Named) fallback for rare/specialized
//! keys. The [`KeyPress`] struct bundles a key with modifiers.
//!
//! # Normalization
//!
//! All string-based parsing normalizes the input by:
//! - Removing whitespace, underscores, and hyphens
//! - Lowercasing everything
//!
//! This means these are all equivalent:
//! - `"LeftArrow"`, `"left_arrow"`, `"left-arrow"`, `"leftarrow"`
//! - `"PageUp"`, `"page_up"`, `"Page Up"`, `"pageup"`
//! - `"Ctrl"`, `"ctrl"`, `"CTRL"`
//!
//! # Key aliases
//!
//! Common aliases are recognized to reduce doc-checking:
//! - Arrow keys: `"left"` / `"arrowleft"` / `"leftarrow"`
//! - Enter: `"enter"` / `"return"`
//! - Escape: `"esc"` / `"escape"`
//! - Backspace: `"bs"` / `"backspace"`
//! - Delete: `"del"` / `"delete"`
//! - Page navigation: `"pageup"` / `"pgup"`, `"pagedown"` / `"pgdown"` / `"pgdn"`
//!
//! # Combo strings
//!
//! [`KeyPress`] parses modifier+key combos from strings:
//! - `"Ctrl+s"`, `"Shift+Enter"`, `"Ctrl+Shift+ArrowUp"`
//! - `"Ctrl + Left_Arrow"` (whitespace around `+` is fine)
//!
//! # Modifier aliases
//!
//! - `ctrl` / `control` - physical Ctrl key
//! - `shift`
//! - `alt` / `option` / `opt`
//! - `command` / `cmd` - platform shortcut key (Ctrl on Linux/Windows, Cmd on macOS)
//! - `logo` / `super` / `win` / `meta` - physical Logo/Super/Command key

use std::{error::Error, fmt, str::FromStr};

use crate::protocol::KeyModifiers;

// ---------------------------------------------------------------------------
// Key enum
// ---------------------------------------------------------------------------

/// A keyboard key.
///
/// Common keys have dedicated variants for compile-time safety and
/// IDE autocomplete. Rare keys (media, TV remote, IME) use the
/// [`Named`](Key::Named) fallback with the iced/winit PascalCase
/// name string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    // -- Navigation --
    /// Arrow Up.
    ArrowUp,
    /// Arrow Down.
    ArrowDown,
    /// Arrow Left.
    ArrowLeft,
    /// Arrow Right.
    ArrowRight,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,

    // -- Editing --
    /// Enter.
    Enter,
    /// Tab.
    Tab,
    /// Space.
    Space,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Insert.
    Insert,
    /// Escape.
    Escape,

    // -- Modifiers (as key events, not as modifiers on combos) --
    /// Shift.
    Shift,
    /// Control.
    Control,
    /// Alt.
    Alt,
    /// Super.
    Super,

    // -- Function keys --
    /// F1.
    F1,
    /// F2.
    F2,
    /// F3.
    F3,
    /// F4.
    F4,
    /// F5.
    F5,
    /// F6.
    F6,
    /// F7.
    F7,
    /// F8.
    F8,
    /// F9.
    F9,
    /// F10.
    F10,
    /// F11.
    F11,
    /// F12.
    F12,

    // -- Common extras --
    /// Caps Lock.
    CapsLock,
    /// Num Lock.
    NumLock,
    /// Scroll Lock.
    ScrollLock,
    /// Print Screen.
    PrintScreen,
    /// Pause.
    Pause,
    /// Context Menu.
    ContextMenu,
    /// Copy.
    Copy,
    /// Cut.
    Cut,
    /// Paste.
    Paste,
    /// Undo.
    Undo,
    /// Redo.
    Redo,

    // -- Single character --
    /// Char.
    Char(char),

    /// A named key not covered by the common variants above.
    ///
    /// Uses the iced/winit PascalCase name (e.g. "MediaPlay",
    /// "BrowserBack", "LaunchMail"). Forward-compatible: new iced
    /// key names work without updating this enum.
    Named(String),
}

impl Key {
    /// The canonical wire-format name for this key.
    ///
    /// Returns the PascalCase name that the renderer and iced use.
    pub fn wire_name(&self) -> String {
        match self {
            Self::ArrowUp => "ArrowUp".into(),
            Self::ArrowDown => "ArrowDown".into(),
            Self::ArrowLeft => "ArrowLeft".into(),
            Self::ArrowRight => "ArrowRight".into(),
            Self::Home => "Home".into(),
            Self::End => "End".into(),
            Self::PageUp => "PageUp".into(),
            Self::PageDown => "PageDown".into(),
            Self::Enter => "Enter".into(),
            Self::Tab => "Tab".into(),
            Self::Space => "Space".into(),
            Self::Backspace => "Backspace".into(),
            Self::Delete => "Delete".into(),
            Self::Insert => "Insert".into(),
            Self::Escape => "Escape".into(),
            Self::Shift => "Shift".into(),
            Self::Control => "Control".into(),
            Self::Alt => "Alt".into(),
            Self::Super => "Super".into(),
            Self::F1 => "F1".into(),
            Self::F2 => "F2".into(),
            Self::F3 => "F3".into(),
            Self::F4 => "F4".into(),
            Self::F5 => "F5".into(),
            Self::F6 => "F6".into(),
            Self::F7 => "F7".into(),
            Self::F8 => "F8".into(),
            Self::F9 => "F9".into(),
            Self::F10 => "F10".into(),
            Self::F11 => "F11".into(),
            Self::F12 => "F12".into(),
            Self::CapsLock => "CapsLock".into(),
            Self::NumLock => "NumLock".into(),
            Self::ScrollLock => "ScrollLock".into(),
            Self::PrintScreen => "PrintScreen".into(),
            Self::Pause => "Pause".into(),
            Self::ContextMenu => "ContextMenu".into(),
            Self::Copy => "Copy".into(),
            Self::Cut => "Cut".into(),
            Self::Paste => "Paste".into(),
            Self::Undo => "Undo".into(),
            Self::Redo => "Redo".into(),
            Self::Char(c) => c.to_string(),
            Self::Named(name) => name.clone(),
        }
    }

    /// Parse a key name from the wire protocol.
    ///
    /// Unknown key names are preserved through [`Key::Named`] so newer
    /// renderer or iced key names can pass through older SDKs.
    pub fn from_wire(s: &str) -> Self {
        let trimmed = s.trim();
        let mut chars = trimmed.chars();
        if let (Some(ch), None) = (chars.next(), chars.next()) {
            return Self::Char(ch);
        }

        parse_known_key_normalized(&normalize(trimmed))
            .unwrap_or_else(|| Self::Named(trimmed.to_string()))
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.wire_name())
    }
}

/// Parse a key name from a normalized string (lowercase, no
/// whitespace/underscores/hyphens).
fn parse_key_normalized(s: &str) -> Key {
    parse_known_key_normalized(s).unwrap_or_else(|| Key::Named(s.to_string()))
}

fn parse_known_key_normalized(s: &str) -> Option<Key> {
    Some(match s {
        // Navigation
        "arrowup" | "up" | "uparrow" => Key::ArrowUp,
        "arrowdown" | "down" | "downarrow" => Key::ArrowDown,
        "arrowleft" | "left" | "leftarrow" => Key::ArrowLeft,
        "arrowright" | "right" | "rightarrow" => Key::ArrowRight,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdown" | "pgdn" => Key::PageDown,

        // Editing
        "enter" | "return" => Key::Enter,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "backspace" | "bs" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "insert" | "ins" => Key::Insert,
        "escape" | "esc" => Key::Escape,

        // Modifiers as keys
        "shift" => Key::Shift,
        "control" | "ctrl" => Key::Control,
        "alt" | "option" | "opt" => Key::Alt,
        "super" | "logo" | "meta" | "command" | "cmd" | "win" => Key::Super,

        // Function keys
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,

        // Common extras
        "capslock" | "caps" => Key::CapsLock,
        "numlock" | "num" => Key::NumLock,
        "scrolllock" => Key::ScrollLock,
        "printscreen" | "prtsc" | "print" => Key::PrintScreen,
        "pause" | "break" => Key::Pause,
        "contextmenu" | "menu" => Key::ContextMenu,
        "copy" => Key::Copy,
        "cut" => Key::Cut,
        "paste" => Key::Paste,
        "undo" => Key::Undo,
        "redo" => Key::Redo,

        // Single character: preserve original case since 'a' and 'A'
        // are different keys (shift state).
        s if s.len() == 1 => Key::Char(s.chars().next().unwrap()),

        _ => return None,
    })
}

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        let trimmed = s.trim();
        // Single characters preserve their case (a and A are different keys).
        if trimmed.len() == 1 {
            return Key::Char(trimmed.chars().next().unwrap());
        }
        parse_key_normalized(&normalize(trimmed))
    }
}

impl From<String> for Key {
    fn from(s: String) -> Self {
        Key::from(s.as_str())
    }
}

impl From<char> for Key {
    fn from(c: char) -> Self {
        Key::Char(c)
    }
}

// ---------------------------------------------------------------------------
// KeyPress (key + modifiers)
// ---------------------------------------------------------------------------

/// A key press event: a key combined with modifier state.
///
/// Parses combo strings like `"Ctrl+s"`, `"Shift + Enter"`,
/// `"Ctrl+Shift+ArrowUp"`. Also converts from a bare [`Key`]
/// (no modifiers) or a `(Key, KeyModifiers)` tuple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPress {
    /// Key.
    pub key: Key,
    /// Active modifier keys.
    pub modifiers: KeyModifiers,
}

/// Error returned when a combo string contains an unknown modifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseKeyPressError {
    modifier: String,
}

impl ParseKeyPressError {
    /// Unknown modifier segment from the combo string.
    pub fn modifier(&self) -> &str {
        &self.modifier
    }
}

impl fmt::Display for ParseKeyPressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown key modifier {:?}", self.modifier)
    }
}

impl Error for ParseKeyPressError {}

impl KeyPress {
    /// Construct a new value.
    pub fn new(key: Key, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }

    /// Parse from the wire protocol payload.
    ///
    /// Accepts three formats:
    /// - Combined: `{"combo": "Ctrl+s"}` (preferred)
    /// - Explicit: `{"key": "s", "modifiers": {"ctrl": true}}`
    /// - Legacy combined: `{"key": "ctrl+s"}` (key field contains combo)
    pub fn from_wire(payload: &serde_json::Value) -> Option<Self> {
        // Try combined format first (preferred).
        if let Some(combo) = payload.get("combo").and_then(|v| v.as_str()) {
            return combo.parse().ok();
        }

        let key_str = payload.get("key").and_then(|v| v.as_str())?;

        // Explicit modifiers take priority.
        if let Some(mods) = payload.get("modifiers") {
            let get_bool = |key| mods.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
            let modifiers = KeyModifiers {
                shift: get_bool("shift"),
                ctrl: get_bool("ctrl"),
                alt: get_bool("alt"),
                logo: get_bool("logo"),
                command: get_bool("command"),
            };
            return Some(Self {
                key: Key::from_wire(key_str),
                modifiers,
            });
        }

        // No explicit modifiers: parse key field as a combo string
        // (handles "ctrl+s" in the key field).
        key_str.parse().ok()
    }
}

impl FromStr for KeyPress {
    type Err = ParseKeyPressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Split on '+' preserving structure, then normalize each part.
        let parts: Vec<&str> = s.split('+').collect();

        if parts.len() == 1 {
            // No '+': just a key name.
            return Ok(Self {
                key: Key::from(parts[0].trim()),
                modifiers: KeyModifiers::default(),
            });
        }

        let mut modifiers = KeyModifiers::default();
        for part in &parts[..parts.len() - 1] {
            let trimmed = part.trim();
            let normalized = normalize(trimmed);
            match normalized.as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "shift" => modifiers.shift = true,
                "alt" | "option" | "opt" => modifiers.alt = true,
                "logo" | "super" | "win" | "meta" => modifiers.logo = true,
                // "command"/"cmd" sets the platform-aware command
                // field. The renderer resolves this to the correct
                // physical modifier at event time: Ctrl on Linux/
                // Windows, Cmd (Logo) on macOS.
                "command" | "cmd" => modifiers.command = true,
                "" => {}
                _ => {
                    return Err(ParseKeyPressError {
                        modifier: trimmed.to_string(),
                    });
                }
            }
        }

        let key = Key::from(parts.last().unwrap().trim());
        Ok(Self { key, modifiers })
    }
}

impl From<&str> for KeyPress {
    fn from(s: &str) -> Self {
        s.parse().unwrap_or_else(|_| Self {
            key: Key::from(s.trim()),
            modifiers: KeyModifiers::default(),
        })
    }
}

impl From<String> for KeyPress {
    fn from(s: String) -> Self {
        KeyPress::from(s.as_str())
    }
}

impl From<Key> for KeyPress {
    fn from(key: Key) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::default(),
        }
    }
}

impl From<(Key, KeyModifiers)> for KeyPress {
    fn from((key, modifiers): (Key, KeyModifiers)) -> Self {
        Self { key, modifiers }
    }
}

impl fmt::Display for KeyPress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.modifiers.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.shift {
            parts.push("Shift".to_string());
        }
        if self.modifiers.alt {
            parts.push("Alt".to_string());
        }
        if self.modifiers.logo {
            parts.push("Super".to_string());
        }
        if self.modifiers.command {
            parts.push("Command".to_string());
        }
        parts.push(self.key.wire_name());
        write!(f, "{}", parts.join("+"))
    }
}

// ---------------------------------------------------------------------------
// MouseButton
// ---------------------------------------------------------------------------

/// A mouse button for canvas and pointer interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MouseButton {
    #[default]
    /// Left.
    Left,
    /// Right.
    Right,
    /// Middle.
    Middle,
    /// Back.
    Back,
    /// Forward.
    Forward,
}

impl MouseButton {
    /// Set or construct `wire_name`.
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
            Self::Back => "back",
            Self::Forward => "forward",
        }
    }

    /// Parse from a wire string. Returns `None` for unrecognized values.
    pub fn from_wire(s: &str) -> Option<Self> {
        match normalize(s).as_str() {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "middle" | "center" => Some(Self::Middle),
            "back" => Some(Self::Back),
            "forward" => Some(Self::Forward),
            _ => None,
        }
    }
}

impl From<&str> for MouseButton {
    fn from(s: &str) -> Self {
        Self::from_wire(s).unwrap_or(Self::Left)
    }
}

impl fmt::Display for MouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire_name())
    }
}

// ---------------------------------------------------------------------------
// PointerKind
// ---------------------------------------------------------------------------

/// The type of pointing device that generated an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PointerKind {
    #[default]
    /// Mouse.
    Mouse,
    /// Touch.
    Touch,
    /// Pen.
    Pen,
}

impl PointerKind {
    /// Set or construct `wire_name`.
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::Mouse => "mouse",
            Self::Touch => "touch",
            Self::Pen => "pen",
        }
    }

    /// Parse from a wire string. Returns `None` for unrecognized values.
    pub fn from_wire(s: &str) -> Option<Self> {
        match normalize(s).as_str() {
            "mouse" => Some(Self::Mouse),
            "touch" => Some(Self::Touch),
            "pen" => Some(Self::Pen),
            _ => None,
        }
    }
}

impl From<&str> for PointerKind {
    fn from(s: &str) -> Self {
        Self::from_wire(s).unwrap_or(Self::Mouse)
    }
}

impl fmt::Display for PointerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire_name())
    }
}

// ---------------------------------------------------------------------------
// InteractAction
// ---------------------------------------------------------------------------

/// An automation interaction action.
///
/// These map to the actions the renderer's interact handler supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractAction {
    /// Click.
    Click,
    /// Type Text.
    TypeText,
    /// Submit.
    Submit,
    /// Toggle.
    Toggle,
    /// Select.
    Select,
    /// Slide.
    Slide,
    /// Paste.
    Paste,
    /// Scroll.
    Scroll,
    /// Sort.
    Sort,
    /// Pane Focus Cycle.
    PaneFocusCycle,
    /// Press.
    Press,
    /// Release.
    Release,
    /// Type Key.
    TypeKey,
    /// Move To.
    MoveTo,
    /// Canvas Press.
    CanvasPress,
    /// Canvas Release.
    CanvasRelease,
    /// Canvas Move.
    CanvasMove,
}

impl InteractAction {
    /// Set or construct `wire_name`.
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::TypeText => "type_text",
            Self::Submit => "submit",
            Self::Toggle => "toggle",
            Self::Select => "select",
            Self::Slide => "slide",
            Self::Paste => "paste",
            Self::Scroll => "scroll",
            Self::Sort => "sort",
            Self::PaneFocusCycle => "pane_focus_cycle",
            Self::Press => "press",
            Self::Release => "release",
            Self::TypeKey => "type_key",
            Self::MoveTo => "move_to",
            Self::CanvasPress => "canvas_press",
            Self::CanvasRelease => "canvas_release",
            Self::CanvasMove => "canvas_move",
        }
    }

    /// Construct from a wire.
    pub fn from_wire(s: &str) -> Option<Self> {
        Some(match normalize(s).as_str() {
            "click" => Self::Click,
            "typetext" | "type" => Self::TypeText,
            "submit" => Self::Submit,
            "toggle" => Self::Toggle,
            "select" => Self::Select,
            "slide" => Self::Slide,
            "paste" => Self::Paste,
            "scroll" => Self::Scroll,
            "sort" => Self::Sort,
            "panefocuscycle" => Self::PaneFocusCycle,
            "press" => Self::Press,
            "release" => Self::Release,
            "typekey" => Self::TypeKey,
            "moveto" | "move" => Self::MoveTo,
            "canvaspress" => Self::CanvasPress,
            "canvasrelease" => Self::CanvasRelease,
            "canvasmove" => Self::CanvasMove,
            _ => return None,
        })
    }
}

impl fmt::Display for InteractAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire_name())
    }
}

// ---------------------------------------------------------------------------
// EffectKind
// ---------------------------------------------------------------------------

/// The kind of platform effect, matching [`EffectRequest`](crate::ops::EffectRequest) variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectKind {
    /// File Open.
    FileOpen,
    /// File Open Multiple.
    FileOpenMultiple,
    /// File Save.
    FileSave,
    /// Directory Select.
    DirectorySelect,
    /// Directory Select Multiple.
    DirectorySelectMultiple,
    /// Clipboard Read.
    ClipboardRead,
    /// Clipboard Write.
    ClipboardWrite,
    /// Clipboard Read Html.
    ClipboardReadHtml,
    /// Clipboard Write Html.
    ClipboardWriteHtml,
    /// Clipboard Clear.
    ClipboardClear,
    /// Clipboard Read Primary.
    ClipboardReadPrimary,
    /// Clipboard Write Primary.
    ClipboardWritePrimary,
    /// Notification.
    Notification,
}

impl EffectKind {
    /// Set or construct `wire_name`.
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::FileOpen => "file_open",
            Self::FileOpenMultiple => "file_open_multiple",
            Self::FileSave => "file_save",
            Self::DirectorySelect => "directory_select",
            Self::DirectorySelectMultiple => "directory_select_multiple",
            Self::ClipboardRead => "clipboard_read",
            Self::ClipboardWrite => "clipboard_write",
            Self::ClipboardReadHtml => "clipboard_read_html",
            Self::ClipboardWriteHtml => "clipboard_write_html",
            Self::ClipboardClear => "clipboard_clear",
            Self::ClipboardReadPrimary => "clipboard_read_primary",
            Self::ClipboardWritePrimary => "clipboard_write_primary",
            Self::Notification => "notification",
        }
    }
}

impl EffectKind {
    /// Parse from a string, returning None for unrecognized kinds.
    pub fn from_wire(s: &str) -> Option<Self> {
        Some(match normalize(s).as_str() {
            "fileopen" => Self::FileOpen,
            "fileopenmultiple" => Self::FileOpenMultiple,
            "filesave" => Self::FileSave,
            "directoryselect" => Self::DirectorySelect,
            "directoryselectmultiple" => Self::DirectorySelectMultiple,
            "clipboardread" => Self::ClipboardRead,
            "clipboardwrite" => Self::ClipboardWrite,
            "clipboardreadhtml" => Self::ClipboardReadHtml,
            "clipboardwritehtml" => Self::ClipboardWriteHtml,
            "clipboardclear" => Self::ClipboardClear,
            "clipboardreadprimary" => Self::ClipboardReadPrimary,
            "clipboardwriteprimary" => Self::ClipboardWritePrimary,
            "notification" => Self::Notification,
            _ => return None,
        })
    }
}

impl fmt::Display for EffectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire_name())
    }
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/// Normalize a string for forgiving lookup.
///
/// Strips whitespace, underscores, and hyphens, then lowercases.
/// This makes `"LeftArrow"`, `"left_arrow"`, `"left-arrow"`, and
/// `"left arrow"` all equivalent.
pub fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_and_lowercases() {
        assert_eq!(normalize("LeftArrow"), "leftarrow");
        assert_eq!(normalize("left_arrow"), "leftarrow");
        assert_eq!(normalize("left-arrow"), "leftarrow");
        assert_eq!(normalize("Left Arrow"), "leftarrow");
        assert_eq!(normalize("PAGE_UP"), "pageup");
        assert_eq!(normalize("Ctrl"), "ctrl");
    }

    #[test]
    fn key_from_str_named_keys() {
        assert_eq!(Key::from("Enter"), Key::Enter);
        assert_eq!(Key::from("enter"), Key::Enter);
        assert_eq!(Key::from("return"), Key::Enter);
        assert_eq!(Key::from("ESCAPE"), Key::Escape);
        assert_eq!(Key::from("esc"), Key::Escape);
        assert_eq!(Key::from("Tab"), Key::Tab);
        assert_eq!(Key::from("Backspace"), Key::Backspace);
        assert_eq!(Key::from("bs"), Key::Backspace);
        assert_eq!(Key::from("Delete"), Key::Delete);
        assert_eq!(Key::from("del"), Key::Delete);
        assert_eq!(Key::from("Space"), Key::Space);
    }

    #[test]
    fn key_from_str_arrows() {
        assert_eq!(Key::from("ArrowLeft"), Key::ArrowLeft);
        assert_eq!(Key::from("left_arrow"), Key::ArrowLeft);
        assert_eq!(Key::from("left"), Key::ArrowLeft);
        assert_eq!(Key::from("Left"), Key::ArrowLeft);
        assert_eq!(Key::from("LeftArrow"), Key::ArrowLeft);
        assert_eq!(Key::from("ArrowUp"), Key::ArrowUp);
        assert_eq!(Key::from("up"), Key::ArrowUp);
    }

    #[test]
    fn key_from_str_page_nav() {
        assert_eq!(Key::from("PageUp"), Key::PageUp);
        assert_eq!(Key::from("page_up"), Key::PageUp);
        assert_eq!(Key::from("pgup"), Key::PageUp);
        assert_eq!(Key::from("PageDown"), Key::PageDown);
        assert_eq!(Key::from("pgdn"), Key::PageDown);
    }

    #[test]
    fn key_from_str_function_keys() {
        assert_eq!(Key::from("F1"), Key::F1);
        assert_eq!(Key::from("f12"), Key::F12);
    }

    #[test]
    fn key_from_str_single_char() {
        assert_eq!(Key::from("a"), Key::Char('a'));
        assert_eq!(Key::from("1"), Key::Char('1'));
    }

    #[test]
    fn key_from_str_unknown_falls_to_named() {
        assert_eq!(Key::from("MediaPlay"), Key::Named("mediaplay".into()));
    }

    #[test]
    fn key_from_wire_preserves_unknown_name() {
        assert_eq!(Key::from_wire("MediaPlay"), Key::Named("MediaPlay".into()));
        assert_eq!(Key::from_wire("FutureKey"), Key::Named("FutureKey".into()));
    }

    #[test]
    fn key_from_wire_keeps_known_names_forgiving() {
        assert_eq!(Key::from_wire("left_arrow"), Key::ArrowLeft);
        assert_eq!(Key::from_wire("RETURN"), Key::Enter);
        assert_eq!(Key::from_wire("A"), Key::Char('A'));
    }

    #[test]
    fn keypress_from_str_simple() {
        let kp = KeyPress::from("Enter");
        assert_eq!(kp.key, Key::Enter);
        assert_eq!(kp.modifiers, KeyModifiers::default());
    }

    #[test]
    fn keypress_from_str_with_modifier() {
        let kp = KeyPress::from("Ctrl+s");
        assert_eq!(kp.key, Key::Char('s'));
        assert!(kp.modifiers.ctrl);
        assert!(!kp.modifiers.shift);
    }

    #[test]
    fn keypress_from_str_multiple_modifiers() {
        let kp = KeyPress::from("Ctrl+Shift+Enter");
        assert_eq!(kp.key, Key::Enter);
        assert!(kp.modifiers.ctrl);
        assert!(kp.modifiers.shift);
    }

    #[test]
    fn keypress_from_str_spaces_around_plus() {
        let kp = KeyPress::from("Ctrl + Left_Arrow");
        assert_eq!(kp.key, Key::ArrowLeft);
        assert!(kp.modifiers.ctrl);
    }

    #[test]
    fn keypress_from_str_modifier_aliases() {
        // "command"/"cmd" sets the platform-aware command field.
        // The renderer resolves it to ctrl or logo at event time.
        let kp = KeyPress::from("Command+s");
        assert!(kp.modifiers.command);
        assert!(!kp.modifiers.ctrl);
        assert!(!kp.modifiers.logo);

        let kp = KeyPress::from("Option+a");
        assert!(kp.modifiers.alt);

        // "super"/"logo"/"win"/"meta" set the physical logo key
        let kp = KeyPress::from("Win+e");
        assert!(kp.modifiers.logo);

        let kp = KeyPress::from("Super+e");
        assert!(kp.modifiers.logo);

        // "ctrl" is always the physical Ctrl key
        let kp = KeyPress::from("Ctrl+s");
        assert!(kp.modifiers.ctrl);
        assert!(!kp.modifiers.command);
    }

    #[test]
    fn keypress_from_str_malformed() {
        // Empty string: no modifiers, falls through to Named("").
        let kp = KeyPress::from("");
        assert_eq!(kp.key, Key::Named(String::new()));
        assert_eq!(kp.modifiers, KeyModifiers::default());

        // Bare '+': both segments empty. No known modifiers set,
        // key parses as Named("").
        let kp = KeyPress::from("+");
        assert_eq!(kp.key, Key::Named(String::new()));
        assert_eq!(kp.modifiers, KeyModifiers::default());

        // Trailing '+': modifier present but key segment empty.
        // Modifier is applied, key falls through to Named("").
        let kp = KeyPress::from("Ctrl+");
        assert_eq!(kp.key, Key::Named(String::new()));
        assert!(kp.modifiers.ctrl);

        let err = "Foo+s".parse::<KeyPress>().unwrap_err();
        assert_eq!(err.modifier(), "Foo");

        // Leading '+': empty modifier segment dropped, key parses
        // normally.
        let kp = KeyPress::from("+s");
        assert_eq!(kp.key, Key::Char('s'));
        assert_eq!(kp.modifiers, KeyModifiers::default());
    }

    #[test]
    fn keypress_from_str_unknown_modifier_is_literal_key() {
        let kp = KeyPress::from("Crtl+s");
        assert_eq!(kp.key, Key::Named("crtl+s".to_string()));
        assert_eq!(kp.modifiers, KeyModifiers::default());
    }

    #[test]
    fn keypress_from_wire_rejects_unknown_modifier_combo() {
        let payload = serde_json::json!({"combo": "Crtl+s"});
        assert_eq!(KeyPress::from_wire(&payload), None);
    }

    #[test]
    fn keypress_from_wire_combo() {
        let payload = serde_json::json!({"combo": "Shift+Enter"});
        let kp = KeyPress::from_wire(&payload).unwrap();
        assert_eq!(kp.key, Key::Enter);
        assert!(kp.modifiers.shift);
    }

    #[test]
    fn keypress_from_wire_explicit() {
        let payload = serde_json::json!({"key": "s", "modifiers": {"ctrl": true}});
        let kp = KeyPress::from_wire(&payload).unwrap();
        assert_eq!(kp.key, Key::Char('s'));
        assert!(kp.modifiers.ctrl);
    }

    #[test]
    fn keypress_from_wire_explicit_preserves_unknown_key_name() {
        let payload = serde_json::json!({"key": "MediaPlay", "modifiers": {"ctrl": true}});
        let kp = KeyPress::from_wire(&payload).unwrap();
        assert_eq!(kp.key, Key::Named("MediaPlay".into()));
        assert!(kp.modifiers.ctrl);
    }

    #[test]
    fn keypress_from_wire_command_alias() {
        let payload = serde_json::json!({"key": "s", "modifiers": {"command": true}});
        let kp = KeyPress::from_wire(&payload).unwrap();
        assert!(kp.modifiers.command);
        assert!(!kp.modifiers.ctrl);
    }

    #[test]
    fn keypress_display_includes_command_modifier() {
        let kp = KeyPress::from("Command+s");
        assert_eq!(kp.to_string(), "Command+s");
    }

    #[test]
    fn mouse_button_from_str() {
        assert_eq!(MouseButton::from("left"), MouseButton::Left);
        assert_eq!(MouseButton::from("Right"), MouseButton::Right);
        assert_eq!(MouseButton::from("MIDDLE"), MouseButton::Middle);
        assert_eq!(MouseButton::from("center"), MouseButton::Middle);
        assert_eq!(MouseButton::from("unknown"), MouseButton::Left);
    }

    #[test]
    fn interact_action_from_wire() {
        assert_eq!(
            InteractAction::from_wire("click"),
            Some(InteractAction::Click)
        );
        assert_eq!(
            InteractAction::from_wire("type_text"),
            Some(InteractAction::TypeText)
        );
        assert_eq!(
            InteractAction::from_wire("canvas_press"),
            Some(InteractAction::CanvasPress)
        );
        assert_eq!(InteractAction::from_wire("unknown"), None);
    }

    #[test]
    fn effect_kind_from_wire() {
        assert_eq!(
            EffectKind::from_wire("file_open"),
            Some(EffectKind::FileOpen)
        );
        assert_eq!(
            EffectKind::from_wire("clipboard_read"),
            Some(EffectKind::ClipboardRead)
        );
        assert_eq!(
            EffectKind::from_wire("FileOpen"),
            Some(EffectKind::FileOpen)
        );
        assert_eq!(EffectKind::from_wire("nonsense"), None);
    }
}
