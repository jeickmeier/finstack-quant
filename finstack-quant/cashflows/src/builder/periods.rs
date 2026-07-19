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
    build_schedule_period, generate_periods_with_adjustment, is_regular_period,
    validate_unique_payment_dates,
};
use super::emission::compute_reset_date;
use super::specs::ScheduleParams;

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
    pub bdc: BusinessDayConvention,
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
            frequency: schedule.freq,
            stub: schedule.stub,
            bdc: schedule.bdc,
            calendar_id: &schedule.calendar_id,
            end_of_month: schedule.end_of_month,
            day_count: schedule.dc,
            payment_lag_days: schedule.payment_lag_days,
            reset_lag_days,
            adjust_accrual_dates: schedule.adjust_accrual_dates,
        }
    }
}

fn enrich_period(
    mut period: SchedulePeriod,
    params: &BuildPeriodsParams<'_>,
    cal: &dyn finstack_quant_core::dates::HolidayCalendar,
) -> finstack_quant_core::Result<SchedulePeriod> {
    // When `adjust_accrual_dates` is set, boundaries arrive already adjusted
    // from date generation, so regularity is assessed on adjusted dates.
    // That can misclassify regular periods as stubs (ACT/ACT ICMA then falls
    // back to the quasi-coupon grid). Left as-is: fixing it changes accrual.
    let regular = is_regular_period(period.accrual_start, period.accrual_end, params.frequency);
    // ACT/ACT ICMA reference period: for regular periods the coupon period is
    // the accrual period itself (exact ISMA accrual). For stub periods the
    // reference period is not cleanly derivable from a single period, so it
    // is left unset and core falls back to the quasi-coupon grid anchored on
    // the accrual start (frequency-based subdivision).
    let dc_ctx = DayCountContext {
        calendar: Some(cal),
        frequency: Some(params.frequency),
        bus_basis: None,
        coupon_period: regular.then_some((period.accrual_start, period.accrual_end)),
        end_is_termination_date: period.accrual_end >= params.end,
    };
    period.accrual_year_fraction =
        params
            .day_count
            .year_fraction(period.accrual_start, period.accrual_end, dc_ctx)?;
    period.reset_date = params
        .reset_lag_days
        .map(|lag| compute_reset_date(period.accrual_start, lag, params.bdc, cal))
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
/// use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
/// use time::Month;
///
/// let period = build_single_period(BuildPeriodsParams {
///     start: Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
///     end: Date::from_calendar_date(2025, Month::April, 15).expect("valid date"),
///     frequency: Tenor::quarterly(),
///     stub: StubKind::None,
///     bdc: BusinessDayConvention::ModifiedFollowing,
///     calendar_id: "weekends_only",
///     end_of_month: false,
///     day_count: DayCount::Act360,
///     payment_lag_days: 2,
///     reset_lag_days: Some(2),
///     adjust_accrual_dates: false,
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
        params.bdc,
        params.payment_lag_days,
        cal,
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
/// use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
/// use time::Month;
///
/// let periods = build_periods(BuildPeriodsParams {
///     start: Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
///     end: Date::from_calendar_date(2026, Month::January, 15).expect("valid date"),
///     frequency: Tenor::quarterly(),
///     stub: StubKind::None,
///     bdc: BusinessDayConvention::ModifiedFollowing,
///     calendar_id: "weekends_only",
///     end_of_month: false,
///     day_count: DayCount::Act360,
///     payment_lag_days: 2,
///     reset_lag_days: Some(2),
///     adjust_accrual_dates: false,
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
        params.bdc,
        params.end_of_month,
        params.payment_lag_days,
        params.calendar_id,
        params.adjust_accrual_dates,
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
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 2,
            reset_lag_days: Some(2),
            adjust_accrual_dates: false,
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

    #[test]
    fn build_periods_can_adjust_accrual_boundaries() {
        let params = BuildPeriodsParams {
            start: Date::from_calendar_date(2029, Month::May, 4).expect("valid date"),
            end: Date::from_calendar_date(2030, Month::May, 4).expect("valid date"),
            frequency: Tenor::annual(),
            stub: StubKind::None,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 2,
            reset_lag_days: None,
            adjust_accrual_dates: true,
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
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 2,
            reset_lag_days: None,
            adjust_accrual_dates: false,
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
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 0,
            reset_lag_days: None,
            adjust_accrual_dates: false,
        };

        let err = build_periods(params).expect_err("payment-date collision must be rejected");
        assert!(err.to_string().contains("same payment date"));
    }
}
