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
use super::specs::RollRule;
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
/// regular period of length `frequency`.
///
/// A period is regular when stepping one tenor forward from `accrual_start`
/// lands exactly on `accrual_end`, or stepping one tenor backward from
/// `accrual_end` lands exactly on `accrual_start` (the backward check covers
/// backward-generated schedules whose month-end anchors clamp on the forward
/// step). Used to label genuine stub coupons (`CFKind::Stub`) and to decide
/// whether the period itself can serve as the ACT/ACT ICMA reference period.
pub(crate) fn is_regular_period(accrual_start: Date, accrual_end: Date, frequency: Tenor) -> bool {
    if accrual_start >= accrual_end {
        return false;
    }
    step_tenor(accrual_start, frequency, 1) == Some(accrual_end)
        || step_tenor(accrual_end, frequency, -1) == Some(accrual_start)
}

/// ACT/ACT ICMA quasi-coupon reference anchored on the contractual stub side.
pub(crate) fn icma_coupon_period(
    accrual_start: Date,
    accrual_end: Date,
    frequency: Tenor,
    stub: StubKind,
    end_of_month: bool,
) -> Option<(Date, Date)> {
    if is_regular_period(accrual_start, accrual_end, frequency) {
        return Some((accrual_start, accrual_end));
    }
    let roll = |date: Date| {
        if end_of_month {
            date.end_of_month()
        } else {
            date
        }
    };
    match stub {
        StubKind::ShortBack | StubKind::LongBack => Some((
            accrual_start,
            roll(step_tenor(accrual_start, frequency, 1)?),
        )),
        _ => Some((roll(step_tenor(accrual_end, frequency, -1)?), accrual_end)),
    }
}

/// Build one skeletal schedule period from raw accrual bounds and payment lag.
///
/// When `adjust_accrual_dates` is set, both accrual boundaries are
/// business-day adjusted (ISDA 2006 §4.10 adjusted Calculation Periods),
/// mirroring [`generate_periods_with_adjustment`]; the raw bounds are retained
/// as the period's unadjusted anchors either way.
pub(crate) fn build_schedule_period(
    accrual_start: Date,
    accrual_end: Date,
    business_day_convention: BusinessDayConvention,
    payment_lag_days: i32,
    cal: &dyn HolidayCalendar,
    adjust_accrual_dates: bool,
) -> finstack_quant_core::Result<SchedulePeriod> {
    let adjusted_end = adjust(accrual_end, business_day_convention, cal)?;
    let payment_date = if payment_lag_days == 0 {
        adjusted_end
    } else {
        adjusted_end.add_business_days(payment_lag_days, cal)?
    };
    let (acc_start, acc_end) = if adjust_accrual_dates {
        let adj_start = adjust(accrual_start, business_day_convention, cal)?;
        if adj_start >= adjusted_end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "accrual adjustment collapsed period [{accrual_start}, {accrual_end}) to zero \
                 length (adjusted to [{adj_start}, {adjusted_end}))"
            )));
        }
        (adj_start, adjusted_end)
    } else {
        (accrual_start, accrual_end)
    };
    Ok(SchedulePeriod {
        accrual_start: acc_start,
        accrual_end: acc_end,
        payment_date,
        reset_date: None,
        accrual_year_fraction: 0.0,
        unadjusted_start: accrual_start,
        unadjusted_end: accrual_end,
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
    // Regularity uses unadjusted dates (ISDA 2006 §4.16(c)).
    let first_or_last = periods
        .iter()
        .filter(|period| {
            !is_regular_period(period.unadjusted_start, period.unadjusted_end, frequency)
        })
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

/// Test helper: generate skeletal periods with unadjusted accrual boundaries.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_periods(
    start: Date,
    end: Date,
    frequency: Tenor,
    stub: StubKind,
    business_day_convention: BusinessDayConvention,
    end_of_month: bool,
    payment_lag_days: i32,
    calendar_id: &str,
) -> finstack_quant_core::Result<Vec<SchedulePeriod>> {
    generate_periods_with_adjustment(
        start,
        end,
        frequency,
        stub,
        business_day_convention,
        end_of_month,
        payment_lag_days,
        calendar_id,
        false,
        RollRule::None,
    )
}

/// Generate skeletal periods and apply the optional accrual-boundary policy.
///
/// Enrichment (day counts and reset dates) remains the caller's job because the
/// compiler needs raw periods for coupon-specific metadata.
///
/// `roll_rule` selects the core `ScheduleBuilder` anchor mode: the IMM modes
/// force a quarterly grid (overriding `frequency`/`stub` with quarterly /
/// short-back), and `RollRule::CdsImm` anchors the first period at the CDS
/// roll date preceding `start` (post-Big-Bang front accrual), so the first
/// accrual start may lie before `start`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_periods_with_adjustment(
    start: Date,
    end: Date,
    frequency: Tenor,
    stub: StubKind,
    business_day_convention: BusinessDayConvention,
    end_of_month: bool,
    payment_lag_days: i32,
    calendar_id: &str,
    adjust_accrual_dates: bool,
    roll_rule: RollRule,
) -> finstack_quant_core::Result<Vec<SchedulePeriod>> {
    if payment_lag_days < 0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "payment_lag_days must be non-negative; got {payment_lag_days}"
        )));
    }
    let builder = ScheduleBuilder::new(start, end)?
        .frequency(frequency)
        .stub_rule(stub)
        .end_of_month(end_of_month);
    let builder = match roll_rule {
        RollRule::None => builder,
        RollRule::Imm => builder.imm(),
        RollRule::CdsImm => builder.cds_imm(),
    };
    let cal = resolve_calendar_strict(calendar_id)?;

    let schedule = builder.build()?;
    let dates = schedule.dates;

    if dates.len() < 2 {
        return Ok(Vec::new());
    }

    // Share each BDC-adjusted anchor across adjacent periods; `dates[0]` is not a payment date.
    let mut adjusted: Vec<Date> = Vec::with_capacity(dates.len());
    for (index, &anchor) in dates.iter().enumerate() {
        if index == 0 && !adjust_accrual_dates {
            adjusted.push(anchor);
        } else {
            adjusted.push(adjust(anchor, business_day_convention, cal)?);
        }
    }

    let payment_dates: Option<Vec<Date>> = if adjust_accrual_dates {
        let mut resolved = Vec::with_capacity(dates.len() - 1);
        for window in adjusted.windows(2) {
            let adjusted_end = window[1];
            resolved.push(if payment_lag_days == 0 {
                adjusted_end
            } else {
                adjusted_end.add_business_days(payment_lag_days, cal)?
            });
        }
        Some(resolved)
    } else {
        None
    };
    let mut resolved_payments = payment_dates.into_iter().flatten();

    let mut periods = Vec::with_capacity(dates.len() - 1);
    for (raw, adj) in dates.windows(2).zip(adjusted.windows(2)) {
        let (raw_start, raw_end) = (raw[0], raw[1]);
        let adjusted_end = adj[1];
        let payment_date = match resolved_payments.next() {
            Some(date) => date,
            None if payment_lag_days == 0 => adjusted_end,
            None => adjusted_end.add_business_days(payment_lag_days, cal)?,
        };

        let (accrual_start, accrual_end) = if adjust_accrual_dates {
            let (adj_start, adj_end) = (adj[0], adjusted_end);
            if adj_start >= adj_end {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "accrual adjustment collapsed period [{raw_start}, {raw_end}) to zero length \
                     (adjusted to [{adj_start}, {adj_end}))"
                )));
            }
            (adj_start, adj_end)
        } else {
            (raw_start, raw_end)
        };

        periods.push(SchedulePeriod {
            accrual_start,
            accrual_end,
            payment_date,
            reset_date: None,
            accrual_year_fraction: 0.0,
            unadjusted_start: raw_start,
            unadjusted_end: raw_end,
        });
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
    fn adjust_is_idempotent_for_every_convention() {
        let cal = resolve_calendar_strict("usny").expect("usny calendar");
        let conventions = [
            BusinessDayConvention::Unadjusted,
            BusinessDayConvention::Following,
            BusinessDayConvention::ModifiedFollowing,
            BusinessDayConvention::Preceding,
            BusinessDayConvention::ModifiedPreceding,
            BusinessDayConvention::Nearest,
        ];

        let mut date = d(2025, 1, 1);
        let end = d(2026, 1, 1);
        while date < end {
            for business_day_convention in conventions {
                let once = adjust(date, business_day_convention, cal).expect("adjustment succeeds");
                let twice =
                    adjust(once, business_day_convention, cal).expect("adjustment succeeds");
                assert_eq!(
                    once, twice,
                    "adjust must be idempotent for {business_day_convention:?} at {date}"
                );
            }
            date += time::Duration::days(1);
        }
    }

    #[test]
    fn adjusted_accrual_boundaries_are_shared_between_consecutive_periods() {
        for (stub, start, end) in [
            (StubKind::None, d(2025, 1, 31), d(2027, 1, 31)),
            (StubKind::ShortFront, d(2025, 2, 14), d(2027, 1, 31)),
            (StubKind::ShortBack, d(2025, 1, 31), d(2026, 11, 15)),
        ] {
            let periods = generate_periods_with_adjustment(
                start,
                end,
                Tenor::quarterly(),
                stub,
                BusinessDayConvention::ModifiedFollowing,
                true,
                0,
                "usny",
                true,
                RollRule::None,
            )
            .expect("schedule builds");

            assert!(periods.len() >= 2, "need multiple periods for {stub:?}");
            for pair in periods.windows(2) {
                assert_eq!(
                    pair[0].accrual_end, pair[1].accrual_start,
                    "adjusted boundaries must be shared ({stub:?})"
                );
            }
        }
    }

    #[test]
    fn payment_date_equals_adjusted_accrual_end_without_lag() {
        let periods = generate_periods_with_adjustment(
            d(2025, 1, 31),
            d(2027, 1, 31),
            Tenor::quarterly(),
            StubKind::None,
            BusinessDayConvention::ModifiedFollowing,
            true,
            0,
            "usny",
            true,
            RollRule::None,
        )
        .expect("schedule builds");

        assert!(!periods.is_empty());
        for period in &periods {
            assert_eq!(
                period.payment_date, period.accrual_end,
                "zero-lag payment date must equal the adjusted accrual end"
            );
        }
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
    fn adjusted_accrual_regular_periods_are_not_stubs() {
        // Regularity uses unadjusted dates (ISDA 2006 §4.16(c)).
        let periods = generate_periods_with_adjustment(
            d(2025, 1, 4), // Saturday anchor
            d(2026, 1, 4), // Sunday anchor
            Tenor::semi_annual(),
            StubKind::None,
            BusinessDayConvention::ModifiedFollowing,
            false,
            0,
            "weekends_only",
            true,
            RollRule::None,
        )
        .expect("schedule builds");

        assert_eq!(periods.len(), 2);
        let (_, _, stubs) =
            index_period_schedule(periods, Tenor::semi_annual()).expect("period index");
        assert!(
            stubs.is_empty(),
            "adjusted regular periods must not be tagged as stubs: {stubs:?}"
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
