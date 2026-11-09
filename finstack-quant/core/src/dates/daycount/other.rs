//! Actual/fixed and business-day convention implementations.

use time::{Date, Month};

use super::DayCountContext;
use crate::InputError;
use crate::dates::Tenor;
use crate::dates::tenor::TenorUnit;

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
    let (frequency, (coupon_start, coupon_end)) = act_365l_context(ctx)?;
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
        count_feb_29_in_interval(coupon_start, coupon_end) > 0
    } else {
        time::util::is_leap_year(coupon_end.year())
    };

    Ok(actual_days / if leap { 366.0 } else { 365.0 })
}

/// The ACT/365L frequency and enclosing coupon period, required by the
/// convention.
fn act_365l_context(ctx: DayCountContext<'_>) -> crate::Result<(Tenor, (Date, Date))> {
    let frequency = ctx.frequency.ok_or_else(|| {
        crate::Error::Validation("ACT/365L requires coupon frequency in DayCountContext".into())
    })?;
    let coupon_period = ctx.coupon_period.ok_or_else(|| {
        crate::Error::Validation(
            "ACT/365L requires the enclosing coupon_period in DayCountContext".into(),
        )
    })?;
    Ok((frequency, coupon_period))
}

/// Check that ACT/365L's required context is present.
pub(super) fn validate_act_365l_context(ctx: DayCountContext<'_>) -> crate::Result<()> {
    act_365l_context(ctx).map(|_| ())
}

/// Count February 29 dates falling in the interval `(start, end]` (exclusive of
/// start, inclusive of end).
fn count_feb_29_in_interval(start: Date, end: Date) -> i64 {
    let mut leap_days: i64 = 0;
    for year in start.year()..=end.year() {
        if time::util::is_leap_year(year)
            && let Ok(feb_29) = Date::from_calendar_date(year, Month::February, 29)
            && feb_29 > start
            && feb_29 <= end
        {
            leap_days += 1;
        }
    }
    leap_days
}

// NL/365 helper
/// Calculate year fraction for NL/365 (Actual/365 No Leap).
///
/// Counts actual days in `(start, end]` excluding any February 29, divided by
/// a fixed 365-day year. Precondition: `start < end`.
pub(super) fn year_fraction_nl_365(start: Date, end: Date) -> f64 {
    let actual_days = (end - start).whole_days();
    let leap_days = count_feb_29_in_interval(start, end);
    (actual_days - leap_days) as f64 / 365.0
}

/// Bus/252: business days of the context calendar in `[start, end)` over
/// the basis (252 unless overridden). The context was validated by the
/// caller; a count outside `0..=elapsed days` is an error.
pub(super) fn year_fraction_bus252(
    start: Date,
    end: Date,
    ctx: DayCountContext<'_>,
) -> crate::Result<f64> {
    let cal = ctx.calendar.ok_or(InputError::MissingCalendarForBus252)?;
    let basis = ctx.bus_basis.unwrap_or(252);
    let business_days = cal.count_business_days(start, end)?;
    let elapsed = (end - start).whole_days();
    if business_days < 0 || i64::from(business_days) > elapsed {
        return Err(crate::Error::Internal(format!(
            "calendar counted {business_days} business days in {elapsed} calendar days \
             from {start} to {end}"
        )));
    }
    Ok(f64::from(business_days) / f64::from(basis))
}
