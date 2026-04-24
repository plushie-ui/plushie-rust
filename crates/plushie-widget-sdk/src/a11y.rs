//! Accessibility override widget.
//!
//! Wraps a child widget to intercept [`operate`] calls and apply
//! host-side accessibility overrides (role, label, description, etc.)
//! to the accessibility tree. When `hidden` is set, the widget and all
//! its descendants are removed from the accessibility tree while
//! remaining visible and interactive for sighted users.
//!
//! [`operate`]: iced::advanced::widget::Widget::operate

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;

use iced::advanced::Shell;
use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::operation::accessible::{self, Accessible};
use iced::advanced::widget::{self, Widget};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

use plushie_core::types::PlushieType;
use plushie_core::types::{A11y, Role};

// ---------------------------------------------------------------------------
// A11yOverrides: parsed from the `a11y` JSON prop
// ---------------------------------------------------------------------------

/// Accessibility overrides parsed from the `a11y` JSON prop.
///
/// Wraps a [`plushie_core::types::A11y`] internally and converts
/// to iced accessible types on demand. When present on a node, the
/// renderer wraps the child widget in an internal a11y override layer
/// that intercepts iced's `operate` call to apply these overrides to
/// the accessibility tree.
///
/// Widget authors return this from [`PlushieWidget::infer_a11y`] to
/// provide automatic accessibility annotations (e.g., using a
/// placeholder prop as the accessible description).
///
/// [`PlushieWidget::infer_a11y`]: crate::registry::PlushieWidget::infer_a11y
#[derive(Debug, Clone, Default)]
pub struct A11yOverrides {
    core: A11y,
    // Iced widget IDs derived from the core string IDs. Stored here
    // so we can hand out references in `apply_to` / `to_accessible`.
    labelled_by: Option<widget::Id>,
    described_by: Option<widget::Id>,
    error_message: Option<widget::Id>,
    active_descendant: Option<widget::Id>,
    radio_group: Option<Vec<widget::Id>>,
}

impl A11yOverrides {
    /// Construct from a core `A11y` value.
    ///
    /// Converts string IDs to iced `widget::Id` values.
    pub fn from_core(core: &A11y) -> Self {
        let labelled_by = core
            .labelled_by
            .as_ref()
            .map(|s| widget::Id::from(s.clone()));
        let described_by = core
            .described_by
            .as_ref()
            .map(|s| widget::Id::from(s.clone()));
        let error_message = core
            .error_message
            .as_ref()
            .map(|s| widget::Id::from(s.clone()));
        let active_descendant = core
            .active_descendant
            .as_ref()
            .map(|s| widget::Id::from(s.clone()));
        let radio_group = core
            .radio_group
            .as_ref()
            .map(|ids| ids.iter().map(|s| widget::Id::from(s.clone())).collect());

        Self {
            core: core.clone(),
            labelled_by,
            described_by,
            error_message,
            active_descendant,
            radio_group,
        }
    }

    /// Parse accessibility overrides from a node's props.
    ///
    /// Returns `None` if no `a11y` key exists or if the `a11y` object
    /// contains no meaningful overrides.
    ///
    /// Explicit `false` values keep the wrapper for fields where false
    /// can clear inferred or widget-provided state. `hidden: false` is
    /// the exception because hidden only controls subtree suppression,
    /// and false means "do not suppress".
    pub fn from_props(props: &plushie_core::protocol::Props) -> Option<Self> {
        let core = A11y::extract(props, "a11y")?;
        let result = Self::from_core(&core);

        // Only wrap when there's something to do.
        if result.core.hidden.unwrap_or(false) || result.has_overrides() {
            Some(result)
        } else {
            None
        }
    }

    /// Returns true if any override would affect the accessible node.
    ///
    /// Excludes `hidden` which is handled separately (subtree
    /// suppression rather than property override).
    ///
    /// State fields count as overrides for any explicit value because
    /// `Some(false)` can clear inferred or widget-provided true values.
    /// `hidden` is handled separately and only `hidden: true`
    /// suppresses a subtree.
    pub(crate) fn has_overrides(&self) -> bool {
        let c = &self.core;
        c.role.is_some()
            || c.label.is_some()
            || c.description.is_some()
            || c.expanded.is_some()
            || c.live.is_some()
            || c.level.is_some()
            || c.mnemonic.is_some()
            || c.required.is_some()
            || c.busy.is_some()
            || c.invalid.is_some()
            || c.modal.is_some()
            || c.read_only.is_some()
            || c.toggled.is_some()
            || c.selected.is_some()
            || c.value.is_some()
            || c.orientation.is_some()
            || self.labelled_by.is_some()
            || self.described_by.is_some()
            || self.error_message.is_some()
            || c.disabled.is_some()
            || c.position_in_set.is_some()
            || c.size_of_set.is_some()
            || c.has_popup.is_some()
            || self.active_descendant.is_some()
            || self.radio_group.is_some()
    }

    /// Merge these overrides into a base [`Accessible`], returning a
    /// new struct with override values taking precedence.
    ///
    /// - `Option` fields: override wins if `Some`, falls back to base.
    /// - `bool` state fields: `Some(v)` overrides the widget's
    ///   auto-detected state and `None` falls back to the base value.
    /// - `hidden`: handled at the interception layer (subtree
    ///   suppression) rather than as a property merge. See the
    ///   `operate()` and `traverse()` methods.
    fn apply_to<'a>(&'a self, base: &Accessible<'a>) -> Accessible<'a> {
        let c = &self.core;
        let role_iced = c.role.as_ref().map(iced_convert::a11y_role);
        let live_iced = c.live.as_ref().map(iced_convert::a11y_live);
        let orientation_iced = c.orientation.as_ref().map(iced_convert::a11y_orientation);
        let has_popup_iced = c.has_popup.as_ref().map(iced_convert::a11y_has_popup);
        let value_override = c.value.as_deref().map(accessible::Value::Text);

        Accessible {
            role: role_iced.unwrap_or(base.role),
            label: c.label.as_deref().or(base.label),
            description: c.description.as_deref().or(base.description),
            expanded: c.expanded.or(base.expanded),
            live: live_iced.or(base.live),
            level: c.level.or(base.level),
            required: c.required.unwrap_or(base.required),
            busy: c.busy.unwrap_or(base.busy),
            invalid: c.invalid.unwrap_or(base.invalid),
            modal: c.modal.unwrap_or(base.modal),
            read_only: c.read_only.unwrap_or(base.read_only),
            mnemonic: c.mnemonic.or(base.mnemonic),
            toggled: c.toggled.or(base.toggled),
            selected: c.selected.or(base.selected),
            value: value_override.or(base.value),
            orientation: orientation_iced.or(base.orientation),
            labelled_by: self.labelled_by.as_ref().or(base.labelled_by),
            described_by: self.described_by.as_ref().or(base.described_by),
            error_message: self.error_message.as_ref().or(base.error_message),
            disabled: c.disabled.unwrap_or(base.disabled),
            position_in_set: c.position_in_set.or(base.position_in_set),
            size_of_set: c.size_of_set.or(base.size_of_set),
            has_popup: has_popup_iced.or(base.has_popup),
            active_descendant: self.active_descendant.as_ref().or(base.active_descendant),
            radio_group: self.radio_group.as_deref().or(base.radio_group),
            ..base.clone()
        }
    }

    /// Build an [`Accessible`] from overrides alone, using defaults
    /// for all widget-internal fields.
    ///
    /// Used when upgrading a container (which normally has no accessible
    /// node) to an accessible node because the host set a11y overrides.
    pub(crate) fn to_accessible(&self) -> Accessible<'_> {
        self.apply_to(&Accessible::default())
    }

    /// Create overrides with just a description (for placeholder auto-inference).
    pub(crate) fn with_description(description: String) -> Self {
        Self::from_core(&A11y::with_description(description))
    }

    /// Create overrides with just a mnemonic.
    pub(crate) fn with_mnemonic(mnemonic: char) -> Self {
        Self::from_core(&A11y::new().mnemonic(mnemonic))
    }

    /// Create mnemonic overrides from top-level `mnemonic` or `access_key`.
    pub(crate) fn from_mnemonic_props(props: &plushie_core::protocol::Props) -> Option<Self> {
        props
            .get_str("mnemonic")
            .or_else(|| props.get_str("access_key"))
            .and_then(|s| s.chars().next())
            .map(Self::with_mnemonic)
    }

    /// Create overrides with a specific role set.
    pub(crate) fn with_role(role: Role) -> Self {
        Self::from_core(&A11y::new().role(role))
    }

    /// Access the underlying core [`A11y`].
    pub fn core(&self) -> &A11y {
        &self.core
    }

    /// Merge two [`A11yOverrides`]. Fields set on `explicit` win; fields
    /// only set on `inferred` are preserved. Delegates to
    /// [`A11y::merge`] for per-field semantics.
    pub fn merge(inferred: &Self, explicit: &Self) -> Self {
        Self::from_core(&A11y::merge(&inferred.core, &explicit.core))
    }

    /// Whether the widget is hidden from the accessibility tree.
    pub(crate) fn hidden(&self) -> bool {
        self.core.hidden.unwrap_or(false)
    }

    /// The semantic role, converted to iced's accessible type.
    pub(crate) fn role(&self) -> Option<accessible::Role> {
        self.core.role.as_ref().map(iced_convert::a11y_role)
    }

    /// The label string.
    #[cfg(test)]
    pub(crate) fn label(&self) -> Option<&str> {
        self.core.label.as_deref()
    }

    /// The description string.
    #[cfg(test)]
    pub(crate) fn description(&self) -> Option<&str> {
        self.core.description.as_deref()
    }

    /// The toggled state.
    pub(crate) fn toggled(&self) -> Option<bool> {
        self.core.toggled
    }

    /// The selected state.
    pub(crate) fn selected(&self) -> Option<bool> {
        self.core.selected
    }

    /// Position in a set (1-based).
    pub(crate) fn position_in_set(&self) -> Option<usize> {
        self.core.position_in_set
    }
}

// ---------------------------------------------------------------------------
// Convenience: parse a role string via the core type
// ---------------------------------------------------------------------------

/// Parse a role string into the corresponding [`accessible::Role`] enum value.
///
/// Used by the canvas `role` prop where the value arrives as a raw string
/// rather than a structured `a11y` object.
pub(crate) fn parse_role_str(s: &str) -> Option<accessible::Role> {
    let val = serde_json::Value::String(s.to_owned());
    Role::wire_decode(&val)
        .as_ref()
        .map(iced_convert::a11y_role)
}

// ---------------------------------------------------------------------------
// A11yOverride: transparent single-child wrapper widget
// ---------------------------------------------------------------------------

/// A widget that wraps a single child and intercepts [`operate`] to
/// apply accessibility overrides from the host-side `a11y` prop.
///
/// All methods except [`operate`] delegate directly to the child.
///
/// [`operate`]: Widget::operate
pub(crate) struct A11yOverride<'a, R: PlushieRenderer = iced::Renderer> {
    child: Element<'a, Message, iced::Theme, R>,
    overrides: A11yOverrides,
}

impl<'a, R: PlushieRenderer> A11yOverride<'a, R> {
    /// Wrap `child` with the given accessibility overrides.
    pub(crate) fn wrap(
        child: Element<'a, Message, iced::Theme, R>,
        overrides: A11yOverrides,
    ) -> Self {
        Self { child, overrides }
    }
}

impl<R: PlushieRenderer> Widget<Message, iced::Theme, R> for A11yOverride<'_, R> {
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(&[self.child.as_widget()]);
    }

    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.child.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &R,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.child
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut R,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.child.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: iced::mouse::Cursor,
        renderer: &R,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.child.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &Rectangle,
        renderer: &R,
    ) -> iced::mouse::Interaction {
        self.child.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &R,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, R>> {
        self.child.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &R,
        operation: &mut dyn widget::Operation,
    ) {
        let mut interceptor = A11yInterceptor {
            inner: operation,
            overrides: &self.overrides,
        };
        self.child.as_widget_mut().operate(
            &mut tree.children[0],
            layout,
            renderer,
            &mut interceptor,
        );
    }
}

impl<'a, R: PlushieRenderer> From<A11yOverride<'a, R>> for Element<'a, Message, iced::Theme, R> {
    fn from(wrapper: A11yOverride<'a, R>) -> Self {
        Element::new(wrapper)
    }
}

// ---------------------------------------------------------------------------
// A11yInterceptor: intercepts accessible/container calls
// ---------------------------------------------------------------------------

/// An [`Operation`] wrapper that intercepts accessibility-related calls
/// to apply overrides or suppress them entirely (when hidden).
///
/// - `accessible()`: merges host overrides with widget-declared values.
/// - `container()`: upgrades to an accessible node when overrides are set.
/// - When `hidden`: drops accessible/container/text calls for the entire
///   subtree while forwarding non-a11y operations normally.
struct A11yInterceptor<'a, 'b> {
    inner: &'a mut dyn widget::Operation,
    overrides: &'b A11yOverrides,
}

/// Forwards non-intercepted [`Operation`] methods to `self.inner`.
/// Centralises the delegation so it only needs updating in one place
/// if iced adds new methods to the trait.
macro_rules! forward_operation {
    () => {
        fn focusable(
            &mut self,
            id: Option<&widget::Id>,
            bounds: Rectangle,
            state: &mut dyn widget::operation::focusable::Focusable,
        ) {
            self.inner.focusable(id, bounds, state);
        }

        fn scrollable(
            &mut self,
            id: Option<&widget::Id>,
            bounds: Rectangle,
            content_bounds: Rectangle,
            translation: Vector,
            state: &mut dyn widget::operation::scrollable::Scrollable,
        ) {
            self.inner
                .scrollable(id, bounds, content_bounds, translation, state);
        }

        fn text_input(
            &mut self,
            id: Option<&widget::Id>,
            bounds: Rectangle,
            state: &mut dyn widget::operation::text_input::TextInput,
        ) {
            self.inner.text_input(id, bounds, state);
        }

        fn custom(
            &mut self,
            id: Option<&widget::Id>,
            bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            self.inner.custom(id, bounds, state);
        }

        fn finish(&self) -> widget::operation::Outcome<()> {
            self.inner.finish()
        }
    };
}

impl widget::Operation for A11yInterceptor<'_, '_> {
    fn accessible(
        &mut self,
        id: Option<&widget::Id>,
        bounds: Rectangle,
        accessible: &Accessible<'_>,
    ) {
        if self.overrides.hidden() {
            return; // Hidden from AT.
        }
        let overridden = self.overrides.apply_to(accessible);
        self.inner.accessible(id, bounds, &overridden);
    }

    fn container(&mut self, id: Option<&widget::Id>, bounds: Rectangle) {
        if self.overrides.hidden() {
            return; // Hidden from AT.
        }
        if self.overrides.has_overrides() {
            // Upgrade container to accessible node so overrides take
            // effect. Without this, container-type widgets (column, row,
            // etc.) would silently ignore a11y overrides because they
            // only call container(), never accessible().
            let node = self.overrides.to_accessible();
            self.inner.accessible(id, bounds, &node);
        } else {
            self.inner.container(id, bounds);
        }
    }

    fn text(&mut self, id: Option<&widget::Id>, bounds: Rectangle, text: &str) {
        if self.overrides.hidden() {
            return; // Hidden from AT.
        }
        self.inner.text(id, bounds, text);
    }

    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn widget::Operation)) {
        if self.overrides.hidden() {
            // Propagate suppression through the entire subtree.
            self.inner.traverse(&mut |inner_op| {
                let mut nested = A11yInterceptor {
                    inner: inner_op,
                    overrides: self.overrides,
                };
                operate(&mut nested);
            });
        } else {
            // Overrides apply only to the direct child; grandchildren
            // pass through to the inner operation unmodified.
            self.inner.traverse(operate);
        }
    }

    forward_operation!();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::widget::Operation;
    use plushie_core::types::{HasPopup, Live, Orientation, Role};
    use serde_json::json;

    fn wire(v: serde_json::Value) -> plushie_core::protocol::Props {
        plushie_core::protocol::Props::from_json(v)
    }

    // -- from_props -----------------------------------------------------------

    #[test]
    fn from_props_none_when_no_a11y() {
        let props = wire(json!({"label": "Click me"}));
        assert!(A11yOverrides::from_props(&props).is_none());
    }

    #[test]
    fn from_props_none_when_empty_a11y() {
        let props = plushie_core::protocol::Props::from_json(json!({"a11y": {}}));
        assert!(A11yOverrides::from_props(&props).is_none());
    }

    #[test]
    fn from_props_none_when_hidden_false_only() {
        let props = plushie_core::protocol::Props::from_json(json!({"a11y": {"hidden": false}}));
        assert!(A11yOverrides::from_props(&props).is_none());
    }

    #[test]
    fn from_props_preserves_false_state_flags() {
        let props = plushie_core::protocol::Props::from_json(json!({
            "a11y": {
                "required": false,
                "invalid": false,
                "modal": false,
                "read_only": false
            }
        }));
        let overrides = A11yOverrides::from_props(&props).unwrap();
        assert_eq!(overrides.core.required, Some(false));
        assert_eq!(overrides.core.invalid, Some(false));
        assert_eq!(overrides.core.modal, Some(false));
        assert_eq!(overrides.core.read_only, Some(false));
    }

    #[test]
    fn from_props_parses_label() {
        let overrides =
            A11yOverrides::from_props(&wire(json!({"a11y": {"label": "Close"}}))).unwrap();
        assert_eq!(overrides.label(), Some("Close"));
    }

    #[test]
    fn from_props_parses_role() {
        let overrides =
            A11yOverrides::from_props(&wire(json!({"a11y": {"role": "heading"}}))).unwrap();
        assert_eq!(overrides.role(), Some(accessible::Role::Heading));
    }

    #[test]
    fn from_props_parses_hidden() {
        let overrides =
            A11yOverrides::from_props(&wire(json!({"a11y": {"hidden": true}}))).unwrap();
        assert!(overrides.hidden());
    }

    #[test]
    fn from_props_parses_all_fields() {
        let props = plushie_core::protocol::Props::from_json(json!({
            "a11y": {
                "role": "alert",
                "label": "Error message",
                "description": "Something went wrong",
                "hidden": false,
                "expanded": true,
                "required": true,
                "level": 2,
                "live": "assertive",
                "busy": true,
                "invalid": true,
                "modal": true,
                "read_only": true,
                "mnemonic": "E",
                "toggled": true,
                "selected": false,
                "value": "42%",
                "orientation": "vertical",
                "labelled_by": "label-id",
                "described_by": "desc-id",
                "error_message": "err-id",
                "disabled": true,
                "position_in_set": 3,
                "size_of_set": 10,
                "has_popup": "menu"
            }
        }));
        let o = A11yOverrides::from_props(&props).unwrap();
        assert_eq!(o.role(), Some(accessible::Role::Alert));
        assert_eq!(o.label(), Some("Error message"));
        assert_eq!(o.description(), Some("Something went wrong"));
        assert!(!o.hidden());
        assert_eq!(o.core.expanded, Some(true));
        assert_eq!(o.core.required, Some(true));
        assert_eq!(o.core.level, Some(2));
        assert_eq!(o.core.live, Some(Live::Assertive));
        assert_eq!(o.core.busy, Some(true));
        assert_eq!(o.core.invalid, Some(true));
        assert_eq!(o.core.modal, Some(true));
        assert_eq!(o.core.read_only, Some(true));
        assert_eq!(o.core.mnemonic, Some('E'));
        assert_eq!(o.toggled(), Some(true));
        assert_eq!(o.selected(), Some(false));
        assert_eq!(o.core.value.as_deref(), Some("42%"));
        assert_eq!(o.core.orientation, Some(Orientation::Vertical));
        assert!(o.labelled_by.is_some());
        assert!(o.described_by.is_some());
        assert!(o.error_message.is_some());
        assert_eq!(o.core.disabled, Some(true));
        assert_eq!(o.position_in_set(), Some(3));
        assert_eq!(o.core.size_of_set, Some(10));
        assert_eq!(o.core.has_popup, Some(HasPopup::Menu));
    }

    #[test]
    fn from_core_converts_correctly() {
        let core = A11y::new().role(Role::Button).label("Save").disabled(true);
        let result = A11yOverrides::from_core(&core);
        assert_eq!(result.role(), Some(accessible::Role::Button));
        assert_eq!(result.label(), Some("Save"));
        assert_eq!(result.core.disabled, Some(true));
    }

    // -- parse_role_str -------------------------------------------------------

    #[test]
    fn parse_role_str_covers_all_variants() {
        let cases = [
            ("alert", accessible::Role::Alert),
            ("alert_dialog", accessible::Role::AlertDialog),
            ("button", accessible::Role::Button),
            ("canvas", accessible::Role::Canvas),
            ("check_box", accessible::Role::CheckBox),
            ("combo_box", accessible::Role::ComboBox),
            ("dialog", accessible::Role::Dialog),
            ("document", accessible::Role::Document),
            ("generic_container", accessible::Role::GenericContainer),
            ("group", accessible::Role::Group),
            ("heading", accessible::Role::Heading),
            ("image", accessible::Role::Image),
            ("label", accessible::Role::Label),
            ("link", accessible::Role::Link),
            ("list", accessible::Role::List),
            ("list_item", accessible::Role::ListItem),
            ("menu", accessible::Role::Menu),
            ("menu_bar", accessible::Role::MenuBar),
            ("menu_item", accessible::Role::MenuItem),
            ("meter", accessible::Role::Meter),
            ("multiline_text_input", accessible::Role::MultilineTextInput),
            ("navigation", accessible::Role::Navigation),
            ("progress_indicator", accessible::Role::ProgressIndicator),
            ("radio_button", accessible::Role::RadioButton),
            ("radio_group", accessible::Role::RadioGroup),
            ("region", accessible::Role::Region),
            ("scroll_bar", accessible::Role::ScrollBar),
            ("scroll_view", accessible::Role::ScrollView),
            ("search", accessible::Role::Search),
            ("separator", accessible::Role::Separator),
            ("slider", accessible::Role::Slider),
            ("static_text", accessible::Role::StaticText),
            ("status", accessible::Role::Status),
            ("switch", accessible::Role::Switch),
            ("tab", accessible::Role::Tab),
            ("tab_list", accessible::Role::TabList),
            ("tab_panel", accessible::Role::TabPanel),
            ("table", accessible::Role::Table),
            ("table_row", accessible::Role::Row),
            ("table_cell", accessible::Role::Cell),
            ("column_header", accessible::Role::ColumnHeader),
            ("text_input", accessible::Role::TextInput),
            ("toolbar", accessible::Role::Toolbar),
            ("tooltip", accessible::Role::Tooltip),
            ("tree", accessible::Role::Tree),
            ("tree_item", accessible::Role::TreeItem),
            ("window", accessible::Role::Window),
        ];
        for (input, expected) in cases {
            assert_eq!(
                parse_role_str(input),
                Some(expected),
                "parse_role_str({input:?})"
            );
        }
        // Semantic aliases.
        assert_eq!(parse_role_str("radio"), Some(accessible::Role::RadioButton));
        assert_eq!(
            parse_role_str("text_editor"),
            Some(accessible::Role::MultilineTextInput)
        );
        assert_eq!(
            parse_role_str("progress_bar"),
            Some(accessible::Role::ProgressIndicator)
        );
        assert_eq!(parse_role_str("row"), Some(accessible::Role::Row));
        assert_eq!(parse_role_str("cell"), Some(accessible::Role::Cell));
        assert_eq!(
            parse_role_str("container"),
            Some(accessible::Role::GenericContainer)
        );
        assert_eq!(
            parse_role_str("generic"),
            Some(accessible::Role::GenericContainer)
        );
        // Concatenated forms are NOT accepted.
        assert_eq!(parse_role_str("alertdialog"), None);
        assert_eq!(parse_role_str("combobox"), None);
        assert_eq!(parse_role_str("listitem"), None);
        assert_eq!(parse_role_str("menubar"), None);
        assert_eq!(parse_role_str("scrollbar"), None);
        assert_eq!(parse_role_str("columnheader"), None);
        assert_eq!(parse_role_str("unknown_thing"), None);
    }

    // -- level validation -----------------------------------------------------

    #[test]
    fn level_rejects_out_of_range() {
        for n in [0, 7, 100] {
            let props = plushie_core::protocol::Props::from_json(json!({"a11y": {"level": n}}));
            assert!(A11yOverrides::from_props(&props).is_none());
        }
    }

    #[test]
    fn level_accepts_1_through_6() {
        for n in 1..=6 {
            let props = plushie_core::protocol::Props::from_json(json!({"a11y": {"level": n}}));
            let o = A11yOverrides::from_props(&props).unwrap();
            assert_eq!(o.core.level, Some(n as usize));
        }
    }

    // -- mnemonic edge cases --------------------------------------------------

    #[test]
    fn mnemonic_takes_first_char() {
        let o = A11yOverrides::from_props(&wire(json!({"a11y": {"mnemonic": "Save"}}))).unwrap();
        assert_eq!(o.core.mnemonic, Some('S'));
    }

    #[test]
    fn mnemonic_none_when_empty_string() {
        let props = plushie_core::protocol::Props::from_json(json!({"a11y": {"mnemonic": ""}}));
        assert!(A11yOverrides::from_props(&props).is_none());
    }

    // -- has_overrides --------------------------------------------------------

    #[test]
    fn has_overrides_false_when_default() {
        assert!(!A11yOverrides::default().has_overrides());
    }

    #[test]
    fn has_overrides_true_for_false_state_flags() {
        let overrides = A11yOverrides::from_core(
            &A11y::new()
                .required(false)
                .invalid(false)
                .modal(false)
                .read_only(false),
        );
        assert!(overrides.has_overrides());
    }

    #[test]
    fn has_overrides_true_for_each_field() {
        let cases: Vec<A11yOverrides> = vec![
            A11yOverrides::from_core(&A11y::new().role(Role::Button)),
            A11yOverrides::from_core(&A11y::new().label("x")),
            A11yOverrides::from_core(&A11y::new().required(true)),
            A11yOverrides::from_core(&A11y::new().toggled(true)),
            A11yOverrides::from_core(&A11y::new().orientation(Orientation::Horizontal)),
            A11yOverrides::from_core(&A11y::new().labelled_by("x")),
        ];
        for (i, o) in cases.iter().enumerate() {
            assert!(o.has_overrides(), "case {i} should have overrides");
        }
    }

    // -- apply_to -------------------------------------------------------------

    #[test]
    fn apply_to_overrides_win() {
        let overrides =
            A11yOverrides::from_core(&A11y::new().label("Override").role(Role::Navigation));
        let base = Accessible {
            role: accessible::Role::Group,
            label: Some("Original"),
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert_eq!(merged.role, accessible::Role::Navigation);
        assert_eq!(merged.label, Some("Override"));
    }

    #[test]
    fn apply_to_falls_back_to_base() {
        let overrides = A11yOverrides::default();
        let base = Accessible {
            role: accessible::Role::Button,
            label: Some("Click"),
            disabled: true,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert_eq!(merged.role, accessible::Role::Button);
        assert_eq!(merged.label, Some("Click"));
        assert!(merged.disabled);
    }

    #[test]
    fn apply_to_sets_true_state_flags() {
        let overrides = A11yOverrides::from_core(&A11y::new().required(true));
        let base = Accessible {
            busy: true,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert!(merged.required);
        assert!(merged.busy);
    }

    #[test]
    fn apply_to_default_override_preserves_base_bools() {
        // Unset overrides must not clear true base values.
        let overrides = A11yOverrides::default();
        let base = Accessible {
            required: true,
            invalid: true,
            modal: true,
            read_only: true,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert!(
            merged.required,
            "default override must not clear base required"
        );
        assert!(
            merged.invalid,
            "default override must not clear base invalid"
        );
        assert!(merged.modal, "default override must not clear base modal");
        assert!(
            merged.read_only,
            "default override must not clear base read_only"
        );
    }

    #[test]
    fn apply_to_both_true_stays_true() {
        let overrides = A11yOverrides::from_core(
            &A11y::new()
                .required(true)
                .invalid(true)
                .modal(true)
                .read_only(true),
        );
        let base = Accessible {
            required: true,
            invalid: true,
            modal: true,
            read_only: true,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert!(merged.required);
        assert!(merged.invalid);
        assert!(merged.modal);
        assert!(merged.read_only);
    }

    #[test]
    fn apply_to_false_state_flags_clear_base() {
        let overrides = A11yOverrides::from_core(
            &A11y::new()
                .required(false)
                .invalid(false)
                .modal(false)
                .read_only(false),
        );
        let base = Accessible {
            required: true,
            invalid: true,
            modal: true,
            read_only: true,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert!(!merged.required);
        assert!(!merged.invalid);
        assert!(!merged.modal);
        assert!(!merged.read_only);
    }

    #[test]
    fn merge_explicit_false_state_flags_clear_inferred_true() {
        let inferred = A11yOverrides::from_core(
            &A11y::new()
                .required(true)
                .invalid(true)
                .modal(true)
                .read_only(true),
        );
        let explicit = A11yOverrides::from_props(&wire(json!({
            "a11y": {
                "required": false,
                "invalid": false,
                "modal": false,
                "read_only": false
            }
        })))
        .unwrap();
        let merged = A11yOverrides::merge(&inferred, &explicit);

        assert_eq!(merged.core.required, Some(false));
        assert_eq!(merged.core.invalid, Some(false));
        assert_eq!(merged.core.modal, Some(false));
        assert_eq!(merged.core.read_only, Some(false));
    }

    #[test]
    fn busy_override_wins_over_base() {
        let overrides = A11yOverrides::from_core(&A11y::new().busy(false));
        let base = Accessible {
            busy: true,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert!(!merged.busy);
    }

    #[test]
    fn busy_none_uses_base() {
        let overrides = A11yOverrides::default();
        let base = Accessible {
            busy: true,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert!(merged.busy);
    }

    #[test]
    fn to_accessible_uses_defaults_for_base() {
        let overrides =
            A11yOverrides::from_core(&A11y::new().role(Role::Navigation).label("Main nav"));
        let node = overrides.to_accessible();
        assert_eq!(node.role, accessible::Role::Navigation);
        assert_eq!(node.label, Some("Main nav"));
        assert!(!node.disabled);
    }

    // -- with_description -----------------------------------------------------

    #[test]
    fn with_description_sets_only_description() {
        let overrides = A11yOverrides::with_description("Placeholder hint".to_string());
        assert_eq!(overrides.description(), Some("Placeholder hint"));
        assert!(overrides.label().is_none());
        assert!(overrides.role().is_none());
        assert!(!overrides.hidden());
    }

    // -- disabled, position_in_set, size_of_set, has_popup --------------------

    #[test]
    fn from_props_parses_disabled() {
        let o = A11yOverrides::from_props(&wire(json!({"a11y": {"disabled": true}}))).unwrap();
        assert_eq!(o.core.disabled, Some(true));
    }

    #[test]
    fn from_props_parses_disabled_false() {
        let o = A11yOverrides::from_props(&wire(json!({"a11y": {"disabled": false}}))).unwrap();
        assert_eq!(o.core.disabled, Some(false));
    }

    #[test]
    fn from_props_parses_position_in_set() {
        let o = A11yOverrides::from_props(&wire(json!({"a11y": {"position_in_set": 3}}))).unwrap();
        assert_eq!(o.position_in_set(), Some(3));
    }

    #[test]
    fn from_props_parses_size_of_set() {
        let o = A11yOverrides::from_props(&wire(json!({"a11y": {"size_of_set": 10}}))).unwrap();
        assert_eq!(o.core.size_of_set, Some(10));
    }

    #[test]
    fn from_props_parses_has_popup() {
        let cases = [
            ("listbox", HasPopup::Listbox),
            ("menu", HasPopup::Menu),
            ("dialog", HasPopup::Dialog),
            ("tree", HasPopup::Tree),
            ("grid", HasPopup::Grid),
        ];
        for (input, expected) in cases {
            let o =
                A11yOverrides::from_props(&wire(json!({"a11y": {"has_popup": input}}))).unwrap();
            assert_eq!(o.core.has_popup, Some(expected), "has_popup({input:?})");
        }
    }

    #[test]
    fn has_overrides_true_for_new_fields() {
        let cases: Vec<A11yOverrides> = vec![
            A11yOverrides::from_core(&A11y::new().disabled(true)),
            A11yOverrides::from_core(&A11y::new().position_in_set(1)),
            A11yOverrides::from_core(&A11y::new().size_of_set(5)),
            A11yOverrides::from_core(&A11y::new().has_popup(HasPopup::Dialog)),
        ];
        for (i, o) in cases.iter().enumerate() {
            assert!(
                o.has_overrides(),
                "new field case {i} should have overrides"
            );
        }
    }

    #[test]
    fn apply_to_disabled_override_replaces_base() {
        let overrides = A11yOverrides::from_core(&A11y::new().disabled(true));
        let base = Accessible {
            disabled: false,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert!(merged.disabled);
    }

    #[test]
    fn apply_to_disabled_none_preserves_base() {
        let overrides = A11yOverrides::default();
        let base = Accessible {
            disabled: true,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert!(merged.disabled);
    }

    #[test]
    fn apply_to_disabled_can_enable() {
        let overrides = A11yOverrides::from_core(&A11y::new().disabled(false));
        let base = Accessible {
            disabled: true,
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert!(!merged.disabled);
    }

    #[test]
    fn apply_to_position_in_set_override_wins() {
        let overrides = A11yOverrides::from_core(&A11y::new().position_in_set(5));
        let base = Accessible {
            position_in_set: Some(1),
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert_eq!(merged.position_in_set, Some(5));
    }

    #[test]
    fn apply_to_size_of_set_falls_back_to_base() {
        let overrides = A11yOverrides::default();
        let base = Accessible {
            size_of_set: Some(10),
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert_eq!(merged.size_of_set, Some(10));
    }

    #[test]
    fn apply_to_has_popup_override_wins() {
        let overrides = A11yOverrides::from_core(&A11y::new().has_popup(HasPopup::Grid));
        let base = Accessible {
            has_popup: Some(accessible::HasPopup::Listbox),
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert_eq!(merged.has_popup, Some(accessible::HasPopup::Grid));
    }

    #[test]
    fn apply_to_has_popup_falls_back_to_base() {
        let overrides = A11yOverrides::default();
        let base = Accessible {
            has_popup: Some(accessible::HasPopup::Menu),
            ..Default::default()
        };
        let merged = overrides.apply_to(&base);
        assert_eq!(merged.has_popup, Some(accessible::HasPopup::Menu));
    }

    // -- A11yInterceptor integration tests ------------------------------------

    struct RecordingOperation {
        accessible_calls: Vec<RecordedAccessible>,
        container_calls: Vec<bool>,
        text_calls: Vec<String>,
    }

    #[allow(dead_code)]
    struct RecordedAccessible {
        role: accessible::Role,
        label: Option<String>,
        disabled: bool,
        position_in_set: Option<usize>,
        size_of_set: Option<usize>,
        has_popup: Option<accessible::HasPopup>,
    }

    impl RecordingOperation {
        fn new() -> Self {
            Self {
                accessible_calls: Vec::new(),
                container_calls: Vec::new(),
                text_calls: Vec::new(),
            }
        }
    }

    impl widget::Operation for RecordingOperation {
        fn accessible(
            &mut self,
            _id: Option<&widget::Id>,
            _bounds: Rectangle,
            accessible: &Accessible<'_>,
        ) {
            self.accessible_calls.push(RecordedAccessible {
                role: accessible.role,
                label: accessible.label.map(String::from),
                disabled: accessible.disabled,
                position_in_set: accessible.position_in_set,
                size_of_set: accessible.size_of_set,
                has_popup: accessible.has_popup,
            });
        }

        fn container(&mut self, _id: Option<&widget::Id>, _bounds: Rectangle) {
            self.container_calls.push(true);
        }

        fn text(&mut self, _id: Option<&widget::Id>, _bounds: Rectangle, text: &str) {
            self.text_calls.push(text.to_owned());
        }

        fn focusable(
            &mut self,
            _id: Option<&widget::Id>,
            _bounds: Rectangle,
            _state: &mut dyn widget::operation::focusable::Focusable,
        ) {
        }

        fn scrollable(
            &mut self,
            _id: Option<&widget::Id>,
            _bounds: Rectangle,
            _content_bounds: Rectangle,
            _translation: Vector,
            _state: &mut dyn widget::operation::scrollable::Scrollable,
        ) {
        }

        fn text_input(
            &mut self,
            _id: Option<&widget::Id>,
            _bounds: Rectangle,
            _state: &mut dyn widget::operation::text_input::TextInput,
        ) {
        }

        fn custom(
            &mut self,
            _id: Option<&widget::Id>,
            _bounds: Rectangle,
            _state: &mut dyn std::any::Any,
        ) {
        }

        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
            operate(self);
        }

        fn finish(&self) -> widget::operation::Outcome<()> {
            widget::operation::Outcome::None
        }
    }

    #[test]
    fn interceptor_merges_overrides_with_base_accessible() {
        let overrides =
            A11yOverrides::from_core(&A11y::new().label("Override label").role(Role::Link));
        let base = Accessible {
            role: accessible::Role::Button,
            label: Some("Click me"),
            disabled: true,
            ..Default::default()
        };
        let mut recording = RecordingOperation::new();
        {
            let mut interceptor = A11yInterceptor {
                inner: &mut recording,
                overrides: &overrides,
            };
            interceptor.accessible(None, Rectangle::default(), &base);
        }
        assert_eq!(recording.accessible_calls.len(), 1);
        let call = &recording.accessible_calls[0];
        assert_eq!(call.role, accessible::Role::Link);
        assert_eq!(call.label.as_deref(), Some("Override label"));
        assert!(call.disabled);
    }

    #[test]
    fn interceptor_hidden_suppresses_accessible() {
        let overrides = A11yOverrides::from_core(&A11y::new().hidden(true));
        let base = Accessible {
            role: accessible::Role::Button,
            label: Some("Hidden button"),
            ..Default::default()
        };
        let mut recording = RecordingOperation::new();
        {
            let mut interceptor = A11yInterceptor {
                inner: &mut recording,
                overrides: &overrides,
            };
            interceptor.accessible(None, Rectangle::default(), &base);
        }
        assert!(recording.accessible_calls.is_empty());
    }

    #[test]
    fn interceptor_hidden_suppresses_text() {
        let overrides = A11yOverrides::from_core(&A11y::new().hidden(true));
        let mut recording = RecordingOperation::new();
        {
            let mut interceptor = A11yInterceptor {
                inner: &mut recording,
                overrides: &overrides,
            };
            interceptor.text(None, Rectangle::default(), "should not appear");
        }
        assert!(recording.text_calls.is_empty());
    }

    #[test]
    fn interceptor_container_upgrades_when_overrides_present() {
        let overrides = A11yOverrides::from_core(&A11y::new().role(Role::Group).label("Nav group"));
        let mut recording = RecordingOperation::new();
        {
            let mut interceptor = A11yInterceptor {
                inner: &mut recording,
                overrides: &overrides,
            };
            interceptor.container(None, Rectangle::default());
        }
        assert!(recording.container_calls.is_empty());
        assert_eq!(recording.accessible_calls.len(), 1);
        let call = &recording.accessible_calls[0];
        assert_eq!(call.role, accessible::Role::Group);
        assert_eq!(call.label.as_deref(), Some("Nav group"));
    }

    #[test]
    fn interceptor_container_passes_through_without_overrides() {
        let overrides = A11yOverrides::default();
        let mut recording = RecordingOperation::new();
        {
            let mut interceptor = A11yInterceptor {
                inner: &mut recording,
                overrides: &overrides,
            };
            interceptor.container(None, Rectangle::default());
        }
        assert_eq!(recording.container_calls.len(), 1);
        assert!(recording.accessible_calls.is_empty());
    }

    #[test]
    fn interceptor_hidden_suppresses_container() {
        let overrides = A11yOverrides::from_core(&A11y::new().hidden(true));
        let mut recording = RecordingOperation::new();
        {
            let mut interceptor = A11yInterceptor {
                inner: &mut recording,
                overrides: &overrides,
            };
            interceptor.container(None, Rectangle::default());
        }
        assert!(recording.container_calls.is_empty());
        assert!(recording.accessible_calls.is_empty());
    }

    #[test]
    fn interceptor_traverse_propagates_hidden_to_children() {
        let overrides = A11yOverrides::from_core(&A11y::new().hidden(true));
        let mut recording = RecordingOperation::new();
        {
            let mut interceptor = A11yInterceptor {
                inner: &mut recording,
                overrides: &overrides,
            };
            interceptor.traverse(&mut |child_op| {
                let base = Accessible {
                    role: accessible::Role::Button,
                    label: Some("Child button"),
                    ..Default::default()
                };
                child_op.accessible(None, Rectangle::default(), &base);
                child_op.text(None, Rectangle::default(), "child text");
            });
        }
        assert!(recording.accessible_calls.is_empty());
        assert!(recording.text_calls.is_empty());
    }
}
