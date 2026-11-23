//! Date construction, ISO parsing, and Unix-epoch day conversion helpers.

use time::Date;

// Time constants – canonical "days per year" values

/// Calendar days per year under the ACT/365 Fixed day-count convention.
///
/// This is the standard denominator for most derivative day-count calculations
/// (swaps, bonds, CDS, XVA, margin). Use this as the default unless a model
/// explicitly requires leap-year adjustment.
///
/// # Convention
///
/// ACT/365 Fixed always divides by 365.0, regardless of whether the accrual
/// period contains a leap day. This is the most common convention in swap
/// and credit derivative markets.
pub const CALENDAR_DAYS_PER_YEAR: f64 = 365.0;

/// Parse an ISO 8601 (extended) date string into a [`Date`].
///
/// Convenience wrapper around [`time::Date::parse`] using the
/// [`time::format_description::well_known::Iso8601::DEFAULT`] format. Returns
/// a structured [`crate::Error::Validation`] on failure with the offending
/// input echoed in the message.
///
/// # Arguments
///
/// * `s` - ISO 8601 extended date text, such as `2025-03-19`.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] if `s` is not a valid ISO 8601 date.
pub fn parse_iso_date(s: &str) -> crate::Result<Date> {
    let format = time::format_description::well_known::Iso8601::DEFAULT;
    Date::parse(s, &format)
        .map_err(|e| crate::Error::Validation(format!("Invalid date '{s}': {e}")))
}

/// Safe date creation helper that returns a Result instead of panicking.
///
/// This is a safer alternative to `Date::from_calendar_date(...).unwrap()`
/// that provides proper error handling for invalid dates like February 30th.
///
/// # Arguments
///
/// * `year` - Calendar year (can be negative for BCE dates)
/// * `month` - Month from the `time` crate's [`time::Month`] enum
/// * `day` - Day of month (1-31 depending on month)
///
/// # Returns
///
/// A valid [`Date`] if the combination of year, month, and day is valid.
///
/// # Errors
///
/// Returns [`InputError::InvalidDate`](crate::InputError::InvalidDate) when:
/// - Day is 0 or exceeds the number of days in the given month
/// - Day is 29 for February in a non-leap year
/// - Day is 30 or 31 for February
/// - Day is 31 for months with only 30 days (April, June, September, November)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::create_date;
/// use time::Month;
///
/// // Valid date
/// let date = create_date(2025, Month::January, 15)?;
///
/// // Invalid date - returns error instead of panic
/// let result = create_date(2025, Month::February, 30); // Returns Err
/// assert!(result.is_err());
///
/// // Leap year handling
/// let leap = create_date(2024, Month::February, 29)?; // OK - 2024 is a leap year
/// let non_leap = create_date(2025, Month::February, 29); // Err - 2025 is not
/// assert!(non_leap.is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn create_date(year: i32, month: time::Month, day: u8) -> crate::Result<Date> {
    Date::from_calendar_date(year, month, day)
        .map_err(|_| crate::InputError::InvalidDate {
            year,
            month: month as u8,
            day,
        })
        .map_err(Into::into)
}

// Epoch conversion utilities

/// Julian day number of the Unix epoch (1970-01-01).
const UNIX_EPOCH_JULIAN_DAY: i32 = 2_440_588;

/// Convert a date to days since Unix epoch (1970-01-01).
///
/// Returns a signed integer: dates before the epoch are negative.
///
/// # Examples
/// ```
/// use finstack_quant_core::dates::{days_since_epoch, Date};
/// use time::Month;
///
/// let epoch = Date::from_calendar_date(1970, Month::January, 1).expect("valid");
/// assert_eq!(days_since_epoch(epoch), 0);
///
/// let day_after = Date::from_calendar_date(1970, Month::January, 2).expect("valid");
/// assert_eq!(days_since_epoch(day_after), 1);
/// ```
///
/// # Arguments
///
/// * `date` - Calendar date to express as a signed count of days from
///   1970-01-01 UTC.
pub fn days_since_epoch(date: Date) -> i32 {
    date.to_julian_day() - UNIX_EPOCH_JULIAN_DAY
}

/// Convert days since Unix epoch (1970-01-01) to a [`Date`].
///
/// The exact inverse of [`days_since_epoch`]: signed civil calendar days,
/// with no time-zone, instant or local-calendar meaning.
///
/// # Examples
/// ```
/// use finstack_quant_core::dates::{date_from_epoch_days, Date};
/// use time::Month;
///
/// let date = date_from_epoch_days(0)?;
/// assert_eq!(date, Date::from_calendar_date(1970, Month::January, 1).expect("valid"));
///
/// let y2k = date_from_epoch_days(10957)?;
/// assert_eq!(y2k, Date::from_calendar_date(2000, Month::January, 1).expect("valid"));
/// assert!(date_from_epoch_days(i32::MAX).is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # Arguments
///
/// * `days` - Signed number of calendar days from 1970-01-01.
///
/// # Errors
///
/// Returns `Error::Validation` when the day number lies outside the
/// supported `Date` range.
pub fn date_from_epoch_days(days: i32) -> crate::Result<Date> {
    days.checked_add(UNIX_EPOCH_JULIAN_DAY)
        .and_then(|julian_day| Date::from_julian_day(julian_day).ok())
        .ok_or_else(|| {
            crate::Error::Validation(format!(
                "epoch day {days} is outside the supported date range"
            ))
        })
}
