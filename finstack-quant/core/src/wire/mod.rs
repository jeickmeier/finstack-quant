//! Canonical serde representations whose domain storage cannot describe its
//! JSON contract directly to `schemars`.

use serde::{Deserialize, Serialize};

mod bounded_f64;
mod counts_wire;
mod date_wire;
mod decimal_wire;
mod non_finite;
#[cfg(test)]
mod tests;

pub use bounded_f64::{
    deserialize_closed_unit_interval_f64, deserialize_correlation, deserialize_non_negative_f64,
    deserialize_open_unit_interval_f64, deserialize_optional_positive_f64,
    deserialize_positive_f64, serialize_closed_unit_interval_f64, serialize_correlation,
    serialize_non_negative_f64, serialize_open_unit_interval_f64, serialize_optional_positive_f64,
    serialize_positive_f64, ClosedUnitIntervalF64Wire, CorrelationWire, NonNegativeF64Wire,
    OpenUnitIntervalF64Wire, PercentageQuantityWire, PositiveF64Wire,
};
pub use counts_wire::{count, counts, signed_count};
pub use date_wire::DateWire;
pub use decimal_wire::{decimal, optional_decimal, DecimalWire};
pub use non_finite::{
    non_finite_f64, non_finite_f64_seq, NonFiniteF64Wire, NonFiniteFields, NonFiniteSentinel,
};

/// Numeric schema revision for contracts whose sole supported revision is v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
#[cfg_attr(feature = "json-schema", schemars(transparent))]
pub struct SchemaVersion(
    #[cfg_attr(feature = "json-schema", schemars(range(min = 1, max = 1)))] u32,
);

impl SchemaVersion {
    /// Canonical numeric revision used by every v1-only wire contract.
    pub const CURRENT: Self = Self(1);

    /// Canonical numeric revision as a primitive integer.
    pub const VALUE: u32 = 1;
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let version = u32::deserialize(deserializer)?;
        if version == Self::VALUE {
            Ok(Self(version))
        } else {
            Err(serde::de::Error::custom(format!(
                "unsupported schema_version {version}; expected 1"
            )))
        }
    }
}

impl From<SchemaVersion> for u32 {
    fn from(value: SchemaVersion) -> Self {
        value.0
    }
}

/// Serde name of a unit-variant enum value (the `rename_all` form).
///
/// Lets hosts label enums with the exact string serde owns instead of keeping
/// a second table of variant names.
///
/// # Arguments
///
/// * `value` - Unit-variant enum (or any type) whose serde form is a JSON string.
///
/// # Errors
///
/// Returns [`crate::Error::Internal`] when `value` does not serialize to a
/// JSON string.
pub fn serde_label<T: Serialize>(value: &T) -> crate::Result<String> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(label)) => Ok(label),
        Ok(other) => Err(crate::Error::Internal(format!(
            "expected a string serde form, got {other}"
        ))),
        Err(e) => Err(crate::Error::Internal(e.to_string())),
    }
}

/// Serde tag of an externally tagged enum value.
///
/// A unit variant serializes to a bare string (`"constant"`) and a data
/// variant to a single-key object (`{"inverse_power": {...}}`); the tag is that
/// string or that key. Lets hosts report which variant a value carries with
/// the exact name serde owns.
///
/// # Arguments
///
/// * `value` - Externally tagged enum (or any type) whose serde form is a JSON
///   string or a JSON object with exactly one key.
///
/// # Errors
///
/// Returns [`crate::Error::Internal`] when `value` does not serialize to a
/// JSON string or a single-key JSON object.
pub fn serde_tag<T: Serialize>(value: &T) -> crate::Result<String> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(tag)) => Ok(tag),
        Ok(serde_json::Value::Object(map)) if map.len() == 1 => map
            .into_iter()
            .next()
            .map(|(tag, _)| tag)
            .ok_or_else(|| crate::Error::Internal("empty serde tag object".to_string())),
        Ok(other) => Err(crate::Error::Internal(format!(
            "expected an externally tagged enum value, got {other}"
        ))),
        Err(e) => Err(crate::Error::Internal(e.to_string())),
    }
}

/// Parse a unit-variant enum from its serde name (the inverse of [`serde_label`]).
///
/// # Arguments
///
/// * `label` - Serde string form of the variant, e.g. `"snake_case_name"`.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `label` is not one of the
/// type's serde names; the message carries serde's list of expected values.
pub fn serde_parse<T: serde::de::DeserializeOwned>(label: &str) -> crate::Result<T> {
    serde_json::from_value(serde_json::Value::String(label.to_string()))
        .map_err(|e| crate::Error::Validation(format!("invalid value {label:?}: {e}")))
}
