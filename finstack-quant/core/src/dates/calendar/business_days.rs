//! Holiday calendars and business day adjustment conventions.
//!
//! This module implements industry-standard business day adjustment rules used
//! for cashflow date adjustments in fixed income and derivatives markets.
//!
//! # Core Components
//!
//! 1. [`HolidayCalendar`]: Trait for querying market-specific holidays
//! 2. [`BusinessDayConvention`]: ISDA-standard adjustment rules
//! 3. [`adjust`]: Date adjustment function applying conventions
//!
//! # Business Day Conventions
//!
//! Following ISDA 2006 Definitions Section 4.12:
//! - **Following**: Move to next business day
//! - **Preceding**: Move to previous business day
//! - **ModifiedFollowing**: Following, unless crosses month boundary
//! - **ModifiedPreceding**: Preceding, unless crosses month boundary
//!
//! # Built-in Calendars
//!
//! Standard market calendars included:
//! - **TARGET2**: European Central Bank / SEPA
//! - **NYSE**: New York Stock Exchange
//! - **GBLO**: London Stock Exchange
//! - **USNY**: US Federal Reserve (New York)
//! - **JPTO**: Tokyo Stock Exchange
//!
//! # Standards References
//!
//! - **ISDA**: 2006 ISDA Definitions, Section 4.12 (Business Day Conventions)
//! - **FpML**: BusinessDayConventionEnum
//! - **ISO 20022**: Business Day Convention codes
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::dates::{adjust, BusinessDayConvention, Date};
//! use finstack_quant_core::dates::calendar::TARGET2;
//! use time::Month;
//!
//! let date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"); // New Year (holiday)
//! let adjusted = adjust(date, BusinessDayConvention::Following, &TARGET2)?;
//! assert!(adjusted > date); // Moved to next business day
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

use crate::dates::DateExt;
use crate::error::{Error, InputError};
use time::{Date, Duration};

/// Shared upper bound used across business-day search helpers to avoid runaway loops.
pub(crate) const MAX_BUSINESS_DAY_SEARCH_DAYS: i32 = 100;

/// Seek the nearest business day from `date` moving in steps of `step_days` (±1),
/// searching up to `max_days` steps using the provided `cal`.
///
/// Returns `Some(date)` when found, or `None` if no business day is found within the window.
#[inline]
pub(crate) fn seek_business_day<C: HolidayCalendar + ?Sized>(
    mut date: Date,
    step_days: i32,
    max_days: i32,
    cal: &C,
) -> Option<Date> {
    let mut searched = 0;
    while !cal.is_business_day(date) {
        date += Duration::days(step_days as i64);
        searched += 1;
        if searched > max_days {
            return None;
        }
    }
    Some(date)
}

/// Trait for market-specific holiday calendars.
///
/// Implementors define market-specific holidays by implementing [`is_holiday`](Self::is_holiday).
/// The trait provides a default [`is_business_day`](Self::is_business_day) that automatically
/// treats Saturday/Sunday as non-business days in addition to market holidays.
///
/// # Implementation Guide
///
/// When implementing a custom calendar:
/// 1. Implement only [`is_holiday`](Self::is_holiday) - weekend handling is automatic
/// 2. Return `true` for market-specific non-working days (exchanges closed, bank holidays)
/// 3. Optionally include weekends in `is_holiday` for consistency, but not required
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::HolidayCalendar;
/// use time::Date;
///
/// struct CustomCalendar;
///
/// impl HolidayCalendar for CustomCalendar {
///     fn is_holiday(&self, date: Date) -> bool {
///         // Example: Only New Year's Day
///         date.month() == time::Month::January && date.day() == 1
///     }
/// }
///
/// let cal = CustomCalendar;
/// let new_year = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid date");
/// assert!(cal.is_holiday(new_year));
/// assert!(!cal.is_business_day(new_year)); // Holiday = not business day
/// ```
pub trait HolidayCalendar: Send + Sync {
    /// Returns `true` if `date` is a holiday according to this calendar.
    ///
    /// Notes:
    /// - Many built-in calendars also return `true` for weekends.
    /// - Some calendars intentionally ignore weekends and will return `false`
    ///   for Saturday/Sunday. This only affects `is_holiday`; use
    ///   [`HolidayCalendar::is_business_day`] for business-day logic.
    fn is_holiday(&self, date: Date) -> bool;

    /// Returns `true` if the `date` is a business day according to the
    /// calendar (i.e. not weekend and not holiday).
    ///
    /// Weekend is defined as Saturday/Sunday and is independent of how a
    /// particular calendar chooses to label weekends in [`Self::is_holiday`].
    #[inline]
    fn is_business_day(&self, date: Date) -> bool {
        !date.is_weekend() && !self.is_holiday(date)
    }

    /// Optional human-friendly metadata for the calendar.
    #[inline]
    fn metadata(&self) -> Option<CalendarMetadata> {
        None
    }
}

/// Basic metadata describing a holiday calendar.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct CalendarMetadata {
    /// Lowercase identifier (stable code).
    pub id: &'static str,
    /// Human-readable name of the calendar.
    pub name: &'static str,
    /// Whether weekends are ignored when classifying holidays.
    pub ignore_weekends: bool,
    /// Weekend convention used by this calendar for business-day logic.
    pub weekend_rule: super::types::WeekendRule,
}

/// Business day adjustment conventions per ISDA standards.
///
/// Defines how dates are adjusted when they fall on non-business days
/// (weekends or holidays). Used throughout fixed income and derivatives
/// markets for determining payment dates, fixing dates, and maturity dates.
///
/// # Standards References
///
/// - **ISDA**: 2006 ISDA Definitions, Section 4.12
/// - **FpML**: BusinessDayConventionEnum
/// - **ISO 20022**: Business Day Convention codes
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{adjust, BusinessDayConvention, Date};
/// use finstack_quant_core::dates::calendar::TARGET2;
/// use time::Month;
///
/// // Saturday, January 4, 2025
/// let weekend = Date::from_calendar_date(2025, Month::January, 4).expect("Valid date");
///
/// // Following: moves to next Monday (Jan 6)
/// let adj = adjust(weekend, BusinessDayConvention::Following, &TARGET2)?;
/// assert_eq!(adj.day(), 6);
///
/// // Preceding: moves to previous Friday (Jan 3)
/// let adj = adjust(weekend, BusinessDayConvention::Preceding, &TARGET2)?;
/// assert_eq!(adj.day(), 3);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BusinessDayConvention {
    /// No adjustment - date remains as specified.
    ///
    /// **ISDA**: Section 4.12(a) - "Unadjusted"
    /// **FpML**: "NONE"
    Unadjusted,

    /// Adjust to next business day (may cross month boundary).
    ///
    /// **ISDA**: Section 4.12(b) - "Following Business Day Convention"
    /// **FpML**: "FOLLOWING"
    /// **ISO 20022**: "FWNG"
    Following,

    /// Following, unless that crosses into next month, then preceding.
    ///
    /// Ensures month-end dates don't shift to next month, important for
    /// consistent period calculations in swaps and bonds.
    ///
    /// **ISDA**: Section 4.12(c) - "Modified Following Business Day Convention"
    /// **FpML**: "MODFOLLOWING"
    /// **ISO 20022**: "MODF"
    ModifiedFollowing,

    /// Adjust to previous business day (may cross month boundary).
    ///
    /// **ISDA**: Section 4.12(d) - "Preceding Business Day Convention"
    /// **FpML**: "PRECEDING"
    /// **ISO 20022**: "PREC"
    Preceding,

    /// Preceding, unless that crosses into previous month, then following.
    ///
    /// **ISDA**: Section 4.12(e) - "Modified Preceding Business Day Convention"
    /// **FpML**: "MODPRECEDING"
    ModifiedPreceding,
}

impl core::fmt::Display for BusinessDayConvention {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            BusinessDayConvention::Unadjusted => "Unadjusted",
            BusinessDayConvention::Following => "Following",
            BusinessDayConvention::ModifiedFollowing => "ModifiedFollowing",
            BusinessDayConvention::Preceding => "Preceding",
            BusinessDayConvention::ModifiedPreceding => "ModifiedPreceding",
        };
        f.write_str(s)
    }
}

impl core::str::FromStr for BusinessDayConvention {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Normalize: trim, lowercase, replace hyphens with underscores
        let normalized = s.trim().to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "unadjusted" => Ok(BusinessDayConvention::Unadjusted),
            "following" => Ok(BusinessDayConvention::Following),
            "modified_following" | "modifiedfollowing" => {
                Ok(BusinessDayConvention::ModifiedFollowing)
            }
            "preceding" => Ok(BusinessDayConvention::Preceding),
            "modified_preceding" | "modifiedpreceding" => {
                Ok(BusinessDayConvention::ModifiedPreceding)
            }
            _ => Err(format!("Unknown business day convention: {}", s)),
        }
    }
}

/// Adjust `date` according to `conv` utilising `cal` for holiday lookup.
///
/// Returns an error if no business day is found within 100 days of the input date.
/// This prevents infinite loops when using composite calendars that mark
/// all days as holidays in a range.
///
/// Example:
/// ```
/// use finstack_quant_core::dates::{Date, BusinessDayConvention, adjust};
/// use finstack_quant_core::dates::calendar::TARGET2;
/// use time::Month;
/// let cal = TARGET2;
/// let sat = Date::from_calendar_date(2025, Month::January, 4).expect("Valid date");
/// let adj = adjust(sat, BusinessDayConvention::Following, &cal).expect("Adjustment should succeed");
/// assert_eq!(adj, Date::from_calendar_date(2025, Month::January, 6).expect("Valid date"));
/// ```
///
/// # Arguments
///
/// * `date` - Unadjusted date to test and, if necessary, roll to a business day.
/// * `conv` - Business-day convention that selects the roll direction and
///   month-crossing treatment.
/// * `cal` - Holiday calendar used to determine whether candidate dates are
///   business days.
pub fn adjust<C: HolidayCalendar + ?Sized>(
    date: Date,
    conv: BusinessDayConvention,
    cal: &C,
) -> Result<Date, Error> {
    adjust_with_limit(date, conv, cal, MAX_BUSINESS_DAY_SEARCH_DAYS)
}

/// Adjust `date` according to `conv` utilising `cal` for holiday lookup with a custom search limit.
pub(crate) fn adjust_with_limit<C: HolidayCalendar + ?Sized>(
    date: Date,
    conv: BusinessDayConvention,
    cal: &C,
    max_days: i32,
) -> Result<Date, Error> {
    match conv {
        BusinessDayConvention::Unadjusted => Ok(date),
        BusinessDayConvention::Following => {
            if cal.is_business_day(date) {
                return Ok(date);
            }
            seek_business_day(date, 1, max_days, cal).ok_or({
                Error::Input(InputError::AdjustmentFailed {
                    date,
                    convention: BusinessDayConvention::Following,
                    max_days,
                })
            })
        }
        BusinessDayConvention::ModifiedFollowing => {
            if cal.is_business_day(date) {
                return Ok(date);
            }
            let original_month = date.month();
            let forward = seek_business_day(date, 1, max_days, cal).ok_or(Error::Input(
                InputError::AdjustmentFailed {
                    date,
                    convention: BusinessDayConvention::ModifiedFollowing,
                    max_days,
                },
            ))?;
            if forward.month() == original_month {
                Ok(forward)
            } else {
                seek_business_day(date, -1, max_days, cal).ok_or({
                    Error::Input(InputError::AdjustmentFailed {
                        date,
                        convention: BusinessDayConvention::ModifiedFollowing,
                        max_days,
                    })
                })
            }
        }
        BusinessDayConvention::Preceding => {
            if cal.is_business_day(date) {
                return Ok(date);
            }
            seek_business_day(date, -1, max_days, cal).ok_or({
                Error::Input(InputError::AdjustmentFailed {
                    date,
                    convention: BusinessDayConvention::Preceding,
                    max_days,
                })
            })
        }
        BusinessDayConvention::ModifiedPreceding => {
            if cal.is_business_day(date) {
                return Ok(date);
            }
            let original_month = date.month();
            let back = seek_business_day(date, -1, max_days, cal).ok_or(Error::Input(
                InputError::AdjustmentFailed {
                    date,
                    convention: BusinessDayConvention::ModifiedPreceding,
                    max_days,
                },
            ))?;
            if back.month() == original_month {
                Ok(back)
            } else {
                seek_business_day(date, 1, max_days, cal).ok_or({
                    Error::Input(InputError::AdjustmentFailed {
                        date,
                        convention: BusinessDayConvention::ModifiedPreceding,
                        max_days,
                    })
                })
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Runtime discovery helpers
// -----------------------------------------------------------------------------

/// Returns the identifiers of all built-in holiday calendars that have been
/// compiled into the crate.
///
/// Identifiers are lowercase, stable market codes (e.g., `"gblo"`, `"target2"`).
/// They are suitable for serialization and long-lived pipelines.
///
/// Example using the free resolution API:
/// ```
/// use finstack_quant_core::dates::{available_calendars, calendar_by_id};
/// use finstack_quant_core::types::CalendarId;
/// let ids = available_calendars();
/// let id = CalendarId::from(ids[0]);
/// let maybe = calendar_by_id(id.as_str());
/// assert!(maybe.is_some());
/// ```
#[inline]
pub const fn available_calendars() -> &'static [&'static str] {
    crate::dates::calendar::ALL_IDS
}

#[cfg(test)]
mod serde_tests {
    use super::*;

    #[test]
    fn test_business_day_convention_serde_roundtrip() {
        use serde_json;

        // Test all BusinessDayConvention variants
        let conventions = vec![
            BusinessDayConvention::Unadjusted,
            BusinessDayConvention::Following,
            BusinessDayConvention::ModifiedFollowing,
            BusinessDayConvention::Preceding,
            BusinessDayConvention::ModifiedPreceding,
        ];

        for conv in conventions {
            let json =
                serde_json::to_string(&conv).expect("JSON serialization should succeed in test");
            let deserialized: BusinessDayConvention =
                serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
            assert_eq!(conv, deserialized);
        }
    }

    #[test]
    fn test_business_day_convention_snake_case() {
        use serde_json;

        // Test that the snake_case renaming works
        let conv = BusinessDayConvention::ModifiedFollowing;
        let json = serde_json::to_string(&conv).expect("JSON serialization should succeed in test");
        assert_eq!(json, "\"modified_following\"");

        let conv = BusinessDayConvention::ModifiedPreceding;
        let json = serde_json::to_string(&conv).expect("JSON serialization should succeed in test");
        assert_eq!(json, "\"modified_preceding\"");
    }
}
