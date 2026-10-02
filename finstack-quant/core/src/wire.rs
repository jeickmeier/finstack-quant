//! Canonical serde representations whose domain storage cannot describe its
//! JSON contract directly to `schemars`.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::Date;

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

/// ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
#[cfg_attr(feature = "json-schema", schemars(transparent))]
pub struct DateWire(
    #[cfg_attr(feature = "json-schema", schemars(with = "String", extend("format" = "date")))]
    pub  Date,
);

impl From<Date> for DateWire {
    fn from(value: Date) -> Self {
        Self(value)
    }
}

impl From<DateWire> for Date {
    fn from(value: DateWire) -> Self {
        value.0
    }
}

/// Serde field adapter for a canonical [`DateWire`].
pub mod date {
    use super::{Date, DateWire, Deserialize, Serialize};

    /// Serialize a domain date through its canonical wire type.
    ///
    /// # Arguments
    ///
    /// * `value` - Calendar date to encode as an ISO `YYYY-MM-DD` string.
    /// * `serializer` - Serde serializer that receives the canonical date string.
    pub fn serialize<S>(value: &Date, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        DateWire::from(*value).serialize(serializer)
    }

    /// Deserialize a canonical wire date into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying an ISO `YYYY-MM-DD` date string.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Date, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        DateWire::deserialize(deserializer).map(Into::into)
    }
}

/// Serde field adapter for an optional canonical date.
pub mod optional_date {
    use super::{Date, DateWire, Deserialize, Serialize};

    /// Serialize an optional domain date through its canonical wire type.
    ///
    /// # Arguments
    ///
    /// * `value` - Optional calendar date to encode as an ISO string or JSON `null`.
    /// * `serializer` - Serde serializer that receives the canonical optional date.
    pub fn serialize<S>(value: &Option<Date>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.map(DateWire::from).serialize(serializer)
    }

    /// Deserialize an optional canonical wire date into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying an ISO date string or JSON `null`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Date>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<DateWire>::deserialize(deserializer).map(|value| value.map(Into::into))
    }
}

/// Serde field adapter for an optional pair of canonical dates.
pub mod optional_date_pair {
    use super::{Date, DateWire, Deserialize, Serialize};

    /// Serialize an optional domain-date pair through canonical wire dates.
    ///
    /// # Arguments
    ///
    /// * `value` - Optional `(start, end)` pair to encode as ISO date strings or JSON `null`.
    /// * `serializer` - Serde serializer that receives the canonical optional date pair.
    pub fn serialize<S>(value: &Option<(Date, Date)>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value
            .map(|(start, end)| (DateWire::from(start), DateWire::from(end)))
            .serialize(serializer)
    }

    /// Deserialize an optional canonical date pair into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying two ISO date strings or JSON `null`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<(Date, Date)>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<(DateWire, DateWire)>::deserialize(deserializer)
            .map(|value| value.map(|(start, end)| (Date::from(start), Date::from(end))))
    }
}

/// Serde field adapter for a vector of canonical dates.
pub mod dates {
    use super::{Date, DateWire, Deserialize, Serialize};

    /// Serialize domain dates through canonical wire dates.
    ///
    /// # Arguments
    ///
    /// * `value` - Ordered calendar dates to encode as ISO `YYYY-MM-DD` strings.
    /// * `serializer` - Serde serializer that receives the canonical date array.
    pub fn serialize<S>(value: &[Date], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value
            .iter()
            .copied()
            .map(DateWire::from)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    /// Deserialize canonical wire dates into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying an array of ISO date strings.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Date>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<DateWire>::deserialize(deserializer)
            .map(|values| values.into_iter().map(Into::into).collect())
    }
}

/// Serde field adapter for an optional vector of canonical dates.
pub mod optional_dates {
    use super::{Date, DateWire, Deserialize, Serialize};

    /// Serialize optional domain dates through canonical wire dates.
    ///
    /// # Arguments
    ///
    /// * `value` - Optional ordered dates to encode as ISO strings or JSON `null`.
    /// * `serializer` - Serde serializer that receives the canonical optional date array.
    pub fn serialize<S>(value: &Option<Vec<Date>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value
            .as_ref()
            .map(|values| {
                values
                    .iter()
                    .copied()
                    .map(DateWire::from)
                    .collect::<Vec<_>>()
            })
            .serialize(serializer)
    }

    /// Deserialize optional canonical wire dates into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying an ISO date array or JSON `null`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<Date>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<Vec<DateWire>>::deserialize(deserializer)
            .map(|values| values.map(|values| values.into_iter().map(Into::into).collect()))
    }
}

/// Serde field adapter for dated floating-point values.
pub mod dated_f64_values {
    use super::{Date, DateWire, Deserialize, Serialize};

    /// Serialize dated values through canonical wire dates.
    ///
    /// # Arguments
    ///
    /// * `value` - Ordered `(date, value)` observations whose dates use ISO strings.
    /// * `serializer` - Serde serializer that receives the canonical dated-value array.
    pub fn serialize<S>(value: &[(Date, f64)], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value
            .iter()
            .map(|(date, item)| (DateWire::from(*date), *item))
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    /// Deserialize canonical dated values into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying `(ISO date, number)` observations.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<(Date, f64)>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<(DateWire, f64)>::deserialize(deserializer).map(|values| {
            values
                .into_iter()
                .map(|(date, item)| (date.into(), item))
                .collect()
        })
    }
}

/// Serde field adapter for optional dated floating-point values.
pub mod optional_dated_f64_values {
    use super::{Date, DateWire, Deserialize, Serialize};

    /// Serialize optional dated values through canonical wire dates.
    ///
    /// # Arguments
    ///
    /// * `value` - Optional `(date, value)` observations to encode, or `None` for JSON `null`.
    /// * `serializer` - Serde serializer that receives the canonical optional observations.
    pub fn serialize<S>(value: &Option<Vec<(Date, f64)>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value
            .as_ref()
            .map(|values| {
                values
                    .iter()
                    .map(|(date, item)| (DateWire::from(*date), *item))
                    .collect::<Vec<_>>()
            })
            .serialize(serializer)
    }

    /// Deserialize optional canonical dated values into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying dated numbers or JSON `null`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<(Date, f64)>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<Vec<(DateWire, f64)>>::deserialize(deserializer).map(|values| {
            values.map(|values| {
                values
                    .into_iter()
                    .map(|(date, item)| (date.into(), item))
                    .collect()
            })
        })
    }
}

/// Serde field adapter for dated monetary values.
pub mod dated_money_values {
    use super::{Date, DateWire, Deserialize, Serialize};
    use crate::money::Money;

    /// Serialize dated monetary values through canonical wire dates.
    ///
    /// # Arguments
    ///
    /// * `value` - Ordered `(date, money)` observations, preserving each amount's currency.
    /// * `serializer` - Serde serializer that receives the canonical dated-money array.
    pub fn serialize<S>(value: &[(Date, Money)], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value
            .iter()
            .map(|(date, money)| (DateWire::from(*date), *money))
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    /// Deserialize canonical dated monetary values into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying `(ISO date, money)` observations.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<(Date, Money)>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<(DateWire, Money)>::deserialize(deserializer).map(|values| {
            values
                .into_iter()
                .map(|(date, money)| (date.into(), money))
                .collect()
        })
    }
}

/// Serde field adapter for an optional dated monetary value.
pub mod optional_dated_money {
    use super::{Date, DateWire, Deserialize, Serialize};
    use crate::money::Money;

    /// Serialize an optional dated monetary value through a canonical wire date.
    ///
    /// # Arguments
    ///
    /// * `value` - Optional `(date, money)` value, preserving the amount's currency.
    /// * `serializer` - Serde serializer that receives the canonical value or JSON `null`.
    pub fn serialize<S>(value: &Option<(Date, Money)>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value
            .map(|(date, money)| (DateWire::from(date), money))
            .serialize(serializer)
    }

    /// Deserialize an optional canonical dated monetary value into domain storage.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying dated money or JSON `null`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<(Date, Money)>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<(DateWire, Money)>::deserialize(deserializer)
            .map(|value| value.map(|(date, money)| (date.into(), money)))
    }
}

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

/// Declare a `#[serde(transparent)]` `f64` newtype whose deserializer and
/// generated schema enforce the same finite-range contract.
///
/// `$field_attr` is the `schemars` range attribute applied to the inner
/// field; `$check` is the runtime predicate over `$v` (finiteness is checked
/// first); `$msg` prefixes the validation error (`"<msg>, got <value>"`).
macro_rules! bounded_f64_wire {
    (
        $(#[$doc:meta])*
        $name:ident,
        field: #[$field_attr:meta],
        valid: |$v:ident| $check:expr,
        message: $msg:literal $(,)?
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Serialize)]
        #[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
        #[serde(transparent)]
        #[cfg_attr(feature = "json-schema", schemars(transparent))]
        pub struct $name(#[$field_attr] f64);

        impl $name {
            /// Return the validated primitive value.
            pub const fn into_inner(self) -> f64 {
                self.0
            }
        }

        impl TryFrom<f64> for $name {
            type Error = crate::Error;

            fn try_from($v: f64) -> Result<Self, Self::Error> {
                if $v.is_finite() && $check {
                    Ok(Self($v))
                } else {
                    Err(crate::Error::Validation(format!(
                        concat!($msg, ", got {}"),
                        $v
                    )))
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                Self::try_from(f64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

bounded_f64_wire! {
    /// Finite JSON number that is strictly greater than zero.
    ///
    /// This type is used by serde field adapters so runtime deserialization and
    /// generated schemas enforce the same positive-number contract.
    PositiveF64Wire,
    field: #[cfg_attr(feature = "json-schema", schemars(extend("exclusiveMinimum" = 0.0)))],
    valid: |value| value > 0.0,
    message: "expected a finite number greater than zero",
}

bounded_f64_wire! {
    /// Finite JSON number greater than or equal to zero.
    NonNegativeF64Wire,
    field: #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0)))],
    valid: |value| value >= 0.0,
    message: "expected a finite non-negative number",
}

bounded_f64_wire! {
    /// Finite JSON number in the closed interval `[0, 1]`.
    ClosedUnitIntervalF64Wire,
    field: #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0, max = 1.0)))],
    valid: |value| (0.0..=1.0).contains(&value),
    message: "expected a finite number in [0, 1]",
}

bounded_f64_wire! {
    /// Finite JSON number in the open interval `(0, 1)`.
    OpenUnitIntervalF64Wire,
    field: #[cfg_attr(
        feature = "json-schema",
        schemars(extend("exclusiveMinimum" = 0.0, "exclusiveMaximum" = 1.0))
    )],
    valid: |value| value > 0.0 && value < 1.0,
    message: "expected a finite number strictly between zero and one",
}

bounded_f64_wire! {
    /// Finite correlation coefficient in the closed interval `[-1, 1]`.
    CorrelationWire,
    field: #[cfg_attr(feature = "json-schema", schemars(range(min = -1.0, max = 1.0)))],
    valid: |value| (-1.0..=1.0).contains(&value),
    message: "expected a finite correlation in [-1, 1]",
}

bounded_f64_wire! {
    /// Finite percentage-position quantity in the closed interval `[-100, 100]`.
    PercentageQuantityWire,
    field: #[cfg_attr(feature = "json-schema", schemars(range(min = -100.0, max = 100.0)))],
    valid: |value| (-100.0..=100.0).contains(&value),
    message: "expected a finite percentage quantity in [-100, 100]",
}

/// Deserialize a finite, strictly positive `f64` through [`PositiveF64Wire`].
///
/// # Arguments
///
/// * `deserializer` - Serde input containing a JSON number greater than zero.
pub fn deserialize_positive_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    PositiveF64Wire::deserialize(deserializer).map(PositiveF64Wire::into_inner)
}

/// Serialize a finite, strictly positive `f64` through [`PositiveF64Wire`].
///
/// # Arguments
///
/// * `value` - Number that must be finite and greater than zero.
/// * `serializer` - Serde destination for the validated number.
pub fn serialize_positive_f64<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    PositiveF64Wire::try_from(*value)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

/// Deserialize a finite, non-negative `f64` through [`NonNegativeF64Wire`].
///
/// # Arguments
///
/// * `deserializer` - Serde input containing a JSON number at least zero.
pub fn deserialize_non_negative_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    NonNegativeF64Wire::deserialize(deserializer).map(NonNegativeF64Wire::into_inner)
}

/// Serialize a finite, non-negative `f64` through [`NonNegativeF64Wire`].
///
/// # Arguments
///
/// * `value` - Number that must be finite and at least zero.
/// * `serializer` - Serde destination for the validated number.
pub fn serialize_non_negative_f64<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    NonNegativeF64Wire::try_from(*value)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

/// Deserialize a finite `f64` in the closed interval `[0, 1]`.
///
/// # Arguments
///
/// * `deserializer` - Serde input containing a JSON number in `[0, 1]`.
pub fn deserialize_closed_unit_interval_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    ClosedUnitIntervalF64Wire::deserialize(deserializer).map(ClosedUnitIntervalF64Wire::into_inner)
}

/// Serialize a finite `f64` in the closed interval `[0, 1]`.
///
/// # Arguments
///
/// * `value` - Number that must lie in `[0, 1]`.
/// * `serializer` - Serde destination for the validated number.
pub fn serialize_closed_unit_interval_f64<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    ClosedUnitIntervalF64Wire::try_from(*value)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

/// Deserialize a finite probability in the open interval `(0, 1)`.
///
/// # Arguments
///
/// * `deserializer` - Serde input containing a JSON number in `(0, 1)`.
pub fn deserialize_open_unit_interval_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    OpenUnitIntervalF64Wire::deserialize(deserializer).map(OpenUnitIntervalF64Wire::into_inner)
}

/// Serialize a finite probability in the open interval `(0, 1)`.
///
/// # Arguments
///
/// * `value` - Probability that must lie strictly between zero and one.
/// * `serializer` - Serde destination for the validated probability.
pub fn serialize_open_unit_interval_f64<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    OpenUnitIntervalF64Wire::try_from(*value)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

/// Deserialize a finite correlation coefficient in `[-1, 1]`.
///
/// # Arguments
///
/// * `deserializer` - Serde input containing a JSON number in `[-1, 1]`.
pub fn deserialize_correlation<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    CorrelationWire::deserialize(deserializer).map(CorrelationWire::into_inner)
}

/// Serialize a finite correlation coefficient in `[-1, 1]`.
///
/// # Arguments
///
/// * `value` - Correlation coefficient in the closed interval `[-1, 1]`.
/// * `serializer` - Serde destination for the validated coefficient.
pub fn serialize_correlation<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    CorrelationWire::try_from(*value)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

/// Deserialize an optional finite, strictly positive `f64`.
///
/// # Arguments
///
/// * `deserializer` - Serde input containing a positive number or JSON `null`.
pub fn deserialize_optional_positive_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<PositiveF64Wire>::deserialize(deserializer)
        .map(|value| value.map(PositiveF64Wire::into_inner))
}

/// Serialize an optional finite, strictly positive `f64`.
///
/// # Arguments
///
/// * `value` - Optional positive number; `None` is serialized as JSON `null`.
/// * `serializer` - Serde destination for the validated optional number.
pub fn serialize_optional_positive_f64<S>(
    value: &Option<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    value
        .map(PositiveF64Wire::try_from)
        .transpose()
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct NonFiniteHolder {
        #[serde(with = "super::non_finite_f64")]
        value: f64,
    }

    #[test]
    fn non_finite_f64_round_trips_infinities() {
        // serde_json writes these as `null` and then refuses to read `null`
        // back as an f64, so a ratio documented to reach `+inf` used to lose
        // its value on to_json/from_json.
        for (value, expected_json) in [
            (f64::INFINITY, json!({"value": "inf"})),
            (f64::NEG_INFINITY, json!({"value": "-inf"})),
        ] {
            let holder = NonFiniteHolder { value };
            let encoded = serde_json::to_value(&holder).expect("serialize");
            assert_eq!(encoded, expected_json);
            let decoded: NonFiniteHolder = serde_json::from_value(encoded).expect("deserialize");
            assert_eq!(decoded.value, value);
        }
    }

    #[test]
    fn non_finite_f64_round_trips_nan_and_finite() {
        let nan: NonFiniteHolder = serde_json::from_value(
            serde_json::to_value(NonFiniteHolder { value: f64::NAN }).unwrap(),
        )
        .expect("nan");
        assert!(nan.value.is_nan());

        let finite: NonFiniteHolder =
            serde_json::from_value(json!({"value": 1.5})).expect("finite");
        assert_eq!(finite.value, 1.5);
        // Finite values stay ordinary JSON numbers, so the wire format is
        // unchanged for every payload that never hit a zero denominator.
        assert_eq!(
            serde_json::to_value(NonFiniteHolder { value: 1.5 }).unwrap(),
            json!({"value": 1.5})
        );
    }

    #[test]
    fn non_finite_f64_rejects_null() {
        // `null` is not a value this adapter ever emits; accepting it would
        // silently turn a missing field into NaN.
        assert!(serde_json::from_value::<NonFiniteHolder>(json!({"value": null})).is_err());
    }

    #[test]
    fn non_finite_f64_rejects_unknown_sentinel() {
        assert!(serde_json::from_value::<NonFiniteHolder>(json!({"value": "huge"})).is_err());
    }

    #[test]
    fn parse_sentinel_inverts_serialize() {
        for value in [f64::INFINITY, f64::NEG_INFINITY] {
            let encoded = serde_json::to_value(NonFiniteHolder { value }).expect("serialize");
            let text = encoded["value"].as_str().expect("sentinel string");
            assert_eq!(non_finite_f64::parse_sentinel(text), Some(value));
        }
        assert!(non_finite_f64::parse_sentinel(" NaN ").is_some_and(f64::is_nan));
        assert_eq!(non_finite_f64::parse_sentinel("huge"), None);
    }

    #[test]
    fn date_schema_has_date_format() {
        let schema = serde_json::to_value(schemars::schema_for!(DateWire)).expect("schema");
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["format"], "date");
    }

    #[test]
    fn non_finite_wire_schema_lists_exactly_the_serialized_sentinels() {
        let schema = serde_json::to_value(schemars::schema_for!(NonFiniteF64Wire)).expect("schema");
        let sentinel = &schema["$defs"]["NonFiniteSentinel"];
        let mut spellings: Vec<String> = sentinel["oneOf"]
            .as_array()
            .map(|variants| {
                variants
                    .iter()
                    .map(|variant| variant["const"].as_str().expect("const").to_string())
                    .collect()
            })
            .or_else(|| {
                sentinel["enum"].as_array().map(|values| {
                    values
                        .iter()
                        .map(|value| value.as_str().expect("string").to_string())
                        .collect()
                })
            })
            .expect("sentinel spellings");
        spellings.sort();
        assert_eq!(spellings, ["-inf", "inf", "nan"]);
        for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let wire = serde_json::to_value(NonFiniteF64Wire::from(value)).expect("serialize");
            let adapter = serde_json::to_value(NonFiniteHolder { value }).expect("serialize");
            assert_eq!(adapter["value"], wire, "adapter and wire type agree");
            assert!(spellings.iter().any(|spelling| wire == json!(spelling)));
        }
        assert_eq!(
            serde_json::to_value(NonFiniteF64Wire::from(1.5)).expect("serialize"),
            json!(1.5)
        );
    }

    #[test]
    fn decimal_wire_is_string_only() {
        let decimal: DecimalWire = serde_json::from_value(json!("-1.25e+2")).expect("decimal");
        assert_eq!(decimal.0, Decimal::new(-125, 0));
        assert!(serde_json::from_value::<DecimalWire>(json!(1.25)).is_err());

        let schema = serde_json::to_value(schemars::schema_for!(DecimalWire)).expect("schema");
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["pattern"], r"^-?\d+(\.\d+)?([eE][+-]?\d+)?$");
    }

    #[test]
    fn schema_version_accepts_only_numeric_one() {
        assert_eq!(
            serde_json::from_value::<SchemaVersion>(json!(1)).expect("v1"),
            SchemaVersion::CURRENT
        );
        assert!(serde_json::from_value::<SchemaVersion>(json!(0)).is_err());
        assert!(serde_json::from_value::<SchemaVersion>(json!(2)).is_err());
        assert!(serde_json::from_value::<SchemaVersion>(json!("1")).is_err());
        assert_eq!(
            serde_json::to_value(SchemaVersion::CURRENT).expect("serialize v1"),
            json!(1)
        );

        let schema = serde_json::to_value(schemars::schema_for!(SchemaVersion)).expect("schema");
        assert_eq!(schema["minimum"], 1);
        assert_eq!(schema["maximum"], 1);
    }

    #[test]
    fn bounded_numeric_wires_match_runtime_contracts() {
        for invalid in [json!(0.0), json!(-1.0)] {
            assert!(serde_json::from_value::<PositiveF64Wire>(invalid).is_err());
        }
        assert_eq!(
            serde_json::from_value::<PositiveF64Wire>(json!(0.5))
                .expect("positive")
                .into_inner(),
            0.5
        );

        for invalid in [json!(0.0), json!(1.0), json!(-0.1), json!(1.1)] {
            assert!(serde_json::from_value::<OpenUnitIntervalF64Wire>(invalid).is_err());
        }
        for invalid in [json!(-0.1), json!(1.1)] {
            assert!(serde_json::from_value::<ClosedUnitIntervalF64Wire>(invalid).is_err());
        }
        assert!(serde_json::from_value::<NonNegativeF64Wire>(json!(-0.1)).is_err());
        for invalid in [json!(-1.1), json!(1.1)] {
            assert!(serde_json::from_value::<CorrelationWire>(invalid).is_err());
        }
        for invalid in [json!(-100.1), json!(100.1)] {
            assert!(serde_json::from_value::<PercentageQuantityWire>(invalid).is_err());
        }

        let positive =
            serde_json::to_value(schemars::schema_for!(PositiveF64Wire)).expect("positive schema");
        assert_eq!(positive["exclusiveMinimum"], 0.0);
        let probability = serde_json::to_value(schemars::schema_for!(OpenUnitIntervalF64Wire))
            .expect("probability schema");
        assert_eq!(probability["exclusiveMinimum"], 0.0);
        assert_eq!(probability["exclusiveMaximum"], 1.0);
        let closed_probability =
            serde_json::to_value(schemars::schema_for!(ClosedUnitIntervalF64Wire))
                .expect("closed probability schema");
        assert_eq!(closed_probability["minimum"], 0.0);
        assert_eq!(closed_probability["maximum"], 1.0);
        let non_negative = serde_json::to_value(schemars::schema_for!(NonNegativeF64Wire))
            .expect("non-negative schema");
        assert_eq!(non_negative["minimum"], 0.0);
        let correlation = serde_json::to_value(schemars::schema_for!(CorrelationWire))
            .expect("correlation schema");
        assert_eq!(correlation["minimum"], -1.0);
        assert_eq!(correlation["maximum"], 1.0);
        let percentage = serde_json::to_value(schemars::schema_for!(PercentageQuantityWire))
            .expect("percentage schema");
        assert_eq!(percentage["minimum"], -100.0);
        assert_eq!(percentage["maximum"], 100.0);
    }
}

/// Serde field adapter for an `f64` that may legitimately be non-finite.
///
/// `serde_json` writes `f64::INFINITY` and `f64::NAN` as JSON `null` and then
/// refuses to read `null` back as an `f64`, so a field documented to reach
/// `+∞` silently loses its value on a `to_json` / `from_json` round trip.
/// Ratio metrics hit this whenever a denominator is zero — a profit factor
/// with wins and no losses is `+∞`, not missing data.
///
/// This adapter encodes non-finite values as the strings `"inf"`, `"-inf"`,
/// and `"nan"`, leaving finite values as ordinary JSON numbers.
///
/// # Examples
/// ```rust
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct Ratios {
///     #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
///     profit_factor: f64,
/// }
///
/// let json = serde_json::to_string(&Ratios { profit_factor: f64::INFINITY })?;
/// assert_eq!(json, r#"{"profit_factor":"inf"}"#);
/// assert_eq!(serde_json::from_str::<Ratios>(&json)?.profit_factor, f64::INFINITY);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub mod non_finite_f64 {
    use serde::{Deserialize, Serialize};

    /// Wire form: a number when finite, a sentinel string otherwise.
    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Number(f64),
        Sentinel(String),
    }

    /// Serialize an `f64`, encoding non-finite values as sentinel strings.
    ///
    /// # Arguments
    ///
    /// * `value` - Value to encode; `±∞` and `NaN` become strings.
    /// * `serializer` - Serde serializer receiving the number or sentinel.
    ///
    /// # Errors
    ///
    /// Propagates any failure from the underlying serializer.
    pub fn serialize<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if value.is_finite() {
            return Wire::Number(*value).serialize(serializer);
        }
        let sentinel = if value.is_nan() {
            "nan"
        } else if value.is_sign_positive() {
            "inf"
        } else {
            "-inf"
        };
        Wire::Sentinel(sentinel.to_string()).serialize(serializer)
    }

    /// Deserialize an `f64` that may arrive as a sentinel string.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying a number or sentinel.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is `null`, or is neither a number nor a
    /// recognized sentinel (`"inf"`, `"+inf"`, `"-inf"`, `"infinity"`, `"nan"`).
    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match Wire::deserialize(deserializer)? {
            Wire::Number(value) => Ok(value),
            Wire::Sentinel(text) => parse_sentinel(&text).ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "expected a number or one of \"inf\", \"-inf\", \"nan\"; got {:?}",
                    text.trim().to_ascii_lowercase()
                ))
            }),
        }
    }

    /// Decode one sentinel string written by [`serialize`].
    ///
    /// Hosts whose number type represents `±∞` and `NaN` natively (JavaScript)
    /// use this to turn a serialized sentinel back into a number, so the
    /// sentinel vocabulary has a single owner.
    ///
    /// # Arguments
    ///
    /// * `text` - Candidate sentinel. Matching ignores surrounding whitespace
    ///   and ASCII case; `"inf"`, `"+inf"`, `"infinity"` and `"+infinity"` map
    ///   to `+∞`, `"-inf"` and `"-infinity"` to `-∞`, and `"nan"` to `NaN`.
    ///
    /// # Returns
    ///
    /// The decoded value, or `None` when `text` is not a recognized sentinel.
    #[must_use]
    pub fn parse_sentinel(text: &str) -> Option<f64> {
        match text.trim().to_ascii_lowercase().as_str() {
            "inf" | "+inf" | "infinity" | "+infinity" => Some(f64::INFINITY),
            "-inf" | "-infinity" => Some(f64::NEG_INFINITY),
            "nan" => Some(f64::NAN),
            _ => None,
        }
    }
}

/// An `f64` in the [`non_finite_f64`] wire form: a JSON number when finite,
/// otherwise one of the strings `"inf"`, `"-inf"` or `"nan"`.
///
/// Contract types store plain `f64` fields with
/// `#[serde(with = "finstack_quant_core::wire::non_finite_f64")]` and name this
/// type in `#[schemars(with = ...)]`, so the generated schema describes the
/// sentinel strings the serializer actually writes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum NonFiniteF64Wire {
    /// A finite value, written as a JSON number.
    Finite(f64),
    /// A non-finite value, written as its sentinel string.
    NonFinite(NonFiniteSentinel),
}

/// Sentinel string the [`non_finite_f64`] adapter writes for a non-finite `f64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum NonFiniteSentinel {
    /// `+∞`, written as `"inf"`.
    Inf,
    /// `-∞`, written as `"-inf"`.
    #[serde(rename = "-inf")]
    NegInf,
    /// `NaN`, written as `"nan"`.
    Nan,
}

impl From<f64> for NonFiniteF64Wire {
    fn from(value: f64) -> Self {
        if value.is_nan() {
            Self::NonFinite(NonFiniteSentinel::Nan)
        } else if value == f64::INFINITY {
            Self::NonFinite(NonFiniteSentinel::Inf)
        } else if value == f64::NEG_INFINITY {
            Self::NonFinite(NonFiniteSentinel::NegInf)
        } else {
            Self::Finite(value)
        }
    }
}

/// Serde adapter for a `Vec<f64>` whose elements may be `±∞` or `NaN`.
///
/// Each element uses the [`non_finite_f64`] wire form (a number when finite, a
/// sentinel string otherwise), so a series with undefined points survives a
/// JSON round trip instead of turning into `null`s that cannot be read back.
///
/// # Examples
///
/// ```rust
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Series {
///     #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
///     values: Vec<f64>,
/// }
///
/// let json = serde_json::to_string(&Series { values: vec![1.5, f64::NAN] })?;
/// assert_eq!(json, r#"{"values":[1.5,"nan"]}"#);
/// let back: Series = serde_json::from_str(&json)?;
/// assert!(back.values[1].is_nan());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub mod non_finite_f64_seq {
    use serde::{Deserialize, Serialize};

    /// One element in the [`super::non_finite_f64`] wire form.
    #[derive(Serialize, Deserialize)]
    #[serde(transparent)]
    struct Element(#[serde(with = "super::non_finite_f64")] f64);

    /// Serialize a slice of `f64`, encoding non-finite elements as sentinels.
    ///
    /// # Arguments
    ///
    /// * `values` - Elements to encode in order; `±∞` and `NaN` become strings.
    /// * `serializer` - Serde serializer receiving the sequence.
    ///
    /// # Errors
    ///
    /// Propagates any failure from the underlying serializer.
    pub fn serialize<S>(values: &[f64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(values.iter().map(|value| Element(*value)))
    }

    /// Deserialize a sequence whose elements may be sentinel strings.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying the sequence.
    ///
    /// # Errors
    ///
    /// Returns an error when an element is `null` or is neither a number nor a
    /// recognized sentinel (see [`super::non_finite_f64::parse_sentinel`]).
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<f64>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Vec::<Element>::deserialize(deserializer)?
            .into_iter()
            .map(|Element(value)| value)
            .collect())
    }
}

/// Names the top-level fields of a result type that serialize through
/// [`non_finite_f64`].
///
/// Those fields reach JSON as sentinel strings when they are `±∞` or `NaN`.
/// Hosts with a native non-finite number type (JavaScript) read this list to
/// restore the numbers after serialization, instead of keeping their own copy
/// of the field names. Every implementation carries a unit test that fills
/// each `f64` field with `NaN` and checks that exactly these keys come out as
/// sentinel strings, so the list cannot drift from the serde attributes.
pub trait NonFiniteFields {
    /// Serialized names of the fields annotated with
    /// `#[serde(with = "finstack_quant_core::wire::non_finite_f64")]`.
    const NON_FINITE_FIELDS: &'static [&'static str];
}

/// Serde adapter for a count that hosts must receive as an ordinary number.
///
/// Result payloads that embed a Monte Carlo `ValuationResult` are serialized
/// for JavaScript with 64-bit integers mapped to `BigInt`, because MC seeds
/// span the full `u64` range. A `usize` count next to such a result would
/// otherwise also arrive as a `BigInt`. This adapter writes the count as a
/// `u32`, which every serializer emits as a plain number; the JSON text is
/// unchanged. Deserialization accepts any unsigned integer that fits `usize`.
///
/// # Examples
///
/// ```rust
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Summary {
///     #[serde(with = "finstack_quant_core::wire::count")]
///     num_steps: usize,
/// }
///
/// let json = serde_json::to_string(&Summary { num_steps: 3 })?;
/// assert_eq!(json, r#"{"num_steps":3}"#);
/// assert_eq!(serde_json::from_str::<Summary>(&json)?.num_steps, 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub mod count {
    use serde::Deserialize;

    /// Serialize a `usize` count as a `u32`.
    ///
    /// # Arguments
    ///
    /// * `value` - Non-negative count to encode; must not exceed `u32::MAX`.
    /// * `serializer` - Serde serializer receiving the 32-bit count.
    ///
    /// # Errors
    ///
    /// Returns a serializer error when `value` exceeds `u32::MAX`.
    pub fn serialize<S>(value: &usize, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = u32::try_from(*value).map_err(serde::ser::Error::custom)?;
        serializer.serialize_u32(value)
    }

    /// Deserialize a `usize` count.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying a non-negative integer.
    ///
    /// # Errors
    ///
    /// Returns a deserializer error when the value is not a non-negative
    /// integer that fits `usize`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<usize, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        usize::deserialize(deserializer)
    }
}

/// Serde adapter for a list of counts or indices that hosts must receive as
/// ordinary numbers.
///
/// The list form of [`count`]: each element is written as a `u32` so that a
/// BigInt-preserving JavaScript serializer still emits plain numbers.
pub mod counts {
    use serde::ser::SerializeSeq;
    use serde::Deserialize;

    /// Serialize a list of `usize` values as `u32` elements.
    ///
    /// # Arguments
    ///
    /// * `values` - Non-negative counts or indices; each must not exceed `u32::MAX`.
    /// * `serializer` - Serde serializer receiving the sequence.
    ///
    /// # Errors
    ///
    /// Returns a serializer error when any element exceeds `u32::MAX`.
    pub fn serialize<S>(values: &[usize], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(values.len()))?;
        for value in values {
            let value = u32::try_from(*value).map_err(serde::ser::Error::custom)?;
            seq.serialize_element(&value)?;
        }
        seq.end()
    }

    /// Deserialize a list of `usize` values.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying a sequence of
    ///   non-negative integers.
    ///
    /// # Errors
    ///
    /// Returns a deserializer error when the value is not a sequence of
    /// non-negative integers that fit `usize`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<usize>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<usize>::deserialize(deserializer)
    }
}

/// Serde adapter for a signed count (for example a day span) that hosts must
/// receive as an ordinary number.
///
/// The signed form of [`count`]: the value is written as an `i32` so that a
/// BigInt-preserving JavaScript serializer still emits a plain number.
pub mod signed_count {
    use serde::Deserialize;

    /// Serialize an `i64` count as an `i32`.
    ///
    /// # Arguments
    ///
    /// * `value` - Signed count to encode; must lie within the `i32` range.
    /// * `serializer` - Serde serializer receiving the 32-bit count.
    ///
    /// # Errors
    ///
    /// Returns a serializer error when `value` lies outside the `i32` range.
    pub fn serialize<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = i32::try_from(*value).map_err(serde::ser::Error::custom)?;
        serializer.serialize_i32(value)
    }

    /// Deserialize an `i64` count.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying an integer.
    ///
    /// # Errors
    ///
    /// Returns a deserializer error when the value is not an integer that
    /// fits `i64`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        i64::deserialize(deserializer)
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
