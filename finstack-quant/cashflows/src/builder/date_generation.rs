//! Date and schedule generation utilities.
//!
//! This module provides functions for generating payment date schedules based on
//! frequency, stub rules, and business day conventions.
//!
//! ## Responsibilities
//!
//! - Generate skeletal periods for the canonical `build_periods` API
//! - Classify compiler periods using contractual stub metadata
//! - Apply business day adjustments using calendars

use super::periods::{BuildPeriodsParams, SchedulePeriod};
use super::specs::RollRule;
use finstack_quant_core::dates::calendar_by_id_strict;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, GridMode, HolidayCalendar, Schedule, StubKind,
    Tenor, TenorUnit, adjust, is_cds_date, is_imm_date, next_cds_date, prev_cds_date,
};

/// Reject convention combinations that cannot supply an unambiguous ICMA grid.
///
/// # Arguments
///
/// * `start` - Unadjusted contractual start, also the anchor for back stubs.
/// * `end` - Unadjusted contractual end, also the anchor for front stubs.
/// * `stub` - Effective stub convention after resolving an explicit roll grid.
/// * `end_of_month` - Whether intermediate nominal dates roll to month-end.
/// * `day_count` - Coupon accrual convention whose reference-grid requirements are checked.
/// * `roll_rule` - Plain month grid, CDS twentieth grid, or third-Wednesday IMM grid.
pub(crate) fn validate_accrual_grid(
    start: Date,
    end: Date,
    stub: StubKind,
    end_of_month: bool,
    day_count: DayCount,
    roll_rule: RollRule,
) -> finstack_quant_core::Result<()> {
    if end_of_month && !matches!(roll_rule, RollRule::None) {
        return Err(finstack_quant_core::Error::Validation(
            "end_of_month cannot be combined with an IMM or CDS IMM roll grid".into(),
        ));
    }
    if day_count == DayCount::ActActIsma {
        if matches!(roll_rule, RollRule::Imm) {
            return Err(finstack_quant_core::Error::Validation(
                "ACT/ACT ICMA requires a nominal month grid; third-Wednesday IMM accrual is not supported".into(),
            ));
        }
        let anchor = match stub {
            StubKind::ShortFront | StubKind::LongFront => end,
            _ => start,
        };
        if end_of_month && anchor != anchor.end_of_month() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "ACT/ACT ICMA end-of-month schedules require the regular grid anchor {anchor} \
                 to be month-end; use a front stub for a month-end maturity or a back stub \
                 for a month-end start"
            )));
        }
    }
    Ok(())
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
            date.add_days(days).ok()
        }
        TenorUnit::Weeks => {
            let days = i64::from(tenor.count())
                .checked_mul(7)?
                .checked_mul(i64::from(sign))?;
            date.add_days(days).ok()
        }
        TenorUnit::Months => {
            let months = i32::try_from(tenor.count()).ok()?.checked_mul(sign)?;
            date.add_months(months).ok()
        }
        TenorUnit::Years => {
            let months = i32::try_from(tenor.count())
                .ok()?
                .checked_mul(12)?
                .checked_mul(sign)?;
            date.add_months(months).ok()
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
///
/// # Arguments
///
/// * `accrual_start` - Unadjusted coupon start, including any interior program cut.
/// * `accrual_end` - Unadjusted coupon end before payment-date adjustment.
/// * `frequency` - Effective nominal coupon tenor, quarterly for CDS IMM.
/// * `stub` - Selects the adjacent reference coupon for a plain-grid stub.
/// * `end_of_month` - Whether a plain-grid reference date must retain month-end rolling.
/// * `roll_rule` - CDS IMM uses the twentieth grid even for a clipped front coupon.
///
/// # Errors
///
/// Returns an error when the reference coupon lies outside the date range.
pub(crate) fn icma_coupon_period(
    accrual_start: Date,
    accrual_end: Date,
    frequency: Tenor,
    stub: StubKind,
    end_of_month: bool,
    roll_rule: RollRule,
) -> finstack_quant_core::Result<(Date, Date)> {
    if matches!(roll_rule, RollRule::CdsImm) {
        let reference_start = if is_cds_date(accrual_start) {
            accrual_start
        } else {
            prev_cds_date(accrual_start)?
        };
        return Ok((reference_start, next_cds_date(reference_start)?));
    }
    if is_regular_period(accrual_start, accrual_end, frequency) {
        return Ok((accrual_start, accrual_end));
    }
    let roll = |date: Date| {
        if end_of_month {
            date.end_of_month()
        } else {
            date
        }
    };
    let out_of_range = || {
        finstack_quant_core::Error::Validation(format!(
            "the reference coupon of {accrual_start}..{accrual_end} at {frequency} lies outside \
             the date range"
        ))
    };
    match stub {
        StubKind::ShortBack | StubKind::LongBack => Ok((
            accrual_start,
            roll(step_tenor(accrual_start, frequency, 1).ok_or_else(out_of_range)?),
        )),
        _ => Ok((
            roll(step_tenor(accrual_end, frequency, -1).ok_or_else(out_of_range)?),
            accrual_end,
        )),
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
    if accrual_start >= accrual_end {
        return Err(finstack_quant_core::Error::Validation(
            "a schedule period requires start strictly before end".into(),
        ));
    }
    let adjusted_end = adjust(accrual_end, business_day_convention, cal)?;
    let adjusted_start = if adjust_accrual_dates {
        adjust(accrual_start, business_day_convention, cal)?
    } else {
        accrual_start
    };
    assemble_period(
        (accrual_start, accrual_end),
        (adjusted_start, adjusted_end),
        payment_lag_days,
        adjust_accrual_dates,
        cal,
    )
}

// Both callers validate raw bounds and lag before adjusting anchors. Schedule
// generation shares each adjusted anchor between its adjacent periods.
fn assemble_period(
    raw_bounds: (Date, Date),
    adjusted_bounds: (Date, Date),
    payment_lag_days: i32,
    adjust_accrual_dates: bool,
    cal: &dyn HolidayCalendar,
) -> finstack_quant_core::Result<SchedulePeriod> {
    let (raw_start, raw_end) = raw_bounds;
    let (adjusted_start, adjusted_end) = adjusted_bounds;
    let payment_date = if payment_lag_days == 0 {
        adjusted_end
    } else {
        adjusted_end.add_business_days(payment_lag_days, cal)?
    };
    let (accrual_start, accrual_end) = if adjust_accrual_dates {
        if adjusted_start >= adjusted_end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "accrual adjustment collapsed period [{raw_start}, {raw_end}) to zero length \
                 (adjusted to [{adjusted_start}, {adjusted_end}))"
            )));
        }
        adjusted_bounds
    } else {
        raw_bounds
    };
    Ok(SchedulePeriod {
        accrual_start,
        accrual_end,
        payment_date,
        reset_date: None,
        accrual_year_fraction: 0.0,
        unadjusted_start: raw_start,
        unadjusted_end: raw_end,
    })
}

/// One contractual period and its original-grid stub classification.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CompiledPeriod {
    pub(crate) period: SchedulePeriod,
    pub(crate) is_stub: bool,
}

/// Classify periods without duplicating their accrual and payment dates.
///
/// # Arguments
///
/// * `periods` - Ordered contractual accrual periods and their adjusted payment dates.
/// * `frequency` - Effective coupon tenor used to classify plain-grid regular coupons.
/// * `roll_rule` - Explicit roll grid distinguishing regular coupons from stubs.
///
/// # Errors
///
/// Rejects distinct periods that adjust to the same payment date.
pub(crate) fn compile_periods(
    periods: Vec<SchedulePeriod>,
    frequency: Tenor,
    roll_rule: RollRule,
) -> finstack_quant_core::Result<Vec<CompiledPeriod>> {
    validate_unique_payment_dates(&periods)?;
    Ok(periods
        .into_iter()
        .map(|period| {
            // Regularity uses unadjusted dates (ISDA 2006 §4.16(c)).
            let start = period.unadjusted_start;
            let end = period.unadjusted_end;
            let is_stub = match roll_rule {
                RollRule::Imm => !is_regular_imm_period(start, end, frequency),
                RollRule::CdsImm => !is_cds_date(start) || next_cds_date(start).ok() != Some(end),
                RollRule::None => !is_regular_period(start, end, frequency),
            };
            CompiledPeriod { period, is_stub }
        })
        .collect())
}

/// An IMM-rolled period is regular when it runs between IMM dates exactly one
/// `frequency` of months apart.
fn is_regular_imm_period(start: Date, end: Date, frequency: Tenor) -> bool {
    let month_index = |date: Date| i64::from(date.year()) * 12 + i64::from(u8::from(date.month()));
    is_imm_date(start)
        && is_imm_date(end)
        && frequency
            .months()
            .is_some_and(|months| month_index(end) - month_index(start) == i64::from(months))
}

pub(crate) fn validate_unique_payment_dates(
    periods: &[SchedulePeriod],
) -> finstack_quant_core::Result<()> {
    let mut period_map: finstack_quant_core::FxHashMap<Date, SchedulePeriod> =
        finstack_quant_core::FxHashMap::default();
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
    generate_periods_with_adjustment(&BuildPeriodsParams {
        start,
        end,
        frequency,
        stub,
        business_day_convention,
        calendar_id,
        end_of_month,
        day_count: DayCount::Act360,
        payment_lag_days,
        reset_lag_days: None,
        adjust_accrual_dates: false,
        roll_rule: RollRule::None,
    })
}

/// Generate skeletal periods and apply the optional accrual-boundary policy.
///
/// Reads the window, frequency, stub, business-day convention, end-of-month
/// flag, payment lag, calendar, accrual-adjustment flag and roll rule from
/// `params`; `day_count` and `reset_lag_days` are ignored here. Enrichment (day
/// counts and reset dates) remains the caller's job because the compiler needs
/// raw periods for coupon-specific metadata.
///
/// `roll_rule` and `end_of_month` select the core schedule grid.
/// `RollRule::Imm` rolls the regular periods on IMM dates at `frequency` with
/// `stub` (`GridMode::QuarterlyImm`; the regular anchor must be an IMM date),
/// and `end_of_month` selects `GridMode::EndOfMonth` (a month-end anchor);
/// the two together are an error.
/// `RollRule::CdsImm` rolls on CDS dates (the 20th of March, June,
/// September and December) at the leg's own `frequency` and `stub`; the
/// regular anchor (`start` for back or no stub, `end` for front stubs) must
/// itself be a CDS roll date, since none is substituted (record
/// CORE-CONVENTIONS-008).
pub(crate) fn generate_periods_with_adjustment(
    params: &BuildPeriodsParams<'_>,
) -> finstack_quant_core::Result<Vec<SchedulePeriod>> {
    let &BuildPeriodsParams {
        start,
        end,
        frequency,
        stub,
        business_day_convention,
        calendar_id,
        end_of_month,
        payment_lag_days,
        adjust_accrual_dates,
        roll_rule,
        ..
    } = params;
    if payment_lag_days < 0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "payment_lag_days must be non-negative; got {payment_lag_days}"
        )));
    }
    let grid = match (roll_rule, end_of_month) {
        (RollRule::Imm, true) => {
            return Err(finstack_quant_core::Error::Validation(
                "an IMM roll rule cannot also use the end-of-month grid".to_string(),
            ));
        }
        (RollRule::Imm, false) => GridMode::QuarterlyImm,
        (_, true) => GridMode::EndOfMonth,
        (_, false) => GridMode::Natural,
    };
    // Only the raw grid is taken from the core schedule; payment adjustment
    // and lags are applied below with this leg's own calendar, so the core
    // payment series is deliberately unadjusted.
    let builder = Schedule::builder(start, end)?
        .frequency(frequency)
        .stub_rule(stub)
        .grid(grid)
        .unadjusted();
    // A CDS grid is the natural grid from a regular anchor on a CDS roll
    // date (the start for back stubs, the end for front stubs); no anchor is
    // substituted (record CORE-CONVENTIONS-008).
    if matches!(roll_rule, RollRule::CdsImm) {
        let anchor = if matches!(stub, StubKind::ShortFront | StubKind::LongFront) {
            end
        } else {
            start
        };
        if !is_cds_date(anchor) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "a cds_imm roll rule needs its regular anchor ({anchor}, the {} with a {stub} \
                 stub) on a CDS roll date (20 Mar/Jun/Sep/Dec)",
                if anchor == end { "end" } else { "start" }
            )));
        }
    }
    let cal = calendar_by_id_strict(calendar_id)?;
    // A built schedule always has at least two grid dates (record
    // CORE-CONVENTIONS-008 #13), so there is no empty-period fallback.
    let schedule = builder.build()?;
    let dates = schedule.get_dates();

    // Share each BDC-adjusted anchor across adjacent periods; `dates[0]` is not a payment date.
    let mut adjusted: Vec<Date> = Vec::with_capacity(dates.len());
    for (index, &anchor) in dates.iter().enumerate() {
        if index == 0 && !adjust_accrual_dates {
            adjusted.push(anchor);
        } else {
            adjusted.push(adjust(anchor, business_day_convention, cal)?);
        }
    }

    let mut periods = Vec::with_capacity(dates.len() - 1);
    for (raw, adj) in dates.windows(2).zip(adjusted.windows(2)) {
        periods.push(assemble_period(
            (raw[0], raw[1]),
            (adj[0], adj[1]),
            payment_lag_days,
            adjust_accrual_dates,
            cal,
        )?);
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

    /// Quarterly, unstubbed, Modified Following on `usny` with adjusted
    /// accruals and no lag; tests override the conventions they vary.
    fn window(start: Date, end: Date) -> BuildPeriodsParams<'static> {
        BuildPeriodsParams {
            start,
            end,
            frequency: Tenor::quarterly(),
            stub: StubKind::None,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 0,
            reset_lag_days: None,
            adjust_accrual_dates: true,
            roll_rule: RollRule::None,
        }
    }

    #[test]
    fn adjust_is_idempotent_for_every_convention() {
        let cal = calendar_by_id_strict("usny").expect("usny calendar");
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
            let periods = generate_periods_with_adjustment(&BuildPeriodsParams {
                stub,
                end_of_month: true,
                ..window(start, end)
            })
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
        let periods = generate_periods_with_adjustment(&BuildPeriodsParams {
            end_of_month: true,
            ..window(d(2025, 1, 31), d(2027, 1, 31))
        })
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
        let compiled =
            compile_periods(periods, Tenor::semi_annual(), RollRule::None).expect("period index");
        assert!(
            compiled.iter().all(|entry| !entry.is_stub),
            "regular periods must not be tagged as stubs"
        );
    }

    #[test]
    fn adjusted_accrual_regular_periods_are_not_stubs() {
        // Regularity uses unadjusted dates (ISDA 2006 §4.16(c)).
        let periods = generate_periods_with_adjustment(&BuildPeriodsParams {
            frequency: Tenor::semi_annual(),
            calendar_id: "weekends_only",
            // Saturday and Sunday anchors.
            ..window(d(2025, 1, 4), d(2026, 1, 4))
        })
        .expect("schedule builds");

        assert_eq!(periods.len(), 2);
        let compiled =
            compile_periods(periods, Tenor::semi_annual(), RollRule::None).expect("period index");
        assert!(
            compiled.iter().all(|entry| !entry.is_stub),
            "adjusted regular periods must not be tagged as stubs: {compiled:?}"
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
        let compiled =
            compile_periods(periods, Tenor::semi_annual(), RollRule::None).expect("period index");
        assert!(
            compiled
                .iter()
                .any(|entry| entry.is_stub && entry.period.payment_date == first.payment_date),
            "short-front stub must be tagged"
        );
        assert_eq!(
            compiled.iter().filter(|entry| entry.is_stub).count(),
            1,
            "only the genuine stub period is tagged"
        );
    }

    #[test]
    fn explicit_periods_match_generated_periods_across_adjustments_and_lags() {
        for calendar_id in ["weekends_only", "usny"] {
            let calendar = calendar_by_id_strict(calendar_id).expect("calendar");
            for convention in [
                BusinessDayConvention::Unadjusted,
                BusinessDayConvention::Following,
                BusinessDayConvention::ModifiedFollowing,
                BusinessDayConvention::Preceding,
            ] {
                for adjust_accrual in [false, true] {
                    for lag in [0, 2] {
                        let periods = generate_periods_with_adjustment(&BuildPeriodsParams {
                            frequency: Tenor::monthly(),
                            business_day_convention: convention,
                            calendar_id,
                            end_of_month: true,
                            payment_lag_days: lag,
                            adjust_accrual_dates: adjust_accrual,
                            ..window(d(2025, 1, 31), d(2025, 12, 31))
                        })
                        .expect("generated monthly periods");
                        for period in periods {
                            let explicit = build_schedule_period(
                                period.unadjusted_start,
                                period.unadjusted_end,
                                convention,
                                lag,
                                calendar,
                                adjust_accrual,
                            )
                            .expect("explicit period");
                            assert_eq!(explicit.accrual_start, period.accrual_start);
                            assert_eq!(explicit.accrual_end, period.accrual_end);
                            assert_eq!(explicit.payment_date, period.payment_date);
                            assert_eq!(explicit.unadjusted_start, period.unadjusted_start);
                            assert_eq!(explicit.unadjusted_end, period.unadjusted_end);
                            assert_eq!(explicit.reset_date, period.reset_date);
                            assert_eq!(
                                explicit.accrual_year_fraction,
                                period.accrual_year_fraction
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn explicit_and_generated_periods_reject_collapsed_accrual_bounds() {
        let start = d(2025, 1, 4);
        let end = d(2025, 1, 5);
        let calendar = calendar_by_id_strict("weekends_only").expect("calendar");
        let explicit = build_schedule_period(
            start,
            end,
            BusinessDayConvention::Following,
            2,
            calendar,
            true,
        )
        .expect_err("weekend accrual boundaries both adjust to Monday");
        let generated = generate_periods_with_adjustment(&BuildPeriodsParams {
            frequency: Tenor::daily(),
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only",
            payment_lag_days: 2,
            ..window(start, end)
        })
        .expect_err("generated boundaries also collapse");
        assert_eq!(explicit.to_string(), generated.to_string());
        assert!(explicit.to_string().contains("collapsed period"));
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

        let res = super::compile_periods(periods, Tenor::daily(), RollRule::None);
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
