//! Decimal wire form and its optional adapter.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Exact decimal encoded only as a JSON string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
#[cfg_attr(feature = "json-schema", schemars(transparent))]
pub struct DecimalWire(
    #[serde(with = "rust_decimal::serde::str")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "String", regex(pattern = r"^-?\d+(\.\d+)?([eE][+-]?\d+)?$"))
    )]
    pub Decimal,
);

impl From<Decimal> for DecimalWire {
    fn from(value: Decimal) -> Self {
        Self(value)
    }
}

impl From<DecimalWire> for Decimal {
    fn from(value: DecimalWire) -> Self {
        value.0
    }
}

/// Serde field adapter for a canonical [`DecimalWire`].
pub mod decimal {
    use super::{Decimal, DecimalWire, Deserialize, Serialize};

    /// Serialize a domain decimal through its canonical wire type.
    ///
    /// # Arguments
    ///
    /// * `value` - Exact decimal to encode as a JSON string without binary rounding.
    /// * `serializer` - Serde serializer that receives the canonical decimal string.
    pub fn serialize<S>(value: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        DecimalWire::from(*value).serialize(serializer)
    }

    /// Deserialize a canonical wire decimal into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying a canonical decimal string.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        DecimalWire::deserialize(deserializer).map(Into::into)
    }
}

/// Serde field adapter for an optional canonical decimal.
pub mod optional_decimal {
    use super::{Decimal, DecimalWire, Deserialize, Serialize};

    /// Serialize an optional domain decimal through its canonical wire type.
    ///
    /// # Arguments
    ///
    /// * `value` - Optional exact decimal to encode as a string or JSON `null`.
    /// * `serializer` - Serde serializer that receives the canonical optional decimal.
    pub fn serialize<S>(value: &Option<Decimal>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.map(DecimalWire::from).serialize(serializer)
    }

    /// Deserialize an optional canonical wire decimal into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying a decimal string or JSON `null`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<DecimalWire>::deserialize(deserializer).map(|value| value.map(Into::into))
    }
}
