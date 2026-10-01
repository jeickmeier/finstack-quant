//! Actual/fixed and business-day convention implementations.

use time::{Date, Month};

use super::DayCountContext;
use crate::dates::tenor::TenorUnit;
use crate::error::InputError;

// ACT/365L helper
/// Calculate year fraction for Act/365L convention per ICMA Rule 251.1(i)(c).
///
/// The denominator rule depends on the coupon frequency supplied via
/// [`DayCountContext`]:
///
/// - **Annual**: 366 if February 29 falls in the full coupon period
///   `(coupon_start, coupon_end]`, else 365.
/// - **Non-annual**: 366 if the next coupon date falls in a leap year, else 365.
///
/// The requested accrual slice must lie within `ctx.coupon_period`; its end
/// date does not change the coupon's denominator.
pub(super) fn year_fraction_act_365l(
    start: Date,
    end: Date,
    ctx: DayCountContext<'_>,
) -> crate::Result<f64> {
    let frequency = ctx.frequency.ok_or_else(|| {
        crate::Error::Validation("ACT/365L requires coupon frequency in DayCountContext".into())
    })?;
    let (coupon_start, coupon_end) = ctx.coupon_period.ok_or_else(|| {
        crate::Error::Validation(
            "ACT/365L requires the enclosing coupon_period in DayCountContext".into(),
        )
    })?;
    if coupon_start >= coupon_end || start < coupon_start || end > coupon_end {
        return Err(crate::Error::Validation(
            "ACT/365L accrual dates must lie within a strictly increasing coupon_period".into(),
        ));
    }

    let actual_days = (end - start).whole_days() as f64;

    let annual = matches!(
        (frequency.unit(), frequency.count()),
        (TenorUnit::Years, 1) | (TenorUnit::Months, 12)
    );

    let leap = if annual {
        interval_contains_feb_29(coupon_start, coupon_end)
    } else {
        time::util::is_leap_year(coupon_end.year())
    };

    Ok(actual_days / if leap { 366.0 } else { 365.0 })
}

/// Check if February 29 falls in the interval `(start, end]` (exclusive of
/// start, inclusive of end) per ICMA Rule 251.
fn interval_contains_feb_29(start: Date, end: Date) -> bool {
    let start_year = start.year();
    let end_year = end.year();

    for year in start_year..=end_year {
        if time::util::is_leap_year(year) {
            if let Ok(feb_29) = Date::from_calendar_date(year, Month::February, 29) {
                if feb_29 > start && feb_29 <= end {
                    return true;
                }
            }
        }
    }
    false
}

// NL/365 helper
/// Calculate year fraction for NL/365 (Actual/365 No Leap).
///
/// Counts actual days in `(start, end]` excluding any February 29, divided by
/// a fixed 365-day year.
pub(super) fn year_fraction_nl_365(start: Date, end: Date) -> f64 {
    if start == end {
        return 0.0;
    }

    let actual_days = (end - start).whole_days();
    let mut leap_days: i64 = 0;
    for year in start.year()..=end.year() {
        if time::util::is_leap_year(year) {
            if let Ok(feb_29) = Date::from_calendar_date(year, Month::February, 29) {
                if feb_29 > start && feb_29 <= end {
                    leap_days += 1;
                }
            }
        }
    }
    (actual_days - leap_days) as f64 / 365.0
}

/// Bus/252 with context extraction - validates calendar is present and basis is non-zero.
pub(super) fn year_fraction_bus252(
    start: Date,
    end: Date,
    ctx: DayCountContext<'_>,
) -> crate::Result<f64> {
    let cal = ctx.calendar.ok_or(InputError::MissingCalendarForBus252)?;
    let basis = ctx.bus_basis.unwrap_or(252);
    if basis == 0 {
        return Err(InputError::InvalidBusBasis { basis }.into());
    }
    let biz_days = cal.count_business_days(start, end) as f64;
    Ok(biz_days / f64::from(basis))
}
