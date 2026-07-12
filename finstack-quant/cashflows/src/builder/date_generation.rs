//! Date and schedule generation utilities.
//!
//! This module provides functions for generating payment date schedules based on
//! frequency, stub rules, and business day conventions.
//!
//! ## Responsibilities
//!
//! - Generate period schedules with `build_dates` (strict validation)
//! - Create `PeriodSchedule` with helper maps for previous date lookups
//! - Apply business day adjustments using calendars

use super::calendar::resolve_calendar_strict;
use finstack_quant_core::dates::{
    adjust, BusinessDayConvention, Date, DateExt, HolidayCalendar, ScheduleBuilder, StubKind,
    Tenor, TenorUnit,
};

/// Accrual period with payment timing.
///
/// This is the canonical period type used across the cashflow builder (schedule
/// compilation) and rates instruments (enriched with reset dates and year
/// fractions). Fields that are not relevant in a given context are left at
/// their defaults (`None` / `0.0`).
#[derive(Debug, Clone, Copy)]
pub struct SchedulePeriod {
    /// Accrual start date (inclusive).
    pub accrual_start: Date,
    /// Accrual end date (exclusive boundary).
    pub accrual_end: Date,
    /// Payment date after applying payment lag.
    pub payment_date: Date,
    /// Optional reset/fixing date for floating legs.
    pub reset_date: Option<Date>,
    /// Accrual year fraction for the period (0.0 when not computed).
    pub accrual_year_fraction: f64,
}

/// Period schedule output with business day adjustment tracking.
#[derive(Debug, Clone)]
pub struct PeriodSchedule {
    /// Generated accrual/payment periods.
    pub periods: Vec<SchedulePeriod>,
    /// Payment dates for each period.
    pub dates: Vec<Date>,
    /// Set of payment dates whose accrual period is irregular (a genuine stub).
    ///
    /// A period is regular when stepping one schedule tenor forward from its
    /// accrual start (or backward from its accrual end) lands exactly on the
    /// other boundary; all other periods are stubs. A vanilla bullet schedule
    /// therefore has an empty set, while a short-front schedule contains only
    /// the first payment date. See `is_regular_period`.
    pub first_or_last: finstack_quant_core::HashSet<Date>,
}

/// Step `date` by `sign * tenor` using calendar-clamped month arithmetic.
///
/// Mirrors the stepping used by schedule generation (`add_months` clamps the
/// day-of-month into short months) so regularity checks agree with the
/// generated anchor grid.
fn step_tenor(date: Date, tenor: Tenor, sign: i32) -> Option<Date> {
    match tenor.unit() {
        TenorUnit::Days => {
            let days = i64::from(tenor.count()).checked_mul(i64::from(sign))?;
            Some(date + time::Duration::days(days))
        }
        TenorUnit::Weeks => {
            let days = i64::from(tenor.count())
                .checked_mul(7)?
                .checked_mul(i64::from(sign))?;
            Some(date + time::Duration::days(days))
        }
        TenorUnit::Months => {
            let months = i32::try_from(tenor.count()).ok()?.checked_mul(sign)?;
            Some(date.add_months(months))
        }
        TenorUnit::Years => {
            let months = i32::try_from(tenor.count())
                .ok()?
                .checked_mul(12)?
                .checked_mul(sign)?;
            Some(date.add_months(months))
        }
    }
}

/// Returns `true` when the accrual span `[accrual_start, accrual_end)` is a
/// regular period of length `freq`.
///
/// A period is regular when stepping one tenor forward from `accrual_start`
/// lands exactly on `accrual_end`, or stepping one tenor backward from
/// `accrual_end` lands exactly on `accrual_start` (the backward check covers
/// backward-generated schedules whose month-end anchors clamp on the forward
/// step). Used to label genuine stub coupons (`CFKind::Stub`) and to decide
/// whether the period itself can serve as the ACT/ACT ICMA reference period.
pub(crate) fn is_regular_period(accrual_start: Date, accrual_end: Date, freq: Tenor) -> bool {
    if accrual_start >= accrual_end {
        return false;
    }
    step_tenor(accrual_start, freq, 1) == Some(accrual_end)
        || step_tenor(accrual_end, freq, -1) == Some(accrual_start)
}

/// Business-day adjust both accrual boundaries of `period` in place.
///
/// Used for swap-style schedules where calculation periods are themselves
/// adjusted (ISDA 2006 §4.10; ARRC SOFR conventions). The payment date is
/// left untouched: it is already derived from `adjust(accrual_end, bdc)` plus
/// any payment lag, so it agrees with the adjusted accrual end.
///
/// # Errors
///
/// Returns `Error::Validation` when adjustment collapses the period to zero
/// length (adjusted start == adjusted end), which would otherwise silently
/// produce a zero-coupon period.
pub(crate) fn adjust_period_accruals(
    period: &mut SchedulePeriod,
    bdc: BusinessDayConvention,
    cal: &dyn HolidayCalendar,
) -> finstack_quant_core::Result<()> {
    let adj_start = adjust(period.accrual_start, bdc, cal)?;
    let adj_end = adjust(period.accrual_end, bdc, cal)?;
    if adj_start >= adj_end {
        return Err(finstack_quant_core::Error::Validation(format!(
            "accrual adjustment collapsed period [{}, {}) to zero length (adjusted to [{}, {}))",
            period.accrual_start, period.accrual_end, adj_start, adj_end
        )));
    }
    period.accrual_start = adj_start;
    period.accrual_end = adj_end;
    Ok(())
}

/// Build one skeletal schedule period from accrual bounds and payment lag.
pub(crate) fn build_schedule_period(
    accrual_start: Date,
    accrual_end: Date,
    bdc: BusinessDayConvention,
    payment_lag_days: i32,
    cal: &dyn HolidayCalendar,
) -> finstack_quant_core::Result<SchedulePeriod> {
    let adjusted_payment = adjust(accrual_end, bdc, cal)?;
    let payment_date = if payment_lag_days == 0 {
        adjusted_payment
    } else {
        adjusted_payment.add_business_days(payment_lag_days, cal)?
    };
    Ok(SchedulePeriod {
        accrual_start,
        accrual_end,
        payment_date,
        reset_date: None,
        accrual_year_fraction: 0.0,
    })
}

/// Payment-date indexed schedule: `(payment_dates, period_by_payment_date, stub_payment_dates)`.
pub(crate) type IndexedPeriodSchedule = (
    Vec<Date>,
    finstack_quant_core::HashMap<Date, SchedulePeriod>,
    finstack_quant_core::HashSet<Date>,
);

/// Convert a generated schedule into the compiler's payment-date index form.
///
/// # Errors
///
/// Returns `Error::Validation` when two distinct accrual periods adjust to the
/// same payment date (e.g. a daily-tenor schedule rolling a weekend with
/// `Following`, or a one-day stub adjacent to a holiday). A last-writer-wins
/// map would silently drop one of the periods' coupons.
pub(crate) fn index_period_schedule(
    schedule: PeriodSchedule,
) -> finstack_quant_core::Result<IndexedPeriodSchedule> {
    let PeriodSchedule {
        periods,
        dates,
        first_or_last,
    } = schedule;
    let mut period_map: finstack_quant_core::HashMap<Date, SchedulePeriod> =
        finstack_quant_core::HashMap::default();
    period_map.reserve(periods.len());
    for period in &periods {
        if let Some(previous) = period_map.insert(period.payment_date, *period) {
            return Err(duplicate_payment_date_error(
                period.payment_date,
                previous,
                *period,
            ));
        }
    }
    Ok((dates, period_map, first_or_last))
}

pub(crate) fn validate_unique_payment_dates(
    periods: &[SchedulePeriod],
) -> finstack_quant_core::Result<()> {
    let mut period_map: finstack_quant_core::HashMap<Date, SchedulePeriod> =
        finstack_quant_core::HashMap::default();
    period_map.reserve(periods.len());
    for period in periods {
        if let Some(previous) = period_map.insert(period.payment_date, *period) {
            return Err(duplicate_payment_date_error(
                period.payment_date,
                previous,
                *period,
            ));
        }
    }
    Ok(())
}

fn duplicate_payment_date_error(
    payment_date: Date,
    previous: SchedulePeriod,
    period: SchedulePeriod,
) -> finstack_quant_core::Error {
    finstack_quant_core::Error::Validation(format!(
        "two accrual periods adjust to the same payment date {}: [{}, {}) and [{}, {}); \
         use a coarser frequency or a different business-day convention",
        payment_date,
        previous.accrual_start,
        previous.accrual_end,
        period.accrual_start,
        period.accrual_end
    ))
}

/// Build a schedule between start/end with strict error handling.
///
/// # Errors
///
/// Returns `finstack_quant_core::Error` when:
/// - `calendar_id` is provided but calendar is not found
/// - Schedule generation fails due to invalid date ranges
/// - Business day adjustment fails
/// - `payment_lag_days` is negative (payment lags are forward-only)
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::dates::{Date, Tenor, BusinessDayConvention, StubKind, create_date};
/// use finstack_quant_cashflows::builder::date_generation::build_dates;
/// use time::Month;
///
/// let start = create_date(2025, Month::January, 15)?;
/// let end = create_date(2025, Month::July, 15)?;
/// let sched = build_dates(
///     start,
///     end,
///     Tenor::quarterly(),
///     StubKind::None,
///     BusinessDayConvention::Following,
///     false,
///     0,
///     "weekends_only",
/// )?;
/// assert!(sched.dates.len() >= 2);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[allow(clippy::too_many_arguments)]
pub fn build_dates(
    start: Date,
    end: Date,
    freq: Tenor,
    stub: StubKind,
    bdc: BusinessDayConvention,
    end_of_month: bool,
    payment_lag_days: i32,
    calendar_id: &str,
) -> finstack_quant_core::Result<PeriodSchedule> {
    if payment_lag_days < 0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "payment_lag_days must be non-negative; got {payment_lag_days}"
        )));
    }
    let builder = ScheduleBuilder::new(start, end)?
        .frequency(freq)
        .stub_rule(stub)
        .end_of_month(end_of_month);
    let cal = resolve_calendar_strict(calendar_id)?;

    let schedule = builder.build()?;
    let dates = schedule.dates;

    if dates.len() < 2 {
        return Ok(PeriodSchedule {
            periods: Vec::new(),
            dates: Vec::new(),
            first_or_last: finstack_quant_core::HashSet::default(),
        });
    }

    let mut periods = Vec::with_capacity(dates.len().saturating_sub(1));
    let mut payment_dates = Vec::with_capacity(dates.len().saturating_sub(1));
    let mut first_or_last = finstack_quant_core::HashSet::default();

    for window in dates.windows(2) {
        let period = build_schedule_period(window[0], window[1], bdc, payment_lag_days, cal)?;
        // Tag genuine stub periods only: a period is a stub when its accrual
        // span deviates from the schedule tenor. Positional first/last tagging
        // mislabeled every regular first/last coupon as `CFKind::Stub` in the
        // wire format .
        if !is_regular_period(period.accrual_start, period.accrual_end, freq) {
            first_or_last.insert(period.payment_date);
        }
        payment_dates.push(period.payment_date);
        periods.push(period);
    }

    Ok(PeriodSchedule {
        periods,
        dates: payment_dates,
        first_or_last,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("Valid month (1-12)"), day)
            .expect("Valid test date")
    }

    #[test]
    fn build_dates_errors_on_unknown_calendar() {
        let res = build_dates(
            d(2025, 1, 1),
            d(2025, 4, 1),
            Tenor::quarterly(),
            StubKind::None,
            BusinessDayConvention::Following,
            false,
            0,
            "NOT_A_CAL",
        );
        assert!(res.is_err());
    }

    #[test]
    fn regular_schedule_has_no_stub_periods() {
        let schedule = build_dates(
            d(2025, 1, 15),
            d(2026, 1, 15),
            Tenor::semi_annual(),
            StubKind::None,
            BusinessDayConvention::Following,
            false,
            0,
            "weekends_only",
        )
        .expect("schedule should build");

        assert_eq!(schedule.periods.len(), 2);
        assert!(
            schedule.first_or_last.is_empty(),
            "regular periods must not be tagged as stubs"
        );
    }

    #[test]
    fn short_front_stub_is_tagged() {
        let schedule = build_dates(
            d(2025, 1, 10),
            d(2026, 1, 15),
            Tenor::semi_annual(),
            StubKind::ShortFront,
            BusinessDayConvention::Following,
            false,
            0,
            "weekends_only",
        )
        .expect("schedule should build");

        let first = &schedule.periods[0];
        assert!(
            schedule.first_or_last.contains(&first.payment_date),
            "short-front stub must be tagged"
        );
        assert_eq!(
            schedule.first_or_last.len(),
            1,
            "only the genuine stub period is tagged"
        );
    }

    #[test]
    fn negative_payment_lag_is_rejected() {
        let res = build_dates(
            d(2025, 1, 1),
            d(2026, 1, 1),
            Tenor::quarterly(),
            StubKind::None,
            BusinessDayConvention::Following,
            false,
            -2,
            "weekends_only",
        );
        assert!(res.is_err(), "negative payment lag must be rejected");
    }

    #[test]
    fn duplicate_adjusted_payment_dates_are_rejected() {
        // Daily tenor across a weekend with Following: Sat and Sun both adjust
        // to Monday, colliding with the Sun->Mon period's payment date.
        let schedule = build_dates(
            d(2025, 1, 3), // Friday
            d(2025, 1, 6), // Monday
            Tenor::daily(),
            StubKind::None,
            BusinessDayConvention::Following,
            false,
            0,
            "weekends_only",
        )
        .expect("raw schedule should build");

        let res = super::index_period_schedule(schedule);
        let err = res.expect_err("duplicate adjusted payment dates must error");
        assert!(
            err.to_string().contains("same payment date"),
            "error should describe the payment-date collision: {err}"
        );
    }

    #[test]
    fn payment_lag_applies_after_business_day_adjustment() {
        let schedule = build_dates(
            d(2029, 5, 4),
            d(2030, 5, 4),
            Tenor::annual(),
            StubKind::None,
            BusinessDayConvention::ModifiedFollowing,
            false,
            2,
            "usny",
        )
        .expect("schedule should build");

        assert_eq!(schedule.periods[0].payment_date, d(2030, 5, 8));
    }
}
