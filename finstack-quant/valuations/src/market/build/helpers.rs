//! Shared helpers for quote-to-instrument builders.

use finstack_quant_core::dates::{
    adjust, BusinessDayConvention, CalendarRegistry, Date, DateExt, HolidayCalendar,
};
use finstack_quant_core::Error;
use finstack_quant_core::Result;

/// Resolve a holiday calendar by ID from the global registry.
pub(crate) fn resolve_calendar(id: &str) -> Result<&'static dyn HolidayCalendar> {
    CalendarRegistry::global()
        .resolve_str(id)
        .ok_or_else(|| Error::calendar_not_found_with_suggestions(id, &[]))
}

/// Resolve the spot date given settlement lag and market conventions.
pub(crate) fn resolve_spot_date(
    as_of: Date,
    calendar_id: &str,
    settlement_days: i32,
    bdc: BusinessDayConvention,
) -> Result<Date> {
    let cal = resolve_calendar(calendar_id)?;
    let spot = as_of.add_business_days(settlement_days, cal)?;
    adjust(spot, bdc, cal)
}
