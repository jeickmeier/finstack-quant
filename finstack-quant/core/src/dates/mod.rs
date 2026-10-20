//! Date utilities: business-day adjustments, day-counts, schedules, IMM helpers.
//!
//! This module wraps the [`time`](https://docs.rs/time) crate and exposes
//! domain-specific helpers commonly needed by pricing engines through
//! `finstack_quant_core::dates`. `Date` is re-exported here; `Month`, `Duration`,
//! `Weekday` and the other `time` types are imported from `time` directly.
//!
//! # Highlights
//! - re-export of `time::Date` (import every other `time` type from `time` itself)
//! - holiday calendars and business-day conventions (`adjust`, `HolidayCalendar`)
//! - schedule generation utilities (`ScheduleBuilder`)
//! - IMM/third-Wednesday helpers for derivatives roll dates
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::dates::{
//!     adjust, build_periods, calendar_by_id_strict, BusinessDayConvention, Date, Tenor,
//!     Schedule, create_date,
//! };
//! use time::{Duration, Month};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//! let target2 = calendar_by_id_strict("target2")?;
//! let trade_date = create_date(2024, Month::March, 29)?;
//! let adjusted = adjust(trade_date, BusinessDayConvention::Following, target2)?;
//! assert!(adjusted >= trade_date);
//!
//! let end = trade_date + Duration::days(365);
//! let schedule = Schedule::builder(trade_date, end)?
//!     .frequency(Tenor::quarterly())
//!     .build()?;
//! assert!(schedule.dates.len() >= 4);
//!
//! let periods = build_periods("2024Q1..Q4", None)?;
//! assert_eq!(periods.periods.len(), 4);
//! # Ok(())
//! # }
//! ```
//!
//! # Holiday Calendars
//!
//! ## Features
//!
//! - **27 built-in market calendars**: major exchanges, central banks, and
//!   settlement systems, generated at build time from `data/calendars/*.json`
//!   (see [`available_calendars`](crate::dates::available_calendars) for the exact identifier list)
//! - **Rule-based definitions**: JSON-defined rules for transparency and auditability
//! - **Cached rule evaluation**: cached years are materialized lazily into a
//!   process-wide holiday bitset and business-day prefix sums; out-of-range
//!   dates continue to scan the calendar's `&'static` rules directly
//! - **Composite calendars**: `+`-joined ids (for example `"nyse+gblo"`) resolve
//!   to a union calendar for multi-currency schedules
//! - **Business day adjustments**: Following, Modified Following, Preceding,
//!   Modified Preceding, Nearest conventions
//!
//! ## Lookup Cost
//!
//! The first lookup for a calendar year inside the cache range materializes
//! a 366-bit raw rule-holiday mask and business-day prefix sums. Subsequent
//! holiday and business-day predicates are constant-time bit lookups, while
//! interval counts combine at most one prefix-sum lookup per year. Dates outside
//! the cache range retain direct rule scanning.
//!
//! The cache is an implementation detail: public predicates are unchanged.
//! [`HolidayCalendar::is_holiday`](crate::dates::HolidayCalendar::is_holiday) still applies each calendar's
//! `ignore_weekends` behavior, and business-day checks still combine raw rule
//! holidays with the calendar's configured weekend rule.
//!
//! ## Supported Date Range
//!
//! The year cache and astronomical/lunar tables cover **1970-2150**. This is
//! a computational range, not certification of market holidays throughout it.
//! Published coverage is calendar-specific: SGSI moving holidays cover 2024-2027,
//! NSE/BSE moving holidays cover 2024-2026, and Matariki dates end in 2052.
//! Consult each calendar's source notes in `data/calendars/*.json` and the
//! independent annual fixtures. Recurring rules outside published coverage may omit closures;
//! the boolean predicate API cannot report an unsupported-year error.
//! Years outside the cache range still evaluate directly and emit a one-time
//! warning; table-based holidays may be unavailable. Trading, settlement,
//! banking/accrual, and fixing-publication calendars are distinct contracts.
//!
//! ## Key Concepts
//!
//! ### Holiday vs. Business Day
//!
//! - **Holiday**: Non-working date as defined by a specific market calendar
//!   (e.g., Christmas, Lunar New Year, bank holidays)
//! - **Business day**: Any day that is not a weekend (Saturday/Sunday) AND not
//!   a market-specific holiday
//!
//! Many calendars include weekends in their holiday definitions for convenience,
//! while others intentionally omit them. Regardless, [`HolidayCalendar::is_business_day`](crate::dates::HolidayCalendar::is_business_day)
//! uses the configured weekend rule (Saturday/Sunday for built-in calendars).
//!
//! **Guideline**: Use `is_business_day` for scheduling and date adjustments.
//! Use `is_holiday` only when you need market-specific holiday information.
//!
//! ## Calendar Types
//!
//! - **Exchange calendars**: `nyse`, `jpx`, `hkex`, `tsx`, `cme`, `eurex`, etc.
//! - **Settlement calendars**: `target2` (Eurozone), `usny` (Federal Reserve
//!   Bank of New York), `sifma` (US bond market), `sofr`
//! - **Country calendars**: banking holidays such as `gblo` (London), `jpto`
//!   (Tokyo) and `defr` (Frankfurt); see [`available_calendars`] for the full list

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

pub use time::Date;

mod date_extensions;

pub use date_extensions::DateExt;

mod daycount;

pub use daycount::{
    days_30_360, days_30e_360_isda, DayCount, DayCountContext, DayCountContextState,
    Thirty360Convention,
};

pub(crate) use calendar::business_days::canonical_calendar_id;
pub use calendar::business_days::{
    adjust, available_calendars, calendar_by_id_strict, resolve_calendar, resolve_joint_calendar,
    BusinessDayConvention, CalendarMetadata, HolidayCalendar,
};
pub use calendar::WEEKENDS_ONLY;

mod schedule;

pub use schedule::{
    Schedule, ScheduleBuilder, ScheduleErrorPolicy, ScheduleSpec, ScheduleWarning, StubKind,
};

pub(crate) use calendar::composite::CompositeCalendar;

mod imm;
mod tenor;

pub mod fx;

pub use tenor::{Tenor, TenorUnit};

pub use imm::{
    estimated_sifma_settlement_date, imm_option_expiry, is_cds_date, is_imm_date, next_cds_date,
    next_imm, next_imm_option_expiry, next_semiannual_cds_maturity, next_sifma_settlement,
    next_third_friday, prev_cds_date, prev_cds_semiannual_roll, sifma_settlement_date,
    third_friday, third_wednesday, SifmaSettlementClass,
};

mod calendar;

mod periods;
pub use periods::{
    build_fiscal_periods, build_periods, FiscalConfig, Period, PeriodId, PeriodKind, PeriodPlan,
};

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
/// Returns `None` if the resulting date is outside the range supported by the
/// `time` crate.
///
/// # Examples
/// ```
/// use finstack_quant_core::dates::{date_from_epoch_days, Date};
/// use time::Month;
///
/// let date = date_from_epoch_days(0);
/// assert_eq!(date, Some(Date::from_calendar_date(1970, Month::January, 1).expect("valid")));
///
/// let y2k = date_from_epoch_days(10957);
/// assert_eq!(y2k, Some(Date::from_calendar_date(2000, Month::January, 1).expect("valid")));
/// ```
///
/// # Arguments
///
/// * `days` - Signed number of calendar days from 1970-01-01 UTC. Values that
///   exceed the `time` crate's date range return `None`.
pub fn date_from_epoch_days(days: i32) -> Option<Date> {
    let julian_day = days.checked_add(UNIX_EPOCH_JULIAN_DAY)?;
    Date::from_julian_day(julian_day).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn test_create_date_valid_dates() {
        // Test normal valid dates
        assert!(create_date(2025, Month::January, 15).is_ok());
        assert!(create_date(2024, Month::February, 29).is_ok()); // Leap year
        assert!(create_date(2000, Month::February, 29).is_ok()); // Leap year (century divisible by 400)
        assert!(create_date(2023, Month::December, 31).is_ok());
        assert!(create_date(1970, Month::January, 1).is_ok()); // Unix epoch
    }

    #[test]
    fn test_create_date_invalid_dates() {
        // Test invalid February dates
        let result = create_date(2023, Month::February, 29);
        assert!(result.is_err());
        match result.expect_err("Should fail for invalid date") {
            crate::Error::Input(crate::InputError::InvalidDate { year, month, day }) => {
                assert_eq!(year, 2023);
                assert_eq!(month, 2); // February
                assert_eq!(day, 29);
            }
            _ => panic!("Expected InvalidDate error"),
        }

        // Test non-leap year February 29
        let result = create_date(2023, Month::February, 29);
        assert!(result.is_err());

        // Test invalid day numbers
        assert!(create_date(2023, Month::January, 0).is_err());
        assert!(create_date(2023, Month::January, 32).is_err());
        assert!(create_date(2023, Month::April, 31).is_err()); // April has 30 days
        assert!(create_date(2023, Month::June, 31).is_err()); // June has 30 days

        // Test extreme invalid dates
        assert!(create_date(2023, Month::January, 255).is_err());
    }

    #[test]
    fn test_create_date_error_message_format() {
        let result = create_date(2023, Month::February, 30);
        assert!(result.is_err());

        let error_msg = format!("{}", result.expect_err("Should fail for invalid date"));
        assert!(error_msg.contains("Invalid calendar date"));
        assert!(error_msg.contains("2023-02-30"));
    }

    #[test]
    fn test_create_date_edge_cases() {
        // Test month boundaries
        assert!(create_date(2023, Month::January, 31).is_ok());
        assert!(create_date(2023, Month::March, 31).is_ok());
        assert!(create_date(2023, Month::April, 30).is_ok());
        assert!(create_date(2023, Month::May, 31).is_ok());

        // Test leap year edge cases
        assert!(create_date(1900, Month::February, 28).is_ok()); // Not leap year (century not divisible by 400)
        assert!(create_date(1900, Month::February, 29).is_err()); // Not leap year
        assert!(create_date(2000, Month::February, 29).is_ok()); // Leap year (century divisible by 400)
        assert!(create_date(2100, Month::February, 28).is_ok()); // Not leap year
        assert!(create_date(2100, Month::February, 29).is_err()); // Not leap year
    }

    #[test]
    fn test_create_date_year_boundaries() {
        // Test reasonable year boundaries
        assert!(create_date(-9999, Month::January, 1).is_ok());
        assert!(create_date(9999, Month::December, 31).is_ok());

        // Test year 0 (should be valid in time crate)
        assert!(create_date(0, Month::January, 1).is_ok());
    }

    #[test]
    fn test_create_date_comparison_with_direct_usage() {
        // Verify that create_date produces the same results as direct Date::from_calendar_date for valid dates
        let valid_cases = vec![
            (2023, Month::January, 15),
            (2024, Month::February, 29), // Leap year
            (2023, Month::December, 31),
            (1970, Month::January, 1),
        ];

        for (year, month, day) in valid_cases {
            let direct_result = Date::from_calendar_date(year, month, day);
            let helper_result = create_date(year, month, day);

            assert!(direct_result.is_ok());
            assert!(helper_result.is_ok());
            assert_eq!(
                direct_result.expect("Direct result should succeed in test"),
                helper_result.expect("Helper result should succeed in test")
            );
        }
    }
}
