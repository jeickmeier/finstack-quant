//! Cashflow schedule compiler: transforms high-level programs into concrete schedules.
//!
//! This module takes the cashflow builder inputs (fixed/floating coupon specs,
//! optional coupon/payment programs, fees, and notional behavior) and produces
//! deterministic, validated schedules that downstream cashflow generation uses.
//!
//! Responsibilities:
//! - Partition the `[issue, maturity]` horizon into windows using coupon/payment
//!   program boundaries.
//! - For each window, derive date schedules according to frequency, stub, BDC,
//!   calendar, and day‑count conventions.
//! - Materialize per‑window coupon specs (fixed or floating) and selected
//!   payment split (Cash | PIK | Split) with precise coverage semantics.
//! - Compile fee specifications into periodic and fixed fee schedules.
//! - Collect a stable, de‑duplicated set of relevant dates (issue, maturity,
//!   coupon/payment/fee dates, and custom amortization dates).
//!
//! Determinism and validation follow the project invariants: windows must be
//! within `[issue, maturity]`, coverage must be unique (no overlapping coupon
//! pieces without containment), and every produced schedule must contain at
//! least two dates.

use crate::builder::{AmortizationSpec, Notional};

use finstack_quant_core::dates::{Date, DayCount, HolidayCalendar, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::InputError;
use rust_decimal::Decimal;

use super::calendar::resolve_calendar_strict;
use super::date_generation::{
    adjust_period_accruals, build_dates, index_period_schedule, SchedulePeriod,
};
use super::rate_helpers::ResolvedFloatingRateSpec;
use super::specs::{
    CouponType, FeeAccrualBasis, FeeBase, FeeSpec, FixedCouponSpec, FloatingCouponSpec,
    FloatingRateSpec, ScheduleParams,
};

type PeriodMap = finstack_quant_core::HashMap<Date, SchedulePeriod>;
type DateSet = finstack_quant_core::HashSet<Date>;

/// Result type for schedule building with metadata.
type ScheduleWithMeta = (Vec<Date>, PeriodMap, DateSet);

#[derive(Debug, Clone, Copy)]
pub(super) struct DateWindow {
    pub(super) start: Date,
    pub(super) end: Date, // exclusive
}

impl DateWindow {
    fn new(start: Date, end: Date) -> Self {
        Self { start, end }
    }

    fn is_within(self, issue: Date, maturity: Date) -> bool {
        self.start >= issue && self.end <= maturity && self.start < self.end
    }

    fn covers_range(self, start: Date, end: Date) -> bool {
        self.start <= start && end <= self.end
    }

    fn contains_window(self, other: DateWindow) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

/// Build dates and metadata using the date_generation module.
///
/// This helper wraps `date_generation::build_dates` / `build_dates` and
/// extracts the `prev` map and `first_or_last` set required by the cashflow compiler.
fn build_dates_with_meta(
    window: DateWindow,
    params: &ScheduleParams,
) -> finstack_quant_core::Result<ScheduleWithMeta> {
    let mut schedule = build_dates(
        window.start,
        window.end,
        params.freq,
        params.stub,
        params.bdc,
        params.end_of_month,
        params.payment_lag_days,
        &params.calendar_id,
    )?;
    // Swap convention (ISDA 2006 §4.10; ARRC SOFR conventions): roll both
    // accrual boundaries with the schedule's BDC before year fractions and
    // overnight observation windows are derived. Bond schedules keep
    // unadjusted accrual (the default).
    if params.adjust_accrual_dates {
        let cal = resolve_calendar_strict(&params.calendar_id)?;
        for period in &mut schedule.periods {
            adjust_period_accruals(period, params.bdc, cal)?;
        }
    }
    index_period_schedule(schedule)
}

/// Compiled fixed-coupon schedule produced by [`compute_coupon_schedules`].
#[derive(Clone)]
pub(crate) struct FixedSchedule {
    pub(crate) spec: FixedCouponSpec,
    pub(crate) calendar: &'static dyn HolidayCalendar,
    pub(crate) dates: Vec<Date>,
    pub(crate) prev: PeriodMap,
    pub(crate) first_last: DateSet,
}

/// Compiled floating-coupon schedule produced by [`compute_coupon_schedules`].
#[derive(Clone)]
pub(crate) struct FloatSchedule {
    pub(crate) spec: FloatingCouponSpec,
    pub(crate) calendar: &'static dyn HolidayCalendar,
    pub(crate) fixing_calendar: &'static dyn HolidayCalendar,
    pub(crate) runtime_spec: ResolvedFloatingRateSpec,
    pub(crate) dates: Vec<Date>,
    pub(crate) prev: PeriodMap,
    /// Payment dates whose accrual period is a genuine stub (irregular span).
    pub(crate) first_last: DateSet,
}

/// Periodic fee schedule prepared from fee specs.
///
/// Represents a normalized, per‑period fee defined by a base, annualized bps,
/// a day‑count, and the concrete schedule over which it accrues.
///
/// Fields:
/// - `base` (`FeeBase`): Notional/cash‑based fee base.
/// - `bps` (`Decimal`): Annualized basis points applied to the base. Uses Decimal for exact representation.
/// - `dc` (`DayCount`): Day‑count convention for accrual.
/// - `freq` (`Tenor`): Payment frequency (needed for Act/Act ISMA day count context).
/// - `calendar` (`HolidayCalendar`): Resolved calendar used for Bus/252 and Act/Act ISMA contexts.
/// - `dates` (`Vec<Date>`): Inclusive/exclusive boundary dates for accrual periods.
/// - `prev` (`HashMap<Date, SchedulePeriod>`): Period details keyed by payment date.
#[derive(Clone)]
pub(super) struct PeriodicFee {
    pub(super) base: FeeBase,
    pub(super) bps: Decimal,
    pub(super) dc: DayCount,
    pub(super) freq: Tenor,
    pub(super) calendar: &'static dyn HolidayCalendar,
    pub(super) dates: Vec<Date>,
    pub(super) prev: PeriodMap,
    pub(super) accrual_basis: FeeAccrualBasis,
}

/// Convenience alias for a list of compiled periodic-fee schedules.
pub(super) type PeriodicFees = Vec<PeriodicFee>;
/// Convenience alias for a list of one-off `(date, amount)` fixed fees.
pub(super) type FixedFees = Vec<(Date, Money)>;

pub(super) fn build_fee_schedules(
    issue: Date,
    maturity: Date,
    fees: &[FeeSpec],
) -> finstack_quant_core::Result<(PeriodicFees, FixedFees)> {
    //! Build periodic and fixed fee schedules from input `FeeSpec`s.
    //!
    //! Arguments:
    //! - `issue` (`Date`): Instrument start date (inclusive).
    //! - `maturity` (`Date`): Instrument end date (inclusive horizon endpoint).
    //! - `fees` (`&[FeeSpec]`): Collection of fee specifications to compile.
    //!
    //! Returns:
    //! - `Ok((PeriodicFees, FixedFees))` where `PeriodicFees` contains
    //!   normalized periodic fee programs (bps/day‑count/schedule), and
    //!   `FixedFees` contains explicit (`Date`, `Money`) pairs.
    //!
    //! Errors:
    //! - `Error::Validation` if a fixed fee is dated after maturity (it could
    //!   never be emitted) or a periodic fee produces an empty schedule (the
    //!   message names the offending fee). Pre-issue fixed fees are allowed
    //!   (delayed-funding structures).
    //!
    //! Example:
    //! ```rust
    //! use finstack_quant_core::dates::{Date, DayCount, Tenor, BusinessDayConvention};
    //! use finstack_quant_cashflows::builder::{FeeSpec, FeeBase};
    //! use finstack_quant_core::dates::StubKind;
    //! use rust_decimal_macros::dec;
    //! use time::Month;
    //!
    //! let issue = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
    //! let maturity = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    //! let fees = vec![
    //!     FeeSpec::PeriodicBps {
    //!         base: FeeBase::Drawn,
    //!         bps: dec!(50),
    //!         freq: Tenor::quarterly(),
    //!         dc: DayCount::Act360,
    //!         bdc: BusinessDayConvention::Following,
    //!         calendar_id: "weekends_only".to_string(),
    //!         stub: StubKind::None,
    //!         accrual_basis: Default::default(),
    //!     }
    //! ];
    //! // Note: build_fee_schedules would be called here
    //! ```
    let mut periodic_fees: PeriodicFees = Vec::new();
    let mut fixed_fees: FixedFees = Vec::new();
    for fee in fees {
        match fee {
            FeeSpec::Fixed { date, amount } => {
                // Pre-issue fixed fees are supported (delayed-funding /
                // commitment structures emit them at their stated date), but a
                // fee dated after maturity can never be reached by the
                // emission loop and would be silently dropped.
                if *date > maturity {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "fixed fee dated {} is after maturity {}; fees must be dated on or \
                         before the instrument maturity",
                        date, maturity
                    )));
                }
                if amount.amount() != 0.0 {
                    fixed_fees.push((*date, *amount))
                }
            }
            FeeSpec::PeriodicBps {
                base,
                bps,
                freq,
                dc,
                bdc,
                calendar_id,
                stub,
                accrual_basis,
            } => {
                let schedule = ScheduleParams {
                    freq: *freq,
                    dc: *dc,
                    bdc: *bdc,
                    calendar_id: calendar_id.clone(),
                    stub: *stub,
                    end_of_month: false,
                    payment_lag_days: 0,
                    adjust_accrual_dates: false,
                };
                let (dates, prev, _) =
                    build_dates_with_meta(DateWindow::new(issue, maturity), &schedule)?;
                if dates.is_empty() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "periodic fee ({bps} bps, {freq} on '{calendar_id}') produced an empty \
                         schedule over [{issue}, {maturity}]"
                    )));
                }
                let calendar = resolve_calendar_strict(calendar_id)?;
                periodic_fees.push(PeriodicFee {
                    base: base.clone(),
                    bps: *bps,
                    dc: *dc,
                    freq: *freq,
                    calendar,
                    dates,
                    prev,
                    accrual_basis: accrual_basis.clone(),
                });
            }
        }
    }
    Ok((periodic_fees, fixed_fees))
}

#[derive(Debug, Clone)]
pub(super) enum CouponSpec {
    Fixed {
        rate: Decimal,
    },
    Float {
        rate_spec: FloatingRateSpec,
    },
    StepUp {
        initial_rate: Decimal,
        step_schedule: Vec<(finstack_quant_core::dates::Date, Decimal)>,
    },
}

#[derive(Debug, Clone)]
pub(super) struct CouponProgramPiece {
    pub(super) window: DateWindow,
    pub(super) schedule: ScheduleParams,
    pub(super) coupon: CouponSpec,
}

#[derive(Debug, Clone)]
pub(super) struct PaymentProgramPiece {
    pub(super) window: DateWindow,
    pub(super) split: CouponType, // Cash | PIK | Split
}

#[derive(Clone)]
pub(super) struct CompiledSchedules {
    pub(super) fixed_schedules: Vec<FixedSchedule>,
    pub(super) float_schedules: Vec<FloatSchedule>,
}

/// Collect all relevant dates for cashflow schedule building.
///
/// Uses Vec with sort_unstable + dedup instead of BTreeSet for better
/// performance with typical schedule sizes (<10k dates).
pub(super) fn collect_dates(
    issue: Date,
    maturity: Date,
    fixed_schedules: &[FixedSchedule],
    float_schedules: &[FloatSchedule],
    periodic_fee_date_slices: &[&[Date]],
    fixed_fees: &[(Date, Money)],
    notional: &Notional,
) -> Vec<Date> {
    // Estimate capacity: issue/maturity + 3 dates per schedule period + fees + amort dates
    let estimated_periods: usize = fixed_schedules
        .iter()
        .map(|s| s.dates.len())
        .chain(float_schedules.iter().map(|s| s.dates.len()))
        .sum();
    let estimated_capacity = 2 + estimated_periods * 3 + fixed_fees.len() + 16;

    let mut dates: Vec<Date> = Vec::with_capacity(estimated_capacity);
    dates.push(issue);
    dates.push(maturity);

    // Collect all fixed coupon dates (accrual boundaries + payment dates)
    for schedule in fixed_schedules {
        for period in schedule.prev.values() {
            dates.push(period.accrual_start);
            dates.push(period.accrual_end);
            dates.push(period.payment_date);
        }
    }

    // Collect all floating coupon dates (accrual boundaries + payment dates)
    for schedule in float_schedules {
        for period in schedule.prev.values() {
            dates.push(period.accrual_start);
            dates.push(period.accrual_end);
            dates.push(period.payment_date);
        }
    }

    // Collect all periodic fee dates (accrual boundaries + payment dates)
    for dates_slice in periodic_fee_date_slices {
        dates.extend_from_slice(dates_slice);
    }

    // Collect all fixed fee dates
    for (d, _) in fixed_fees {
        dates.push(*d);
    }

    // Collect amortization dates
    match &notional.amort {
        AmortizationSpec::CustomPrincipal { items } => {
            for (d, _) in items {
                dates.push(*d);
            }
        }
        AmortizationSpec::StepRemaining { schedule } => {
            for (d, _) in schedule {
                dates.push(*d);
            }
        }
        _ => {}
    }

    dates.sort_unstable();
    dates.dedup();
    dates
}

struct StepUpCompileInput<'a> {
    split: CouponType,
    initial_rate: Decimal,
    step_schedule: &'a [(Date, Decimal)],
    schedule: &'a ScheduleParams,
    calendar: &'static dyn HolidayCalendar,
    dates: &'a [Date],
    prev: &'a finstack_quant_core::HashMap<Date, SchedulePeriod>,
    first_last: &'a finstack_quant_core::HashSet<Date>,
}

fn compile_step_up_schedules(input: StepUpCompileInput<'_>) -> Vec<FixedSchedule> {
    struct RateGroup {
        rate: Decimal,
        dates: Vec<Date>,
        prev: PeriodMap,
        first_last: DateSet,
    }

    impl RateGroup {
        fn new(
            rate: Decimal,
            payment_date: Date,
            period: SchedulePeriod,
            is_first_or_last: bool,
        ) -> Self {
            let mut prev = PeriodMap::default();
            prev.insert(payment_date, period);

            let mut first_last = DateSet::default();
            if is_first_or_last {
                first_last.insert(payment_date);
            }

            Self {
                rate,
                dates: vec![payment_date],
                prev,
                first_last,
            }
        }

        fn push(&mut self, payment_date: Date, period: SchedulePeriod, is_first_or_last: bool) {
            self.dates.push(payment_date);
            self.prev.insert(payment_date, period);
            if is_first_or_last {
                self.first_last.insert(payment_date);
            }
        }

        fn into_fixed_schedule(self, input: &StepUpCompileInput<'_>) -> FixedSchedule {
            FixedSchedule {
                spec: FixedCouponSpec::from_parts(input.split, self.rate, input.schedule.clone()),
                calendar: input.calendar,
                dates: self.dates,
                prev: self.prev,
                first_last: self.first_last,
            }
        }
    }

    let rate_for = |period_start: Date| -> Decimal {
        let mut rate = input.initial_rate;
        for (step_date, step_rate) in input.step_schedule {
            if *step_date <= period_start {
                rate = *step_rate;
            } else {
                break;
            }
        }
        rate
    };

    let mut rate_groups: Vec<RateGroup> = Vec::new();
    for &payment_date in input.dates {
        if let Some(period) = input.prev.get(&payment_date) {
            let period_rate = rate_for(period.accrual_start);
            let extend_last = rate_groups
                .last()
                .map(|group| group.rate == period_rate)
                .unwrap_or(false);
            let is_first_or_last = input.first_last.contains(&payment_date);

            if extend_last {
                if let Some(last) = rate_groups.last_mut() {
                    last.push(payment_date, *period, is_first_or_last);
                }
            } else {
                rate_groups.push(RateGroup::new(
                    period_rate,
                    payment_date,
                    *period,
                    is_first_or_last,
                ));
            }
        }
    }

    rate_groups
        .into_iter()
        .map(|group| group.into_fixed_schedule(&input))
        .collect()
}

fn select_coupon_piece(
    pieces: &[CouponProgramPiece],
    start: Date,
    end: Date,
) -> finstack_quant_core::Result<&CouponProgramPiece> {
    let mut chosen = None;
    for piece in pieces {
        if piece.window.covers_range(start, end) {
            if chosen.is_some() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ambiguous coupon coverage: more than one coupon window covers \
                     [{start}, {end})"
                )));
            }
            chosen = Some(piece);
        }
    }
    chosen.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "no coupon window covers [{start}, {end}); coupon program must cover the \
             full [issue, maturity] horizon"
        ))
    })
}

fn select_payment_split(
    pieces: &[PaymentProgramPiece],
    start: Date,
    end: Date,
) -> finstack_quant_core::Result<CouponType> {
    let mut chosen: Option<&PaymentProgramPiece> = None;

    for piece in pieces {
        if !piece.window.covers_range(start, end) {
            continue;
        }

        match chosen {
            None => chosen = Some(piece),
            Some(current) if current.window.contains_window(piece.window) => {
                chosen = Some(piece);
            }
            Some(current) if piece.window.contains_window(current.window) => {}
            Some(current) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "overlapping payment windows [{}, {}) and [{}, {}) cover [{start}, {end}) \
                     without containment; nest payment windows or make them disjoint",
                    current.window.start, current.window.end, piece.window.start, piece.window.end
                )))
            }
        }
    }

    Ok(chosen.map(|piece| piece.split).unwrap_or(CouponType::Cash))
}

pub(super) fn compute_coupon_schedules(
    builder: &crate::builder::orchestrator::CashFlowBuilder,
    issue: Date,
    maturity: Date,
) -> finstack_quant_core::Result<CompiledSchedules> {
    //! Compile coupon and payment programs into concrete date schedules.
    //!
    //! This function processes the programmatic windowing model to generate
    //! concrete schedules. Payment windows can sparsely override the split policy;
    //! missing windows default to `Cash`.
    //!
    //! Arguments:
    //! - `builder` (`&CashFlowBuilder`): Source of coupon/payment programs.
    //! - `issue` (`Date`): Start date (inclusive).
    //! - `maturity` (`Date`): End date (inclusive horizon endpoint).
    //!
    //! Returns:
    //! - `Ok(CompiledSchedules)` with per‑window fixed and floating schedules
    //!   and the exact specs used for each schedule.
    //!
    //! Errors:
    //! - `Error::Validation` (naming the offending window) for out‑of‑range
    //!   windows, overlapping windows without containment, or ambiguous
    //!   coverage selection.
    //! - `InputError::TooFewPoints` if any derived schedule has no dates.
    //!
    //! Example:
    //! ```rust
    //! use finstack_quant_core::dates::{Date, Tenor, DayCount, BusinessDayConvention};
    //! use finstack_quant_cashflows::builder::{FixedCouponSpec, CouponType};
    //! use finstack_quant_core::dates::StubKind;
    //! use rust_decimal_macros::dec;
    //! use time::Month;
    //!
    //! let issue = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
    //! let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
    //! // Note: CashFlowBuilder would be created here
    //! let fixed_spec = FixedCouponSpec {
    //!     coupon_type: CouponType::Cash,
    //!     rate: dec!(0.05),
    //!     freq: Tenor::semi_annual(),
    //!     dc: DayCount::Thirty360,
    //!     bdc: BusinessDayConvention::Following,
    //!     calendar_id: "usny".to_string(),
    //!     end_of_month: false,
    //!     payment_lag_days: 0,
    //!     stub: StubKind::None,
    //! };
    //! // Note: compute_coupon_schedules would be called here
    //! ```
    let coupon_pieces: &[CouponProgramPiece] = &builder.coupon_program;

    // If there are no coupon pieces at all and no payment windows, return empty schedules
    if coupon_pieces.is_empty() && builder.payment_program.is_empty() {
        return Ok(CompiledSchedules {
            fixed_schedules: Vec::new(),
            float_schedules: Vec::new(),
        });
    }

    // Payment pieces (PIK toggles) — may be sparse; missing windows default to Cash
    let payment_pieces: &[PaymentProgramPiece] = &builder.payment_program;

    // Validate windows are within [issue, maturity] and build boundary grid
    // Use Vec + sort_unstable for better performance than BTreeSet for small N
    let mut bounds: Vec<Date> =
        Vec::with_capacity(2 + coupon_pieces.len() * 2 + payment_pieces.len() * 2);
    bounds.push(issue);
    bounds.push(maturity);
    for p in coupon_pieces {
        if !p.window.is_within(issue, maturity) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "coupon window [{}, {}) is outside the instrument horizon [{issue}, {maturity}] \
                 or empty",
                p.window.start, p.window.end
            )));
        }
        bounds.push(p.window.start);
        bounds.push(p.window.end);
    }
    for p in payment_pieces {
        if !p.window.is_within(issue, maturity) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "payment window [{}, {}) is outside the instrument horizon [{issue}, {maturity}] \
                 or empty",
                p.window.start, p.window.end
            )));
        }
        bounds.push(p.window.start);
        bounds.push(p.window.end);
    }
    bounds.sort_unstable();
    bounds.dedup();
    let grid: Vec<Date> = bounds;
    if grid.len() < 2 {
        return Err(InputError::TooFewPoints.into());
    }

    let mut fixed_schedules: Vec<FixedSchedule> = Vec::new();
    let mut float_schedules: Vec<FloatSchedule> = Vec::new();

    for w in grid.windows(2) {
        let s = w[0];
        let e = w[1];
        if s >= e {
            continue;
        }

        let chosen_coupon = select_coupon_piece(coupon_pieces, s, e)?;
        let split = select_payment_split(payment_pieces, s, e)?;

        let (dates, prev, first_or_last) =
            build_dates_with_meta(DateWindow::new(s, e), &chosen_coupon.schedule)?;
        if dates.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        let calendar = resolve_calendar_strict(&chosen_coupon.schedule.calendar_id)?;

        match &chosen_coupon.coupon {
            CouponSpec::Fixed { rate } => {
                let spec =
                    FixedCouponSpec::from_parts(split, *rate, chosen_coupon.schedule.clone());
                fixed_schedules.push(FixedSchedule {
                    spec,
                    calendar,
                    dates,
                    prev,
                    first_last: first_or_last,
                });
            }
            CouponSpec::StepUp {
                initial_rate,
                step_schedule,
            } => {
                if step_schedule.windows(2).any(|w| w[0].0 >= w[1].0) {
                    return Err(finstack_quant_core::Error::Validation(
                        "StepUp coupon schedule requires finite rates and strictly increasing effective dates".into(),
                    ));
                }
                fixed_schedules.extend(compile_step_up_schedules(StepUpCompileInput {
                    split,
                    initial_rate: *initial_rate,
                    step_schedule,
                    schedule: &chosen_coupon.schedule,
                    calendar,
                    dates: &dates,
                    prev: &prev,
                    first_last: &first_or_last,
                }));
            }
            CouponSpec::Float { rate_spec } => {
                let spec = FloatingCouponSpec {
                    rate_spec: rate_spec.clone(),
                    coupon_type: split,
                    freq: chosen_coupon.schedule.freq,
                    stub: chosen_coupon.schedule.stub,
                };
                let runtime_spec = ResolvedFloatingRateSpec::try_from(&spec.rate_spec)?;
                let calendar = resolve_calendar_strict(&spec.rate_spec.calendar_id)?;
                let fixing_calendar_id = spec
                    .rate_spec
                    .fixing_calendar_id
                    .as_deref()
                    .unwrap_or(&spec.rate_spec.calendar_id);
                let fixing_calendar = resolve_calendar_strict(fixing_calendar_id)?;
                float_schedules.push(FloatSchedule {
                    spec,
                    calendar,
                    fixing_calendar,
                    runtime_spec,
                    dates,
                    prev,
                    first_last: first_or_last,
                });
            }
        }
    }

    Ok(CompiledSchedules {
        fixed_schedules,
        float_schedules,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::orchestrator::CashFlowBuilder;
    use crate::builder::specs::{CouponType, FixedCouponSpec};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), day)
            .expect("valid date")
    }

    /// Annual fixed coupon over the full `[issue, maturity]` horizon.
    fn annual_fixed() -> FixedCouponSpec {
        FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal_macros::dec!(0.05),
            freq: Tenor::annual(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        }
    }

    fn builder_with_full_horizon_fixed(issue: Date, maturity: Date) -> CashFlowBuilder {
        let mut builder = CashFlowBuilder::default();
        let _ = builder.principal(
            finstack_quant_core::money::Money::new(1_000_000.0, Currency::USD),
            issue,
            maturity,
        );
        let _ = builder.fixed_cf(annual_fixed());
        builder
    }

    #[test]
    fn no_payment_windows_default_all_cash() {
        let issue = d(2025, 1, 1);
        let maturity = d(2029, 1, 1);
        let builder = builder_with_full_horizon_fixed(issue, maturity);

        let compiled =
            compute_coupon_schedules(&builder, issue, maturity).expect("schedule compiles");

        assert!(compiled.float_schedules.is_empty());
        assert_eq!(compiled.fixed_schedules.len(), 1);
        assert_eq!(
            compiled.fixed_schedules[0].spec.coupon_type,
            CouponType::Cash
        );
    }

    #[test]
    fn nested_payment_window_override_wins() {
        // Inner PIK window nested inside the implicit full-horizon Cash default.
        let issue = d(2025, 1, 1);
        let maturity = d(2029, 1, 1);
        let mut builder = builder_with_full_horizon_fixed(issue, maturity);
        let _ = builder.add_payment_window(d(2026, 1, 1), d(2028, 1, 1), CouponType::PIK);

        let compiled =
            compute_coupon_schedules(&builder, issue, maturity).expect("nested window compiles");

        // Grid {2025,2026,2028,2029} -> three fixed schedules; the middle one
        // (the nested window) takes the PIK split, the outer two default Cash.
        assert_eq!(compiled.fixed_schedules.len(), 3);
        let splits: Vec<CouponType> = compiled
            .fixed_schedules
            .iter()
            .map(|s| s.spec.coupon_type)
            .collect();
        assert_eq!(
            splits,
            vec![CouponType::Cash, CouponType::PIK, CouponType::Cash]
        );
    }

    #[test]
    fn non_nested_overlapping_payment_windows_rejected() {
        // Windows [2025,2027) and [2026,2029) overlap on [2026,2027) but
        // neither contains the other -> ambiguous coverage.
        let issue = d(2025, 1, 1);
        let maturity = d(2029, 1, 1);
        let mut builder = builder_with_full_horizon_fixed(issue, maturity);
        let _ = builder.add_payment_window(issue, d(2027, 1, 1), CouponType::PIK);
        let _ = builder.add_payment_window(d(2026, 1, 1), maturity, CouponType::Cash);

        let Err(err) = compute_coupon_schedules(&builder, issue, maturity) else {
            panic!("overlapping non-nested payment windows must be rejected");
        };
        assert!(
            err.to_string().contains("payment windows"),
            "error should name the overlapping payment windows: {err}"
        );
    }

    #[test]
    fn payment_window_outside_horizon_rejected() {
        let issue = d(2025, 1, 1);
        let maturity = d(2029, 1, 1);
        let mut builder = builder_with_full_horizon_fixed(issue, maturity);
        // Window extends one year past maturity.
        let _ = builder.add_payment_window(issue, d(2030, 1, 1), CouponType::PIK);

        let Err(err) = compute_coupon_schedules(&builder, issue, maturity) else {
            panic!("payment window beyond maturity must be rejected");
        };
        assert!(
            err.to_string().contains("horizon"),
            "error should name the instrument horizon: {err}"
        );
    }
}
