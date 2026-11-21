//! Display unit metadata for structured result fields.
//!
//! Family-specific field-unit tables live in the owning family crates and are
//! re-exported alongside [`MetricFieldUnit`] from the umbrella crate's
//! `results` module.

/// The display unit of one scalar field of a result struct.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MetricFieldUnit {
    /// Wire name of the field.
    pub field: String,
    /// Unit text as the field documents it, such as `USD per bp` or `years`.
    pub label: String,
    /// Canonical Rust field the unit is documented on.
    pub source: String,
}
