use iced::advanced::widget::operation::accessible;
use iced::widget::{button, column, container, row, rule, scrollable, text};
use iced::{Element, Fill, Length, Theme, alignment};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::a11y::{A11yOverride, A11yOverrides};
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{self as core_types, PlushieType};

/// Wrap an element with an accessibility role override.
fn with_role<'a, R: PlushieRenderer>(
    element: Element<'a, Message, Theme, R>,
    role: accessible::Role,
) -> Element<'a, Message, Theme, R> {
    A11yOverride::wrap(
        element,
        A11yOverrides {
            role: Some(role),
            ..A11yOverrides::default()
        },
    )
    .into()
}

/// Parsed column descriptor from the "columns" prop.
struct TableColumn {
    key: String,
    label: String,
    align: alignment::Horizontal,
    width: Length,
    sortable: bool,
}

fn parse_table_columns(props: &plushie_core::protocol::Props) -> Vec<TableColumn> {
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
                        .and_then(|v| v.as_str())
                        .and_then(value_to_horizontal_alignment)
                        .unwrap_or(alignment::Horizontal::Left);
                    let width = col
                        .get("width")
                        .and_then(value_to_length)
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

struct TableProps {
    header: Option<bool>,
    separator: Option<bool>,
    width: Option<core_types::Length>,
    padding: Option<core_types::Padding>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    header_text_size: Option<f32>,
    row_text_size: Option<f32>,
    cell_spacing: Option<f32>,
    row_spacing: Option<f32>,
    separator_thickness: Option<f32>,
}

impl TableProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            header: bool::extract(p, "header"),
            separator: bool::extract(p, "separator"),
            width: core_types::Length::extract(p, "width"),
            padding: core_types::Padding::extract(p, "padding"),
            sort_by: String::extract(p, "sort_by"),
            sort_order: String::extract(p, "sort_order"),
            header_text_size: f32::extract(p, "header_text_size"),
            row_text_size: f32::extract(p, "row_text_size"),
            cell_spacing: f32::extract(p, "cell_spacing"),
            row_spacing: f32::extract(p, "row_spacing"),
            separator_thickness: f32::extract(p, "separator_thickness"),
        }
    }
}

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
        let width = tp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Fill);
        let show_header = tp.header.unwrap_or(true);
        let table_id = node.id.clone();

        let header_text_size = tp.header_text_size;
        let row_text_size = tp.row_text_size;

        let cell_spacing = tp.cell_spacing;
        let row_spacing = tp.row_spacing;
        let separator_thickness = tp.separator_thickness.unwrap_or(1.0);
        // separator_color: keep as raw prop access (returns iced::Color directly)
        let separator_color = prop_color(&node.props, "separator_color");

        let sort_by = tp.sort_by;
        let sort_order = tp.sort_order;

        // Columns and rows: keep as raw prop access (complex array of objects)
        let columns = parse_table_columns(&node.props);

        // Rows: keep as raw prop access (complex array of objects)
        let rows_val = node.props.get_value("rows");
        let rows: Vec<&Value> = rows_val
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().collect())
            .unwrap_or_default();

        if columns.is_empty() {
            return text("(empty table)").into();
        }

        let mut table_rows: Vec<Element<'a, Message, Theme, R>> = Vec::new();

        // Header row (conditional)
        if show_header {
            let header_cells: Vec<Element<'a, Message, Theme, R>> = columns
                .iter()
                .map(|col| {
                    // Build sort indicator if this column is currently sorted.
                    let sort_indicator = if sort_by.as_deref() == Some(&col.key) {
                        match sort_order.as_deref() {
                            Some("asc") => " \u{25B2}",
                            Some("desc") => " \u{25BC}",
                            _ => "",
                        }
                    } else {
                        ""
                    };

                    let label_text = format!("{}{}", col.label, sort_indicator);

                    let cell_elem: Element<'a, Message, Theme, R> = if col.sortable {
                        let window_id = ctx.window_id.to_string();
                        let click_id = table_id.clone();
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
                                    data: serde_json::json!({"column": click_key}),
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
                    with_role(cell_elem, accessible::Role::ColumnHeader)
                })
                .collect();
            let mut header = row(header_cells).width(Fill);
            if let Some(cs) = cell_spacing {
                header = header.spacing(cs);
            }
            table_rows.push(with_role(header.into(), accessible::Role::Row));

            // Separator
            let show_separator = tp.separator.unwrap_or(true);
            if show_separator {
                let sep: Element<'a, Message, Theme, R> = if let Some(sep_col) = separator_color {
                    rule::horizontal(separator_thickness)
                        .style(move |_theme: &iced::Theme| rule::Style {
                            color: sep_col,
                            radius: Default::default(),
                            fill_mode: rule::FillMode::Full,
                            snap: true,
                        })
                        .into()
                } else {
                    rule::horizontal(separator_thickness).into()
                };
                table_rows.push(sep);
            }
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
                    if let Some(sz) = row_text_size {
                        cell = cell.size(sz);
                    }
                    let cell_elem: Element<'a, Message, Theme, R> =
                        container(cell).width(col.width).align_x(col.align).into();
                    with_role(cell_elem, accessible::Role::Cell)
                })
                .collect();
            let mut data_row_elem = row(cells).width(Fill);
            if let Some(cs) = cell_spacing {
                data_row_elem = data_row_elem.spacing(cs);
            }
            table_rows.push(with_role(data_row_elem.into(), accessible::Role::Row));
        }

        let mut table_col = column(table_rows).width(width);

        if let Some(rs) = row_spacing {
            table_col = table_col.spacing(rs);
        }

        if let Some(ref p) = tp.padding {
            table_col = table_col.padding(iced_convert::padding(p));
        }

        with_role(scrollable(table_col).into(), accessible::Role::Table)
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(TableWidget)
    }
}
