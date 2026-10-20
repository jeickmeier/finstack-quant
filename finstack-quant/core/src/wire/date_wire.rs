//! Date wire form: the `schemars` stand-in for `time::Date` fields.
//!
//! `time::Date` already serializes as an ISO `YYYY-MM-DD` string (core enables
//! `time`'s `serde-human-readable`), so date fields use plain serde and name
//! [`DateWire`] only in `#[schemars(with = …)]`.

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
