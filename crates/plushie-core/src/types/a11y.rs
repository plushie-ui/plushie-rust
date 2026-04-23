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
    /// Alert.
    Alert,
    /// Alert Dialog.
    AlertDialog,
    /// Button.
    Button,
    /// Canvas.
    Canvas,
    #[plushie(wire = "check_box")]
    /// Check Box.
    CheckBox,
    #[plushie(wire = "table_cell", aliases = ["cell"])]
    /// Cell.
    Cell,
    /// Column Header.
    ColumnHeader,
    /// Combo Box.
    ComboBox,
    /// Dialog.
    Dialog,
    /// Document.
    Document,
    #[plushie(aliases = ["container", "generic"])]
    /// Generic Container.
    GenericContainer,
    /// Group.
    Group,
    /// Heading.
    Heading,
    /// Image.
    Image,
    /// Label.
    Label,
    /// Link.
    Link,
    /// List.
    List,
    /// List Item.
    ListItem,
    /// Menu.
    Menu,
    /// Menu Bar.
    MenuBar,
    /// Menu Item.
    MenuItem,
    /// Meter.
    Meter,
    #[plushie(aliases = ["text_editor"])]
    /// Multiline Text Input.
    MultilineTextInput,
    /// Navigation.
    Navigation,
    #[plushie(aliases = ["progress_bar"])]
    /// Progress Indicator.
    ProgressIndicator,
    #[plushie(aliases = ["radio"])]
    /// Radio Button.
    RadioButton,
    /// Radio Group.
    RadioGroup,
    /// Region.
    Region,
    #[plushie(wire = "table_row", aliases = ["row"])]
    /// Row.
    Row,
    /// Scroll Bar.
    ScrollBar,
    /// Scroll View.
    ScrollView,
    /// Search.
    Search,
    /// Separator.
    Separator,
    /// Slider.
    Slider,
    /// Static Text.
    StaticText,
    /// Status.
    Status,
    /// Switch.
    Switch,
    /// Tab.
    Tab,
    /// Tab List.
    TabList,
    /// Tab Panel.
    TabPanel,
    /// Table.
    Table,
    /// Text Input.
    TextInput,
    /// Toolbar.
    Toolbar,
    /// Tooltip.
    Tooltip,
    /// Tree.
    Tree,
    /// Tree Item.
    TreeItem,
    /// Window.
    Window,
}

// ---------------------------------------------------------------------------
// Live enum
// ---------------------------------------------------------------------------

/// Live region urgency level.
///
/// Two variants only. Absence of a `live` value on an `A11y` struct
/// (the `None` case on `Option<Live>`) means "no live region": the
/// AccessKit node simply does not carry a live-region attribute.
/// There is no `Off` variant; demoting a region back to non-live is
/// done by clearing the field, not by setting a third value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PlushieEnum)]
#[plushie_type(name = "live")]
pub enum Live {
    /// Polite.
    Polite,
    /// Assertive.
    Assertive,
}

// ---------------------------------------------------------------------------
// Orientation enum
// ---------------------------------------------------------------------------

/// Widget orientation (horizontal or vertical).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PlushieEnum)]
#[plushie_type(name = "orientation")]
pub enum Orientation {
    /// Horizontal.
    Horizontal,
    /// Vertical.
    Vertical,
}

// ---------------------------------------------------------------------------
// HasPopup enum
// ---------------------------------------------------------------------------

/// Type of popup a widget triggers when activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PlushieEnum)]
#[plushie_type(name = "has_popup")]
pub enum HasPopup {
    /// Listbox.
    Listbox,
    /// Menu.
    Menu,
    /// Dialog.
    Dialog,
    /// Tree.
    Tree,
    /// Grid.
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
/// use plushie_core::types::{A11y, Role, Live};
///
/// let a11y = A11y::new()
///     .role(Role::Button)
///     .label("Save document")
///     .live(Live::Polite);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct A11y {
    /// Accessibility role.
    pub role: Option<Role>,
    /// Accessible or visible label.
    pub label: Option<String>,
    /// Extended description.
    pub description: Option<String>,
    /// Whether the widget is hidden from assistive technology.
    /// `None` means not specified (inherits from base/inference).
    pub hidden: Option<bool>,
    /// Whether the widget is expanded.
    pub expanded: Option<bool>,
    /// Whether input is required.
    pub required: Option<bool>,
    /// Heading level (1 through 6, corresponding to h1-h6).
    /// Values outside this range are rejected during wire decode.
    pub level: Option<usize>,
    /// Live-region politeness.
    pub live: Option<Live>,
    /// Busy.
    pub busy: Option<bool>,
    /// Invalid.
    pub invalid: Option<bool>,
    /// Whether the dialog is modal.
    pub modal: Option<bool>,
    /// Whether the field is read-only.
    pub read_only: Option<bool>,
    /// Mnemonic.
    pub mnemonic: Option<char>,
    /// Toggle state.
    pub toggled: Option<bool>,
    /// Selection state.
    pub selected: Option<bool>,
    /// Typed payload value.
    pub value: Option<String>,
    /// Layout orientation.
    pub orientation: Option<Orientation>,
    /// Disabled state.
    pub disabled: Option<bool>,
    /// 1-based index in a set of peers.
    pub position_in_set: Option<usize>,
    /// Total number of peers.
    pub size_of_set: Option<usize>,
    /// IDs of labelling widgets.
    pub labelled_by: Option<String>,
    /// IDs of describing widgets.
    pub described_by: Option<String>,
    /// Error message.
    pub error_message: Option<String>,
    /// Currently active descendant ID.
    pub active_descendant: Option<String>,
    /// Radio group.
    pub radio_group: Option<Vec<String>>,
    /// Popup kind, if any.
    pub has_popup: Option<HasPopup>,
    /// Elixir-only field: resolved at build time to populate `label`.
    /// Included for wire completeness; the renderer ignores it.
    pub label_from: Option<String>,
}

// ---------------------------------------------------------------------------
// Builder methods
// ---------------------------------------------------------------------------

impl A11y {
    /// Construct a new value.
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

    /// Set or construct `role`.
    pub fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    /// Set or construct `label`.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set or construct `description`.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set or construct `hidden`.
    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = Some(hidden);
        self
    }

    /// Set or construct `expanded`.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Set or construct `required`.
    pub fn required(mut self, required: bool) -> Self {
        self.required = Some(required);
        self
    }

    /// Set or construct `level`.
    pub fn level(mut self, level: usize) -> Self {
        self.level = Some(level);
        self
    }

    /// Set or construct `live`.
    pub fn live(mut self, live: Live) -> Self {
        self.live = Some(live);
        self
    }

    /// Set or construct `busy`.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = Some(busy);
        self
    }

    /// Set or construct `invalid`.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = Some(invalid);
        self
    }

    /// Set or construct `modal`.
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = Some(modal);
        self
    }

    /// Set or construct `read_only`.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = Some(read_only);
        self
    }

    /// Set or construct `mnemonic`.
    pub fn mnemonic(mut self, mnemonic: char) -> Self {
        self.mnemonic = Some(mnemonic);
        self
    }

    /// Set or construct `toggled`.
    pub fn toggled(mut self, toggled: bool) -> Self {
        self.toggled = Some(toggled);
        self
    }

    /// Set or construct `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Set or construct `value`.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Set or construct `orientation`.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Set or construct `disabled`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Set or construct `position_in_set`.
    pub fn position_in_set(mut self, pos: usize) -> Self {
        self.position_in_set = Some(pos);
        self
    }

    /// Set or construct `size_of_set`.
    pub fn size_of_set(mut self, size: usize) -> Self {
        self.size_of_set = Some(size);
        self
    }

    /// Set or construct `labelled_by`.
    pub fn labelled_by(mut self, id: impl Into<String>) -> Self {
        self.labelled_by = Some(id.into());
        self
    }

    /// Set or construct `described_by`.
    pub fn described_by(mut self, id: impl Into<String>) -> Self {
        self.described_by = Some(id.into());
        self
    }

    /// Set or construct `error_message`.
    pub fn error_message(mut self, id: impl Into<String>) -> Self {
        self.error_message = Some(id.into());
        self
    }

    /// Set or construct `active_descendant`.
    pub fn active_descendant(mut self, id: impl Into<String>) -> Self {
        self.active_descendant = Some(id.into());
        self
    }

    /// Set or construct `radio_group`.
    pub fn radio_group(mut self, ids: Vec<String>) -> Self {
        self.radio_group = Some(ids);
        self
    }

    /// Set or construct `has_popup`.
    pub fn has_popup(mut self, popup: HasPopup) -> Self {
        self.has_popup = Some(popup);
        self
    }

    /// Set or construct `label_from`.
    pub fn label_from(mut self, id: impl Into<String>) -> Self {
        self.label_from = Some(id.into());
        self
    }

    /// Merge two A11y values. Non-None fields in `overrides` take
    /// precedence; None fields fall back to `base`.
    ///
    /// This mirrors the Elixir SDK's `A11y.merge/2`: widget defaults
    /// are the base, user-provided a11y props are the overrides. The
    /// result preserves defaults for any field the user didn't specify.
    ///
    /// ```
    /// use plushie_core::types::{A11y, Role};
    ///
    /// let widget_default = A11y::new().role(Role::Slider).label("Volume");
    /// let user_override = A11y::new().label("Master Volume");
    /// let merged = A11y::merge(&widget_default, &user_override);
    /// // Keeps role from default, takes label from override.
    /// assert_eq!(merged.role, Some(Role::Slider));
    /// assert_eq!(merged.label.as_deref(), Some("Master Volume"));
    /// ```
    pub fn merge(base: &A11y, overrides: &A11y) -> A11y {
        A11y {
            role: overrides.role.or(base.role),
            label: overrides.label.clone().or_else(|| base.label.clone()),
            description: overrides
                .description
                .clone()
                .or_else(|| base.description.clone()),
            hidden: overrides.hidden.or(base.hidden),
            expanded: overrides.expanded.or(base.expanded),
            required: overrides.required.or(base.required),
            level: overrides.level.or(base.level),
            live: overrides.live.or(base.live),
            busy: overrides.busy.or(base.busy),
            invalid: overrides.invalid.or(base.invalid),
            modal: overrides.modal.or(base.modal),
            read_only: overrides.read_only.or(base.read_only),
            mnemonic: overrides.mnemonic.or(base.mnemonic),
            toggled: overrides.toggled.or(base.toggled),
            selected: overrides.selected.or(base.selected),
            value: overrides.value.clone().or_else(|| base.value.clone()),
            orientation: overrides.orientation.or(base.orientation),
            disabled: overrides.disabled.or(base.disabled),
            position_in_set: overrides.position_in_set.or(base.position_in_set),
            size_of_set: overrides.size_of_set.or(base.size_of_set),
            labelled_by: overrides
                .labelled_by
                .clone()
                .or_else(|| base.labelled_by.clone()),
            described_by: overrides
                .described_by
                .clone()
                .or_else(|| base.described_by.clone()),
            error_message: overrides
                .error_message
                .clone()
                .or_else(|| base.error_message.clone()),
            active_descendant: overrides
                .active_descendant
                .clone()
                .or_else(|| base.active_descendant.clone()),
            radio_group: overrides
                .radio_group
                .clone()
                .or_else(|| base.radio_group.clone()),
            has_popup: overrides.has_popup.or(base.has_popup),
            label_from: overrides
                .label_from
                .clone()
                .or_else(|| base.label_from.clone()),
        }
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
        let hidden = obj.get("hidden").and_then(|v| v.as_bool());
        let expanded = obj.get("expanded").and_then(|v| v.as_bool());
        let required = obj.get("required").and_then(|v| v.as_bool());

        let level = obj.get("level").and_then(|v| v.as_u64()).and_then(|n| {
            let n = n as usize;
            if (1..=6).contains(&n) { Some(n) } else { None }
        });

        let live = obj.get("live").and_then(Live::wire_decode);
        let busy = obj.get("busy").and_then(|v| v.as_bool());
        let invalid = obj.get("invalid").and_then(|v| v.as_bool());
        let modal = obj.get("modal").and_then(|v| v.as_bool());
        let read_only = obj.get("read_only").and_then(|v| v.as_bool());

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
        if let Some(hidden) = self.hidden {
            m.insert("hidden", PropValue::Bool(hidden));
        }
        if let Some(expanded) = self.expanded {
            m.insert("expanded", PropValue::Bool(expanded));
        }
        if let Some(required) = self.required {
            m.insert("required", PropValue::Bool(required));
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
        if let Some(invalid) = self.invalid {
            m.insert("invalid", PropValue::Bool(invalid));
        }
        if let Some(modal) = self.modal {
            m.insert("modal", PropValue::Bool(modal));
        }
        if let Some(read_only) = self.read_only {
            m.insert("read_only", PropValue::Bool(read_only));
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
        assert_eq!(a.hidden, Some(true));
        assert_eq!(a.expanded, Some(false));
        assert_eq!(a.required, Some(true));
        assert_eq!(a.level, Some(2));
        assert_eq!(a.live, Some(Live::Polite));
        assert_eq!(a.busy, Some(true));
        assert_eq!(a.invalid, Some(true));
        assert_eq!(a.modal, Some(true));
        assert_eq!(a.read_only, Some(true));
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
        assert_eq!(a.hidden, None);
        assert_eq!(a.required, None);
        assert_eq!(a.invalid, None);
        assert_eq!(a.modal, None);
        assert_eq!(a.read_only, None);
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
