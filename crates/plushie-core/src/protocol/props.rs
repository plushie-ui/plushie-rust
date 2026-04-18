//! Typed prop storage for the widget tree.
//!
//! [`Props`] wraps a [`PropMap`], a small ordered vector of
//! `(String, PropValue)` pairs. Both direct-mode SDK builders and
//! wire-mode JSON input produce the same underlying shape: wire
//! deserialisation walks the `serde_json::Value` once and converts
//! each entry into a [`PropValue`].
//!
//! Earlier versions of this module had two variants (`Typed` and
//! `Wire`) so wire props could wrap `serde_json::Value` without
//! converting. That traded a one-time deserialize cost for a
//! per-access branch on every accessor, and several accessors were
//! silently variant-asymmetric (e.g. `get` returned `None` for the
//! typed variant). The render path dominates the cost; unifying on
//! `PropMap` removes the footgun without a measurable hit.
//!
//! # Null-valued entries are wire-canonical "absent"
//!
//! The wire protocol encodes prop removal by sending `null` in an
//! `update_props` op. There is no way to transmit "set this key to
//! an explicit null value." As a result, equality on [`Props`] and
//! [`PropMap`] treats null-valued entries as equivalent to absent
//! entries, so round-tripping a tree through diff + apply is
//! lossless. See the `PartialEq` impl on [`PropMap`] below.

use serde_json::Value;

// ---------------------------------------------------------------------------
// PropValue
// ---------------------------------------------------------------------------

/// A typed prop value. Covers all value types the widget system uses.
///
/// Mirrors JSON's type system but without serde allocation overhead.
/// Primitive values are stored inline (no boxing).
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    /// Null.
    Null,
    /// Bool.
    Bool(bool),
    /// F64.
    F64(f64),
    /// I64.
    I64(i64),
    /// U64.
    U64(u64),
    /// Str.
    Str(String),
    /// Array.
    Array(Vec<PropValue>),
    /// Object.
    Object(PropMap),
}

impl PropValue {
    /// Borrow the value as a str.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Borrow the value as a f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64(v) => Some(*v),
            Self::I64(v) => Some(*v as f64),
            Self::U64(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Borrow the value as a bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Borrow the value as a i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::I64(v) => Some(*v),
            Self::U64(v) => i64::try_from(*v).ok(),
            Self::F64(v) => Some(*v as i64),
            _ => None,
        }
    }

    /// Borrow the value as a u64.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U64(v) => Some(*v),
            Self::I64(v) => u64::try_from(*v).ok(),
            Self::F64(v) if *v >= 0.0 => Some(*v as u64),
            _ => None,
        }
    }

    /// Borrow the value as a array.
    pub fn as_array(&self) -> Option<&[PropValue]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Borrow the value as a object.
    pub fn as_object(&self) -> Option<&PropMap> {
        match self {
            Self::Object(m) => Some(m),
            _ => None,
        }
    }

    /// Returns true when the null condition holds.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

// From impls for ergonomic construction.
impl From<bool> for PropValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}
impl From<f32> for PropValue {
    fn from(v: f32) -> Self {
        Self::F64(v as f64)
    }
}
impl From<f64> for PropValue {
    fn from(v: f64) -> Self {
        Self::F64(v)
    }
}
impl From<i32> for PropValue {
    fn from(v: i32) -> Self {
        Self::I64(v as i64)
    }
}
impl From<i64> for PropValue {
    fn from(v: i64) -> Self {
        Self::I64(v)
    }
}
impl From<u32> for PropValue {
    fn from(v: u32) -> Self {
        Self::U64(v as u64)
    }
}
impl From<u64> for PropValue {
    fn from(v: u64) -> Self {
        Self::U64(v)
    }
}
impl From<&str> for PropValue {
    fn from(v: &str) -> Self {
        Self::Str(v.to_string())
    }
}
impl From<String> for PropValue {
    fn from(v: String) -> Self {
        Self::Str(v)
    }
}

// ---------------------------------------------------------------------------
// PropValue <-> serde_json::Value conversion
// ---------------------------------------------------------------------------

impl From<Value> for PropValue {
    fn from(v: Value) -> Self {
        match v {
            Value::Null => Self::Null,
            Value::Bool(b) => Self::Bool(b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::I64(i)
                } else if let Some(u) = n.as_u64() {
                    Self::U64(u)
                } else {
                    Self::F64(n.as_f64().unwrap_or(0.0))
                }
            }
            Value::String(s) => Self::Str(s),
            Value::Array(arr) => Self::Array(arr.into_iter().map(PropValue::from).collect()),
            Value::Object(map) => Self::Object(PropMap::from_json_map(map)),
        }
    }
}

impl From<PropValue> for Value {
    fn from(v: PropValue) -> Self {
        match v {
            PropValue::Null => Value::Null,
            PropValue::Bool(b) => Value::Bool(b),
            PropValue::F64(f) => serde_json::json!(f),
            PropValue::I64(i) => Value::Number(i.into()),
            PropValue::U64(u) => Value::Number(u.into()),
            PropValue::Str(s) => Value::String(s),
            PropValue::Array(arr) => Value::Array(arr.into_iter().map(Value::from).collect()),
            PropValue::Object(map) => Value::Object(map.into_json_map()),
        }
    }
}

// ---------------------------------------------------------------------------
// PropMap
// ---------------------------------------------------------------------------

/// Ordered map of prop key-value pairs.
///
/// Uses a `Vec` for storage since widget props are typically small
/// (5-15 entries). Linear scan is faster than hashing for this size.
///
/// # Wire serialisation key order
///
/// Props are serialised to JSON via
/// [`into_json_map`](PropMap::into_json_map) which collects into a
/// `serde_json::Map`. The workspace compiles `serde_json` **without**
/// the `preserve_order` feature, so `Map` is an alphabetical-key
/// `BTreeMap` equivalent. Downstream consumers (wire protocol,
/// `TestSession::tree_hash`, golden-file tests) rely on this.
/// Enabling `preserve_order` would make JSON serialisation
/// insertion-ordered and break golden-file stability silently.
/// See `tree_hash` for the regression test pinning this.
#[derive(Debug, Clone, Default)]
pub struct PropMap(Vec<(String, PropValue)>);

impl PartialEq for PropMap {
    /// Wire-canonical equality: null-valued entries are equivalent to
    /// absent entries. The wire protocol encodes key removal by sending
    /// `null`, so `{}` and `{"a": null}` are indistinguishable downstream
    /// and must compare equal here. Without this, `tree_diff` +
    /// `apply_patch` could not round-trip trees whose only difference
    /// is a null-valued prop, because there is no protocol op that adds
    /// an explicit null-valued key.
    fn eq(&self, other: &Self) -> bool {
        let non_null =
            |pairs: &[(String, PropValue)]| pairs.iter().filter(|(_, v)| !v.is_null()).count();
        if non_null(&self.0) != non_null(&other.0) {
            return false;
        }
        self.0
            .iter()
            .filter(|(_, v)| !v.is_null())
            .all(|(k, v)| match other.get(k) {
                Some(ov) if !ov.is_null() => ov == v,
                _ => false,
            })
    }
}

impl Eq for PropMap {}

impl PropMap {
    /// Construct a new value.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Return a new value with the capacity set.
    pub fn with_capacity(cap: usize) -> Self {
        Self(Vec::with_capacity(cap))
    }

    /// Get a prop value by key.
    pub fn get(&self, key: &str) -> Option<&PropValue> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Get a mutable reference to a prop value by key.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut PropValue> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Insert or replace a prop value.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<PropValue>) {
        let key = key.into();
        let value = value.into();
        if let Some(entry) = self.0.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        } else {
            self.0.push((key, value));
        }
    }

    /// Remove a prop by key, returning the old value if present.
    pub fn remove(&mut self, key: &str) -> Option<PropValue> {
        let idx = self.0.iter().position(|(k, _)| k == key)?;
        Some(self.0.remove(idx).1)
    }

    /// Set or construct `contains_key`.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.iter().any(|(k, _)| k == key)
    }

    /// Returns true when the empty condition holds.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Set or construct `len`.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Iterate over (key, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &PropValue)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Iterate over keys.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }

    /// Convert from a serde_json Map.
    pub fn from_json_map(map: serde_json::Map<String, Value>) -> Self {
        Self(
            map.into_iter()
                .map(|(k, v)| (k, PropValue::from(v)))
                .collect(),
        )
    }

    /// Convert to a serde_json Map.
    pub fn into_json_map(self) -> serde_json::Map<String, Value> {
        self.0
            .into_iter()
            .map(|(k, v)| (k, Value::from(v)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

/// Prop storage for [`TreeNode`](super::TreeNode).
///
/// Wraps a [`PropMap`]. Both direct-mode SDK builders and wire-mode
/// JSON deserialisation land in the same representation; accessors
/// are plain delegations with no per-variant branching.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Props(PropMap);

impl Props {
    /// Construct from a `serde_json::Value`. Non-object values (a stray
    /// string, number, etc.) become an empty [`PropMap`] rather than a
    /// panic, so malformed wire input degrades gracefully.
    pub fn from_json(value: Value) -> Self {
        match value {
            Value::Object(map) => Self(PropMap::from_json_map(map)),
            _ => Self(PropMap::new()),
        }
    }

    /// Get a string prop.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.as_str()
    }

    /// Get a numeric prop as f64.
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.0.get(key)?.as_f64()
    }

    /// Get a numeric prop as f32.
    pub fn get_f32(&self, key: &str) -> Option<f32> {
        self.get_f64(key).map(|v| v as f32)
    }

    /// Get a boolean prop.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.0.get(key)?.as_bool()
    }

    /// Get an integer prop as i64.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.0.get(key)?.as_i64()
    }

    /// Get an unsigned integer prop as u64.
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.0.get(key)?.as_u64()
    }

    /// Check if a key exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Convert to a JSON Value for consumption by prop_helpers.
    ///
    /// Always allocates (converts PropMap to JSON Map). Callers that
    /// only need field-by-field access should use the typed accessors
    /// directly instead.
    pub fn as_value_cow(&self) -> std::borrow::Cow<'_, Value> {
        std::borrow::Cow::Owned(Value::Object(self.0.clone().into_json_map()))
    }

    /// Borrow the underlying [`PropMap`].
    pub fn as_prop_map(&self) -> &PropMap {
        &self.0
    }

    /// Mutably borrow the underlying [`PropMap`].
    pub fn as_prop_map_mut(&mut self) -> &mut PropMap {
        &mut self.0
    }

    /// Get a prop by key as `&PropValue`.
    pub fn get(&self, key: &str) -> Option<&PropValue> {
        self.0.get(key)
    }

    /// Get a prop value as an owned `Value`. Allocates. Use sparingly;
    /// prefer the typed accessors (`get_str`, `get_f64`, etc.) when
    /// possible.
    pub fn get_value(&self, key: &str) -> Option<Value> {
        self.0.get(key).map(|pv| Value::from(pv.clone()))
    }

    /// Convert to a `serde_json::Value` (for wire serialization).
    pub fn to_value(&self) -> Value {
        Value::Object(self.0.clone().into_json_map())
    }

    /// True if the props contain an object/map structure. Always
    /// true for the unified representation.
    pub fn is_object(&self) -> bool {
        true
    }
}

impl From<PropMap> for Props {
    fn from(map: PropMap) -> Self {
        Self(map)
    }
}

// ---------------------------------------------------------------------------
// Serde: Props serializes as a JSON object and deserializes from any Value
// (non-object inputs collapse to an empty PropMap).
// ---------------------------------------------------------------------------

impl serde::Serialize for Props {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Props {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(Self::from_json(value))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prop_map_insert_and_get() {
        let mut map = PropMap::new();
        map.insert("label", "Save");
        map.insert("size", 18.0f64);
        map.insert("disabled", false);

        assert_eq!(map.get("label").unwrap().as_str(), Some("Save"));
        assert_eq!(map.get("size").unwrap().as_f64(), Some(18.0));
        assert_eq!(map.get("disabled").unwrap().as_bool(), Some(false));
        assert!(map.get("missing").is_none());
    }

    #[test]
    fn prop_map_insert_replaces() {
        let mut map = PropMap::new();
        map.insert("value", 1.0f64);
        map.insert("value", 2.0f64);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("value").unwrap().as_f64(), Some(2.0));
    }

    #[test]
    fn prop_map_remove() {
        let mut map = PropMap::new();
        map.insert("a", "hello");
        map.insert("b", "world");
        assert_eq!(map.remove("a").unwrap().as_str(), Some("hello"));
        assert_eq!(map.len(), 1);
        assert!(map.get("a").is_none());
    }

    #[test]
    fn props_typed_accessors() {
        let mut map = PropMap::new();
        map.insert("title", "Hello");
        map.insert("size", 24.0f64);
        map.insert("visible", true);
        let props = Props::from(map);

        assert_eq!(props.get_str("title"), Some("Hello"));
        assert_eq!(props.get_f64("size"), Some(24.0));
        assert_eq!(props.get_f32("size"), Some(24.0));
        assert_eq!(props.get_bool("visible"), Some(true));
        assert!(props.get_str("missing").is_none());
    }

    #[test]
    fn props_wire_accessors() {
        let props = Props::from_json(json!({"title": "Hello", "size": 24.0, "visible": true}));

        assert_eq!(props.get_str("title"), Some("Hello"));
        assert_eq!(props.get_f64("size"), Some(24.0));
        assert_eq!(props.get_bool("visible"), Some(true));
    }

    #[test]
    fn props_deserialize_round_trip_accessors() {
        let json_str = r#"{"a": 1, "b": "x", "c": true}"#;
        let props: Props = serde_json::from_str(json_str).unwrap();
        assert_eq!(props.get_i64("a"), Some(1));
        assert_eq!(props.get_str("b"), Some("x"));
        assert_eq!(props.get_bool("c"), Some(true));
    }

    #[test]
    fn props_from_non_object_json_is_empty() {
        let props = Props::from_json(json!("stray string"));
        assert!(props.as_prop_map().is_empty());
        assert!(props.is_object());
        assert_eq!(props.get_str("anything"), None);
    }

    #[test]
    fn props_null_entries_are_absent_for_eq() {
        let mut with_null = PropMap::new();
        with_null.insert("content", "hello");
        with_null.insert("size", PropValue::Null);
        let empty_size = PropMap::new();
        let mut plain = empty_size.clone();
        plain.insert("content", "hello");

        assert_eq!(Props::from(with_null), Props::from(plain));
    }

    #[test]
    fn props_typed_eq_wire() {
        let mut map = PropMap::new();
        map.insert("content", "hello");
        map.insert("size", 18.0f64);
        let typed = Props::from(map);

        let wire = Props::from_json(json!({"content": "hello", "size": 18.0}));

        assert_eq!(typed, wire);
    }

    #[test]
    fn prop_value_round_trip_through_json() {
        let mut map = PropMap::new();
        map.insert("text", "hello");
        map.insert("num", 42.0f64);
        map.insert("flag", true);
        map.insert(
            "items",
            PropValue::Array(vec![PropValue::from(1.0f64), PropValue::from(2.0f64)]),
        );

        let json_map = map.clone().into_json_map();
        let round_tripped = PropMap::from_json_map(json_map);
        assert_eq!(map, round_tripped);
    }

    #[test]
    fn props_serializes_as_json_object() {
        let mut map = PropMap::new();
        map.insert("label", "Save");
        let props = Props::from(map);

        let json_str = serde_json::to_string(&props).unwrap();
        assert!(json_str.contains("\"label\":\"Save\""));
    }

    #[test]
    fn props_deserializes_to_prop_map() {
        let json_str = r#"{"label":"Save","size":18}"#;
        let props: Props = serde_json::from_str(json_str).unwrap();
        assert_eq!(props.get_str("label"), Some("Save"));
        assert_eq!(props.get_i64("size"), Some(18));
    }

    #[test]
    fn props_default_is_empty() {
        let props = Props::default();
        assert!(props.as_prop_map().is_empty());
    }

    #[test]
    fn prop_value_numeric_coercion() {
        assert_eq!(PropValue::I64(42).as_f64(), Some(42.0));
        assert_eq!(PropValue::U64(42).as_f64(), Some(42.0));
        assert_eq!(PropValue::F64(42.0).as_i64(), Some(42));
        assert_eq!(PropValue::I64(42).as_u64(), Some(42));
    }

    // ---------------------------------------------------------------------------
    // Alphabetical key ordering invariant
    //
    // `tree_hash` and golden-file tests depend on `serde_json::to_string`
    // producing alphabetical-key output. That holds only when `serde_json`'s
    // `preserve_order` feature is NOT enabled. This test inserts keys in
    // non-alphabetical order and asserts the serialised string is
    // alphabetical.
    //
    // If this test ever fails, a transitive dependency has enabled
    // `preserve_order`; golden files across the workspace will break.
    // ---------------------------------------------------------------------------

    #[test]
    fn props_serialise_keys_alphabetically() {
        let mut map = PropMap::new();
        // Insert in reverse-alphabetical order.
        map.insert("zebra", "z");
        map.insert("mango", "m");
        map.insert("apple", "a");
        let props = Props::from(map);

        let json_str = serde_json::to_string(&props).unwrap();
        // Alphabetical: apple, mango, zebra.
        let expected = r#"{"apple":"a","mango":"m","zebra":"z"}"#;
        assert_eq!(
            json_str, expected,
            "serde_json Props serialisation must be alphabetical; \
             if this fails, serde_json's preserve_order feature may have \
             leaked in via a transitive dependency"
        );
    }

    #[test]
    fn nested_props_serialise_keys_alphabetically() {
        // Nested objects must also be alphabetical.
        let mut inner = PropMap::new();
        inner.insert("width", 100.0f64);
        inner.insert("height", 50.0f64);
        let mut outer = PropMap::new();
        outer.insert("z_field", PropValue::Object(inner));
        outer.insert("a_field", "a");
        let props = Props::from(outer);

        let json_str = serde_json::to_string(&props).unwrap();
        assert_eq!(
            json_str,
            r#"{"a_field":"a","z_field":{"height":50.0,"width":100.0}}"#
        );
    }
}
