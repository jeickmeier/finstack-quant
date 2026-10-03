//! Actual/Actual convention implementations.

use time::{Date, Month};

use super::DayCountContext;
use crate::dates::date_extensions::DateExt;
use crate::dates::tenor::TenorUnit;
use crate::error::InputError;

const MAX_ACT_ACT_ISMA_PERIODS: usize = 512;

/// Calculate ACT/ACT (ICMA/ISMA) year fraction using explicit reference coupon boundaries.
///
/// This helper is intended for irregular first/last coupons where the regular
/// coupon period cannot be inferred from `start`, `end`, and `frequency` alone.
/// The `reference_start`/`reference_end` pair must describe one unadjusted regular
/// coupon period from the contractual schedule. Its nominal month length is
/// inferred by rounding `12 * reference_days / 365`, so February clamping does
/// not shorten the coupon tenor. The endpoints must reproduce the same nominal
/// month grid; business-day-adjusted payment dates are not reference boundaries.
/// Nominal boundaries outside that reference are generated from the original
/// unclamped endpoint; if both endpoints are month-end, the grid retains EOM.
///
/// Use this helper when you already know the surrounding regular coupon period
/// from the bond schedule. For regular coupons, prefer
/// [`super::DayCount::ActActIsma`] with a [`DayCountContext`] that
/// supplies only the coupon frequency.
///
/// # Arguments
///
/// * `start` - Accrual start date of the coupon being measured
/// * `end` - Accrual end date of the coupon being measured
/// * `reference_start` - Unadjusted start of the corresponding regular coupon period.
/// * `reference_end` - Unadjusted end of that regular coupon period on the same nominal month grid.
///
/// # Returns
///
/// The ICMA/ISMA year fraction for the irregular coupon period.
///
/// # Errors
///
/// Returns an error if the accrual dates are reversed, the reference period is
/// invalid, its endpoints do not reproduce one nominal month grid, or alignment
/// exceeds the supported number of nominal periods.
///
/// # References
///
/// - ICMA convention background: `docs/REFERENCES.md#icma-rule-book`
pub fn act_act_isma_year_fraction_with_reference_period(
    start: Date,
    end: Date,
    reference_start: Date,
    reference_end: Date,
) -> crate::Result<f64> {
    if start > end {
        return Err(InputError::InvalidDateRange.into());
    }
    if start == end {
        return Ok(0.0);
    }
    if reference_start >= reference_end {
        return Err(InputError::InvalidDateRange.into());
    }

    let reference_days = (reference_end - reference_start).whole_days() as f64;
    let period_months = (12.0 * reference_days / 365.0).round() as u32;
    if period_months == 0 {
        return Err(InputError::Invalid.into());
    }
    let coupon_length_years = period_months as f64 / 12.0;
    let preserve_eom = reference_start == reference_start.end_of_month()
        && reference_end == reference_end.end_of_month();
    // The larger day identifies the unclamped roll anchor: Jan 30 / Feb 28
    // and Feb 28 / Mar 30 both describe a roll on the 30th. Generate every
    // nominal date from that original anchor, never from a February clamp.
    let (anchor, anchor_index) = if reference_start.day() >= reference_end.day() {
        (reference_start, 0)
    } else {
        (reference_end, 1)
    };
    let period_months = i32::try_from(period_months).map_err(|_| InputError::Invalid)?;
    let boundary = |index: i32| -> crate::Result<Date> {
        if index.unsigned_abs() as usize >= MAX_ACT_ACT_ISMA_PERIODS {
            return Err(crate::Error::Validation(
                "ACT/ACT ISMA reference-period traversal exceeded its supported range".into(),
            ));
        }
        let months = (index - anchor_index)
            .checked_mul(period_months)
            .ok_or(InputError::Invalid)?;
        let shifted = anchor.add_months(months)?;
        Ok(if preserve_eom {
            shifted.end_of_month()
        } else {
            shifted
        })
    };
    if boundary(0)? != reference_start || boundary(1)? != reference_end {
        return Err(crate::Error::Validation(
            "ACT/ACT ISMA reference endpoints must share an unadjusted nominal month grid".into(),
        ));
    }
    let mut index = 0;
    let mut period_start = reference_start;
    let mut period_end = reference_end;
    while start < period_start {
        index -= 1;
        period_end = period_start;
        period_start = boundary(index)?;
    }
    while start >= period_end {
        index += 1;
        period_start = period_end;
        period_end = boundary(index + 1)?;
    }

    let mut total = 0.0;
    loop {
        if period_start >= period_end {
            return Err(InputError::InvalidDateRange.into());
        }
        let overlap_start = start.max(period_start);
        let overlap_end = end.min(period_end);
        let overlap_days = (overlap_end - overlap_start).whole_days() as f64;
        let coupon_days = (period_end - period_start).whole_days() as f64;
        total += overlap_days / coupon_days * coupon_length_years;
        if end <= period_end {
            return Ok(total);
        }
        index += 1;
        period_start = period_end;
        period_end = boundary(index + 1)?;
    }
}
// ACT/ACT (ISDA) helper
pub(super) fn year_fraction_act_act_isda(start: Date, end: Date) -> crate::Result<f64> {
    if start == end {
        return Ok(0.0);
    }

    if start.year() == end.year() {
        let denom = days_in_year(start.year()) as f64;
        let days = (end - start).whole_days() as f64;
        return Ok(days / denom);
    }

    // Days from start to 31-Dec of start year (inclusive of start, exclusive of next year 1-Jan).
    let start_year_end = crate::dates::create_date(start.year() + 1, Month::January, 1)?;
    let days_start_year = (start_year_end - start).whole_days() as f64;
    let mut frac = days_start_year / days_in_year(start.year()) as f64;

    // Preserve per-year addition: bulk addition differs by one ULP for some
    // seeds because IEEE addition is not associative.
    for _year in (start.year() + 1)..end.year() {
        frac += 1.0; // each full year counts as exactly 1.0
    }

    // Days from 1-Jan of end year to end date
    let start_of_end_year = crate::dates::create_date(end.year(), Month::January, 1)?;
    let days_end_year = (end - start_of_end_year).whole_days() as f64;
    frac += days_end_year / days_in_year(end.year()) as f64;

    Ok(frac)
}

// Context-aware helpers for year_fraction_impl

/// ACT/ACT (ISMA) with context extraction.
///
/// When `ctx.coupon_period` is set, delegates to
/// [`act_act_isma_year_fraction_with_reference_period`] for exact
/// mid-coupon or stub accrual. Otherwise the frequency-only path is used
/// only when `[start, end)` is a regular period of `frequency`; irregular
/// coupons without a reference period return
/// [`InputError::MissingCouponPeriodForActActIsma`].
pub(super) fn year_fraction_act_act_isma_with_ctx(
    start: Date,
    end: Date,
    ctx: DayCountContext<'_>,
) -> crate::Result<f64> {
    let frequency = ctx
        .frequency
        .ok_or(InputError::MissingFrequencyForActActIsma)?;
    if let Some((ref_start, ref_end)) = ctx.coupon_period {
        return act_act_isma_year_fraction_with_reference_period(start, end, ref_start, ref_end);
    }
    let (months, coupon_fraction) = match frequency.unit() {
        TenorUnit::Months => (frequency.count() as i32, frequency.count() as f64 / 12.0),
        TenorUnit::Years => (frequency.count() as i32 * 12, frequency.count() as f64),
        TenorUnit::Weeks | TenorUnit::Days => {
            return Err(InputError::ActActIsmaUnsupportedFrequency {
                frequency: frequency.to_string(),
            }
            .into());
        }
    };
    if start == end {
        return Ok(0.0);
    }
    let mut total = 0.0;
    // Anchor every boundary on start, preserving month-end clamping. Only
    // complete periods are admitted without explicit reference dates.
    if months > 0 {
        for k in 1..=MAX_ACT_ACT_ISMA_PERIODS as i32 {
            let Some(step) = k.checked_mul(months) else {
                break;
            };
            let Ok(boundary) = start.add_months(step) else {
                break;
            };
            total += coupon_fraction;
            if boundary == end {
                return Ok(total);
            }
            if boundary > end {
                break;
            }
        }
    }
    Err(InputError::MissingCouponPeriodForActActIsma.into())
}

#[inline]
const fn days_in_year(year: i32) -> i32 {
    if time::util::is_leap_year(year) {
        366
    } else {
        365
    }
}

/// ACT/ACT AFB (Association Française des Banques / Actual/Actual Euro).
///
/// QuantLib `ActualActual::AFB`: walk whole years backwards from `end` until
/// the candidate is before `start`, then divide the residual actual days by
/// 366 if 29 February lies in `[start, residual_end)`, else 365.
pub(super) fn year_fraction_act_act_afb(start: Date, end: Date) -> f64 {
    if start == end {
        return 0.0;
    }

    let mut residual_end = end;
    let mut whole_years = 0.0;

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
            whole_years += 1.0;
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
    whole_years + days / den
}

/// True when 29 February lies in the half-open interval `[start, end)`.
fn feb29_in_half_open(start: Date, end: Date) -> bool {
    for year in start.year()..=end.year() {
        if time::util::is_leap_year(year) {
            if let Ok(feb_29) = Date::from_calendar_date(year, Month::February, 29) {
                if feb_29 >= start && feb_29 < end {
                    return true;
                }
            }
        }
    }
    false
}
