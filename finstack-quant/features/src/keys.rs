//! Canonical text for timestamp entries of key columns.
//!
//! Key columns (`entity`, `order`, `time_key`, `groups`) are opaque strings
//! ordered lexicographically. A timestamp only sorts chronologically as text
//! when every entry uses one timezone and one fixed precision, so hosts that
//! accept native timestamp objects format them here instead of inventing
//! their own spelling.

use finstack_quant_core::dates::{OffsetDateTime, PrimitiveDateTime};

/// Order key for a timezone-naive timestamp: wall time at nanosecond precision.
///
/// Produces `YYYY-MM-DDTHH:MM:SS.fffffffff` (always nine fractional digits),
/// so naive keys of one column sort chronologically as strings.
///
/// # Arguments
///
/// * `datetime` - Naive wall-clock timestamp; no timezone is assumed or
///   applied.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Date, Month, PrimitiveDateTime};
/// use finstack_quant_features::naive_datetime_order_key;
///
/// let date = Date::from_calendar_date(2025, Month::January, 2)?;
/// let at = PrimitiveDateTime::new(date, time::Time::from_hms(9, 30, 0)?);
/// assert_eq!(naive_datetime_order_key(at), "2025-01-02T09:30:00.000000000");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn naive_datetime_order_key(datetime: PrimitiveDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}",
        datetime.year(),
        u8::from(datetime.month()),
        datetime.day(),
        datetime.hour(),
        datetime.minute(),
        datetime.second(),
        datetime.nanosecond()
    )
}

/// Order key for a timezone-aware timestamp: the UTC instant at nanosecond
/// precision with a `+00:00` suffix.
///
/// Aware timestamps in any offset normalize to UTC, so equal instants give
/// equal keys and keys sort chronologically as strings. Do not mix aware and
/// naive keys in one column: they do not order against each other.
///
/// # Arguments
///
/// * `datetime` - Timezone-aware timestamp; converted to UTC before
///   formatting.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Date, Month, PrimitiveDateTime};
/// use finstack_quant_features::datetime_order_key;
///
/// let date = Date::from_calendar_date(2025, Month::January, 2)?;
/// let wall = PrimitiveDateTime::new(date, time::Time::from_hms(9, 30, 0)?);
/// let at = wall.assume_offset(time::UtcOffset::from_hms(1, 0, 0)?);
/// assert_eq!(datetime_order_key(at), "2025-01-02T08:30:00.000000000+00:00");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn datetime_order_key(datetime: OffsetDateTime) -> String {
    let utc = datetime.to_offset(time::UtcOffset::UTC);
    format!(
        "{}+00:00",
        naive_datetime_order_key(PrimitiveDateTime::new(utc.date(), utc.time()))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Date, Month};

    fn wall(hour: u8, nanos: u32) -> PrimitiveDateTime {
        let date = Date::from_calendar_date(2025, Month::March, 9).unwrap();
        PrimitiveDateTime::new(date, time::Time::from_hms_nano(hour, 5, 7, nanos).unwrap())
    }

    #[test]
    fn naive_keys_use_fixed_nanosecond_precision() {
        assert_eq!(
            naive_datetime_order_key(wall(1, 1_000)),
            "2025-03-09T01:05:07.000001000"
        );
        assert!(naive_datetime_order_key(wall(1, 999)) < naive_datetime_order_key(wall(1, 1_000)));
    }

    #[test]
    fn aware_keys_normalize_to_utc() {
        let plus_two = wall(3, 0).assume_offset(time::UtcOffset::from_hms(2, 0, 0).unwrap());
        let utc = wall(1, 0).assume_utc();
        assert_eq!(datetime_order_key(plus_two), datetime_order_key(utc));
        assert_eq!(
            datetime_order_key(utc),
            "2025-03-09T01:05:07.000000000+00:00"
        );
    }
}
