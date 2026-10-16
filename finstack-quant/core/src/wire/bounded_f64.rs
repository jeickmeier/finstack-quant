//! Bounded `f64` newtypes (positive, non-negative, unit intervals, correlation, percentage) and their field adapters.

use serde::{Deserialize, Serialize};

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
