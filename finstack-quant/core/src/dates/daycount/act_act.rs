//! Actual/Actual convention implementations.

use smallvec::SmallVec;
use time::{Date, Month};

use super::DayCountContext;
use crate::dates::date_extensions::DateExt;
use crate::dates::tenor::TenorUnit;
use crate::dates::Tenor;
use crate::error::InputError;

const MAX_ACT_ACT_ISMA_RECURSION_DEPTH: usize = 512;

/// Calculate ACT/ACT (ICMA/ISMA) year fraction using explicit reference coupon boundaries.
///
/// This helper is intended for irregular first/last coupons where the regular
/// coupon period cannot be inferred from `start`, `end`, and `frequency` alone.
/// The `reference_start`/`reference_end` pair must describe one regular coupon
/// period from the underlying schedule.
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
/// * `reference_start` - Start of the corresponding regular coupon period
/// * `reference_end` - End of the corresponding regular coupon period
///
/// # Returns
///
/// The ICMA/ISMA year fraction for the irregular coupon period.
///
/// # Errors
///
/// Returns an error if the accrual dates are reversed, the reference period is
/// invalid, or the algorithm would need an implausibly deep recursion to align
/// the supplied reference period.
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

    let period_months = reference_start.months_until(reference_end);
    if period_months == 0 {
        return Err(InputError::Invalid.into());
    }
    let coupon_length_years = period_months as f64 / 12.0;
    let preserve_eom = reference_start == reference_start.end_of_month()
        && reference_end == reference_end.end_of_month();
    #[derive(Clone, Copy)]
    struct Traversal {
        period_months: u32,
        coupon_length_years: f64,
        preserve_eom: bool,
    }
    let traversal = Traversal {
        period_months,
        coupon_length_years,
        preserve_eom,
    };

    fn recurse(
        start: Date,
        end: Date,
        reference_start: Date,
        reference_end: Date,
        traversal: Traversal,
        depth: usize,
    ) -> crate::Result<f64> {
        if start == end {
            return Ok(0.0);
        }
        if depth >= MAX_ACT_ACT_ISMA_RECURSION_DEPTH {
            tracing::warn!(
                "ACT/ACT ISMA reference-period traversal exceeded maximum depth of {MAX_ACT_ACT_ISMA_RECURSION_DEPTH}"
            );
            return Err(InputError::Invalid.into());
        }
        if reference_start >= reference_end {
            return Err(InputError::InvalidDateRange.into());
        }

        if start >= reference_start && end <= reference_end {
            let accrual_days = (end - start).whole_days() as f64;
            let reference_days = (reference_end - reference_start).whole_days() as f64;
            if reference_days <= 0.0 {
                return Err(InputError::Invalid.into());
            }
            return Ok((accrual_days / reference_days) * traversal.coupon_length_years);
        }

        let period_months_i32 =
            i32::try_from(traversal.period_months).map_err(|_| InputError::Invalid)?;
        let shift = |date: Date, months: i32| {
            let shifted = date.add_months(months);
            if traversal.preserve_eom {
                shifted.end_of_month()
            } else {
                shifted
            }
        };

        if end <= reference_start {
            let previous_start = shift(reference_start, -period_months_i32);
            return recurse(
                start,
                end,
                previous_start,
                reference_start,
                traversal,
                depth + 1,
            );
        }

        if start >= reference_end {
            let next_end = shift(reference_end, period_months_i32);
            return recurse(start, end, reference_end, next_end, traversal, depth + 1);
        }

        if start < reference_start {
            let previous_start = shift(reference_start, -period_months_i32);
            return Ok(recurse(
                start,
                reference_start,
                previous_start,
                reference_start,
                traversal,
                depth + 1,
            )? + recurse(
                reference_start,
                end,
                reference_start,
                reference_end,
                traversal,
                depth + 1,
            )?);
        }

        if end > reference_end {
            let next_end = shift(reference_end, period_months_i32);
            return Ok(recurse(
                start,
                reference_end,
                reference_start,
                reference_end,
                traversal,
                depth + 1,
            )? + recurse(
                reference_end,
                end,
                reference_end,
                next_end,
                traversal,
                depth + 1,
            )?);
        }

        Err(InputError::Invalid.into())
    }

    recurse(start, end, reference_start, reference_end, traversal, 0)
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
    match frequency.unit() {
        TenorUnit::Weeks | TenorUnit::Days => {
            return Err(InputError::ActActIsmaUnsupportedFrequency {
                frequency: frequency.to_string(),
            }
            .into());
        }
        TenorUnit::Months | TenorUnit::Years => {}
    }
    if start == end {
        return Ok(0.0);
    }
    if is_regular_frequency_period(start, end, frequency) {
        year_fraction_act_act_isma(start, end, frequency)
    } else {
        Err(InputError::MissingCouponPeriodForActActIsma.into())
    }
}

/// Returns true when `[start, end)` is an integer number of `frequency` coupons.
///
/// A span is regular when some positive multiple of the tenor steps from
/// `start` to `end`, or the same check run backward from `end` (month-end
/// clamping can make the forward step miss). Irregular stubs fail both
/// directions and require an explicit `coupon_period`.
fn is_regular_frequency_period(start: Date, end: Date, frequency: Tenor) -> bool {
    if start >= end {
        return false;
    }
    let months = match frequency.unit() {
        TenorUnit::Months => frequency.count() as i32,
        TenorUnit::Years => frequency.count() as i32 * 12,
        TenorUnit::Weeks | TenorUnit::Days => return false,
    };
    if months <= 0 {
        return false;
    }
    let mut k: i32 = 1;
    while k <= MAX_ACT_ACT_ISMA_RECURSION_DEPTH as i32 {
        let Some(step) = k.checked_mul(months) else {
            break;
        };
        if start.add_months(step) == end || end.add_months(-step) == start {
            return true;
        }
        if start.add_months(step) > end && end.add_months(-step) < start {
            break;
        }
        k += 1;
    }
    false
}

// ACT/ACT (ISMA/ICMA) helper
/// Calculate year fraction for ACT/ACT (ISMA/ICMA) convention with coupon-period awareness.
fn year_fraction_act_act_isma(start: Date, end: Date, frequency: Tenor) -> crate::Result<f64> {
    if start == end {
        return Ok(0.0);
    }

    // Coupon length in years based on frequency (e.g., 0.5 for semi-annual, 0.25 for quarterly).
    // ISMA/ICMA is defined for regular coupon periods; treat Week/Day frequencies as invalid.
    let coupon_length_years = match frequency.unit() {
        TenorUnit::Months => frequency.count() as f64 / 12.0,
        TenorUnit::Years => frequency.count() as f64,
        TenorUnit::Weeks | TenorUnit::Days => {
            return Err(InputError::ActActIsmaUnsupportedFrequency {
                frequency: frequency.to_string(),
            }
            .into());
        }
    };

    // For ISMA, we need to work with quasi-coupon periods.
    //
    // The quasi-coupon grid is anchored on `start` itself: each boundary is
    // `start + k·frequency` computed directly from the unadjusted anchor
    // (k-multiples, roll-day preserved with per-month clamping), NOT by
    // chaining `prev + frequency`. Chained stepping from `start - frequency` (the
    // previous implementation) lost the roll day for month-end starts: a
    // regular EOM semi-annual period [2025-08-31, 2026-02-28) drifted to a
    // grid ending Aug 28 and returned 181/184 × 0.5 ≈ 0.49185 instead of
    // exactly 0.5 .
    let months_per_period = match frequency.unit() {
        TenorUnit::Months => frequency.count() as i32,
        TenorUnit::Years => frequency.count() as i32 * 12,
        // Unreachable: rejected above when computing `coupon_length_years`.
        TenorUnit::Weeks | TenorUnit::Days => {
            return Err(InputError::ActActIsmaUnsupportedFrequency {
                frequency: frequency.to_string(),
            }
            .into());
        }
    };
    if months_per_period <= 0 {
        return Err(InputError::ActActIsmaUnsupportedFrequency {
            frequency: frequency.to_string(),
        }
        .into());
    }

    let mut total_fraction = 0.0;

    // Optimization: Manually generate dates to avoid heap allocation of ScheduleBuilder
    // Most ISMA calculations involve very few periods, but long-dated bonds (15+ years)
    // with semi-annual coupons can have 30+ periods. Using 32 elements covers ~16 years
    // of semi-annual coupons without heap allocation.
    let mut periods: SmallVec<[Date; 32]> = SmallVec::new();
    periods.push(start);
    let mut k: i32 = 1;
    loop {
        let boundary = start.add_months(k * months_per_period);
        periods.push(boundary);
        if boundary >= end {
            break;
        }
        k += 1;
    }

    // Find the periods that overlap with our [start, end) interval
    for window in periods.windows(2) {
        let period_start = window[0];
        let period_end = window[1];

        let overlap_start = start.max(period_start);
        let overlap_end = end.min(period_end);

        if overlap_start < overlap_end {
            // Numerator: actual days in the overlapping slice
            let days_in_overlap = (overlap_end - overlap_start).whole_days() as f64;

            // Denominator (ISMA): actual days in the coupon period that contains this slice
            let coupon_days = (period_end - period_start).whole_days() as f64;
            if coupon_days <= 0.0 {
                return Err(InputError::Invalid.into());
            }

            // Year fraction = (days in slice / days in coupon period) × coupon period in years
            total_fraction += (days_in_overlap / coupon_days) * coupon_length_years;
        }
    }

    Ok(total_fraction)
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

    loop {
        let mut candidate = residual_end.add_months(-12);
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
