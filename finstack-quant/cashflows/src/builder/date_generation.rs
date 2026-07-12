//! Date and schedule generation utilities.
//!
//! This module provides functions for generating payment date schedules based on
//! frequency, stub rules, and business day conventions.
//!
//! ## Responsibilities
//!
//! - Generate skeletal periods for the canonical `build_periods` API
//! - Create helper maps for compiler previous-date lookups
//! - Apply business day adjustments using calendars

use super::calendar::resolve_calendar_strict;
use super::periods::SchedulePeriod;
use finstack_quant_core::dates::{
    adjust, BusinessDayConvention, Date, DateExt, HolidayCalendar, ScheduleBuilder, StubKind,
    Tenor, TenorUnit,
};

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
pub(crate) type IndexedPeriods = (
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
    periods: Vec<SchedulePeriod>,
    frequency: Tenor,
) -> finstack_quant_core::Result<IndexedPeriods> {
    let dates = periods.iter().map(|period| period.payment_date).collect();
    let first_or_last = periods
        .iter()
        .filter(|period| !is_regular_period(period.accrual_start, period.accrual_end, frequency))
        .map(|period| period.payment_date)
        .collect();
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

/// Generate skeletal periods between start/end with strict error handling.
///
/// # Errors
///
/// Returns `finstack_quant_core::Error` when:
/// - `calendar_id` is provided but calendar is not found
/// - Schedule generation fails due to invalid date ranges
/// - Business day adjustment fails
/// - `payment_lag_days` is negative (payment lags are forward-only)
///
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_periods(
    start: Date,
    end: Date,
    freq: Tenor,
    stub: StubKind,
    bdc: BusinessDayConvention,
    end_of_month: bool,
    payment_lag_days: i32,
    calendar_id: &str,
) -> finstack_quant_core::Result<Vec<SchedulePeriod>> {
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
        return Ok(Vec::new());
    }

    let mut periods = Vec::with_capacity(dates.len().saturating_sub(1));

    for window in dates.windows(2) {
        let period = build_schedule_period(window[0], window[1], bdc, payment_lag_days, cal)?;
        periods.push(period);
    }

    Ok(periods)
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
    fn generate_periods_errors_on_unknown_calendar() {
        let res = generate_periods(
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
        let periods = generate_periods(
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

        assert_eq!(periods.len(), 2);
        let (_, _, stubs) =
            index_period_schedule(periods, Tenor::semi_annual()).expect("period index");
        assert!(
            stubs.is_empty(),
            "regular periods must not be tagged as stubs"
        );
    }

    #[test]
    fn short_front_stub_is_tagged() {
        let periods = generate_periods(
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

        let first = periods[0];
        let (_, _, stubs) =
            index_period_schedule(periods, Tenor::semi_annual()).expect("period index");
        assert!(
            stubs.contains(&first.payment_date),
            "short-front stub must be tagged"
        );
        assert_eq!(stubs.len(), 1, "only the genuine stub period is tagged");
    }

    #[test]
    fn negative_payment_lag_is_rejected() {
        let res = generate_periods(
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
        let periods = generate_periods(
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

        let res = super::index_period_schedule(periods, Tenor::daily());
        let err = res.expect_err("duplicate adjusted payment dates must error");
        assert!(
            err.to_string().contains("same payment date"),
            "error should describe the payment-date collision: {err}"
        );
    }

    #[test]
    fn payment_lag_applies_after_business_day_adjustment() {
        let periods = generate_periods(
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

        assert_eq!(periods[0].payment_date, d(2030, 5, 8));
    }
}
