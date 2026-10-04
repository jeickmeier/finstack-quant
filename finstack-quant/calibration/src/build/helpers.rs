//! Shared helpers for quote-to-instrument builders.

use finstack_quant_core::dates::{
    adjust, calendar_by_id_strict, BusinessDayConvention, Date, DateExt,
};
use finstack_quant_core::Result;

/// Resolve the spot date given settlement lag and market conventions.
pub(crate) fn resolve_spot_date(
    as_of: Date,
    calendar_id: &str,
    settlement_days: i32,
    business_day_convention: BusinessDayConvention,
) -> Result<Date> {
    let cal = calendar_by_id_strict(calendar_id)?;
    let spot = as_of.add_business_days(settlement_days, cal)?;
    adjust(spot, business_day_convention, cal)
}
