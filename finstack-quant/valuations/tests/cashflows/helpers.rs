//! Shared helper utilities for remaining valuations cashflow tests.

use finstack_quant_core::dates::Date;
use time::Month;

/// Convenience date constructor for tests.
///
/// # Panics
///
/// Panics if the date is invalid (e.g., month not in 1-12, day out of range).
/// The panic message includes the invalid date components for debugging.
pub(crate) fn d(year: i32, month: u8, day: u8) -> Date {
    let month_enum = Month::try_from(month).unwrap_or_else(|_| {
        panic!(
            "Invalid month {} in date {}-{:02}-{:02}",
            month, year, month, day
        )
    });
    Date::from_calendar_date(year, month_enum, day)
        .unwrap_or_else(|_| panic!("Invalid date: {}-{:02}-{:02}", year, month, day))
}
