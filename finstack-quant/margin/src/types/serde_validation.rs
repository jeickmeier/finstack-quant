//! Shared serde validators for numeric schema constraints.

use serde::de::Error as DeError;
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn validate_finite_range(
    value: f64,
    minimum: f64,
    maximum: Option<f64>,
    field: &str,
) -> Result<(), String> {
    let in_range =
        value.is_finite() && value >= minimum && maximum.is_none_or(|maximum| value <= maximum);
    if in_range {
        Ok(())
    } else {
        let expected = maximum.map_or_else(
            || format!("at least {minimum}"),
            |maximum| format!("between {minimum} and {maximum} inclusive"),
        );
        Err(format!("{field} must be finite and {expected}"))
    }
}

fn validate_positive_u32(value: u32, field: &str) -> Result<(), String> {
    (value >= 1)
        .then_some(())
        .ok_or_else(|| format!("{field} must be at least 1"))
}

fn validate_maximum_u8(value: u8, maximum: u8, field: &str) -> Result<(), String> {
    (value <= maximum)
        .then_some(())
        .ok_or_else(|| format!("{field} must be between 0 and {maximum} inclusive"))
}

macro_rules! optional_f64_field {
    ($module:ident, $field:literal, $minimum:expr_2021, $maximum:expr_2021) => {
        pub(super) mod $module {
            use super::*;

            pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = Option::<f64>::deserialize(deserializer)?;
                if let Some(value) = value {
                    validate_finite_range(value, $minimum, $maximum, $field)
                        .map_err(D::Error::custom)?;
                }
                Ok(value)
            }

            pub(crate) fn serialize<S>(
                value: &Option<f64>,
                serializer: S,
            ) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                if let Some(value) = value {
                    validate_finite_range(*value, $minimum, $maximum, $field)
                        .map_err(S::Error::custom)?;
                }
                value.serialize(serializer)
            }
        }
    };
}

macro_rules! f64_field {
    ($module:ident, $field:literal, $minimum:expr_2021, $maximum:expr_2021) => {
        pub(super) mod $module {
            use super::*;

            pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = f64::deserialize(deserializer)?;
                validate_finite_range(value, $minimum, $maximum, $field)
                    .map_err(D::Error::custom)?;
                Ok(value)
            }

            pub(crate) fn serialize<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                validate_finite_range(*value, $minimum, $maximum, $field)
                    .map_err(S::Error::custom)?;
                value.serialize(serializer)
            }
        }
    };
}

optional_f64_field!(min_remaining_years, "min_remaining_years", 0.0, None);
optional_f64_field!(max_remaining_years, "max_remaining_years", 0.0, None);
f64_field!(haircut, "haircut", 0.0, Some(1.0));
f64_field!(fx_haircut_addon, "fx_haircut_addon", 0.0, Some(1.0));
optional_f64_field!(concentration_limit, "concentration_limit", 0.0, Some(1.0));
optional_f64_field!(default_haircut, "default_haircut", 0.0, Some(1.0));

pub(super) mod mpor_days {
    use super::*;

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<u32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        validate_positive_u32(value, "mpor_days").map_err(D::Error::custom)?;
        Ok(value)
    }

    pub(crate) fn serialize<S>(value: &u32, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        validate_positive_u32(*value, "mpor_days").map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

pub(super) mod notification_deadline_hours {
    use super::*;

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<u8, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        validate_maximum_u8(value, 23, "notification_deadline_hours").map_err(D::Error::custom)?;
        Ok(value)
    }

    pub(crate) fn serialize<S>(value: &u8, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        validate_maximum_u8(*value, 23, "notification_deadline_hours").map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}
