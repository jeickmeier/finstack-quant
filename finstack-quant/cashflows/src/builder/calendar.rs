//! Calendar helpers for cashflow scheduling.
//!
//! Calendar ids resolve through `finstack_quant_core::dates::calendar_by_id_strict`,
//! which also serves the holiday-free `"weekends_only"` calendar.

use finstack_quant_core::dates::{adjust, calendar_by_id_strict, BusinessDayConvention, Date};

/// Canonical ID for the weekends-only calendar.
pub const WEEKENDS_ONLY_ID: &str = "weekends_only";

/// Adjust a single date using the strict calendar policy.
///
/// # Arguments
///
/// * `date` - Unadjusted contractual date to roll under the selected
///   business-day convention.
/// * `business_day_convention` - Business-day convention to apply when `date` is not a business
///   day in the resolved calendar.
/// * `calendar_id` - Calendar identifier used for holiday lookup; any id accepted by
///   `calendar_by_id_strict`, including `+`-joined unions and `"weekends_only"`.
///
/// # Returns
///
/// Adjusted business date according to `business_day_convention` and `calendar_id`.
///
/// # Errors
///
/// Returns an error if the calendar ID cannot be resolved or if the underlying
/// date adjustment fails.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::adjust_date;
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
    business_day_convention: BusinessDayConvention,
    calendar_id: &str,
) -> finstack_quant_core::Result<Date> {
    let cal = calendar_by_id_strict(calendar_id)?;
    adjust(date, business_day_convention, cal)
}
