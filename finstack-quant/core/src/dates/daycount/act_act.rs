//! Actual/Actual convention implementations.

use time::{Date, Month};

use super::DayCountContext;
use crate::InputError;
use crate::dates::date_extensions::DateExt;
use crate::dates::tenor::{Tenor, TenorUnit};

/// Resolve the nominal period length in months for supported ICMA coupon frequencies.
fn icma_period_months(frequency: Tenor) -> crate::Result<i32> {
    match (frequency.unit(), frequency.count()) {
        (TenorUnit::Months, m @ (1 | 2 | 3 | 4 | 6 | 12)) => Ok(m as i32),
        (TenorUnit::Years, 1) => Ok(12),
        _ => Err(InputError::ActActIsmaUnsupportedFrequency {
            frequency: frequency.to_string(),
        }
        .into()),
    }
}

/// Validate ACT/ACT (ICMA) context and return `(period_months, (period_start, period_end))`.
fn validated_act_act_isma(ctx: DayCountContext<'_>) -> crate::Result<(i32, (Date, Date))> {
    let frequency = ctx
        .frequency
        .ok_or(InputError::MissingFrequencyForActActIsma)?;
    let period_months = icma_period_months(frequency)?;
    let (period_start, period_end) = ctx
        .coupon_period
        .ok_or(InputError::MissingCouponPeriodForActActIsma)?;
    if period_start >= period_end {
        return Err(InputError::InvalidDateRange.into());
    }

    let month_span = (period_end.year() - period_start.year()) * 12
        + (i32::from(u8::from(period_end.month())) - i32::from(u8::from(period_start.month())));
    let shares_nominal_grid = month_span == period_months
        && (period_start.add_months(period_months) == Ok(period_end)
            || period_end.add_months(-period_months) == Ok(period_start));
    if !shares_nominal_grid {
        return Err(crate::Error::Validation(
            "ACT/ACT ISMA coupon_period endpoints must share one unadjusted nominal month grid of frequency"
                .to_string(),
        ));
    }

    Ok((period_months, (period_start, period_end)))
}

/// ACT/ACT (ISDA): the integer count of complete calendar years between the
/// two partial years, plus at most two partial-year fractions (days over that
/// year's 365 or 366). Precondition: `start < end`.
pub(super) fn year_fraction_act_act_isda(start: Date, end: Date) -> f64 {
    let year_days = |year: i32| f64::from(time::util::days_in_year(year));
    if start.year() == end.year() {
        return (end - start).whole_days() as f64 / year_days(start.year());
    }
    // Days in `[start, Jan 1 of the next year)` and `[Jan 1 of end's year, end)`;
    // the ordinal arithmetic needs no out-of-range January 1 date.
    let start_days =
        i32::from(time::util::days_in_year(start.year())) - i32::from(start.ordinal()) + 1;
    let end_days = i32::from(end.ordinal()) - 1;
    let complete_years = end.year() - start.year() - 1;
    f64::from(complete_years)
        + f64::from(start_days) / year_days(start.year())
        + f64::from(end_days) / year_days(end.year())
}

/// Check ACT/ACT (ICMA) context: both an ICMA coupon frequency in
/// `{1M, 2M, 3M, 4M, 6M, 12M, 1Y}` and a single unadjusted nominal
/// `coupon_period` of that frequency are required.
pub(super) fn validate_act_act_isma_context(ctx: DayCountContext<'_>) -> crate::Result<()> {
    validated_act_act_isma(ctx).map(|_| ())
}

/// Check that `[start, end]` lies within the enclosing ICMA `coupon_period`.
pub(super) fn validate_act_act_isma_interval(
    start: Date,
    end: Date,
    ctx: DayCountContext<'_>,
) -> crate::Result<()> {
    let (period_start, period_end) = ctx
        .coupon_period
        .ok_or(InputError::MissingCouponPeriodForActActIsma)?;
    if start < period_start || end > period_end {
        return Err(crate::Error::Validation(format!(
            "ACT/ACT ISMA accrual [{start}, {end}] must lie within coupon_period [{period_start}, {period_end}]"
        )));
    }
    Ok(())
}

/// Contained single-period ACT/ACT (ICMA/ISMA) year fraction.
///
/// Requires `ctx.frequency` in `{1M, 2M, 3M, 4M, 6M, 12M, 1Y}` (`F = 12 / m`)
/// and `ctx.coupon_period = Some((period_start, period_end))` spanning one
/// unadjusted nominal period of `frequency` with `period_start <= start <= end <= period_end`.
pub(super) fn year_fraction_act_act_isma_with_ctx(
    start: Date,
    end: Date,
    ctx: DayCountContext<'_>,
) -> crate::Result<f64> {
    let (period_months, (period_start, period_end)) = validated_act_act_isma(ctx)?;
    validate_act_act_isma_interval(start, end, ctx)?;
    let actual_days = (end - start).whole_days() as f64;
    let period_days = (period_end - period_start).whole_days() as f64;
    let coupons_per_year = f64::from(12 / period_months);
    Ok(actual_days / (coupons_per_year * period_days))
}

/// ACT/ACT AFB (Association Française des Banques / Actual/Actual Euro).
///
/// QuantLib `ActualActual::AFB`: walk whole years backwards from `end` until
/// the candidate is before `start`, then divide the residual actual days by
/// 366 if 29 February lies in `[start, residual_end)`, else 365. A backward
/// step that cannot be represented lies before every valid `start`, so it
/// ends the walk. Precondition: `start < end`.
pub(super) fn year_fraction_act_act_afb(start: Date, end: Date) -> f64 {
    let mut residual_end = end;
    let mut whole_years: u32 = 0;

    while let Ok(mut candidate) = residual_end.add_months(-12) {
        // QuantLib leap-day alignment: a year-step that lands on 28 February
        // of a leap year is bumped to 29 February.
        if candidate.month() == Month::February
            && candidate.day() == 28
            && time::util::is_leap_year(candidate.year())
        {
            candidate += time::Duration::days(1);
        }
        if candidate >= start {
            whole_years += 1;
            residual_end = candidate;
        } else {
            break;
        }
    }

    let days = (residual_end - start).whole_days() as f64;
    let den = if feb29_in_half_open(start, residual_end) {
        366.0
    } else {
        365.0
    };
    f64::from(whole_years) + days / den
}

/// True when 29 February lies in the half-open interval `[start, end)`.
fn feb29_in_half_open(start: Date, end: Date) -> bool {
    for year in start.year()..=end.year() {
        if time::util::is_leap_year(year)
            && let Ok(feb_29) = Date::from_calendar_date(year, Month::February, 29)
            && feb_29 >= start
            && feb_29 < end
        {
            return true;
        }
    }
    false
}
