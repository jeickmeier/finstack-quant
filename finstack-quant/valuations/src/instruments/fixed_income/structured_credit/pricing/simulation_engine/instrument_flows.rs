//! Pool flows driven by instrument schedules.
//!
//! When a pool holds real instruments ([`InstrumentCollateral`]) the period
//! flows come from each instrument's own contractual schedule, bucketed by
//! legal payment period, instead of the balance-and-rate model. Defaults and
//! prepayments follow the deal's behavioural assumptions (or the
//! instrument's hazard curve when it carries one), applied as survival
//! scaling of the contractual amounts — the representative-line convention
//! the rate engine uses. Collateral draws (revolver utilization increases,
//! delayed-draw term-loan draws, loan-equivalent draws at default) are funded
//! through the reserve account by [`fund_collateral_draws`]; revolver
//! repayments replenish it through [`replenish_reserve_from_repayments`].
//!
//! The bucketed schedules ([`PreparedInstrumentSchedules`]) are built once
//! and shared: the deterministic [`InstrumentScheduleFlowSource`] and the
//! stochastic path source both drive [`run_period`] with per-name inputs
//! ([`NamePeriod`]) — a period default probability, a recovery rate and, for
//! revolvers with simulated utilization, the end-of-period utilization.

use super::exercise::{worst_call_period, ExerciseTerms, ExerciseWindow, RateView};
use super::reserve::{fund_collateral_draws, replenish_reserve_from_repayments};
use super::*;
use crate::cashflow::builder::{CashFlowSchedule, FloatingRateParams};
use crate::instruments::fixed_income::bond::CashflowSpec;
use crate::instruments::fixed_income::revolving_credit::pricing::path_generator::build_credit_spread_params;
use crate::instruments::fixed_income::revolving_credit::pricing::unified::{
    DEFAULT_CREDIT_SPREAD_IMPLIED_VOL, DEFAULT_UTIL_CREDIT_CORR,
};
use crate::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, CreditSpreadProcessSpec, DrawRepaySpec, McConfig, RevolvingCredit,
    StochasticUtilizationSpec, UtilizationProcess,
};
use crate::instruments::fixed_income::structured_credit::types::{
    CallExercisePolicy, CollateralInstrument, InstrumentCollateral,
};
use crate::instruments::fixed_income::term_loan::RateSpec;
use finstack_quant_cashflows::traits::CashflowScheduleSource;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_models::monte_carlo::process::cir::CirProcess;
use rust_decimal::prelude::ToPrimitive;

/// Utilization volatility below which the process is frozen (parity mode),
/// mirroring the standalone facility's Monte Carlo path generator.
pub(super) const FROZEN_UTILIZATION_VOL: f64 = 1e-8;

/// Instrument family, used where the engines treat the kinds differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CollateralKind {
    /// Universal bond.
    Bond,
    /// Term loan.
    TermLoan,
    /// Revolving credit facility.
    Revolver,
}

/// One coupon flow kept as components so a simulated rate path can re-project
/// it through the instrument's own floors, caps and gearing, and a simulated
/// balance can rescale it.
#[derive(Debug, Clone)]
pub(super) struct InterestComponent {
    /// Coupon amount projected on the valuation-date forward curve.
    pub(super) projected: f64,
    /// Balance the coupon accrues on (`projected / (rate × accrual)`), zero
    /// when the schedule recorded no rate.
    pub(super) basis: f64,
    /// Accrual fraction of the coupon.
    pub(super) accrual: f64,
    /// Projected index rate behind a floating coupon, when recorded.
    pub(super) index_rate: Option<f64>,
    /// Fixing date of a floating coupon, when known.
    pub(super) fixing_date: Option<Date>,
    /// All-in annual rate recorded on the flow, when known.
    pub(super) rate: Option<f64>,
}

impl InterestComponent {
    /// Re-projected all-in rate at `index_rate + shift` when the coupon is
    /// floating, its index rate is known, and its fixing is still ahead of
    /// `as_of`.
    fn reprojected_rate(
        &self,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> Option<f64> {
        if self.basis <= 0.0
            || self.accrual <= 0.0
            || self.fixing_date.is_some_and(|fixing| fixing <= as_of)
        {
            return None;
        }
        let (index_rate, params) = (self.index_rate?, params?);
        Some(
            crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                index_rate + shift,
                params,
            ),
        )
    }

    /// Coupon amount under an additive shift of the index rate.
    ///
    /// A zero shift, a fixing already observed on or before `as_of`, or a
    /// component without a recorded index rate returns the projected amount
    /// unchanged; otherwise the coupon is re-projected through the
    /// instrument's floors, caps and gearing at `index_rate + shift`.
    pub(super) fn amount_with_shift(
        &self,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> f64 {
        if shift == 0.0 {
            return self.projected;
        }
        match self.reprojected_rate(shift, as_of, params) {
            Some(all_in) => self.basis * self.accrual * all_in,
            None => self.projected,
        }
    }

    /// Coupon amount accrued on a simulated `balance` instead of the
    /// projected one: re-projected at `index_rate + shift` when the coupon
    /// is floating and unfixed, otherwise rescaled by `balance / basis`.
    pub(super) fn amount_on_balance(
        &self,
        balance: f64,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> f64 {
        if let Some(all_in) = self.reprojected_rate(shift, as_of, params) {
            return balance * self.accrual * all_in;
        }
        if self.basis > 0.0 {
            self.projected * balance / self.basis
        } else {
            self.projected
        }
    }
}

/// Fee family, which decides the balance a simulated fee accrues on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FeeKind {
    /// Commitment fee on the undrawn commitment.
    Commitment,
    /// Usage fee on the drawn balance.
    Usage,
    /// Facility fee on the total commitment, and upfront fees.
    Fixed,
}

/// One fee flow kept as components so a simulated drawn balance can rescale
/// it. Tiered fee rates enter through the schedule's period-average rate.
#[derive(Debug, Clone)]
pub(super) struct FeeComponent {
    /// Fee amount projected on the contractual (or expected) schedule.
    pub(super) projected: f64,
    /// Balance the fee accrued on (`projected / (rate × accrual)`), zero when
    /// the schedule recorded no rate.
    pub(super) basis: f64,
    /// Accrual fraction of the fee.
    pub(super) accrual: f64,
    /// Annual fee rate recorded on the flow, when known.
    pub(super) rate: Option<f64>,
    /// Fee family.
    pub(super) kind: FeeKind,
}

impl FeeComponent {
    /// Fee amount on a simulated position: `drawn` outstanding against
    /// `commitment` available, with `scale` the survival scale applied to
    /// fees that do not depend on utilization.
    pub(super) fn amount_on_balance(&self, drawn: f64, commitment: f64, scale: f64) -> f64 {
        let base = match self.kind {
            FeeKind::Commitment => (commitment - drawn).max(0.0),
            FeeKind::Usage => drawn.max(0.0),
            FeeKind::Fixed => return self.projected * scale,
        };
        match self.rate {
            Some(rate) if self.accrual > 0.0 => base * rate * self.accrual,
            _ if self.basis > 0.0 => self.projected * base / self.basis,
            _ => self.projected * scale,
        }
    }
}

/// Contractual flows of one instrument inside one legal payment period.
#[derive(Debug, Clone, Default)]
pub(super) struct PeriodBucket {
    /// Coupons (fixed, step-up, inflation, stub and floating), as components.
    pub(super) interest: Vec<InterestComponent>,
    /// Upfront, commitment, usage and facility fees, as components.
    pub(super) fees: Vec<FeeComponent>,
    /// Scheduled principal: amortization, revolver repayments, maturity balloon.
    pub(super) repayment: f64,
    /// Principal advanced to the borrower: revolver and delayed draws.
    pub(super) draw: f64,
    /// Interest capitalized into the balance (no cash).
    pub(super) pik: f64,
}

impl PeriodBucket {
    /// Total contractual coupon of the period on the valuation-date curve.
    pub(super) fn interest(&self) -> f64 {
        self.interest.iter().map(|c| c.projected).sum()
    }

    /// Total contractual fees of the period.
    pub(super) fn fees(&self) -> f64 {
        self.fees.iter().map(|f| f.projected).sum()
    }

    /// Total coupon of the period with floating components re-projected under
    /// an additive index shift (see [`InterestComponent::amount_with_shift`]).
    pub(super) fn interest_with_shift(
        &self,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> f64 {
        self.interest
            .iter()
            .map(|c| c.amount_with_shift(shift, as_of, params))
            .sum()
    }

    /// Total coupon of the period accrued on a simulated balance.
    pub(super) fn interest_on_balance(
        &self,
        balance: f64,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> f64 {
        self.interest
            .iter()
            .map(|c| c.amount_on_balance(balance, shift, as_of, params))
            .sum()
    }

    /// Total fees of the period on a simulated position.
    pub(super) fn fees_on_balance(&self, drawn: f64, commitment: f64, scale: f64) -> f64 {
        self.fees
            .iter()
            .map(|f| f.amount_on_balance(drawn, commitment, scale))
            .sum()
    }
}

/// Where an instrument's period default probability comes from.
#[derive(Debug, Clone)]
pub(super) enum PeriodDefaultSource {
    /// Conditional default probability from the instrument's hazard curve.
    HazardCurve(String),
    /// The deal's `DefaultModelSpec`.
    DealModel,
}

/// Credit-spread state of a simulated revolver.
#[derive(Debug, Clone)]
pub(super) enum SpreadTerms {
    /// Constant spread (no dynamics).
    Constant(f64),
    /// CIR spread process started at `initial`.
    Cir {
        /// Process stepped with the QE scheme.
        process: CirProcess,
        /// Initial spread the relative change is measured from.
        initial: f64,
    },
}

impl SpreadTerms {
    /// Spread at the simulation anchor.
    pub(super) fn initial(&self) -> f64 {
        match self {
            Self::Constant(spread) => *spread,
            Self::Cir { initial, .. } => *initial,
        }
    }
}

/// Utilization process of a revolver with simulated draws.
#[derive(Debug, Clone)]
pub(super) struct UtilizationTerms {
    /// Mean-reversion speed per year.
    pub(super) kappa: f64,
    /// Unlinked target utilization.
    pub(super) theta: f64,
    /// Annualized utilization volatility; below [`FROZEN_UTILIZATION_VOL`]
    /// the utilization is frozen.
    pub(super) sigma: f64,
    /// Target shift per unit of relative spread change.
    pub(super) spread_sensitivity: f64,
    /// Correlation between the utilization shock and the name's spread shock.
    pub(super) util_credit_corr: f64,
    /// Spread process.
    pub(super) spread: SpreadTerms,
}

/// Contractual credit margin of a revolver, the rate a simulated draw is
/// valued against the path's fair spread.
#[derive(Debug, Clone, Copy)]
pub(super) enum MarginTerms {
    /// Spread over the floating index (decimal).
    Spread(f64),
    /// Fixed all-in rate; the margin is the rate less the discount curve's
    /// par forward to maturity at the draw date.
    FixedRate(f64),
}

/// Revolver terms the engines need beyond the bucketed schedule.
#[derive(Debug, Clone)]
pub(super) struct RevolverTerms {
    /// Total commitment.
    pub(super) commitment: f64,
    /// Loan-equivalent exposure: fraction of the undrawn commitment drawn at
    /// default.
    pub(super) leq: f64,
    /// Interest accrual day count.
    pub(super) day_count: DayCount,
    /// Contractual margin.
    pub(super) margin: MarginTerms,
    /// Utilization process for stochastic facilities; `None` keeps the
    /// contractual (or expected) draw schedule.
    pub(super) utilization: Option<UtilizationTerms>,
}

/// One instrument's bucketed schedule and resolved terms.
#[derive(Debug, Clone)]
pub(super) struct InstrumentSchedule {
    /// Instrument identifier.
    pub(super) id: String,
    /// Instrument family.
    pub(super) kind: CollateralKind,
    /// Outstanding at the valuation date under the contractual schedule.
    pub(super) opening_balance: f64,
    /// Contractual maturity.
    pub(super) maturity: Date,
    /// Contractual flows per legal period, aligned with the prepared periods.
    pub(super) periods: Vec<PeriodBucket>,
    /// Exercise windows and policies.
    pub(super) exercise: ExerciseTerms,
    /// Default-probability source.
    pub(super) default_source: PeriodDefaultSource,
    /// Explicit recovery fraction; `None` uses the hazard curve or deal recovery.
    pub(super) recovery_rate: Option<f64>,
    /// Floating-rate terms for re-projection; `None` for fixed instruments.
    pub(super) floating_params: Option<FloatingRateParams>,
    /// Revolver terms; `None` for bonds and term loans.
    pub(super) revolver: Option<RevolverTerms>,
}

impl InstrumentSchedule {
    /// All-in rate to accrue a simulated revolver balance at when period `k`
    /// carries no projected coupon (the expected schedule had no balance):
    /// the rate recorded on the nearest bucket with a coupon, else the fixed
    /// rate, else zero.
    fn fallback_rate(&self, k: usize) -> f64 {
        let recorded = |bucket: &PeriodBucket| bucket.interest.iter().find_map(|c| c.rate);
        let n = self.periods.len();
        for offset in 0..n {
            if let Some(rate) = self.periods.get(k + offset).and_then(recorded) {
                return rate;
            }
            if offset > 0 && offset <= k {
                if let Some(rate) = self.periods.get(k - offset).and_then(recorded) {
                    return rate;
                }
            }
        }
        match self.revolver.as_ref().map(|r| r.margin) {
            Some(MarginTerms::FixedRate(rate)) => rate,
            _ => 0.0,
        }
    }
}

/// Running state of one instrument inside one simulation run.
#[derive(Debug, Clone)]
pub(super) struct NameState {
    /// Survival scale applied to contractual amounts: the product of default
    /// and prepayment survivors realized so far.
    pub(super) scale: f64,
    /// `true` once redeemed, matured or fully defaulted.
    pub(super) retired: bool,
    /// Simulated utilization (fraction of commitment) for stochastic revolvers.
    pub(super) utilization: f64,
    /// Simulated credit spread for stochastic revolvers.
    pub(super) spread: f64,
    /// Balance draw funded for the instrument in the latest period.
    pub(super) period_draw: f64,
}

/// Bucketed instrument schedules prepared once per valuation and shared by
/// every path.
pub(crate) struct PreparedInstrumentSchedules {
    /// One schedule per pool row, in pool order.
    pub(super) schedules: Vec<InstrumentSchedule>,
    /// Valuation date the schedules were projected at; fixings on or before
    /// it are observed and never re-projected.
    pub(super) as_of: Date,
    /// Whether any instrument uses the refinancing-incentive call rule, which
    /// needs the deal discount curve each period.
    pub(super) needs_discount_curve: bool,
}

impl PreparedInstrumentSchedules {
    /// Bucket every instrument's contractual schedule onto the prepared legal
    /// periods and resolve exercise, default and revolver terms.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Resolved deal whose pool carries instrument collateral.
    /// * `context` - Market context used to project each instrument's schedule.
    /// * `prepared` - Prepared simulation supplying the valuation date and periods.
    pub(crate) fn prepare(
        instrument: &StructuredCredit,
        context: &MarketContext,
        prepared: &PreparedDealSimulation,
    ) -> Result<Self> {
        let collateral = instrument.pool.instruments.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "instrument-schedule pool flows require instrument collateral".into(),
            )
        })?;
        let as_of = prepared.valuation_date;
        let period_dates: Vec<Date> = prepared.periods.iter().map(|p| p.payment_date).collect();
        if period_dates.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "instrument-schedule pool flows require at least one future payment period".into(),
            ));
        }

        let mut schedules = Vec::with_capacity(collateral.len());
        for held in collateral.iter() {
            schedules.push(build_schedule(
                held,
                collateral,
                context,
                as_of,
                &period_dates,
            )?);
        }
        // The refinancing-incentive rule and the margin of a fixed-rate
        // revolver with simulated draws both read the deal discount curve.
        let needs_discount_curve = schedules.iter().any(|s| {
            matches!(
                s.exercise.call_policy,
                CallExercisePolicy::RefinancingIncentive { .. }
            ) || s.revolver.as_ref().is_some_and(|r| {
                r.utilization.is_some() && matches!(r.margin, MarginTerms::FixedRate(_))
            })
        });
        Ok(Self {
            schedules,
            as_of,
            needs_discount_curve,
        })
    }

    /// Whether any revolver has simulated draws to value against its path's
    /// fair spread.
    pub(crate) fn has_draw_option_cost(&self) -> bool {
        self.schedules
            .iter()
            .any(|s| s.revolver.as_ref().is_some_and(|r| r.utilization.is_some()))
    }

    /// Fresh per-name states for one run.
    pub(super) fn initial_names(&self) -> Vec<NameState> {
        self.schedules
            .iter()
            .map(|schedule| {
                let (utilization, spread) = match &schedule.revolver {
                    Some(terms) => (
                        if terms.commitment > 0.0 {
                            (schedule.opening_balance / terms.commitment).clamp(0.0, 1.0)
                        } else {
                            0.0
                        },
                        terms
                            .utilization
                            .as_ref()
                            .map_or(0.0, |u| u.spread.initial()),
                    ),
                    None => (0.0, 0.0),
                };
                NameState {
                    scale: 1.0,
                    retired: false,
                    utilization,
                    spread,
                    period_draw: 0.0,
                }
            })
            .collect()
    }

    /// The deal discount curve when any instrument's exercise rule needs it.
    pub(super) fn discount_curve(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<Option<Arc<DiscountCurve>>> {
        if self.needs_discount_curve {
            Ok(Some(
                context.get_discount(instrument.discount_curve_id.as_str())?,
            ))
        } else {
            Ok(None)
        }
    }

    /// Verify the contractual draw calendar can be funded period by period,
    /// so a deterministic run never silently caps a draw.
    ///
    /// # Arguments
    ///
    /// * `reserve` - Opening reserve balance.
    /// * `target` - Reserve target that revolver repayments replenish toward.
    /// * `period_dates` - Payment date of each legal period.
    pub(super) fn check_deterministic_funding(
        &self,
        reserve: f64,
        target: Option<f64>,
        period_dates: &[Date],
    ) -> Result<()> {
        let mut reserve = reserve.max(0.0);
        for (k, pay_date) in period_dates.iter().enumerate() {
            let mut requested = 0.0;
            let mut collections = 0.0;
            let mut revolver_repayments = 0.0;
            let mut drawing: Vec<&str> = Vec::new();
            for schedule in &self.schedules {
                let Some(bucket) = schedule.periods.get(k) else {
                    continue;
                };
                if bucket.draw > 0.0 {
                    drawing.push(schedule.id.as_str());
                }
                requested += bucket.draw;
                collections += bucket.repayment;
                if schedule.kind == CollateralKind::Revolver {
                    revolver_repayments += bucket.repayment;
                }
            }
            let available = reserve + collections;
            if requested > available + WRITEDOWN_DE_MINIMIS {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "collateral draws of {requested:.2} on {pay_date} by [{}] exceed the reserve \
                     account ({reserve:.2}) plus that period's principal collections \
                     ({collections:.2}); fund the reserve or reduce the commitments",
                    drawing.join(", ")
                )));
            }
            let from_reserve = requested.min(reserve);
            reserve -= from_reserve;
            let from_principal = requested - from_reserve;
            if let Some(target) = target {
                let room = (collections - from_principal).max(0.0);
                let top_up = revolver_repayments
                    .min(room)
                    .min((target - reserve).max(0.0));
                reserve += top_up;
            }
        }
        Ok(())
    }
}

/// Period default probability and recovery of one instrument from its hazard
/// curve: the probability of defaulting in `(prev_date, pay_date]`
/// conditional on surviving to `prev_date`.
///
/// # Arguments
///
/// * `context` - Market context holding the hazard curve.
/// * `curve_id` - Hazard curve identifier.
/// * `prev_date` - Period start (exclusive).
/// * `pay_date` - Period payment date (inclusive).
/// * `recovery_override` - Instrument recovery that replaces the curve's.
pub(super) fn hazard_period_default(
    context: &MarketContext,
    curve_id: &str,
    prev_date: Date,
    pay_date: Date,
    recovery_override: Option<f64>,
) -> Result<(f64, f64)> {
    let hazard = context.get_hazard(curve_id)?;
    let start = prev_date.max(hazard.base_date());
    let sp = hazard.survival_at_dates(&[start, pay_date.max(start)])?;
    let pd = if sp[0] > 0.0 {
        (1.0 - sp[1] / sp[0]).clamp(0.0, 1.0)
    } else {
        1.0
    };
    Ok((pd, recovery_override.unwrap_or(hazard.recovery_rate())))
}

/// Per-name inputs for one legal period.
#[derive(Debug, Clone, Copy)]
pub(super) struct NamePeriod {
    /// Period default probability (a realized default is `1.0`).
    pub(super) pd: f64,
    /// Recovery fraction applied to this period's default.
    pub(super) recovery_rate: f64,
    /// End-of-period utilization for revolvers with simulated draws; `None`
    /// keeps the bucketed draw schedule.
    pub(super) utilization: Option<f64>,
    /// Interest added on the counterfactual path where earlier draws accrue
    /// at their fair spread instead of the contractual margin (signed, in
    /// pool currency, before the default haircut).
    pub(super) interest_addon: f64,
}

/// One legal period's inputs shared by every instrument.
pub(super) struct PeriodModel<'a> {
    /// Index of the legal period being simulated.
    pub(super) k: usize,
    /// Period start (exclusive).
    pub(super) prev_date: Date,
    /// Period payment date (inclusive).
    pub(super) pay_date: Date,
    /// Behavioural prepayment rate for the period, applied to bonds and loans.
    pub(super) period_smm: f64,
    /// Refinancing-rate view for the incentive exercise rule.
    pub(super) rate_view: RateView<'a>,
    /// Per-name inputs, in pool order.
    pub(super) names: &'a [NamePeriod],
}

/// A funded draw's destination.
enum DrawUse {
    /// Advanced to the borrower: the instrument's balance rises.
    Balance,
    /// Drawn at default and written off, recovering at the given rate.
    LossAtDefault { recovery_rate: f64 },
}

struct DrawRequest {
    index: usize,
    amount: f64,
    destination: DrawUse,
}

/// Simulate one legal period of instrument collateral.
///
/// Applies, per instrument, the period default and recovery, the coupon and
/// fees (re-projected on the rate shift and, for simulated revolvers, on the
/// simulated balance), scheduled or simulated principal, behavioural
/// prepayment and exercise; then funds the period's draws from the reserve
/// and principal collections and replenishes the reserve from revolver
/// repayments.
///
/// # Arguments
///
/// * `prepared` - Shared bucketed schedules.
/// * `names` - Per-name running states of this run, in pool order.
/// * `state` - Simulation state whose pool rows and reserve are updated.
/// * `model` - This period's dates and per-name inputs.
pub(super) fn run_period(
    prepared: &PreparedInstrumentSchedules,
    names: &mut [NameState],
    state: &mut SimulationState<'_>,
    model: PeriodModel<'_>,
) -> Result<PoolFlows> {
    let ccy = state.base_currency;
    let as_of = prepared.as_of;
    let schedules = &prepared.schedules;
    let k = model.k;
    let pay_date = model.pay_date;
    let prev_date = model.prev_date;

    if schedules.len() != state.pool_state.len()
        || names.len() != schedules.len()
        || model.names.len() != schedules.len()
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "instrument schedules ({}) do not align with pool rows ({})",
            schedules.len(),
            state.pool_state.len()
        )));
    }

    // The materialized rows carry closing-date balances; the schedules know
    // the outstanding at the valuation date.
    if k == 0 {
        for (i, schedule) in schedules.iter().enumerate() {
            if !state.pool_state.is_defaulted[i] {
                state.pool_state.balances[i] = schedule.opening_balance;
            }
        }
        state.pool_outstanding = Money::new(state.pool_state.balances.iter().sum(), ccy)?;
    }

    let empty = PeriodBucket::default();
    let shift = state.floating_rate_shift;
    let mut flows = PoolFlows::zero(ccy);
    let mut total_interest = 0.0;
    let mut total_scheduled = 0.0;
    let mut total_prepay = 0.0;
    let mut total_default = 0.0;
    let mut total_recovery = 0.0;
    let mut total_premium = 0.0;
    let mut requests: Vec<DrawRequest> = Vec::new();
    let mut revolver_repayments = 0.0;

    for name in names.iter_mut() {
        name.period_draw = 0.0;
    }
    for (i, schedule) in schedules.iter().enumerate() {
        let name = &mut names[i];
        if name.retired || state.pool_state.is_defaulted[i] {
            continue;
        }
        let input = model.names[i];
        let bucket = schedule.periods.get(k).unwrap_or(&empty);
        let balance = state.pool_state.balances[i].max(0.0);
        let simulated = schedule
            .revolver
            .as_ref()
            .and_then(|terms| input.utilization.map(|u| (terms, u)));
        if balance <= 0.0 && bucket.draw <= 0.0 && bucket.pik <= 0.0 && simulated.is_none() {
            continue;
        }

        let pd = input.pd.clamp(0.0, 1.0);
        let recovery_rate = input.recovery_rate.clamp(0.0, 1.0);
        let scale_bop = name.scale;
        // Interest and fees on the pre-default position, haircut for the
        // defaulting fraction accruing half the period on average.
        let haircut = 1.0 - 0.5 * pd;
        let params = schedule.floating_params.as_ref();
        let carry = match simulated {
            Some((terms, _)) => {
                let commitment = terms.commitment * scale_bop;
                let interest = if bucket.interest.is_empty() {
                    // The expected schedule carried no balance this period;
                    // accrue the simulated balance at the nearest recorded rate.
                    let accrual = terms.day_count.year_fraction(
                        prev_date.max(as_of),
                        pay_date,
                        DayCountContext::default(),
                    )?;
                    balance * accrual * schedule.fallback_rate(k)
                } else {
                    bucket.interest_on_balance(balance, shift, as_of, params)
                };
                interest + bucket.fees_on_balance(balance, commitment, scale_bop)
            }
            None => (bucket.interest_with_shift(shift, as_of, params) + bucket.fees()) * scale_bop,
        };
        total_interest += (carry + input.interest_addon) * haircut;

        // Loan-equivalent exposure: the defaulting fraction draws part of its
        // undrawn commitment, funded through the reserve and lost at once.
        if let Some(terms) = &schedule.revolver {
            if terms.leq > 0.0 && pd > 0.0 {
                let undrawn = (terms.commitment * scale_bop - balance).max(0.0);
                let leq_draw = undrawn * terms.leq * pd;
                if leq_draw > 0.0 {
                    requests.push(DrawRequest {
                        index: i,
                        amount: leq_draw,
                        destination: DrawUse::LossAtDefault { recovery_rate },
                    });
                }
            }
        }

        let default_amt = balance * pd;
        total_default += default_amt;
        total_recovery += default_amt * recovery_rate;
        if pd >= 1.0 - 1e-10 {
            state.pool_state.is_defaulted[i] = true;
            state.pool_state.balances[i] = 0.0;
            name.retired = true;
            name.scale = 0.0;
            continue;
        }

        let scale_after = scale_bop * (1.0 - pd);
        let after_default = balance - default_amt;
        let (repayment, draw_req, pik) = match simulated {
            Some((terms, utilization)) => {
                let target = utilization.clamp(0.0, 1.0) * terms.commitment * scale_after;
                (
                    (after_default - target).max(0.0),
                    (target - after_default).max(0.0),
                    0.0,
                )
            }
            None => (
                (bucket.repayment * scale_after).min(after_default),
                bucket.draw * scale_after,
                bucket.pik * scale_after,
            ),
        };
        let mut remaining = (after_default - repayment).max(0.0);

        // Prepayment on the survivor after scheduled principal; revolvers
        // have no behavioural prepayment, utilization is their model.
        let prepay = if schedule.kind == CollateralKind::Revolver {
            0.0
        } else {
            remaining * model.period_smm
        };
        remaining -= prepay;

        // Call or put exercise on whatever remains.
        let mut redeemed = 0.0;
        if remaining > 0.0 {
            if let Some(redemption) =
                schedule
                    .exercise
                    .evaluate(k, prev_date, pay_date, model.rate_view)?
            {
                let par = remaining;
                let cash = par * redemption.price_pct.min(100.0) / 100.0;
                let premium = par * (redemption.price_pct - 100.0).max(0.0) / 100.0;
                total_premium += premium;
                total_default += par - cash;
                redeemed = cash;
                remaining = 0.0;
                name.retired = true;
            }
        }
        // A simulated revolver retires at its maturity period.
        if simulated.is_some() && pay_date >= schedule.maturity {
            name.retired = true;
        }

        total_scheduled += repayment;
        total_prepay += prepay + redeemed;
        if schedule.kind == CollateralKind::Revolver {
            revolver_repayments += repayment;
        }

        name.scale = if name.retired {
            0.0
        } else if schedule.kind == CollateralKind::Revolver {
            scale_after
        } else {
            scale_after * (1.0 - model.period_smm)
        };
        state.pool_state.balances[i] = remaining + pik;
        if draw_req > 0.0 && !name.retired {
            requests.push(DrawRequest {
                index: i,
                amount: draw_req,
                destination: DrawUse::Balance,
            });
        }
    }

    // Fund this period's draws: reserve first, then principal collections.
    let requested_total: f64 = requests.iter().map(|r| r.amount).sum();
    let collections = total_scheduled + total_prepay;
    let funding = fund_collateral_draws(
        state,
        Money::new(requested_total, ccy)?,
        Money::new(collections, ccy)?,
    )?;
    if funding.unfunded.amount() > 0.0 {
        tracing::warn!(
            target: "finstack_quant_valuations::structured_credit",
            pay_date = %pay_date,
            unfunded = funding.unfunded.amount(),
            "collateral draws exceeded the reserve and principal collections; the \
             shortfall is capped and recorded as unfunded"
        );
    }
    let funded = funding.from_reserve.amount() + funding.from_principal.amount();
    let ratio = if requested_total > 0.0 {
        funded / requested_total
    } else {
        0.0
    };
    for request in requests {
        let amount = request.amount * ratio;
        match request.destination {
            DrawUse::Balance => {
                state.pool_state.balances[request.index] += amount;
                names[request.index].period_draw += amount;
            }
            DrawUse::LossAtDefault { recovery_rate } => {
                total_default += amount;
                total_recovery += amount * recovery_rate;
            }
        }
    }

    // Revolver repayments replenish the reserve toward its target, bounded
    // by the principal cash still available after draw funding.
    let room = (collections - funding.from_principal.amount()).max(0.0);
    let replenished =
        replenish_reserve_from_repayments(state, Money::new(revolver_repayments.min(room), ccy)?)?;

    flows.interest = Money::new(total_interest, ccy)?;
    flows.scheduled_principal = Money::new(total_scheduled, ccy)?;
    flows.prepayment = Money::new(total_prepay, ccy)?;
    flows.default = Money::new(total_default, ccy)?;
    flows.recovery = Money::new(total_recovery, ccy)?;
    flows.call_premium = Money::new(total_premium, ccy)?;
    flows.draw_from_principal = funding.from_principal;
    flows.reserve_replenished = replenished;
    Ok(flows)
}

/// [`PoolFlowSource`] driven by instrument schedules and the deal's
/// deterministic behavioural assumptions.
pub(crate) struct InstrumentScheduleFlowSource {
    prepared: PreparedInstrumentSchedules,
    names: Vec<NameState>,
    inputs: Vec<NamePeriod>,
    next_period: usize,
}

impl InstrumentScheduleFlowSource {
    /// Bucket every instrument's contractual schedule onto the prepared legal
    /// periods and verify the contractual draw calendar can be funded.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Resolved deal whose pool carries instrument collateral.
    /// * `context` - Market context used to project each instrument's schedule.
    /// * `prepared` - Prepared simulation supplying the valuation date and periods.
    pub(crate) fn prepare(
        instrument: &StructuredCredit,
        context: &MarketContext,
        prepared: &PreparedDealSimulation,
    ) -> Result<Self> {
        let schedules = PreparedInstrumentSchedules::prepare(instrument, context, prepared)?;
        let period_dates: Vec<Date> = prepared.periods.iter().map(|p| p.payment_date).collect();
        schedules.check_deterministic_funding(
            instrument.pool.reserve_account.amount(),
            instrument.pool.reserve_target.map(|m| m.amount()),
            &period_dates,
        )?;
        let names = schedules.initial_names();
        let inputs = Vec::with_capacity(names.len());
        Ok(Self {
            prepared: schedules,
            names,
            inputs,
            next_period: 0,
        })
    }
}

impl PoolFlowSource for InstrumentScheduleFlowSource {
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows> {
        let k = self.next_period;
        self.next_period += 1;
        let deal = request.instrument;

        // Deal-level behavioural rates for instruments without a hazard curve.
        let deal_mdr = period_averaged_monthly_rate(
            request.pay_date,
            request.seasoning_months,
            request.months_per_period,
            |_, seasoning| deal.credit_model.default_spec.mdr(seasoning),
        )?;
        let deal_smm = period_averaged_monthly_rate(
            request.pay_date,
            request.seasoning_months,
            request.months_per_period,
            |_, seasoning| deal.credit_model.prepayment_spec.smm(seasoning),
        )?;
        let period_pd = 1.0 - (1.0 - deal_mdr.clamp(0.0, 1.0)).powf(request.months_per_period);
        let period_smm = 1.0 - (1.0 - deal_smm.clamp(0.0, 1.0)).powf(request.months_per_period);
        let deal_recovery = deal.credit_model.recovery_spec.rate;

        self.inputs.clear();
        for (i, schedule) in self.prepared.schedules.iter().enumerate() {
            let alive = !self.names[i].retired && !request.state.pool_state.is_defaulted[i];
            let (pd, recovery_rate) = if !alive {
                (0.0, 0.0)
            } else {
                match &schedule.default_source {
                    PeriodDefaultSource::HazardCurve(id) => hazard_period_default(
                        request.context,
                        id,
                        request.prev_date,
                        request.pay_date,
                        schedule.recovery_rate,
                    )?,
                    PeriodDefaultSource::DealModel => {
                        (period_pd, schedule.recovery_rate.unwrap_or(deal_recovery))
                    }
                }
            };
            self.inputs.push(NamePeriod {
                pd,
                recovery_rate,
                utilization: None,
                interest_addon: 0.0,
            });
        }

        let discount_curve = self.prepared.discount_curve(deal, request.context)?;
        let rate_view = RateView {
            curve: discount_curve.as_deref().map(|c| c as &dyn Discounting),
            shift: request.state.floating_rate_shift,
        };
        run_period(
            &self.prepared,
            &mut self.names,
            request.state,
            PeriodModel {
                k,
                prev_date: request.prev_date,
                pay_date: request.pay_date,
                period_smm,
                rate_view,
                names: &self.inputs,
            },
        )
    }
}

/// Bucket one instrument's schedule and resolve its terms.
fn build_schedule(
    held: CollateralInstrument<'_>,
    collateral: &InstrumentCollateral,
    context: &MarketContext,
    as_of: Date,
    period_dates: &[Date],
) -> Result<InstrumentSchedule> {
    let id = held.id().clone();
    let (kind, schedule, opening_balance, maturity) = match held {
        CollateralInstrument::Bond(bond) => {
            let schedule = bond.raw_cashflow_schedule(context, as_of)?;
            let opening = bond.notional.amount() + past_principal_delta(&schedule, as_of, false);
            (
                CollateralKind::Bond,
                schedule,
                opening.max(0.0),
                bond.maturity,
            )
        }
        CollateralInstrument::TermLoan(loan) => {
            let schedule = loan.raw_cashflow_schedule(context, as_of)?;
            let opening = past_principal_delta(&schedule, as_of, true);
            (
                CollateralKind::TermLoan,
                schedule,
                opening.max(0.0),
                loan.maturity,
            )
        }
        CollateralInstrument::Revolver(facility) => {
            let schedule = facility.raw_cashflow_schedule(context, as_of)?;
            let opening = if facility.is_deterministic() {
                crate::instruments::fixed_income::revolving_credit::cashflow_engine::calculate_drawn_balance_at_date(facility, as_of)?.amount()
            } else {
                facility.drawn_amount.amount()
            };
            (
                CollateralKind::Revolver,
                schedule,
                opening,
                facility.maturity,
            )
        }
    };

    let periods = bucket_flows(&schedule, as_of, period_dates);

    // Exercise terms.
    let (calls, puts): (Vec<ExerciseWindow>, Vec<ExerciseWindow>) = match held {
        CollateralInstrument::Bond(bond) => match &bond.call_put {
            Some(schedule) => (
                schedule
                    .calls
                    .iter()
                    .map(|c| ExerciseWindow {
                        start: c.start_date,
                        end: c.end_date,
                        price_pct: c.price_pct_of_par,
                    })
                    .collect(),
                schedule
                    .puts
                    .iter()
                    .map(|p| ExerciseWindow {
                        start: p.start_date,
                        end: p.end_date,
                        price_pct: p.price_pct_of_par,
                    })
                    .collect(),
            ),
            None => (Vec::new(), Vec::new()),
        },
        CollateralInstrument::TermLoan(loan) => (
            loan.call_schedule
                .as_ref()
                .map(|s| {
                    s.calls
                        .iter()
                        .map(|c| ExerciseWindow {
                            start: c.date,
                            end: loan.maturity,
                            price_pct: c.price_pct_of_par,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Vec::new(),
        ),
        CollateralInstrument::Revolver(_) => (Vec::new(), Vec::new()),
    };
    let fixed_coupon = match held {
        CollateralInstrument::Bond(bond) => fixed_coupon_of(&bond.cashflow_spec)?,
        CollateralInstrument::TermLoan(loan) => match &loan.rate {
            RateSpec::Fixed { rate_bp } => Some(f64::from(*rate_bp) / 10_000.0),
            RateSpec::Floating(_) => None,
        },
        CollateralInstrument::Revolver(facility) => match &facility.base_rate_spec {
            BaseRateSpec::Fixed { rate } => Some(*rate),
            BaseRateSpec::Floating(_) => None,
        },
    };
    let call_policy = collateral.call_policy_for(&id);
    let put_policy = collateral.put_policy_for(&id);
    let worst_period = if call_policy == CallExercisePolicy::Worst && !calls.is_empty() {
        let price = held.quoted_clean_price().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "call policy 'worst' requires a quoted clean price on instrument '{id}'"
            ))
        })?;
        let mut cash_by_period = Vec::with_capacity(periods.len());
        let mut balance_by_period = Vec::with_capacity(periods.len());
        let mut running = opening_balance;
        for bucket in &periods {
            cash_by_period.push(bucket.interest() + bucket.fees() + bucket.repayment);
            running = (running - bucket.repayment + bucket.draw + bucket.pik).max(0.0);
            balance_by_period.push(running);
        }
        worst_call_period(
            as_of,
            price,
            opening_balance,
            period_dates,
            &cash_by_period,
            &balance_by_period,
            &calls,
        )?
    } else {
        None
    };
    let exercise = ExerciseTerms {
        calls,
        puts,
        call_policy,
        put_policy,
        fixed_coupon,
        maturity,
        worst_period,
    };

    let (default_source, recovery_rate) = match held {
        CollateralInstrument::Bond(bond) => (
            bond.credit_curve_id
                .as_ref()
                .map_or(PeriodDefaultSource::DealModel, |c| {
                    PeriodDefaultSource::HazardCurve(c.to_string())
                }),
            None,
        ),
        CollateralInstrument::TermLoan(loan) => (
            loan.credit_curve_id
                .as_ref()
                .map_or(PeriodDefaultSource::DealModel, |c| {
                    PeriodDefaultSource::HazardCurve(c.to_string())
                }),
            None,
        ),
        CollateralInstrument::Revolver(facility) => (
            facility
                .credit_curve_id
                .as_ref()
                .map_or(PeriodDefaultSource::DealModel, |c| {
                    PeriodDefaultSource::HazardCurve(c.to_string())
                }),
            Some(facility.recovery_rate),
        ),
    };
    let floating_params = match held {
        CollateralInstrument::Bond(bond) => floating_params_of(&bond.cashflow_spec)?,
        CollateralInstrument::TermLoan(loan) => match &loan.rate {
            RateSpec::Floating(spec) => Some(FloatingRateParams::try_from(spec)?),
            RateSpec::Fixed { .. } => None,
        },
        CollateralInstrument::Revolver(facility) => match &facility.base_rate_spec {
            BaseRateSpec::Floating(spec) => Some(FloatingRateParams::try_from(spec)?),
            BaseRateSpec::Fixed { .. } => None,
        },
    };
    let revolver = match held {
        CollateralInstrument::Revolver(facility) => Some(RevolverTerms {
            commitment: facility.commitment_amount.amount(),
            leq: facility.leq,
            day_count: facility.day_count,
            margin: match &facility.base_rate_spec {
                BaseRateSpec::Fixed { rate } => MarginTerms::FixedRate(*rate),
                BaseRateSpec::Floating(spec) => MarginTerms::Spread(
                    spec.spread_bp
                        .to_f64()
                        .map(|bp| bp / 10_000.0)
                        .ok_or_else(|| {
                            finstack_quant_core::Error::Validation(format!(
                                "spread {} of '{id}' cannot be represented as f64",
                                spec.spread_bp
                            ))
                        })?,
                ),
            },
            utilization: match &facility.draw_repay_spec {
                DrawRepaySpec::Deterministic(_) => None,
                DrawRepaySpec::Stochastic(spec) => {
                    Some(utilization_terms(facility, spec, context, as_of)?)
                }
            },
        }),
        CollateralInstrument::Bond(_) | CollateralInstrument::TermLoan(_) => None,
    };

    Ok(InstrumentSchedule {
        id: id.to_string(),
        kind,
        opening_balance,
        maturity,
        periods,
        exercise,
        default_source,
        recovery_rate,
        floating_params,
        revolver,
    })
}

/// Utilization and spread process terms of a stochastic revolver, mirroring
/// the standalone facility pricer's configuration (including its synthesized
/// market-anchored spread and utilization–credit correlation when the
/// facility carries a hazard curve but no explicit `McConfig`).
fn utilization_terms(
    facility: &RevolvingCredit,
    spec: &StochasticUtilizationSpec,
    context: &MarketContext,
    as_of: Date,
) -> Result<UtilizationTerms> {
    let UtilizationProcess::MeanReverting {
        target_rate,
        speed,
        volatility,
        spread_sensitivity,
    } = &spec.utilization_process;
    let anchor = as_of.max(facility.commitment_date);
    let synthesized;
    let mc_config: &McConfig = match &spec.mc_config {
        Some(config) => config,
        None => {
            let (credit_spread_process, util_credit_corr) = match &facility.credit_curve_id {
                Some(hazard_id) => (
                    CreditSpreadProcessSpec::MarketAnchored {
                        credit_curve_id: hazard_id.clone(),
                        kappa: 0.1,
                        implied_vol: DEFAULT_CREDIT_SPREAD_IMPLIED_VOL,
                        tenor_years: None,
                    },
                    Some(DEFAULT_UTIL_CREDIT_CORR),
                ),
                None => (CreditSpreadProcessSpec::Constant(0.0), None),
            };
            synthesized = McConfig {
                correlation_matrix: None,
                recovery_rate: facility.recovery_rate,
                credit_spread_process,
                interest_rate_process: None,
                util_credit_corr,
            };
            &synthesized
        }
    };
    let spread = match &mc_config.credit_spread_process {
        CreditSpreadProcessSpec::Constant(spread) => SpreadTerms::Constant(spread.max(0.0)),
        CreditSpreadProcessSpec::Cir { .. } | CreditSpreadProcessSpec::MarketAnchored { .. } => {
            let params = build_credit_spread_params(mc_config, facility, context, anchor)?;
            SpreadTerms::Cir {
                process: CirProcess::new(params.cir),
                initial: params.initial,
            }
        }
    };
    let util_credit_corr = mc_config
        .correlation_matrix
        .map(|m| m[0][2])
        .or(mc_config.util_credit_corr)
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0);
    Ok(UtilizationTerms {
        kappa: *speed,
        theta: *target_rate,
        sigma: *volatility,
        spread_sensitivity: *spread_sensitivity,
        util_credit_corr,
        spread,
    })
}

/// Net principal change implied by flows dated on or before `as_of`.
///
/// Principal kinds carry lender-perspective signs: draws and initial funding
/// are negative (balance up), repayments positive (balance down); PIK
/// capitalization raises the balance. `include_funding` replays the initial
/// funding flow (term loans); bonds start from their notional instead.
fn past_principal_delta(schedule: &CashFlowSchedule, as_of: Date, include_funding: bool) -> f64 {
    let mut delta = 0.0;
    for cf in schedule.get_flows() {
        if cf.date > as_of {
            continue;
        }
        let amount = cf.amount.amount();
        match cf.kind {
            CFKind::Notional
            | CFKind::Amortization
            | CFKind::PrePayment
            | CFKind::RevolvingDraw
            | CFKind::RevolvingRepayment => {
                if amount < 0.0 && !include_funding {
                    continue;
                }
                delta -= amount;
            }
            CFKind::Pik => delta += amount,
            _ => {}
        }
    }
    delta
}

/// Balance a rate-based flow accrued on: `amount / (rate × accrual)`.
fn accrual_basis(cf: &CashFlow) -> f64 {
    match cf.rate {
        Some(rate) if rate.abs() > 1e-12 && cf.accrual_factor > 0.0 => {
            cf.amount.amount() / (rate * cf.accrual_factor)
        }
        _ => 0.0,
    }
}

/// Bucket a schedule's future flows by legal period `(pay_{k-1}, pay_k]`,
/// with flows after the last payment date landing in the last bucket.
fn bucket_flows(
    schedule: &CashFlowSchedule,
    as_of: Date,
    period_dates: &[Date],
) -> Vec<PeriodBucket> {
    let mut periods: Vec<PeriodBucket> = (0..period_dates.len())
        .map(|_| PeriodBucket::default())
        .collect();
    let fixings = &schedule.get_meta().projected_fixings;
    for cf in schedule.get_flows() {
        if cf.date <= as_of {
            continue;
        }
        let k = period_dates
            .iter()
            .position(|pay| *pay >= cf.date)
            .unwrap_or(period_dates.len() - 1);
        let bucket = &mut periods[k];
        let amount = cf.amount.amount();
        match cf.kind {
            CFKind::Fixed | CFKind::InflationCoupon | CFKind::Stub => {
                bucket.interest.push(InterestComponent {
                    projected: amount,
                    basis: accrual_basis(cf),
                    accrual: cf.accrual_factor,
                    index_rate: None,
                    fixing_date: None,
                    rate: cf.rate,
                });
            }
            CFKind::FloatReset => {
                let index_rate = cf.reset_date.and_then(|reset| {
                    fixings
                        .iter()
                        .find(|f| f.date == reset)
                        .and_then(|f| f.value)
                });
                bucket.interest.push(InterestComponent {
                    projected: amount,
                    basis: accrual_basis(cf),
                    accrual: cf.accrual_factor,
                    index_rate,
                    fixing_date: cf.reset_date,
                    rate: cf.rate,
                });
            }
            CFKind::Fee | CFKind::CommitmentFee | CFKind::UsageFee | CFKind::FacilityFee => {
                let kind = match cf.kind {
                    CFKind::CommitmentFee => FeeKind::Commitment,
                    CFKind::UsageFee => FeeKind::Usage,
                    _ => FeeKind::Fixed,
                };
                bucket.fees.push(FeeComponent {
                    projected: amount,
                    basis: accrual_basis(cf),
                    accrual: cf.accrual_factor,
                    rate: cf.rate,
                    kind,
                });
            }
            CFKind::Notional
            | CFKind::Amortization
            | CFKind::PrePayment
            | CFKind::RevolvingDraw
            | CFKind::RevolvingRepayment => {
                if amount < 0.0 {
                    bucket.draw += -amount;
                } else {
                    bucket.repayment += amount;
                }
            }
            CFKind::Pik => bucket.pik += amount,
            _ => {}
        }
    }
    periods
}

fn fixed_coupon_of(spec: &CashflowSpec) -> Result<Option<f64>> {
    let decimal = match spec {
        CashflowSpec::Fixed(fixed) => Some(fixed.rate),
        CashflowSpec::StepUp(step_up) => Some(step_up.initial_rate),
        CashflowSpec::Floating(_) => None,
        CashflowSpec::Amortizing { base, .. } => return fixed_coupon_of(base),
    };
    decimal
        .map(|d| {
            d.to_f64().ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "coupon {d} cannot be represented as f64"
                ))
            })
        })
        .transpose()
}

fn floating_params_of(spec: &CashflowSpec) -> Result<Option<FloatingRateParams>> {
    match spec {
        CashflowSpec::Floating(floating) => {
            Ok(Some(FloatingRateParams::try_from(&floating.rate_spec)?))
        }
        CashflowSpec::Amortizing { base, .. } => floating_params_of(base),
        CashflowSpec::Fixed(_) | CashflowSpec::StepUp(_) => Ok(None),
    }
}
