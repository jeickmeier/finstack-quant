//! `CashFlowBuilder` orchestration: validation, schedule compilation, and
//! emission into a [`CashFlowSchedule`].
//!
//! Fluent builder methods live in `coupon_api`.

use super::pipeline::CompiledCashFlowPlan;
use super::schedule::CashFlowSchedule;
use super::specs::PrincipalEventSpec;
use crate::builder::{AmortizationSpec, Notional, PrincipalExchange};
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::dates::Date;
use finstack_quant_core::decimal::f64_to_decimal;
use finstack_quant_core::money::Money;
use finstack_quant_core::InputError;
use rust_decimal::Decimal;

use super::compiler::{
    build_fee_schedules, compute_coupon_schedules, CompiledSchedules, CouponProgramPiece,
    FeeSchedules, FixedSchedule, FloatSchedule, PaymentProgramPiece,
};
use super::date_generation::CompiledPeriod;
use super::specs::FeeSpec;
use tracing::debug;

/// Internal state accumulated during schedule building.
#[derive(Debug)]
pub(super) struct BuildState {
    pub(super) flows: Vec<CashFlow>,
    pub(super) projected_fixings: Vec<crate::fixings::ProjectedFixing>,
    /// Economic balances in ascending effective-date order for accrual scans.
    pub(super) outstanding_history: Vec<(Date, Decimal)>,
    /// Outstanding balance tracked as `Decimal` for accounting-grade precision.
    ///
    /// Using `Decimal` eliminates f64 accumulation drift that can exceed 1 bp
    /// relative error on very long-dated instruments with many small cashflows
    /// (e.g., 600+ period amortizers). Converted to f64 only at API boundaries
    /// when passing to emission functions that operate in f64 space.
    pub(super) outstanding: Decimal,
    /// Equal installment fixed from the live balance at a LinearBetween start.
    pub(super) linear_between_installment: Option<Decimal>,
}

#[derive(Debug)]
pub(super) struct AmortizationSetup {
    pub(super) final_amortization_date: Option<Date>,
    pub(super) payment_dates: finstack_quant_core::FxHashMap<Date, Date>,
    pub(super) rule: PreparedAmortization,
}

/// Exactly one prepared rule is active for a compiled instrument.
#[derive(Debug)]
pub(super) enum PreparedAmortization {
    None,
    LinearTo {
        installment: Decimal,
        final_notional: Decimal,
    },
    StepRemaining(finstack_quant_core::FxHashMap<Date, Money>),
    OriginalPercent {
        installment: Decimal,
    },
    RemainingPercent {
        fraction: Decimal,
    },
    LinearBetween {
        start: Date,
        end: Date,
        count: usize,
    },
    CustomPrincipal(finstack_quant_core::FxHashMap<Date, Money>),
}

/// Apply an event's cash leg and checked economic balance movement.
pub(super) fn apply_principal_event(
    event: &PrincipalEventSpec,
    outstanding: &mut Decimal,
    flows: &mut Vec<CashFlow>,
) -> finstack_quant_core::Result<()> {
    let cash = event.resolved_cash();
    if event.delta.amount() == 0.0 && cash.amount() == 0.0 {
        return Ok(());
    }
    let flow_amount = if event.kind == CFKind::Amortization {
        cash.amount()
    } else {
        -cash.amount()
    };
    flows.push(
        CashFlow::new(
            event.payment_date,
            None,
            Money::new(flow_amount, cash.currency())?,
            event.kind,
            0.0,
            None,
        )
        .with_principal_delta(event.delta)
        .with_principal_date(event.date),
    );
    *outstanding = outstanding
        .checked_add(f64_to_decimal(event.delta.amount())?)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "outstanding principal exceeds the supported decimal range".into(),
            )
        })?;
    if *outstanding < Decimal::ZERO {
        return Err(finstack_quant_core::Error::Validation(format!(
            "principal event on {} would make outstanding balance negative ({outstanding})",
            event.date
        )));
    }
    Ok(())
}

fn validate_core_inputs(
    b: &CashFlowBuilder,
) -> finstack_quant_core::Result<(Notional, Date, Date)> {
    let initial = b.initial_principal.ok_or_else(|| InputError::NotFound {
        id: "notional (call principal() first)".into(),
    })?;
    let notional = Notional {
        initial,
        amort: b.amortization.clone(),
    };
    let issue = b.issue.ok_or_else(|| InputError::NotFound {
        id: "issue date (call principal() first)".into(),
    })?;
    let maturity = b.maturity.ok_or_else(|| InputError::NotFound {
        id: "maturity date (call principal() first)".into(),
    })?;

    if issue >= maturity {
        return Err(finstack_quant_core::Error::Validation(format!(
            "issue date {} must be strictly before maturity date {}",
            issue, maturity
        )));
    }

    notional.validate()?;

    let out_of_range = notional
        .amort
        .dated_amounts()
        .iter()
        .find(|(date, _)| *date < issue || *date > maturity)
        .map(|(date, _)| *date);
    if let Some(date) = out_of_range {
        return Err(InputError::DateOutOfRange {
            date,
            range: (issue, maturity),
        }
        .into());
    }

    Ok((notional, issue, maturity))
}

fn derive_amortization_setup(
    notional: &Notional,
    fixed_schedules: &[FixedSchedule],
    float_schedules: &[FloatSchedule],
) -> finstack_quant_core::Result<AmortizationSetup> {
    // Rate groups and fixed/float switches partition coupon windows, not amortization cadence.
    let mut payment_dates = finstack_quant_core::FxHashMap::default();
    for period in coupon_periods(fixed_schedules, float_schedules).map(|entry| &entry.period) {
        if let Some(previous) = payment_dates.insert(period.accrual_end, period.payment_date) {
            if previous != period.payment_date {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ambiguous amortization payment dates for accrual boundary {}",
                    period.accrual_end
                )));
            }
        }
    }
    if payment_dates.is_empty()
        && matches!(
            notional.amort,
            AmortizationSpec::LinearTo { .. }
                | AmortizationSpec::PercentOfOriginalPerPeriod { .. }
                | AmortizationSpec::PercentOfRemainingPerPeriod { .. }
                | AmortizationSpec::LinearBetween { .. }
        )
    {
        return Err(finstack_quant_core::Error::Validation("schedule-based amortization needs at least one accrual boundary, but the schedule produced none".to_string()));
    }

    let rule = match &notional.amort {
        AmortizationSpec::None => PreparedAmortization::None,
        AmortizationSpec::StepRemaining { schedule } => {
            PreparedAmortization::StepRemaining(schedule.iter().copied().collect())
        }
        AmortizationSpec::CustomPrincipal { items } => {
            let mut map = finstack_quant_core::FxHashMap::<Date, Money>::default();
            for (date, money) in items.iter().filter(|(_, money)| money.amount() > 0.0) {
                let total = match map.get(date).copied() {
                    Some(existing) => existing.checked_add(*money)?,
                    None => *money,
                };
                map.insert(*date, total);
            }
            PreparedAmortization::CustomPrincipal(map)
        }
        AmortizationSpec::LinearTo { final_notional } => {
            let initial = f64_to_decimal(notional.initial.amount())?;
            let final_notional = f64_to_decimal(final_notional.amount())?;
            let installment = ((initial - final_notional)
                / Decimal::from(payment_dates.len() as u64))
            .max(Decimal::ZERO);
            PreparedAmortization::LinearTo {
                installment,
                final_notional,
            }
        }
        AmortizationSpec::PercentOfOriginalPerPeriod { pct } => {
            let installment = (f64_to_decimal(notional.initial.amount())? * f64_to_decimal(*pct)?)
                .max(Decimal::ZERO);
            PreparedAmortization::OriginalPercent { installment }
        }
        AmortizationSpec::PercentOfRemainingPerPeriod { pct } => {
            PreparedAmortization::RemainingPercent {
                fraction: f64_to_decimal(*pct)?.max(Decimal::ZERO),
            }
        }
        AmortizationSpec::LinearBetween { start, end } => {
            if !payment_dates.contains_key(end) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "LinearBetween end {end} must be a coupon accrual boundary"
                )));
            }
            let count = payment_dates
                .keys()
                .filter(|date| **date > *start && **date <= *end)
                .count();
            if count == 0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "LinearBetween window ({start}, {end}] contains no coupon accrual boundary"
                )));
            }
            PreparedAmortization::LinearBetween {
                start: *start,
                end: *end,
                count,
            }
        }
    };

    let final_amortization_date = payment_dates.keys().max().copied();

    Ok(AmortizationSetup {
        final_amortization_date,
        payment_dates,
        rule,
    })
}

pub(super) fn initialize_build_state(
    issue: Date,
    accrual_start: Date,
    notional: &Notional,
    estimated_dates: usize,
    principal_events: &[PrincipalEventSpec],
    principal_exchange: PrincipalExchange,
) -> finstack_quant_core::Result<BuildState> {
    let estimated_flows = estimated_dates * 3;
    let mut flows: Vec<CashFlow> = Vec::with_capacity(estimated_flows);

    if principal_exchange == PrincipalExchange::InitialAndFinal && notional.initial.amount() != 0.0
    {
        flows.push(CashFlow::new(
            issue,
            None,
            notional.initial.checked_neg(),
            CFKind::Notional,
            0.0,
            None,
        ));
    }

    let mut outstanding = f64_to_decimal(notional.initial.amount())?;
    // CDS IMM front periods can accrue before issue. Preserve the opening
    // balance and every pre-issue movement over that actual accrual interval.
    let history_start = principal_events
        .first()
        .map_or(accrual_start.min(issue), |event| {
            event.date.min(accrual_start).min(issue)
        });
    let mut outstanding_history = Vec::with_capacity(estimated_dates);
    outstanding_history.push((history_start, outstanding));

    for ev in principal_events.iter().filter(|ev| ev.date <= issue) {
        if ev.delta.amount() != 0.0 || ev.resolved_cash().amount() != 0.0 {
            apply_principal_event(ev, &mut outstanding, &mut flows)?;
            match outstanding_history.last_mut() {
                Some((last_date, balance)) if *last_date == ev.date => *balance = outstanding,
                _ => outstanding_history.push((ev.date, outstanding)),
            }
        }
    }

    if outstanding_history
        .last()
        .is_none_or(|(date, _)| *date < issue)
    {
        outstanding_history.push((issue, outstanding));
    }

    Ok(BuildState {
        projected_fixings: Vec::new(),
        flows,
        outstanding_history,
        outstanding,
        linear_between_installment: None,
    })
}

/// Every fixed and floating coupon period, in schedule order.
pub(super) fn coupon_periods<'a>(
    fixed: &'a [FixedSchedule],
    float: &'a [FloatSchedule],
) -> impl Iterator<Item = &'a CompiledPeriod> {
    fixed
        .iter()
        .flat_map(|s| &s.periods)
        .chain(float.iter().flat_map(|s| &s.periods))
}

/// Redemption follows the payment date of the coupon ending at maturity.
/// With no coupon leg, principal is paid on the raw maturity date.
fn compute_redemption_dates(
    maturity: Date,
    fixed_schedules: &[FixedSchedule],
    float_schedules: &[FloatSchedule],
) -> (Date, Date) {
    coupon_periods(fixed_schedules, float_schedules)
        .map(|entry| &entry.period)
        .filter(|period| period.unadjusted_end == maturity)
        .map(|period| (period.payment_date, period.accrual_end))
        .reduce(|previous, dates| (previous.0.max(dates.0), previous.1.max(dates.1)))
        .unwrap_or((maturity, maturity))
}

/// Builder for constructing cashflow schedules with validation.
///
/// Fluent methods live in `coupon_api`; build orchestration lives in this
/// module.
///
/// # Window boundaries
///
/// Coupon windows (`add_fixed_window`, `add_floating_window`,
/// `fixed_to_float`, `float_margin_steps`) and payment windows
/// (`add_payment_window`, `payment_split_program`) all add their dates to one
/// set of horizon cut points. Coupon dates are then generated independently
/// between each pair of adjacent cut points with that piece's
/// `ScheduleParams`; one issue-anchored grid is not split. With the default
/// `ShortFront` stub a piece's regular dates roll back from its end, so an
/// off-cycle boundary moves every earlier coupon date of that piece onto the
/// boundary's roll and puts the stub at the piece's start. With
/// `StubKind::None` a boundary that does not leave whole periods is rejected.
/// `step_up_cf` keeps one full-horizon grid instead: each step applies from
/// the first period starting on or after its date. Snap off-cycle dates onto
/// the accrual grid first when coupon dates must not move.
#[derive(Debug, Clone, Default)]
pub struct CashFlowBuilder {
    pub(super) initial_principal: Option<Money>,
    pub(super) amortization: AmortizationSpec,
    pub(super) issue: Option<Date>,
    pub(super) maturity: Option<Date>,
    pub(super) fees: Vec<FeeSpec>,
    pub(super) principal_events: Vec<PrincipalEventSpec>,
    pub(super) coupon_program: Vec<CouponProgramPiece>,
    pub(super) payment_program: Vec<PaymentProgramPiece>,
    /// Whether to emit issue funding and maturity redemption notionals.
    pub(super) principal_exchange: PrincipalExchange,
    pub(super) pending_error: Option<finstack_quant_core::Error>,
}

/// One-line summary of what has been configured so far, for host `repr`s.
impl std::fmt::Display for CashFlowBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CashFlowBuilder(principal=")?;
        match self.initial_principal {
            Some(principal) => write!(f, "{} {}", principal.amount(), principal.currency())?,
            None => f.write_str("None")?,
        }
        f.write_str(", horizon=")?;
        match (self.issue, self.maturity) {
            (Some(issue), Some(maturity)) => write!(f, "'{issue}'..'{maturity}'")?,
            _ => f.write_str("None")?,
        }
        write!(
            f,
            ", coupon_legs={}, fees={}, principal_events={})",
            self.coupon_program.len(),
            self.fees.len(),
            self.principal_events.len()
        )
    }
}

impl CashFlowBuilder {
    /// Build the cashflow schedule with optional market curves for floating rate projection.
    ///
    /// When curves are provided, floating rate coupons use forward rates:
    /// `coupon = outstanding * calculate_floating_rate(index, params) * year_fraction`,
    /// which by default is `(index + spread_bp * 1e-4) * gearing` within any
    /// floors and caps (see [`calculate_floating_rate`](crate::builder::calculate_floating_rate)).
    ///
    /// Without `as_of`, each forward curve's base date is the observation
    /// cutoff: resets before it need exact fixings under `FIXING:{forward_curve_id}`.
    /// Use [`build_as_of`](Self::build_as_of) to set the cutoff explicitly.
    ///
    /// When a coupon cannot be projected (for example its forward curve is not
    /// in `curves`), the fallback policy on its floating spec controls behavior
    /// (default: error; `SpreadOnly` uses just margin; `FixedRate(r)` uses a fixed
    /// index); see [`FloatingRateFallback`](crate::builder::FloatingRateFallback).
    ///
    /// The builder compiles dates, coupon programs, fees, and principal events
    /// into a canonical ordered [`CashFlowSchedule`]. `curves` is used only for
    /// floating-rate projection; fixed coupons and deterministic fees do not
    /// require a market context.
    ///
    /// # Conventions
    ///
    /// - **Interest base:** coupons accrue on each constant-balance interval;
    ///   principal events change interest from their economic effective date,
    ///   independently of adjusted cash payment dates.
    /// - **Redemption conventions:** business-day adjustment and payment lag
    ///   follow the coupon window ending at maturity. Principal events must
    ///   occur on or before its effective terminal accrual boundary; when
    ///   accrual dates are adjusted with Preceding, this can precede raw maturity.
    /// - **Amortization cadence:** `LinearTo` / `PercentOfOriginalPerPeriod`
    ///   use all contractual coupon windows, including rate steps and fixed/float switches.
    ///   `LinearBetween` uses economic coupon accrual boundaries in `(start, end]`;
    ///   `end` must be an actual boundary. Its equal installment is fixed from the
    ///   outstanding balance after all movements effective on `start`, including PIK.
    ///   PIK and explicit principal events apply before scheduled amortization.
    /// - **Reporting day count:** the schedule's representative day count is
    ///   taken from the first fixed coupon leg, else the first floating leg;
    ///   schedules with no coupon leg default to Act/365F.
    ///
    /// # Errors
    ///
    /// Returns any deferred builder-configuration error, invalid core input or
    /// schedule/coupon/fee/principal-event error, or a floating-rate projection
    /// failure. In particular, a floating spec with the default error fallback
    /// fails when its required market data is unavailable.
    /// Returns a validation error for a principal event after the effective
    /// terminal accrual boundary, even if it falls before raw maturity.
    /// Dated amortization is subject to the same effective terminal boundary;
    /// a `LinearBetween` start before issue or an end without a coupon accrual
    /// boundary is also rejected.
    ///
    /// # Arguments
    ///
    /// * `curves` - Optional market context used to project floating legs.
    ///   Pass `None` for fixed-only schedules; floating specs with the
    ///   default error fallback fail when required curves are missing
    pub fn build(
        &self,
        curves: Option<&finstack_quant_core::market_data::context::MarketContext>,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        self.compile_plan()?.project(curves, None)
    }

    /// Build cashflows using the valuation date to distinguish historical fixings
    /// from projections, independently of the forward curves' base dates.
    ///
    /// # Arguments
    ///
    /// * `curves` - Market context supplying projection curves and exact-date
    ///   historical fixings under `FIXING:{forward_curve_id}`. Required floating
    ///   inputs must be present unless the coupon explicitly configures a fallback.
    /// * `as_of` - Observation cutoff: resets before this date require historical
    ///   fixings; on this date an available fixing takes precedence over projection.
    pub fn build_as_of(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        self.compile_plan()?.project(Some(curves), Some(as_of))
    }

    fn compile_plan(&self) -> finstack_quant_core::Result<CompiledCashFlowPlan> {
        if let Some(err) = &self.pending_error {
            return Err(err.clone());
        }
        let (notional, issue, maturity) = validate_core_inputs(self)?;

        let CompiledSchedules {
            fixed_schedules,
            float_schedules,
        } = compute_coupon_schedules(self, issue, maturity)?;
        let FeeSchedules {
            periodic_fees,
            fixed_fees,
        } = build_fee_schedules(issue, maturity, &self.fees)?;

        let mut principal_events = self.principal_events.clone();
        principal_events.sort_by_key(|ev| ev.date);

        let expected_currency = notional.initial.currency();
        if let Some(ev) = principal_events
            .iter()
            .find(|ev| ev.delta.currency() != expected_currency)
        {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: expected_currency,
                actual: ev.delta.currency(),
            });
        }

        if let Some(ev) = principal_events.iter().find(|ev| ev.date > maturity) {
            return Err(InputError::DateOutOfRange {
                date: ev.date,
                range: (issue, maturity),
            }
            .into());
        }

        let (redemption_date, redemption_effective_date) =
            compute_redemption_dates(maturity, &fixed_schedules, &float_schedules);
        let late_amortization = match &notional.amort {
            AmortizationSpec::LinearBetween { start, end } => {
                if *start < issue {
                    return Err(InputError::DateOutOfRange {
                        date: *start,
                        range: (issue, redemption_effective_date),
                    }
                    .into());
                }
                (*end > redemption_effective_date).then_some(*end)
            }
            _ => notional
                .amort
                .dated_amounts()
                .iter()
                .find(|(date, _)| *date > redemption_effective_date)
                .map(|(date, _)| *date),
        };
        if let Some(date) = late_amortization {
            return Err(finstack_quant_core::Error::Validation(format!(
                "amortization on {date} is after the effective terminal accrual date \
                 {redemption_effective_date}; amortization must occur before or on the \
                 final economic balance boundary"
            )));
        }
        if let Some(event) = principal_events
            .iter()
            .find(|event| event.date > redemption_effective_date)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "principal event on {} is after the effective terminal accrual date {}; \
                 principal events must occur before or on the final economic balance boundary",
                event.date, redemption_effective_date
            )));
        }
        let amort_setup = derive_amortization_setup(&notional, &fixed_schedules, &float_schedules)?;

        let mut plan = CompiledCashFlowPlan {
            notional,
            issue,
            maturity,
            fixed_schedules,
            float_schedules,
            periodic_fees,
            fixed_fees,
            principal_events,
            dates: Vec::new(),
            amort_setup,
            redemption_date,
            redemption_effective_date,
            principal_exchange: self.principal_exchange,
        };
        plan.dates = plan.collect_all_dates()?;
        debug!(
            dates = plan.dates.len(),
            %issue,
            %maturity,
            %redemption_date,
            "cashflow schedule: dates collected"
        );
        Ok(plan)
    }
}
