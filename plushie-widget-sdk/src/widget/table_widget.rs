use iced::widget::{button, column, container, scrollable, text};
use iced::widget::table as iced_table;
use iced::{Element, Length, Theme, alignment};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::a11y::{A11yOverride, A11yOverrides};
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use plushie_core::types::{self as core_types, PlushieType, SortOrder};
use plushie_core::types::{Color as CoreColor, HorizontalAlignment};
use plushie_core::types::a11y::Role;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn with_role<'a, R: PlushieRenderer>(
    element: Element<'a, Message, Theme, R>,
    role: Role,
) -> Element<'a, Message, Theme, R> {
    A11yOverride::wrap(element, A11yOverrides::with_role(role)).into()
}

// ---------------------------------------------------------------------------
// Column spec (parsed from the columns prop)
// ---------------------------------------------------------------------------

struct TableColumn {
    key: String,
    label: String,
    align: alignment::Horizontal,
    width: Length,
    sortable: bool,
}

fn parse_columns(props: &plushie_core::protocol::Props) -> Vec<TableColumn> {
    let cols_val = props.get_value("columns");
    cols_val
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|col| {
                    let key = col.get("key")?.as_str()?.to_owned();
                    let label = col
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&key)
                        .to_owned();
                    let align = col
                        .get("align")
                        .and_then(HorizontalAlignment::wire_decode)
                        .map(iced_convert::horizontal_alignment)
                        .unwrap_or(alignment::Horizontal::Left);
                    let width = col
                        .get("width")
                        .and_then(core_types::Length::wire_decode)
                        .as_ref()
                        .map(iced_convert::length)
                        .unwrap_or(Length::FillPortion(1));
                    let sortable = col
                        .get("sortable")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    Some(TableColumn {
                        key,
                        label,
                        align,
                        width,
                        sortable,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Table props
// ---------------------------------------------------------------------------

#[allow(dead_code)] // separator_color will be used for iced table Style theming
struct TableProps {
    header: bool,
    width: Length,
    height: Option<Length>,
    padding: Option<core_types::Padding>,
    sort_by: Option<String>,
    sort_order: Option<SortOrder>,
    header_text_size: Option<f32>,
    row_text_size: Option<f32>,
    separator: f32,
    separator_color: Option<CoreColor>,
}

impl TableProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;

        Self {
            header: bool::extract(p, "header").unwrap_or(true),
            width: core_types::Length::extract(p, "width")
                .as_ref()
                .map(iced_convert::length)
                .unwrap_or(Length::Fill),
            height: core_types::Length::extract(p, "height")
                .as_ref()
                .map(iced_convert::length),
            padding: core_types::Padding::extract(p, "padding"),
            sort_by: String::extract(p, "sort_by"),
            sort_order: SortOrder::extract(p, "sort_order"),
            header_text_size: f32::extract(p, "header_text_size"),
            row_text_size: f32::extract(p, "row_text_size"),
            separator: f32::extract(p, "separator").unwrap_or(1.0),
            separator_color: CoreColor::extract(p, "separator_color"),
        }
    }
}

// ---------------------------------------------------------------------------
// RowRef: Copy wrapper for iced table's T: Clone bound
// ---------------------------------------------------------------------------

/// Zero-cost row reference passed to iced table column view closures.
///
/// Both RenderCtx and &TreeNode are Copy, so iced's row.clone() in
/// Table::new (line 115) copies two pointers instead of cloning data.
struct RowRef<'a, R: PlushieRenderer> {
    ctx: RenderCtx<'a, R>,
    node: &'a TreeNode,
}

impl<R: PlushieRenderer> Clone for RowRef<'_, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: PlushieRenderer> Copy for RowRef<'_, R> {}

/// Render a single cell's content from a table_cell child node.
fn render_cell_content<'a, R: PlushieRenderer>(
    cell_node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    match cell_node.children.len() {
        0 => text("").into(),
        1 => ctx.render_child(&cell_node.children[0]),
        _ => {
            // Multiple children: stack vertically
            column(
                cell_node
                    .children
                    .iter()
                    .map(|c| ctx.render_child(c)),
            )
            .into()
        }
    }
}

// ---------------------------------------------------------------------------
// Fallback: render rows from the `rows` JSON prop (data shorthand
// where to_node expansion hasn't happened yet).
// ---------------------------------------------------------------------------

fn render_prop_rows<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    columns: &[TableColumn],
    tp: &TableProps,
    ctx: &RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    use iced::widget::row;
    use iced::Fill;

    let rows_val = node.props.get_value("rows");
    let rows: Vec<&Value> = rows_val
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().collect())
        .unwrap_or_default();

    let mut table_rows: Vec<Element<'a, Message, Theme, R>> = Vec::new();

    // Header
    if tp.header {
        let header_cells: Vec<Element<'a, Message, Theme, R>> = columns
            .iter()
            .map(|col| {
                build_header_cell(col, &tp.sort_by, tp.sort_order, tp.header_text_size,
                    &node.id, ctx)
            })
            .collect();
        table_rows.push(with_role(row(header_cells).width(Fill).into(), Role::Row));
    }

    // Data rows
    for data_row in &rows {
        let cells: Vec<Element<'a, Message, Theme, R>> = columns
            .iter()
            .map(|col| {
                let cell_text = data_row
                    .get(&col.key)
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                let mut cell = text(cell_text);
                if let Some(sz) = tp.row_text_size {
                    cell = cell.size(sz);
                }
                let cell_elem: Element<'a, Message, Theme, R> =
                    container(cell).width(col.width).align_x(col.align).into();
                with_role(cell_elem, Role::Cell)
            })
            .collect();
        table_rows.push(with_role(row(cells).width(Fill).into(), Role::Row));
    }

    let mut table_col = column(table_rows).width(tp.width);
    if let Some(ref p) = tp.padding {
        table_col = table_col.padding(iced_convert::padding(p));
    }

    with_role(scrollable(table_col).into(), Role::Table)
}

// ---------------------------------------------------------------------------
// Build a header cell (shared between prop-based and children-based paths)
// ---------------------------------------------------------------------------

fn build_header_cell<'a, R: PlushieRenderer>(
    col: &TableColumn,
    sort_by: &Option<String>,
    sort_order: Option<SortOrder>,
    header_text_size: Option<f32>,
    table_id: &str,
    ctx: &RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let sort_indicator = if sort_by.as_deref() == Some(&col.key) {
        match sort_order {
            Some(SortOrder::Asc) => " \u{25B2}",
            Some(SortOrder::Desc) => " \u{25BC}",
            None => "",
        }
    } else {
        ""
    };

    let label_text = format!("{}{}", col.label, sort_indicator);

    let cell_elem: Element<'a, Message, Theme, R> = if col.sortable {
        let window_id = ctx.window_id.to_string();
        let click_id = table_id.to_string();
        let click_key = col.key.clone();
        let mut label = text(label_text);
        if let Some(sz) = header_text_size {
            label = label.size(sz);
        }
        container(
            button(label)
                .on_press(Message::Event {
                    window_id,
                    id: click_id,
                    data: serde_json::json!(click_key),
                    family: "sort".into(),
                })
                .style(button::text),
        )
        .width(col.width)
        .align_x(col.align)
        .into()
    } else {
        let mut label = text(label_text);
        if let Some(sz) = header_text_size {
            label = label.size(sz);
        }
        container(label).width(col.width).align_x(col.align).into()
    };

    with_role(cell_elem, Role::ColumnHeader)
}

// ---------------------------------------------------------------------------
// Main widget
// ---------------------------------------------------------------------------

pub(crate) struct TableWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for TableWidget {
    fn type_names(&self) -> &[&str] {
        &["table"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let tp = TableProps::from_node(node);
        let columns = parse_columns(&node.props);
        let table_id = &node.id;

        if columns.is_empty() {
            return text("(empty table)").into();
        }

        // Check for children-based rows (table_row children)
        let row_children: Vec<&TreeNode> = node
            .children
            .iter()
            .filter(|c| c.type_name == "table_row")
            .collect();

        // If no row children, fall back to prop-based rows rendering
        if row_children.is_empty() {
            return render_prop_rows(node, &columns, &tp, ctx);
        }

        // -- Children-based rendering using iced::table ----------------------

        // Build iced column specs with RowRef view closures
        let iced_cols: Vec<iced_table::Column<'a, '_, RowRef<'a, R>, Message, Theme, R>> = columns
            .iter()
            .map(|col| {
                let header = build_header_cell(
                    col,
                    &tp.sort_by,
                    tp.sort_order,
                    tp.header_text_size,
                    table_id,
                    ctx,
                );
                let col_key = col.key.clone();
                iced_table::column(header, move |row_ref: RowRef<'a, R>| {
                    // Find the table_cell child matching this column
                    let cell_node = row_ref.node.children.iter().find(|c| {
                        c.type_name == "table_cell"
                            && String::extract(&c.props, "column")
                                .map(|s| s == col_key)
                                .unwrap_or(false)
                    });

                    let cell_elem = match cell_node {
                        Some(cn) => render_cell_content(cn, row_ref.ctx),
                        None => text("").into(),
                    };

                    with_role(cell_elem, Role::Cell)
                })
                .width(col.width)
                .align_x(col.align)
            })
            .collect();

        // Build RowRef iterator from children
        let rows = row_children.iter().map(|rn| RowRef {
            ctx: *ctx,
            node: rn,
        });

        // Construct iced table
        let mut tbl = iced_table::table(iced_cols, rows)
            .width(tp.width)
            .separator(tp.separator);

        if let Some(ref p) = tp.padding {
            tbl = tbl.padding_x(p.left).padding_y(p.top);
        }

        // Wrap in scrollable if height is set
        let table_elem: Element<'a, Message, Theme, R> = if let Some(h) = tp.height {
            scrollable(tbl).height(h).into()
        } else {
            tbl.into()
        };

        with_role(table_elem, Role::Table)
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(TableWidget)
    }
}
