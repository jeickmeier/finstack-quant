//! Decimal wire form and its optional adapter.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Exact decimal encoded only as a JSON string.
///
/// Restoration routes through [`crate::decimal::parse_decimal`], the single
/// exact text-admission rule: JSON numbers, malformed text and inexact values
/// are rejected rather than rounded. Serialization writes canonical
/// fixed-point text with trailing zeros removed and signed zero normalized
/// (`"1.5"`, `"0"`), never scientific notation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(transparent))]
pub struct DecimalWire(
    #[cfg_attr(
        feature = "json-schema",
        schemars(
            with = "String",
            regex(pattern = r"^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$")
        )
    )]
    pub Decimal,
);

/// Canonical fixed-point text of a Decimal: normalized, no `-0`.
fn canonical_text(value: Decimal) -> String {
    if value.is_zero() {
        "0".to_string()
    } else {
        value.normalize().to_string()
    }
}

impl Serialize for DecimalWire {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&canonical_text(self.0))
    }
}

impl<'de> Deserialize<'de> for DecimalWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = DecimalWire;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an exact decimal string such as \"1234.56\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<DecimalWire, E>
            where
                E: serde::de::Error,
            {
                crate::decimal::parse_decimal(value)
                    .map(DecimalWire)
                    .map_err(E::custom)
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

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

#[cfg(test)]
mod tests {
    use super::DecimalWire;
    use rust_decimal::Decimal;

    #[test]
    fn wire_restores_exactly_and_serializes_canonically() {
        let parse = |json: &str| serde_json::from_str::<DecimalWire>(json);
        assert_eq!(parse("\"1.50\"").expect("fixed").0, Decimal::new(150, 2));
        assert_eq!(parse("\"+.5\"").expect("grammar").0, Decimal::new(5, 1));
        assert_eq!(
            parse("\"1.2345e3\"").expect("scientific").0,
            Decimal::new(12345, 1)
        );
        assert_eq!(parse("\"-0\"").expect("zero").0, Decimal::ZERO);
        for bad in ["1.5", "\"1e-29\"", "\"1_0\"", "\"NaN\"", "\" 1 0\"", "null"] {
            assert!(parse(bad).is_err(), "{bad} must be rejected");
        }
        let text = |value: Decimal| serde_json::to_string(&DecimalWire(value)).expect("json");
        assert_eq!(text(Decimal::new(150, 2)), "\"1.5\"");
        assert_eq!(text(Decimal::new(-0, 3)), "\"0\"");
        assert_eq!(
            text(Decimal::new(1, 28)),
            "\"0.0000000000000000000000000001\""
        );
    }
}
