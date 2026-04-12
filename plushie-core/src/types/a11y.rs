//! Accessibility types for Plushie widgets.
//!
//! Provides the core A11y struct and supporting enums (Role, Live,
//! Orientation, HasPopup) with PlushieType wire encoding/decoding.
//! These types are shared between the Elixir SDK, Rust SDK, and
//! the renderer.

use serde_json::Value;

use crate::PlushieEnum;
use crate::protocol::{PropMap, PropValue, Props};

use super::PlushieType;

// ---------------------------------------------------------------------------
// Role enum
// ---------------------------------------------------------------------------

/// Semantic role for an accessible widget node.
///
/// Maps 1:1 to iced's `accessible::Role` variants. Wire format is a
/// snake_case string. Some roles have aliases (e.g., "radio" decodes
/// to `RadioButton`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PlushieEnum)]
#[plushie_type(name = "role")]
pub enum Role {
    Alert,
    AlertDialog,
    Button,
    Canvas,
    #[plushie(wire = "check_box")]
    CheckBox,
    #[plushie(wire = "table_cell", aliases = ["cell"])]
    Cell,
    ColumnHeader,
    ComboBox,
    Dialog,
    Document,
    #[plushie(aliases = ["container", "generic"])]
    GenericContainer,
    Group,
    Heading,
    Image,
    Label,
    Link,
    List,
    ListItem,
    Menu,
    MenuBar,
    MenuItem,
    Meter,
    #[plushie(aliases = ["text_editor"])]
    MultilineTextInput,
    Navigation,
    #[plushie(aliases = ["progress_bar"])]
    ProgressIndicator,
    #[plushie(aliases = ["radio"])]
    RadioButton,
    RadioGroup,
    Region,
    #[plushie(wire = "table_row", aliases = ["row"])]
    Row,
    ScrollBar,
    ScrollView,
    Search,
    Separator,
    Slider,
    StaticText,
    Status,
    Switch,
    Tab,
    TabList,
    TabPanel,
    Table,
    TextInput,
    Toolbar,
    Tooltip,
    Tree,
    TreeItem,
    Window,
}

// ---------------------------------------------------------------------------
// Live enum
// ---------------------------------------------------------------------------

/// Live region urgency level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "live")]
pub enum Live {
    Polite,
    Assertive,
}

// ---------------------------------------------------------------------------
// Orientation enum
// ---------------------------------------------------------------------------

/// Widget orientation (horizontal or vertical).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "orientation")]
pub enum Orientation {
    Horizontal,
    Vertical,
}

// ---------------------------------------------------------------------------
// HasPopup enum
// ---------------------------------------------------------------------------

/// Type of popup a widget triggers when activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
#[plushie_type(name = "has_popup")]
pub enum HasPopup {
    Listbox,
    Menu,
    Dialog,
    Tree,
    Grid,
}

// ---------------------------------------------------------------------------
// A11y struct
// ---------------------------------------------------------------------------

/// Accessibility annotations for a widget node.
///
/// Carries role, label, state flags, and relationship IDs. Used by both
/// widget props (`a11y` key) and canvas interactive shapes.
///
/// Build fluently:
/// ```
/// use plushie_core::types::a11y::{A11y, Role, Live};
///
/// let a11y = A11y::new()
///     .role(Role::Button)
///     .label("Save document")
///     .live(Live::Polite);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct A11y {
    pub role: Option<Role>,
    pub label: Option<String>,
    pub description: Option<String>,
    /// Whether the widget is hidden from assistive technology.
    ///
    /// Wire encoding omits `false` values (the default). Decode treats
    /// a missing key as `false`, so the round-trip is consistent.
    pub hidden: bool,
    pub expanded: Option<bool>,
    pub required: bool,
    /// Heading level (1 through 6, corresponding to h1-h6).
    /// Values outside this range are rejected during wire decode.
    pub level: Option<usize>,
    pub live: Option<Live>,
    pub busy: Option<bool>,
    pub invalid: bool,
    pub modal: bool,
    pub read_only: bool,
    pub mnemonic: Option<char>,
    pub toggled: Option<bool>,
    pub selected: Option<bool>,
    pub value: Option<String>,
    pub orientation: Option<Orientation>,
    pub disabled: Option<bool>,
    pub position_in_set: Option<usize>,
    pub size_of_set: Option<usize>,
    pub labelled_by: Option<String>,
    pub described_by: Option<String>,
    pub error_message: Option<String>,
    pub active_descendant: Option<String>,
    pub radio_group: Option<Vec<String>>,
    pub has_popup: Option<HasPopup>,
    /// Elixir-only field: resolved at build time to populate `label`.
    /// Included for wire completeness; the renderer ignores it.
    pub label_from: Option<String>,
}

// ---------------------------------------------------------------------------
// Builder methods
// ---------------------------------------------------------------------------

impl A11y {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an A11y with just a description set.
    ///
    /// Convenience for widget `infer_a11y` implementations that only
    /// need to expose a description (e.g., from a placeholder prop).
    pub fn with_description(description: impl Into<String>) -> Self {
        Self {
            description: Some(description.into()),
            ..Self::default()
        }
    }

    pub fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn level(mut self, level: usize) -> Self {
        self.level = Some(level);
        self
    }

    pub fn live(mut self, live: Live) -> Self {
        self.live = Some(live);
        self
    }

    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = Some(busy);
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn mnemonic(mut self, mnemonic: char) -> Self {
        self.mnemonic = Some(mnemonic);
        self
    }

    pub fn toggled(mut self, toggled: bool) -> Self {
        self.toggled = Some(toggled);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    pub fn position_in_set(mut self, pos: usize) -> Self {
        self.position_in_set = Some(pos);
        self
    }

    pub fn size_of_set(mut self, size: usize) -> Self {
        self.size_of_set = Some(size);
        self
    }

    pub fn labelled_by(mut self, id: impl Into<String>) -> Self {
        self.labelled_by = Some(id.into());
        self
    }

    pub fn described_by(mut self, id: impl Into<String>) -> Self {
        self.described_by = Some(id.into());
        self
    }

    pub fn error_message(mut self, id: impl Into<String>) -> Self {
        self.error_message = Some(id.into());
        self
    }

    pub fn active_descendant(mut self, id: impl Into<String>) -> Self {
        self.active_descendant = Some(id.into());
        self
    }

    pub fn radio_group(mut self, ids: Vec<String>) -> Self {
        self.radio_group = Some(ids);
        self
    }

    pub fn has_popup(mut self, popup: HasPopup) -> Self {
        self.has_popup = Some(popup);
        self
    }

    pub fn label_from(mut self, id: impl Into<String>) -> Self {
        self.label_from = Some(id.into());
        self
    }
}

// ---------------------------------------------------------------------------
// PlushieType impl
// ---------------------------------------------------------------------------

impl PlushieType for A11y {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;

        let role = obj.get("role").and_then(Role::wire_decode);
        let label = obj.get("label").and_then(|v| v.as_str()).map(String::from);
        let description = obj
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);
        let hidden = obj.get("hidden").and_then(|v| v.as_bool()).unwrap_or(false);
        let expanded = obj.get("expanded").and_then(|v| v.as_bool());
        let required = obj
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let level = obj.get("level").and_then(|v| v.as_u64()).and_then(|n| {
            let n = n as usize;
            if (1..=6).contains(&n) { Some(n) } else { None }
        });

        let live = obj.get("live").and_then(Live::wire_decode);
        let busy = obj.get("busy").and_then(|v| v.as_bool());
        let invalid = obj
            .get("invalid")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let modal = obj.get("modal").and_then(|v| v.as_bool()).unwrap_or(false);
        let read_only = obj
            .get("read_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mnemonic = obj
            .get("mnemonic")
            .and_then(|v| v.as_str())
            .and_then(|s| s.chars().next());

        let toggled = obj.get("toggled").and_then(|v| v.as_bool());
        let selected = obj.get("selected").and_then(|v| v.as_bool());
        let value = obj.get("value").and_then(|v| v.as_str()).map(String::from);
        let orientation = obj.get("orientation").and_then(Orientation::wire_decode);
        let disabled = obj.get("disabled").and_then(|v| v.as_bool());

        let position_in_set = obj
            .get("position_in_set")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let size_of_set = obj
            .get("size_of_set")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        let labelled_by = obj
            .get("labelled_by")
            .and_then(|v| v.as_str())
            .map(String::from);
        let described_by = obj
            .get("described_by")
            .and_then(|v| v.as_str())
            .map(String::from);
        let error_message = obj
            .get("error_message")
            .and_then(|v| v.as_str())
            .map(String::from);
        let active_descendant = obj
            .get("active_descendant")
            .and_then(|v| v.as_str())
            .map(String::from);

        let radio_group = obj
            .get("radio_group")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        let has_popup = obj.get("has_popup").and_then(HasPopup::wire_decode);
        let label_from = obj
            .get("label_from")
            .and_then(|v| v.as_str())
            .map(String::from);

        Some(Self {
            role,
            label,
            description,
            hidden,
            expanded,
            required,
            level,
            live,
            busy,
            invalid,
            modal,
            read_only,
            mnemonic,
            toggled,
            selected,
            value,
            orientation,
            disabled,
            position_in_set,
            size_of_set,
            labelled_by,
            described_by,
            error_message,
            active_descendant,
            radio_group,
            has_popup,
            label_from,
        })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();

        if let Some(ref role) = self.role {
            m.insert("role", role.wire_encode());
        }
        if let Some(ref label) = self.label {
            m.insert("label", PropValue::Str(label.clone()));
        }
        if let Some(ref description) = self.description {
            m.insert("description", PropValue::Str(description.clone()));
        }
        if self.hidden {
            m.insert("hidden", PropValue::Bool(true));
        }
        if let Some(expanded) = self.expanded {
            m.insert("expanded", PropValue::Bool(expanded));
        }
        if self.required {
            m.insert("required", PropValue::Bool(true));
        }
        if let Some(level) = self.level {
            m.insert("level", PropValue::U64(level as u64));
        }
        if let Some(ref live) = self.live {
            m.insert("live", live.wire_encode());
        }
        if let Some(busy) = self.busy {
            m.insert("busy", PropValue::Bool(busy));
        }
        if self.invalid {
            m.insert("invalid", PropValue::Bool(true));
        }
        if self.modal {
            m.insert("modal", PropValue::Bool(true));
        }
        if self.read_only {
            m.insert("read_only", PropValue::Bool(true));
        }
        if let Some(mnemonic) = self.mnemonic {
            m.insert("mnemonic", PropValue::Str(mnemonic.to_string()));
        }
        if let Some(toggled) = self.toggled {
            m.insert("toggled", PropValue::Bool(toggled));
        }
        if let Some(selected) = self.selected {
            m.insert("selected", PropValue::Bool(selected));
        }
        if let Some(ref value) = self.value {
            m.insert("value", PropValue::Str(value.clone()));
        }
        if let Some(ref orientation) = self.orientation {
            m.insert("orientation", orientation.wire_encode());
        }
        if let Some(disabled) = self.disabled {
            m.insert("disabled", PropValue::Bool(disabled));
        }
        if let Some(pos) = self.position_in_set {
            m.insert("position_in_set", PropValue::U64(pos as u64));
        }
        if let Some(size) = self.size_of_set {
            m.insert("size_of_set", PropValue::U64(size as u64));
        }
        if let Some(ref id) = self.labelled_by {
            m.insert("labelled_by", PropValue::Str(id.clone()));
        }
        if let Some(ref id) = self.described_by {
            m.insert("described_by", PropValue::Str(id.clone()));
        }
        if let Some(ref id) = self.error_message {
            m.insert("error_message", PropValue::Str(id.clone()));
        }
        if let Some(ref id) = self.active_descendant {
            m.insert("active_descendant", PropValue::Str(id.clone()));
        }
        if let Some(ref ids) = self.radio_group {
            let arr = ids.iter().map(|s| PropValue::Str(s.clone())).collect();
            m.insert("radio_group", PropValue::Array(arr));
        }
        if let Some(ref popup) = self.has_popup {
            m.insert("has_popup", popup.wire_encode());
        }
        if let Some(ref id) = self.label_from {
            m.insert("label_from", PropValue::Str(id.clone()));
        }

        PropValue::Object(m)
    }

    fn extract(props: &Props, key: &str) -> Option<Self> {
        props.get_value(key).and_then(|v| Self::wire_decode(&v))
    }

    fn type_name() -> &'static str {
        "a11y"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn role_round_trip() {
        let roles = [
            (Role::Button, "button"),
            (Role::CheckBox, "check_box"),
            (Role::GenericContainer, "generic_container"),
            (Role::RadioButton, "radio_button"),
            (Role::ProgressIndicator, "progress_indicator"),
            (Role::Row, "table_row"),
            (Role::Cell, "table_cell"),
        ];
        for (role, expected_name) in &roles {
            let encoded = role.wire_encode();
            assert_eq!(encoded, PropValue::Str(expected_name.to_string()));
            let decoded = Role::wire_decode(&json!(expected_name)).unwrap();
            assert_eq!(*role, decoded);
        }
    }

    #[test]
    fn role_aliases() {
        assert_eq!(Role::wire_decode(&json!("radio")), Some(Role::RadioButton));
        assert_eq!(
            Role::wire_decode(&json!("radio_button")),
            Some(Role::RadioButton)
        );
        assert_eq!(
            Role::wire_decode(&json!("container")),
            Some(Role::GenericContainer)
        );
        assert_eq!(
            Role::wire_decode(&json!("generic")),
            Some(Role::GenericContainer)
        );
        assert_eq!(
            Role::wire_decode(&json!("text_editor")),
            Some(Role::MultilineTextInput)
        );
        assert_eq!(
            Role::wire_decode(&json!("progress_bar")),
            Some(Role::ProgressIndicator)
        );
        assert_eq!(Role::wire_decode(&json!("row")), Some(Role::Row));
        assert_eq!(Role::wire_decode(&json!("cell")), Some(Role::Cell));
    }

    #[test]
    fn role_unknown_returns_none() {
        assert_eq!(
            Role::wire_decode(&json!("widget_that_does_not_exist")),
            None
        );
    }

    #[test]
    fn live_round_trip() {
        assert_eq!(Live::wire_decode(&json!("polite")), Some(Live::Polite));
        assert_eq!(
            Live::wire_decode(&json!("assertive")),
            Some(Live::Assertive)
        );
        assert_eq!(Live::wire_decode(&json!("off")), None);
        assert_eq!(Live::Polite.wire_encode(), PropValue::Str("polite".into()));
    }

    #[test]
    fn orientation_round_trip() {
        assert_eq!(
            Orientation::wire_decode(&json!("horizontal")),
            Some(Orientation::Horizontal)
        );
        assert_eq!(
            Orientation::wire_decode(&json!("vertical")),
            Some(Orientation::Vertical)
        );
        assert_eq!(Orientation::wire_decode(&json!("diagonal")), None);
        assert_eq!(
            Orientation::Vertical.wire_encode(),
            PropValue::Str("vertical".into())
        );
    }

    #[test]
    fn has_popup_round_trip() {
        let variants = [
            (HasPopup::Listbox, "listbox"),
            (HasPopup::Menu, "menu"),
            (HasPopup::Dialog, "dialog"),
            (HasPopup::Tree, "tree"),
            (HasPopup::Grid, "grid"),
        ];
        for (variant, name) in &variants {
            let encoded = variant.wire_encode();
            assert_eq!(encoded, PropValue::Str(name.to_string()));
            let decoded = HasPopup::wire_decode(&json!(name)).unwrap();
            assert_eq!(*variant, decoded);
        }
    }

    #[test]
    fn a11y_decode_full() {
        let val = json!({
            "role": "button",
            "label": "Save",
            "description": "Save your work",
            "hidden": true,
            "expanded": false,
            "required": true,
            "level": 2,
            "live": "polite",
            "busy": true,
            "invalid": true,
            "modal": true,
            "read_only": true,
            "mnemonic": "S",
            "toggled": true,
            "selected": false,
            "value": "42",
            "orientation": "horizontal",
            "disabled": true,
            "position_in_set": 3,
            "size_of_set": 10,
            "labelled_by": "label_1",
            "described_by": "desc_1",
            "error_message": "err_1",
            "active_descendant": "item_3",
            "radio_group": ["opt_a", "opt_b"],
            "has_popup": "menu",
            "label_from": "title_field",
        });

        let a = A11y::wire_decode(&val).unwrap();
        assert_eq!(a.role, Some(Role::Button));
        assert_eq!(a.label.as_deref(), Some("Save"));
        assert_eq!(a.description.as_deref(), Some("Save your work"));
        assert!(a.hidden);
        assert_eq!(a.expanded, Some(false));
        assert!(a.required);
        assert_eq!(a.level, Some(2));
        assert_eq!(a.live, Some(Live::Polite));
        assert_eq!(a.busy, Some(true));
        assert!(a.invalid);
        assert!(a.modal);
        assert!(a.read_only);
        assert_eq!(a.mnemonic, Some('S'));
        assert_eq!(a.toggled, Some(true));
        assert_eq!(a.selected, Some(false));
        assert_eq!(a.value.as_deref(), Some("42"));
        assert_eq!(a.orientation, Some(Orientation::Horizontal));
        assert_eq!(a.disabled, Some(true));
        assert_eq!(a.position_in_set, Some(3));
        assert_eq!(a.size_of_set, Some(10));
        assert_eq!(a.labelled_by.as_deref(), Some("label_1"));
        assert_eq!(a.described_by.as_deref(), Some("desc_1"));
        assert_eq!(a.error_message.as_deref(), Some("err_1"));
        assert_eq!(a.active_descendant.as_deref(), Some("item_3"));
        assert_eq!(a.radio_group, Some(vec!["opt_a".into(), "opt_b".into()]));
        assert_eq!(a.has_popup, Some(HasPopup::Menu));
        assert_eq!(a.label_from.as_deref(), Some("title_field"));
    }

    #[test]
    fn a11y_decode_empty_object() {
        let a = A11y::wire_decode(&json!({})).unwrap();
        assert_eq!(a, A11y::default());
    }

    #[test]
    fn a11y_decode_non_object_returns_none() {
        assert!(A11y::wire_decode(&json!("not an object")).is_none());
        assert!(A11y::wire_decode(&json!(42)).is_none());
    }

    #[test]
    fn a11y_level_clamped() {
        assert_eq!(A11y::wire_decode(&json!({"level": 0})).unwrap().level, None);
        assert_eq!(
            A11y::wire_decode(&json!({"level": 1})).unwrap().level,
            Some(1)
        );
        assert_eq!(
            A11y::wire_decode(&json!({"level": 6})).unwrap().level,
            Some(6)
        );
        assert_eq!(A11y::wire_decode(&json!({"level": 7})).unwrap().level, None);
    }

    #[test]
    fn a11y_bool_defaults() {
        let a = A11y::wire_decode(&json!({})).unwrap();
        assert!(!a.hidden);
        assert!(!a.required);
        assert!(!a.invalid);
        assert!(!a.modal);
        assert!(!a.read_only);
    }

    #[test]
    fn a11y_encode_round_trip() {
        let original = A11y::new()
            .role(Role::Slider)
            .label("Volume")
            .description("Adjust volume level")
            .hidden(true)
            .expanded(true)
            .required(true)
            .level(3)
            .live(Live::Assertive)
            .busy(false)
            .invalid(true)
            .modal(true)
            .read_only(true)
            .mnemonic('V')
            .toggled(false)
            .selected(true)
            .value("75")
            .orientation(Orientation::Vertical)
            .disabled(false)
            .position_in_set(2)
            .size_of_set(5)
            .labelled_by("vol_label")
            .described_by("vol_desc")
            .error_message("vol_err")
            .active_descendant("opt_2")
            .radio_group(vec!["a".into(), "b".into(), "c".into()])
            .has_popup(HasPopup::Listbox)
            .label_from("title");

        let encoded = original.wire_encode();
        let json_val = Value::from(encoded);
        let decoded = A11y::wire_decode(&json_val).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn a11y_encode_skips_defaults() {
        let a = A11y::new().label("Hello");
        let encoded = a.wire_encode();
        if let PropValue::Object(m) = &encoded {
            // Should have "label" but not "hidden", "required", etc.
            assert!(m.get("label").is_some());
            assert!(m.get("hidden").is_none());
            assert!(m.get("required").is_none());
            assert!(m.get("invalid").is_none());
            assert!(m.get("modal").is_none());
            assert!(m.get("read_only").is_none());
        } else {
            panic!("expected Object");
        }
    }

    #[test]
    fn with_description_convenience() {
        let a = A11y::with_description("Enter your email");
        assert_eq!(a.description.as_deref(), Some("Enter your email"));
        assert_eq!(a.role, None);
        assert_eq!(a.label, None);
    }

    #[test]
    fn builder_chaining() {
        let a = A11y::new()
            .role(Role::CheckBox)
            .toggled(true)
            .label("Accept terms");
        assert_eq!(a.role, Some(Role::CheckBox));
        assert_eq!(a.toggled, Some(true));
        assert_eq!(a.label.as_deref(), Some("Accept terms"));
    }
}
