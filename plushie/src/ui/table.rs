//! Table widget builder.
//!
//! Tables display structured data in rows and columns with optional
//! sorting and rich cell content.
//!
//! # Rich composition
//!
//! ```ignore
//! table("users")
//!     .column("name", |c| c.label("Name").sortable(true).width(Length::Fill))
//!     .column("email", |c| c.label("Email"))
//!     .column("actions", |c| c.label(""))
//!     .sort_by("name")
//!     .sort_order(SortOrder::Asc)
//!     .row("u1", |r| r
//!         .cell("name", text("Alice"))
//!         .cell("email", text("alice@example.com"))
//!         .cell("actions", button("del-1", "Delete")))
//! ```
//!
//! # Data shorthand
//!
//! ```ignore
//! table("users")
//!     .columns(&[("name", "Name"), ("email", "Email")])
//!     .data_row("u1", &[("name", "Alice"), ("email", "alice@example.com")])
//! ```

use super::{PropMap, PropValue};

use crate::View;
use crate::types::*;

// ---------------------------------------------------------------------------
// Column spec
// ---------------------------------------------------------------------------

/// Builder for a table column definition.
///
/// Created via [`TableBuilder::column`]. Columns define the structure
/// (key, label, width, alignment) and become part of the `columns` prop
/// on the wire.
pub struct TableColumnSpec {
    key: String,
    label: Option<String>,
    width: Option<Length>,
    min_width: Option<f32>,
    sortable: bool,
    align: Option<HorizontalAlignment>,
}

impl TableColumnSpec {
    fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            label: None,
            width: None,
            min_width: None,
            sortable: false,
            align: None,
        }
    }

    /// Header display text. Defaults to the column key.
    pub fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    /// Column width. Default: fill.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        self.width = Some(w.into());
        self
    }

    /// Minimum column width in pixels.
    pub fn min_width(mut self, px: f32) -> Self {
        self.min_width = Some(px);
        self
    }

    /// Make the header clickable to emit a `sort` event.
    pub fn sortable(mut self, v: bool) -> Self {
        self.sortable = v;
        self
    }

    /// Cell content alignment.
    pub fn align(mut self, a: HorizontalAlignment) -> Self {
        self.align = Some(a);
        self
    }

    fn to_prop_value(&self) -> PropValue {
        let mut m = PropMap::new();
        m.insert("key", PropValue::Str(self.key.clone()));
        m.insert(
            "label",
            PropValue::Str(self.label.as_deref().unwrap_or(&self.key).to_string()),
        );
        if let Some(ref w) = self.width {
            m.insert("width", w.wire_encode());
        }
        if let Some(mw) = self.min_width {
            m.insert("min_width", PropValue::F64(mw as f64));
        }
        if self.sortable {
            m.insert("sortable", PropValue::Bool(true));
        }
        if let Some(ref a) = self.align {
            m.insert("align", a.wire_encode());
        }
        PropValue::Object(m)
    }
}

// ---------------------------------------------------------------------------
// Row builder
// ---------------------------------------------------------------------------

/// Builder for a table row, created via [`TableBuilder::row`].
///
/// Each cell maps a column key to widget content. Cells become
/// `table_cell` children on the wire with a `column` prop.
pub struct TableRowBuilder {
    id: String,
    cells: Vec<(String, View)>,
}

impl TableRowBuilder {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            cells: Vec::new(),
        }
    }

    /// Add a cell for the given column key with widget content.
    pub fn cell(mut self, column_key: &str, content: impl Into<View>) -> Self {
        self.cells.push((column_key.to_string(), content.into()));
        self
    }

    fn into_view(self) -> View {
        let children: Vec<View> = self
            .cells
            .into_iter()
            .map(|(col_key, content)| {
                let mut cell_props = PropMap::new();
                cell_props.insert("column", PropValue::Str(col_key.clone()));
                super::view_node(col_key, "table_cell", cell_props, vec![content])
            })
            .collect();

        super::view_node(self.id, "table_row", PropMap::new(), children)
    }
}

// ---------------------------------------------------------------------------
// Table builder
// ---------------------------------------------------------------------------

/// Table widget builder.
///
/// Columns are metadata (stored as a prop). Rows are children
/// (`table_row` elements) for efficient LIS-based wire diffing.
pub struct TableBuilder {
    id: String,
    props: PropMap,
    column_specs: Vec<TableColumnSpec>,
    children: Vec<View>,
}

/// Create a table widget with an explicit ID.
///
/// The ID is used for event routing (sort) and scoped ID
/// resolution.
pub fn table(id: &str) -> TableBuilder {
    TableBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        column_specs: Vec::new(),
        children: vec![],
    }
}

impl TableBuilder {
    /// Define a column with a key and a configuration closure.
    ///
    /// ```ignore
    /// .column("name", |c| c.label("Name").sortable(true).width(Length::Fill))
    /// ```
    pub fn column(mut self, key: &str, f: impl FnOnce(TableColumnSpec) -> TableColumnSpec) -> Self {
        self.column_specs.push(f(TableColumnSpec::new(key)));
        self
    }

    /// Define columns from (key, label) pairs. Shorthand for simple
    /// tables where all columns use default width and alignment.
    pub fn columns(mut self, cols: &[(&str, &str)]) -> Self {
        for &(key, label) in cols {
            self.column_specs
                .push(TableColumnSpec::new(key).label(label));
        }
        self
    }

    /// Add a row with rich cell content via a closure.
    ///
    /// ```ignore
    /// .row("u1", |r| r
    ///     .cell("name", text("Alice"))
    ///     .cell("email", text("alice@example.com")))
    /// ```
    pub fn row(mut self, id: &str, f: impl FnOnce(TableRowBuilder) -> TableRowBuilder) -> Self {
        self.children.push(f(TableRowBuilder::new(id)).into_view());
        self
    }

    /// Add a text-only row from (column_key, value) pairs.
    ///
    /// Each value is rendered as a text widget.
    pub fn data_row(mut self, id: &str, cells: &[(&str, &str)]) -> Self {
        let row_children: Vec<View> = cells
            .iter()
            .map(|&(col_key, value)| {
                let mut cell_props = PropMap::new();
                cell_props.insert("column", PropValue::Str(col_key.to_string()));
                let text_view = super::view_leaf(format!("{id}/{col_key}/text"), "text", {
                    let mut p = PropMap::new();
                    p.insert("content", PropValue::Str(value.to_string()));
                    p
                });
                super::view_node(
                    col_key.to_string(),
                    "table_cell",
                    cell_props,
                    vec![text_view],
                )
            })
            .collect();

        self.children.push(super::view_node(
            id.to_string(),
            "table_row",
            PropMap::new(),
            row_children,
        ));
        self
    }

    // -- Table-level props ---------------------------------------------------

    /// Table width. Default: fill.
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    /// Table height. Wraps in a scrollable when set.
    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    /// Show the header row. Default: true.
    pub fn header(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "header", v);
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

    /// Divider line thickness in pixels. Set to 0.0 to hide.
    pub fn separator(mut self, thickness: f32) -> Self {
        super::set_prop(&mut self.props, "separator", thickness);
        self
    }

    /// Divider line color.
    pub fn separator_color(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "separator_color", c.into().wire_encode());
        self
    }

    /// Cell internal padding.
    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(
            &mut self.props,
            "padding",
            super::padding_to_value(p.into()),
        );
        self
    }

    /// Text size for auto-generated header labels.
    pub fn header_text_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "header_text_size", s.into().wire_encode());
        self
    }

    /// Text size for data shorthand auto-generated cells.
    pub fn row_text_size(mut self, s: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "row_text_size", s.into().wire_encode());
        self
    }

    /// Max events per second for coalescable events.
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }

    /// Accessibility annotations.
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }
}

impl From<TableBuilder> for View {
    fn from(mut b: TableBuilder) -> View {
        // Encode column specs into the columns prop
        if !b.column_specs.is_empty() {
            let cols: Vec<PropValue> = b.column_specs.iter().map(|c| c.to_prop_value()).collect();
            b.props.insert("columns", PropValue::Array(cols));
        }

        super::view_node(b.id, "table", b.props, b.children)
    }
}
