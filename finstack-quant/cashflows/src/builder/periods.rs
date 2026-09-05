//! Period schedule utilities for rates pricing.
//!
//! Provides canonical accrual/payment periods built on top of the cashflow
//! builder's date generation and calendar policy.

use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::InputError;

use super::calendar::resolve_calendar_strict;
use super::date_generation::{
    build_schedule_period, generate_periods_with_adjustment, icma_coupon_period,
    validate_unique_payment_dates,
};
use super::emission::compute_reset_date;
use super::specs::{RollRule, ScheduleParams};

/// Accrual period with payment timing.
///
/// This is the canonical period type used across the cashflow builder and
/// rates instruments. Fields not relevant in a given context remain at their
/// defaults (`None` / `0.0`).
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
    /// Unadjusted (scheduled) accrual start — the raw roll-grid anchor before
    /// any business-day adjustment. Equals `accrual_start` for unadjusted
    /// schedules. Period regularity (stub vs regular, ISDA 2006 §4.16(c)) is
    /// judged on these dates, never on the adjusted boundaries.
    pub unadjusted_start: Date,
    /// Unadjusted (scheduled) accrual end — see [`Self::unadjusted_start`].
    pub unadjusted_end: Date,
}

/// Parameters for building schedule periods.
///
/// This struct captures the canonical schedule conventions used to derive
/// accrual periods, payment dates, and optional reset dates.
pub struct BuildPeriodsParams<'a> {
    /// Start date of the schedule.
    pub start: Date,
    /// End date (maturity) of the schedule.
    pub end: Date,
    /// Coupon/payment frequency.
    pub frequency: Tenor,
    /// Stub handling convention.
    pub stub: StubKind,
    /// Business day convention for date adjustments.
    pub business_day_convention: BusinessDayConvention,
    /// Holiday calendar identifier (use "weekends_only" for weekends-only adjustments).
    pub calendar_id: &'a str,
    /// Whether to enforce end-of-month rolling.
    pub end_of_month: bool,
    /// Day count convention for accrual fractions.
    pub day_count: DayCount,
    /// Payment lag in business days after accrual end.
    pub payment_lag_days: i32,
    /// Optional reset lag in business days before accrual start.
    pub reset_lag_days: Option<i32>,
    /// Adjust accrual start/end boundaries with the business-day convention.
    pub adjust_accrual_dates: bool,
    /// Roll-date rule for schedule anchors (standard IMM or CDS IMM grids).
    ///
    /// The IMM modes override `frequency`/`stub` with quarterly / short-back;
    /// see [`RollRule`].
    pub roll_rule: RollRule,
}

impl<'a> BuildPeriodsParams<'a> {
    /// Create period-generation parameters from canonical schedule conventions.
    ///
    /// The conversion keeps start/end boundaries and reset lag explicit while
    /// reusing every date, calendar, stub, and day-count convention from the
    /// schedule specification.
    #[must_use]
    pub fn from_schedule(
        schedule: &'a ScheduleParams,
        start: Date,
        end: Date,
        reset_lag_days: Option<i32>,
    ) -> Self {
        Self {
            start,
            end,
            frequency: schedule.frequency,
            stub: schedule.stub,
            business_day_convention: schedule.business_day_convention,
            calendar_id: &schedule.calendar_id,
            end_of_month: schedule.end_of_month,
            day_count: schedule.day_count,
            payment_lag_days: schedule.payment_lag_days,
            reset_lag_days,
            adjust_accrual_dates: schedule.adjust_accrual_dates,
            roll_rule: schedule.roll_rule,
        }
    }
}

fn enrich_period(
    mut period: SchedulePeriod,
    params: &BuildPeriodsParams<'_>,
    cal: &dyn finstack_quant_core::dates::HolidayCalendar,
) -> finstack_quant_core::Result<SchedulePeriod> {
    // Regularity is judged on the unadjusted scheduled dates (ISDA 2006
    // §4.16(c)): business-day adjustment of the accrual boundaries must not
    // reclassify a regular period as an ICMA stub. The accrual year fraction
    // itself still spans the (possibly adjusted) accrual boundaries.
    // ACT/ACT ICMA reference period: regular periods use the unadjusted
    // scheduled span; short and long stubs use the adjacent regular coupon
    // so ICMA Rule 251 can subdivide from a known denominator.
    let day_count_context = DayCountContext {
        calendar: Some(cal),
        frequency: Some(params.frequency),
        bus_basis: None,
        coupon_period: icma_coupon_period(
            period.unadjusted_start,
            period.unadjusted_end,
            params.frequency,
            if matches!(params.roll_rule, RollRule::None) {
                params.stub
            } else {
                StubKind::ShortBack
            },
            params.end_of_month,
        ),
        end_is_termination_date: period.accrual_end >= params.end,
    };
    period.accrual_year_fraction = params.day_count.year_fraction(
        period.accrual_start,
        period.accrual_end,
        day_count_context,
    )?;
    period.reset_date = params
        .reset_lag_days
        .map(|lag| {
            compute_reset_date(
                period.accrual_start,
                lag,
                params.business_day_convention,
                cal,
            )
        })
        .transpose()?;
    Ok(period)
}

fn enrich_periods(
    periods: Vec<SchedulePeriod>,
    params: &BuildPeriodsParams<'_>,
) -> finstack_quant_core::Result<Vec<SchedulePeriod>> {
    let cal = resolve_calendar_strict(params.calendar_id)?;
    let periods = periods
        .into_iter()
        .map(|period| enrich_period(period, params, cal))
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;
    validate_unique_payment_dates(&periods)?;
    Ok(periods)
}

/// Build one canonical schedule period from explicit start and end dates.
///
/// This helper applies the same calendar resolution, lag handling, reset-date
/// logic, and accrual-factor calculation used by the full schedule builder.
///
/// # Arguments
///
/// * `params` - Start/end dates and schedule conventions for the single period.
///
/// # Returns
///
/// A fully populated [`SchedulePeriod`] containing adjusted accrual boundaries,
/// payment date, optional reset date, and accrual year fraction.
///
/// # Errors
///
/// Returns an error if:
///
/// - `params.calendar_id` cannot be resolved
/// - date adjustment fails for the supplied calendar and business-day
///   convention
/// - day-count calculation fails for the supplied convention
/// - reset-date computation fails when `reset_lag_days` is provided
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::periods::{build_single_period, BuildPeriodsParams};
/// use finstack_quant_cashflows::builder::specs::RollRule;
/// use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
/// use time::Month;
///
/// let period = build_single_period(BuildPeriodsParams {
///     start: Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
///     end: Date::from_calendar_date(2025, Month::April, 15).expect("valid date"),
///     frequency: Tenor::quarterly(),
///     stub: StubKind::None,
///     business_day_convention: BusinessDayConvention::ModifiedFollowing,
///     calendar_id: "weekends_only",
///     end_of_month: false,
///     day_count: DayCount::Act360,
///     payment_lag_days: 2,
///     reset_lag_days: Some(2),
///     adjust_accrual_dates: false,
///     roll_rule: RollRule::None,
/// })
/// .expect("single-period build succeeds");
///
/// assert!(period.accrual_start <= period.accrual_end);
/// assert!(period.payment_date >= period.accrual_end);
/// ```
pub fn build_single_period(
    params: BuildPeriodsParams<'_>,
) -> finstack_quant_core::Result<SchedulePeriod> {
    let cal = resolve_calendar_strict(params.calendar_id)?;
    let period = build_schedule_period(
        params.start,
        params.end,
        params.business_day_convention,
        params.payment_lag_days,
        cal,
        params.adjust_accrual_dates,
    )?;
    let mut enriched = enrich_periods(vec![period], &params)?.into_iter();
    enriched
        .next()
        .ok_or_else(|| InputError::TooFewPoints.into())
}

/// Build canonical schedule periods with consistent market conventions.
///
/// # Arguments
///
/// * `params` - Schedule boundaries and conventions used to generate the full
///   period set.
///
/// # Returns
///
/// Ordered schedule periods spanning `params.start` to `params.end`. Returns an
/// empty vector when date generation produces no periods.
///
/// # Errors
///
/// Returns an error if:
///
/// - `params.calendar_id` cannot be resolved
/// - date generation fails for the supplied frequency, stub rule, or business
///   day convention
/// - day-count calculation fails for any generated period
/// - reset-date computation fails when `reset_lag_days` is provided
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::periods::{build_periods, BuildPeriodsParams};
/// use finstack_quant_cashflows::builder::specs::RollRule;
/// use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
/// use time::Month;
///
/// let periods = build_periods(BuildPeriodsParams {
///     start: Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
///     end: Date::from_calendar_date(2026, Month::January, 15).expect("valid date"),
///     frequency: Tenor::quarterly(),
///     stub: StubKind::None,
///     business_day_convention: BusinessDayConvention::ModifiedFollowing,
///     calendar_id: "weekends_only",
///     end_of_month: false,
///     day_count: DayCount::Act360,
///     payment_lag_days: 2,
///     reset_lag_days: Some(2),
///     adjust_accrual_dates: false,
///     roll_rule: RollRule::None,
/// })
/// .expect("period build succeeds");
///
/// assert!(!periods.is_empty());
/// ```
pub fn build_periods(
    params: BuildPeriodsParams<'_>,
) -> finstack_quant_core::Result<Vec<SchedulePeriod>> {
    let periods = generate_periods_with_adjustment(
        params.start,
        params.end,
        params.frequency,
        params.stub,
        params.business_day_convention,
        params.end_of_month,
        params.payment_lag_days,
        params.calendar_id,
        params.adjust_accrual_dates,
        params.roll_rule,
    )?;
    enrich_periods(periods, &params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn params() -> BuildPeriodsParams<'static> {
        BuildPeriodsParams {
            start: Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
            end: Date::from_calendar_date(2025, Month::April, 15).expect("valid date"),
            frequency: Tenor::quarterly(),
            stub: StubKind::None,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 2,
            reset_lag_days: Some(2),
            adjust_accrual_dates: false,
            roll_rule: crate::builder::specs::RollRule::None,
        }
    }

    #[test]
    fn single_period_matches_one_period_schedule() {
        let single = build_single_period(params()).expect("single period");
        let mut schedule = build_periods(params())
            .expect("period schedule")
            .into_iter();
        let scheduled = schedule.next().expect("one period");
        assert!(schedule.next().is_none());

        assert_eq!(single.accrual_start, scheduled.accrual_start);
        assert_eq!(single.accrual_end, scheduled.accrual_end);
        assert_eq!(single.payment_date, scheduled.payment_date);
        assert_eq!(single.reset_date, scheduled.reset_date);
        assert_eq!(
            single.accrual_year_fraction,
            scheduled.accrual_year_fraction
        );
    }

    fn adjusted_params() -> BuildPeriodsParams<'static> {
        BuildPeriodsParams {
            // Sunday start, Saturday end: both boundaries need adjustment.
            start: Date::from_calendar_date(2025, Month::January, 5).expect("valid date"),
            end: Date::from_calendar_date(2025, Month::April, 5).expect("valid date"),
            frequency: Tenor::quarterly(),
            stub: StubKind::None,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 0,
            reset_lag_days: None,
            adjust_accrual_dates: true,
            roll_rule: RollRule::None,
        }
    }

    #[test]
    fn single_period_honors_adjust_accrual_dates() {
        // ISDA 2006 §4.10: when the adjusted-period convention applies, it
        // applies to every Calculation Period — including one-period schedules.
        let single = build_single_period(adjusted_params()).expect("single period");
        assert_eq!(
            single.accrual_start,
            Date::from_calendar_date(2025, Month::January, 6).expect("valid date"),
            "accrual start must be business-day adjusted"
        );
        assert_eq!(
            single.accrual_end,
            Date::from_calendar_date(2025, Month::April, 7).expect("valid date"),
            "accrual end must be business-day adjusted"
        );

        // Parity with the full-schedule path for the same params.
        let mut schedule = build_periods(adjusted_params())
            .expect("period schedule")
            .into_iter();
        let scheduled = schedule.next().expect("one period");
        assert!(schedule.next().is_none());
        assert_eq!(single.accrual_start, scheduled.accrual_start);
        assert_eq!(single.accrual_end, scheduled.accrual_end);
        assert_eq!(single.payment_date, scheduled.payment_date);
        assert_eq!(
            single.accrual_year_fraction,
            scheduled.accrual_year_fraction
        );
    }

    #[test]
    fn adjusted_regular_icma_period_uses_unadjusted_reference_period() {
        // Regularity is a property of the UNADJUSTED scheduled dates (ISDA 2006
        // §4.16(c); QuantLib Schedule::isRegular), and the ICMA reference
        // period is the unadjusted scheduled period. Adjusted accrual days are
        // measured against that reference; accrual spilling past the scheduled
        // end is split into the next quasi-coupon period by core.
        //
        // Schedule: semiannual, anchors 2025-01-04 (Sat) → 2025-07-04 (Fri) →
        // 2026-01-04 (Sun); ModifiedFollowing weekends-only adjustment gives
        // accrual periods [2025-01-06, 2025-07-04) and [2025-07-04, 2026-01-05).
        let params = BuildPeriodsParams {
            start: Date::from_calendar_date(2025, Month::January, 4).expect("valid date"),
            end: Date::from_calendar_date(2026, Month::January, 4).expect("valid date"),
            frequency: Tenor::semi_annual(),
            stub: StubKind::None,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only",
            end_of_month: false,
            day_count: DayCount::ActActIsma,
            payment_lag_days: 0,
            reset_lag_days: None,
            adjust_accrual_dates: true,
            roll_rule: RollRule::None,
        };
        let periods = build_periods(params).expect("periods");
        assert_eq!(periods.len(), 2);

        // Period 1: 179 adjusted days over the 181-day scheduled half-year.
        let expected_first = 179.0 / 181.0 * 0.5;
        // Period 2: full 184-day scheduled half-year plus one day spilling
        // into the next 181-day quasi-coupon period.
        let expected_second = 0.5 + 0.5 / 181.0;
        assert!(
            (periods[0].accrual_year_fraction - expected_first).abs() < 1e-12,
            "first adjusted ICMA period: got {}, expected {expected_first}",
            periods[0].accrual_year_fraction
        );
        assert!(
            (periods[1].accrual_year_fraction - expected_second).abs() < 1e-12,
            "second adjusted ICMA period: got {}, expected {expected_second}",
            periods[1].accrual_year_fraction
        );
    }

    #[test]
    fn short_front_icma_stub_uses_ending_regular_coupon() {
        let params = BuildPeriodsParams {
            start: Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
            end: Date::from_calendar_date(2025, Month::July, 15).expect("valid date"),
            frequency: Tenor::semi_annual(),
            stub: StubKind::ShortFront,
            business_day_convention: BusinessDayConvention::Unadjusted,
            calendar_id: "weekends_only",
            end_of_month: false,
            day_count: DayCount::ActActIsma,
            payment_lag_days: 0,
            reset_lag_days: None,
            adjust_accrual_dates: false,
            roll_rule: RollRule::None,
        };
        let periods = build_periods(params).expect("short front stub period");
        assert_eq!(periods.len(), 1);
        let start = Date::from_calendar_date(2025, Month::March, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::July, 15).expect("valid date");
        let ref_start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let expected =
            (end - start).whole_days() as f64 / (end - ref_start).whole_days() as f64 * 0.5;
        assert!(
            (periods[0].accrual_year_fraction - expected).abs() < 1e-12,
            "short-front ICMA stub: got {}, expected {expected}",
            periods[0].accrual_year_fraction
        );
    }

    #[test]
    fn build_periods_can_adjust_accrual_boundaries() {
        let params = BuildPeriodsParams {
            start: Date::from_calendar_date(2029, Month::May, 4).expect("valid date"),
            end: Date::from_calendar_date(2030, Month::May, 4).expect("valid date"),
            frequency: Tenor::annual(),
            stub: StubKind::None,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 2,
            reset_lag_days: None,
            adjust_accrual_dates: true,
            roll_rule: crate::builder::specs::RollRule::None,
        };
        let periods = build_periods(params).expect("periods");
        assert_eq!(
            periods[0].accrual_end,
            Date::from_calendar_date(2030, Month::May, 6).expect("valid date")
        );

        let params = BuildPeriodsParams {
            start: Date::from_calendar_date(2029, Month::May, 4).expect("valid date"),
            end: Date::from_calendar_date(2030, Month::May, 4).expect("valid date"),
            frequency: Tenor::annual(),
            stub: StubKind::None,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 2,
            reset_lag_days: None,
            adjust_accrual_dates: false,
            roll_rule: crate::builder::specs::RollRule::None,
        };
        let periods = build_periods(params).expect("periods");
        assert_eq!(
            periods[0].accrual_end,
            Date::from_calendar_date(2030, Month::May, 4).expect("valid date")
        );
    }

    #[test]
    fn build_periods_rejects_duplicate_adjusted_payment_dates() {
        let params = BuildPeriodsParams {
            start: Date::from_calendar_date(2025, Month::January, 3).expect("valid date"),
            end: Date::from_calendar_date(2025, Month::January, 6).expect("valid date"),
            frequency: Tenor::daily(),
            stub: StubKind::None,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 0,
            reset_lag_days: None,
            adjust_accrual_dates: false,
            roll_rule: crate::builder::specs::RollRule::None,
        };

        let err = build_periods(params).expect_err("payment-date collision must be rejected");
        assert!(err.to_string().contains("same payment date"));
    }
}
