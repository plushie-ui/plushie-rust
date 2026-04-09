//! Table widget builder.

use serde_json::{Map, Value, json};

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
    props: Map<String, Value>,
    children: Vec<View>,
}

/// Create a table widget.
pub fn table(id: &str) -> TableBuilder {
    TableBuilder {
        id: id.to_string(),
        props: Map::new(),
        children: vec![],
    }
}

impl TableBuilder {
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    pub fn columns(mut self, cols: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let cols: Vec<Value> = cols.into_iter()
            .map(|c| json!({"key": c.as_ref(), "label": c.as_ref()}))
            .collect();
        super::set_prop(&mut self.props, "columns", Value::Array(cols));
        self
    }

    pub fn spacing(mut self, s: f32) -> Self {
        super::set_prop(&mut self.props, "spacing", s);
        self
    }

    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into()));
        self
    }

    pub fn style(mut self, s: impl Into<Style>) -> Self {
        super::set_prop(&mut self.props, "style", super::style_to_value(&s.into()));
        self
    }

    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }

    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.clone());
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
        View::node(b.id, "table", b.props, b.children)
    }
}
