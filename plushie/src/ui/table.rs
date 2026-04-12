//! Table widget builder.

use super::{PropMap, PropValue};

use crate::View;
use crate::types::*;

/// A table widget displaying columnar data with headers.
///
/// ```ignore
/// table("users")
///     .columns(["Name", "Email", "Role"])
///     .rows(users.iter().map(|u| vec![&u.name, &u.email, &u.role]))
///     .width(Fill)
/// ```
pub struct TableBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a table widget.
pub fn table(id: &str) -> TableBuilder {
    TableBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        children: vec![],
    }
}

impl TableBuilder {
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn columns(mut self, cols: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let cols: Vec<PropValue> = cols.into_iter()
            .map(|c| {
                let mut col = PropMap::new();
                col.insert("key", PropValue::Str(c.as_ref().to_string()));
                col.insert("label", PropValue::Str(c.as_ref().to_string()));
                PropValue::Object(col)
            })
            .collect();
        super::set_prop(&mut self.props, "columns", PropValue::Array(cols));
        self
    }

    /// Show the header row.
    pub fn header(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "header", v);
        self
    }

    /// Show a separator line below the header.
    pub fn separator(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "separator", v);
        self
    }

    /// Column key to sort by.
    pub fn sort_by(mut self, column: &str) -> Self {
        super::set_prop(&mut self.props, "sort_by", column);
        self
    }

    /// Sort direction.
    pub fn sort_order(mut self, order: SortOrder) -> Self {
        super::set_prop(&mut self.props, "sort_order", order.wire_encode());
        self
    }

    /// Header row text size in pixels.
    pub fn header_text_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "header_text_size", s.into().wire_encode());
        self
    }

    /// Body row text size in pixels.
    pub fn row_text_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "row_text_size", s.into().wire_encode());
        self
    }

    /// Horizontal spacing between cells in pixels.
    pub fn cell_spacing(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "cell_spacing", s.into().wire_encode());
        self
    }

    /// Vertical spacing between rows in pixels.
    pub fn row_spacing(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "row_spacing", s.into().wire_encode());
        self
    }

    /// Separator line thickness in pixels.
    pub fn separator_thickness(mut self, t: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "separator_thickness", t.into().wire_encode());
        self
    }

    /// Separator line color.
    pub fn separator_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "separator_color", c.into().wire_encode());
        self
    }

    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into()));
        self
    }

    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }

    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }

    /// Add child rows to the table.
    pub fn child(mut self, c: impl Into<View>) -> Self {
        self.children.push(c.into());
        self
    }

    pub fn children<I, V>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<View>,
    {
        self.children.extend(items.into_iter().map(Into::into));
        self
    }
}

impl From<TableBuilder> for View {
    fn from(b: TableBuilder) -> View {
        super::view_node(b.id, "table", b.props, b.children)
    }
}
