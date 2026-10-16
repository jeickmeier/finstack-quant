//! Date wire form and the `#[serde(with = …)]` adapters for date-keyed fields.

use serde::{Deserialize, Serialize};
use time::Date;

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
