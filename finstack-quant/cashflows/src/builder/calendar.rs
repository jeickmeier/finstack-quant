//! Calendar resolution utilities for cashflow scheduling.
//!
//! Provides a strict resolver that accepts explicit calendar IDs and supports
//! a dedicated weekends-only calendar for cases without holiday rules.

use finstack_quant_core::dates::{adjust, BusinessDayConvention, Date};
use finstack_quant_core::dates::{
    available_calendars, calendar_by_id, HolidayCalendar, WEEKENDS_ONLY,
};

/// Canonical ID for the weekends-only calendar.
pub const WEEKENDS_ONLY_ID: &str = "weekends_only";

/// Resolve a calendar ID to a holiday calendar reference.
/// Accepted IDs include the built-in `"weekends_only"` calendar and any
/// calendar resolved by `finstack_quant_core::dates::calendar_by_id`.
///
/// # Arguments
///
/// * `calendar_id` - Calendar identifier to resolve.
///
/// # Returns
///
/// A holiday-calendar reference that can be reused for date adjustment and
/// business-day counting.
///
/// # Errors
///
/// Returns an error if the calendar ID is not recognized.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::calendar::resolve_calendar_strict;
///
/// let cal = resolve_calendar_strict("weekends_only")?;
/// assert!(cal.is_business_day(
///     finstack_quant_core::dates::Date::from_calendar_date(
///         2025,
///         time::Month::January,
///         2,
///     )
///     .expect("valid date"),
/// ));
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn resolve_calendar_strict(
    calendar_id: &str,
) -> finstack_quant_core::Result<&'static dyn HolidayCalendar> {
    if calendar_id == WEEKENDS_ONLY_ID {
        return Ok(&WEEKENDS_ONLY);
    }
    calendar_by_id(calendar_id).ok_or_else(|| {
        finstack_quant_core::Error::calendar_not_found_with_suggestions(
            calendar_id,
            available_calendars(),
        )
    })
}

/// Adjust a single date using the strict calendar policy.
///
/// # Arguments
///
/// * `date` - Unadjusted contractual date to roll under the selected
///   business-day convention.
/// * `bdc` - Business-day convention to apply when `date` is not a business
///   day in the resolved calendar.
/// * `calendar_id` - Calendar identifier used for holiday lookup.
///
/// # Returns
///
/// Adjusted business date according to `bdc` and `calendar_id`.
///
/// # Errors
///
/// Returns an error if the calendar ID cannot be resolved or if the underlying
/// date adjustment fails.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::calendar::adjust_date;
/// use finstack_quant_core::dates::{BusinessDayConvention, Date};
/// use time::Month;
///
/// let saturday = Date::from_calendar_date(2025, Month::January, 4).expect("valid date");
/// let adjusted = adjust_date(
///     saturday,
///     BusinessDayConvention::Following,
///     "weekends_only",
/// )
/// .expect("adjustment succeeds");
///
/// assert!(adjusted >= saturday);
/// ```
pub fn adjust_date(
    date: Date,
    bdc: BusinessDayConvention,
    calendar_id: &str,
) -> finstack_quant_core::Result<Date> {
    let cal = resolve_calendar_strict(calendar_id)?;
    adjust(date, bdc, cal)
}
