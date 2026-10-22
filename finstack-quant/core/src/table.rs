//! Serializable columnar table envelope for tabular result exports.
//!
//! This module provides a lightweight, serde-friendly alternative to returning
//! a third-party DataFrame type from Rust APIs. Callers can inspect columns
//! directly, serialize them for Python/WASM consumers, or convert them into a
//! host-language table type at the binding layer.
//!
//! To hand an envelope to Apache Arrow consumers, use the supporting
//! `finstack-quant-arrow` crate (`finstack_quant_arrow::to_record_batch`).
//! That crate is export-only and is not re-exported by the umbrella package.

use crate::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Serializable columnar table envelope.
///
/// Tables preserve column order and validate that every column has the same row
/// count. Optional metadata can record domain-specific hints such as which
/// column is a metric, what a numeric field represents, or how a host-language
/// binding should interpret the data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct TableEnvelope {
    /// Number of rows in the table.
    pub row_count: usize,
    /// Ordered set of columns.
    pub columns: Vec<TableColumn>,
    /// Table-level metadata for downstream bindings and documentation.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub metadata: IndexMap<String, serde_json::Value>,
}

impl TableEnvelope {
    /// Construct a table from validated columns and metadata.
    ///
    /// # Arguments
    ///
    /// * `columns` - Owned columns in export order, with unique names and
    ///   equal lengths; an empty list gives a zero-row table.
    /// * `metadata` - Owned table-level JSON annotations, preserved in their
    ///   insertion order without changing column values or row counts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if columns have mismatched lengths or if
    /// duplicate column names are provided.
    pub fn new(
        columns: Vec<TableColumn>,
        metadata: IndexMap<String, serde_json::Value>,
    ) -> Result<Self> {
        let row_count = columns.first().map(TableColumn::len).unwrap_or(0);
        let mut seen = crate::FxHashSet::default();

        for column in &columns {
            if column.len() != row_count {
                return Err(Error::Validation(format!(
                    "column '{}' has {} rows but expected {}",
                    column.name,
                    column.len(),
                    row_count
                )));
            }

            if !seen.insert(column.name.clone()) {
                return Err(Error::Validation(format!(
                    "duplicate column name '{}'",
                    column.name
                )));
            }
        }

        Ok(Self {
            row_count,
            columns,
            metadata,
        })
    }

    /// Return whether the table has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.row_count == 0
    }

    /// Look up a column by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Exact, case-sensitive column name to find; returns `None`
    ///   when no column has this name.
    #[must_use]
    pub fn column(&self, name: &str) -> Option<&TableColumn> {
        self.columns.iter().find(|column| column.name == name)
    }
}

/// A single named column in a [`TableEnvelope`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct TableColumn {
    /// Column name.
    pub name: String,
    /// Column values.
    pub data: TableColumnData,
    /// Optional semantic hint for bindings and consumers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<TableColumnRole>,
    /// Optional per-column metadata.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub metadata: IndexMap<String, serde_json::Value>,
}

impl TableColumn {
    /// Create a column with no role or metadata.
    ///
    /// # Arguments
    ///
    /// * `name` - Owned or convertible column name used for exact table
    ///   lookup; uniqueness is validated when constructing a table.
    /// * `data` - Owned typed values whose length determines the column's
    ///   row count; nullable variants preserve missing values as `None`.
    #[must_use]
    pub fn new(name: impl Into<String>, data: TableColumnData) -> Self {
        Self {
            name: name.into(),
            data,
            role: None,
            metadata: IndexMap::new(),
        }
    }

    /// Attach a semantic role to the column.
    ///
    /// # Arguments
    ///
    /// * `role` - Consumer-facing hint describing how to interpret the column;
    ///   replaces an existing role without changing its typed values.
    #[must_use]
    pub fn with_role(mut self, role: TableColumnRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Attach metadata to the column.
    ///
    /// # Arguments
    ///
    /// * `metadata` - Owned JSON annotations replacing the existing metadata
    ///   map; insertion order and typed column values are preserved.
    #[must_use]
    pub fn with_metadata(mut self, metadata: IndexMap<String, serde_json::Value>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Number of rows in the column.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Return whether the column has no values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Column storage variants supported by the table envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "values", rename_all = "snake_case")]
pub enum TableColumnData {
    /// Non-null string column.
    String(Vec<String>),
    /// Nullable string column.
    NullableString(Vec<Option<String>>),
    /// Non-null floating-point column.
    Float64(Vec<f64>),
    /// Nullable floating-point column.
    NullableFloat64(Vec<Option<f64>>),
    /// Non-null unsigned 32-bit integer column.
    UInt32(Vec<u32>),
    /// Nullable unsigned 32-bit integer column.
    NullableUInt32(Vec<Option<u32>>),
    /// Non-null signed 64-bit integer column.
    Int64(Vec<i64>),
    /// Nullable signed 64-bit integer column.
    NullableInt64(Vec<Option<i64>>),
}

impl TableColumnData {
    /// Number of rows in the column.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::String(values) => values.len(),
            Self::NullableString(values) => values.len(),
            Self::Float64(values) => values.len(),
            Self::NullableFloat64(values) => values.len(),
            Self::UInt32(values) => values.len(),
            Self::NullableUInt32(values) => values.len(),
            Self::Int64(values) => values.len(),
            Self::NullableInt64(values) => values.len(),
        }
    }

    /// Return whether the column stores no values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Optional semantic hint for a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TableColumnRole {
    /// Row identifier or primary grouping field.
    Dimension,
    /// Ordinal or time-like axis column.
    Index,
    /// Numeric value column.
    Measure,
    /// Auxiliary classification column.
    Attribute,
}

impl TableColumnRole {
    /// Serde / Arrow metadata wire name (`snake_case`).
    ///
    /// Kept in lockstep with `#[serde(rename_all = "snake_case")]` so Arrow
    /// field metadata and JSON serialization share one spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dimension => "dimension",
            Self::Index => "index",
            Self::Measure => "measure",
            Self::Attribute => "attribute",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn table_validates_column_lengths() {
        let result = TableEnvelope::new(
            vec![
                TableColumn::new("a", TableColumnData::String(vec!["x".into(), "y".into()])),
                TableColumn::new("b", TableColumnData::Float64(vec![1.0])),
            ],
            Default::default(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn table_rejects_duplicate_column_names() {
        let result = TableEnvelope::new(
            vec![
                TableColumn::new("a", TableColumnData::String(vec!["x".into()])),
                TableColumn::new("a", TableColumnData::Float64(vec![1.0])),
            ],
            Default::default(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn table_round_trips_with_metadata() {
        let column = TableColumn::new(
            "period_id",
            TableColumnData::String(vec!["2025Q1".into(), "2025Q2".into()]),
        )
        .with_role(TableColumnRole::Index);

        let mut metadata = IndexMap::new();
        metadata.insert("orientation".to_string(), json!("long"));

        let table = TableEnvelope::new(vec![column], metadata).expect("valid table");
        let json = serde_json::to_string(&table).expect("serializes");
        let restored: TableEnvelope = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(restored.row_count, 2);
        assert_eq!(restored.metadata.get("orientation"), Some(&json!("long")));
        assert_eq!(restored.column("period_id").map(TableColumn::len), Some(2));
    }

    #[test]
    fn empty_table_and_columns_report_empty_state() {
        let table = TableEnvelope::new(vec![], Default::default()).expect("empty table is valid");
        let column = TableColumn::new("empty", TableColumnData::Int64(vec![]));

        assert!(table.is_empty());
        assert!(column.is_empty());
        assert!(column.data.is_empty());
        assert_eq!(table.column("missing"), None);
    }

    #[test]
    fn column_role_as_str_matches_serde_rename() {
        for role in [
            TableColumnRole::Dimension,
            TableColumnRole::Index,
            TableColumnRole::Measure,
            TableColumnRole::Attribute,
        ] {
            let json = serde_json::to_string(&role).expect("serializes");
            assert_eq!(json, format!("\"{}\"", role.as_str()));
        }
    }
}
