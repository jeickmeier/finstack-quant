//! JSON schema types for metric definitions.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Top-level metric registry schema.
///
/// Represents the JSON payload used to load metric definitions into the
/// [`Registry`](crate::registry::Registry).
///
/// # Example JSON
///
/// ```json
/// {
///   "namespace": "fin",
///   "schema_version": 1,
///   "metrics": [
///     {
///       "id": "gross_margin",
///       "name": "Gross Margin %",
///       "formula": "gross_profit / revenue",
///       "description": "Gross profit as percentage of revenue",
///       "category": "margins",
///       "unit_type": "percentage",
///       "requires": ["gross_profit", "revenue"]
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRegistry {
    /// Namespace for all metrics in this registry (e.g., "fin", "custom")
    pub namespace: String,

    /// Schema version for forward compatibility
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// List of metric definitions contained in the registry
    pub metrics: Vec<MetricDefinition>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,
}

/// Individual metric definition.
///
/// Each definition maps directly to a JSON object in a registry file and is
/// converted into a [`NodeSpec`](crate::types::NodeSpec) when registered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    /// Unique identifier within the namespace
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Formula text (in statements DSL)
    pub formula: String,

    /// Description of what this metric represents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Category for grouping (e.g., "margins", "returns", "leverage")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Unit type (percentage, currency, ratio, count)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_type: Option<UnitType>,

    /// List of required node dependencies
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,

    /// Tags for filtering/searching
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,
}

/// Unit type for metric values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitType {
    /// Percentage (0.15 = 15%)
    Percentage,
    /// Currency amount
    Currency,
    /// Ratio (dimensionless)
    Ratio,
    /// Count (integer-like)
    Count,
    /// Time period (days, months, years)
    TimePeriod,
}

fn default_schema_version() -> u32 {
    1
}

impl MetricDefinition {
    /// Compute the fully-qualified metric ID (`namespace.metric_id`).
    ///
    /// # Arguments
    /// * `namespace` - Namespace owning the metric (e.g., `"fin"`)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::registry::MetricDefinition;
    /// # use indexmap::IndexMap;
    /// let def = MetricDefinition {
    ///     id: "gross_margin".into(),
    ///     name: "Gross Margin".into(),
    ///     formula: "gross_profit / revenue".into(),
    ///     description: None,
    ///     category: None,
    ///     unit_type: None,
    ///     requires: vec![],
    ///     tags: vec![],
    ///     meta: indexmap::IndexMap::new(),
    /// };
    /// assert_eq!(def.qualified_id("fin"), "fin.gross_margin");
    /// ```
    pub fn qualified_id(&self, namespace: &str) -> String {
        format!("{}.{}", namespace, self.id)
    }
}
