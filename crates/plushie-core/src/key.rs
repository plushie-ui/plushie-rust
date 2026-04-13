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

use std::fmt;

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
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,

    // -- Editing --
    Enter,
    Tab,
    Space,
    Backspace,
    Delete,
    Insert,
    Escape,

    // -- Modifiers (as key events, not as modifiers on combos) --
    Shift,
    Control,
    Alt,
    Super,

    // -- Function keys --
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // -- Common extras --
    CapsLock,
    NumLock,
    ScrollLock,
    PrintScreen,
    Pause,
    ContextMenu,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,

    // -- Single character --
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
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.wire_name())
    }
}

/// Parse a key name from a normalized string (lowercase, no
/// whitespace/underscores/hyphens).
fn parse_key_normalized(s: &str) -> Key {
    match s {
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

        // Also match single uppercase chars that went through normalize
        // (the caller may have passed a pre-normalized string).
        // This can't happen because normalize lowercases, but be safe.

        // Fallback: preserve original PascalCase name for iced
        // (the normalized form doesn't help here, so we accept
        // that Named keys from strings are lowercased)
        _ => Key::Named(s.to_string()),
    }
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
    pub key: Key,
    pub modifiers: KeyModifiers,
}

impl KeyPress {
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
            return Some(KeyPress::from(combo));
        }

        let key_str = payload.get("key").and_then(|v| v.as_str())?;

        // Explicit modifiers take priority.
        if let Some(mods) = payload.get("modifiers") {
            let get_bool = |key| mods.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
            let modifiers = KeyModifiers {
                shift: get_bool("shift"),
                ctrl: get_bool("ctrl") || get_bool("command"),
                alt: get_bool("alt"),
                logo: get_bool("logo"),
                command: get_bool("command"),
            };
            return Some(Self {
                key: Key::from(key_str),
                modifiers,
            });
        }

        // No explicit modifiers: parse key field as a combo string
        // (handles "ctrl+s" in the key field).
        Some(KeyPress::from(key_str))
    }
}

impl From<&str> for KeyPress {
    fn from(s: &str) -> Self {
        // Split on '+' preserving structure, then normalize each part.
        let parts: Vec<&str> = s.split('+').collect();

        if parts.len() == 1 {
            // No '+': just a key name.
            return Self {
                key: Key::from(parts[0].trim()),
                modifiers: KeyModifiers::default(),
            };
        }

        let mut modifiers = KeyModifiers::default();
        for part in &parts[..parts.len() - 1] {
            let normalized = normalize(part.trim());
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
                _ => {} // Unknown modifier segment ignored
            }
        }

        let key = Key::from(parts.last().unwrap().trim());
        Self { key, modifiers }
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
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }
}

impl From<&str> for MouseButton {
    fn from(s: &str) -> Self {
        match normalize(s).as_str() {
            "right" => Self::Right,
            "middle" | "center" => Self::Middle,
            _ => Self::Left,
        }
    }
}

impl fmt::Display for MouseButton {
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
    Click,
    TypeText,
    Submit,
    Toggle,
    Select,
    Slide,
    Paste,
    Scroll,
    Sort,
    PaneFocusCycle,
    Press,
    Release,
    TypeKey,
    MoveTo,
    CanvasPress,
    CanvasRelease,
    CanvasMove,
}

impl InteractAction {
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
    FileOpen,
    FileOpenMultiple,
    FileSave,
    DirectorySelect,
    DirectorySelectMultiple,
    ClipboardRead,
    ClipboardWrite,
    ClipboardReadHtml,
    ClipboardWriteHtml,
    ClipboardClear,
    ClipboardReadPrimary,
    ClipboardWritePrimary,
    Notification,
}

impl EffectKind {
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
    fn keypress_from_wire_command_alias() {
        let payload = serde_json::json!({"key": "s", "modifiers": {"command": true}});
        let kp = KeyPress::from_wire(&payload).unwrap();
        assert!(kp.modifiers.ctrl);
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
