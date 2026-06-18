//! Shared cashflow simulation engine for structured credit instruments.
//!
//! This module provides pure functions for running period-by-period
//! cashflow simulation through the waterfall engine. Deterministic and
//! stochastic pricing differ only in how they source pool SMM/MDR/recovery
//! assumptions for each legal payment period.

use crate::cashflow::traits::DatedFlows;
use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry;
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::default::{
    PerNameCopulaDefault, PoolGranularity,
};
use crate::instruments::fixed_income::structured_credit::types::{
    AssetPool, PoolState, RecipientType, StructuredCredit, TrancheCashflows, TrancheSeniority,
    TrancheStructure, Waterfall, WaterfallDistribution,
};
use crate::instruments::fixed_income::structured_credit::utils::simulation::RecoveryQueue;
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::CalendarRegistry;
use finstack_quant_core::dates::HolidayCalendar;
use finstack_quant_core::dates::{
    adjust, BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, ScheduleBuilder,
    StubKind,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_monte_carlo::traits::RandomStream;
use std::sync::Arc;

// ============================================================================
// POOL FLOW SOURCES
// ============================================================================

/// Source of pool-level prepayment/default/recovery assumptions for each period.
pub(crate) trait PoolFlowSource {
    /// Calculate pool cashflows for the next legal payment period.
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows>;
}

/// Inputs required to calculate pool flows for one legal payment period.
pub(crate) struct PoolFlowRequest<'a, 's> {
    state: &'a mut SimulationState<'s>,
    instrument: &'a StructuredCredit,
    pay_date: Date,
    prev_date: Date,
    seasoning_months: u32,
    months_per_period: f64,
    context: &'a MarketContext,
}

/// Deterministic pool-flow source using the instrument's base credit model.
pub(crate) struct DeterministicPoolFlowSource;

impl PoolFlowSource for DeterministicPoolFlowSource {
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows> {
        let smm = request
            .instrument
            .calculate_prepayment_rate(request.pay_date, request.seasoning_months)?;
        let mdr = request
            .instrument
            .calculate_default_rate(request.pay_date, request.seasoning_months)?;
        calculate_pool_flows_with_rates(RatedPoolFlowRequest {
            state: request.state,
            pay_date: request.pay_date,
            prev_date: request.prev_date,
            months_per_period: request.months_per_period,
            context: request.context,
            rates: PoolFlowRates {
                smm,
                mdr,
                recovery_rate: request.instrument.credit_model.recovery_spec.rate,
            },
            copula_outcome: None,
        })
    }
}

/// Per-period systematic inputs for finite-pool per-name copula default
/// simulation.
///
/// When present on a [`PeriodPoolShock`], the engine realizes each pool
/// asset's default individually (latent variable `Aᵢ = √ρ·Z + √(1−ρ)·εᵢ`)
/// instead of applying the pool-wide MDR uniformly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PerNamePeriodInput {
    /// Systematic factor `Z` for the payment period, shared by every name.
    pub(crate) systematic_z: f64,
    /// Per-name *unconditional* marginal default probability for the period.
    /// Homogeneous pools share one value; the threshold `Φ⁻¹(PDₜ)` is
    /// recomputed per name to support heterogeneous pools.
    pub(crate) marginal_pd: f64,
}

/// Aggregated scenario assumptions for a legal payment period.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PeriodPoolShock {
    /// Equivalent monthly SMM for the payment period.
    pub(crate) smm: f64,
    /// Equivalent monthly MDR for the payment period.
    ///
    /// Used as the pool-wide default rate when `per_name` is `None` (the LHP
    /// fast-path), and ignored for assets when per-name simulation is active.
    pub(crate) mdr: f64,
    /// Recovery rate applied to defaults in the payment period.
    pub(crate) recovery_rate: f64,
    /// Per-name copula inputs. `Some` ⇒ realize defaults name-by-name;
    /// `None` ⇒ apply the pool-wide LHP MDR.
    pub(crate) per_name: Option<PerNamePeriodInput>,
}

impl PeriodPoolShock {
    /// Construct a pool-wide (LHP / non-copula) shock with no per-name plan.
    pub(crate) fn pool_wide(smm: f64, mdr: f64, recovery_rate: f64) -> Self {
        Self {
            smm,
            mdr,
            recovery_rate,
            per_name: None,
        }
    }
}

/// Per-path per-name copula default engine carried by a scenario flow source.
///
/// Owns the path's idiosyncratic-draw RNG substream so that per-name `εᵢ`
/// draws are deterministic and order-stable (period → name index). The
/// `simulator` is shared (cheap `Arc` clone of the copula kernel) across
/// paths; only the RNG is per-path.
///
/// # Antithetic pairing
///
/// When `antithetic` is `true` this engine is the *second member* of an
/// antithetic pair: it shares its RNG substream with the first member and
/// **negates** every idiosyncratic `εᵢ` draw. Combined with the systematic
/// factor `Z` being negated by `monte_carlo_factor_sets`, the copula latent
/// variable `Aᵢ = √ρ·Z + √(1−ρ)·εᵢ` becomes `−Aᵢ` for the paired path — the
/// genuine antithetic variate. Without this the per-name idiosyncratic
/// channel of paired paths would be independent, defeating the variance
/// reduction and making the reported confidence interval too narrow.
///
/// The Student-t mixing variable `W` is drawn from the same shared substream
/// and is *not* negated: the χ²-based mixing is asymmetric, and standard
/// antithetic treatment for the Student-t copula negates only the Gaussian
/// components while keeping the mixing common to the pair.
pub(crate) struct PerNameDefaultEngine {
    simulator: Arc<PerNameCopulaDefault>,
    granularity: PoolGranularity,
    rng: PhiloxRng,
    /// `true` ⇒ second member of an antithetic pair; negate idiosyncratic draws.
    antithetic: bool,
    /// Idiosyncratic (name-specific) recovery volatility. When `> 0`, each
    /// defaulted name recovers at its own rate scattered around the period
    /// systematic recovery; `0` ⇒ every default recovers at the period rate
    /// (no per-name dispersion, e.g. constant recovery).
    idiosyncratic_recovery_vol: f64,
}

impl PerNameDefaultEngine {
    /// Create a per-name engine for one scenario path (independent draws).
    pub(crate) fn new(
        simulator: Arc<PerNameCopulaDefault>,
        granularity: PoolGranularity,
        rng: PhiloxRng,
        idiosyncratic_recovery_vol: f64,
    ) -> Self {
        Self {
            simulator,
            granularity,
            rng,
            antithetic: false,
            idiosyncratic_recovery_vol,
        }
    }

    /// Create the *antithetic partner* per-name engine for a scenario path.
    ///
    /// `rng` must be the SAME substream the paired path uses; this engine
    /// negates every idiosyncratic `εᵢ` draw so the copula latent variable is
    /// the antithetic variate of its partner.
    pub(crate) fn new_antithetic(
        simulator: Arc<PerNameCopulaDefault>,
        granularity: PoolGranularity,
        rng: PhiloxRng,
        idiosyncratic_recovery_vol: f64,
    ) -> Self {
        Self {
            simulator,
            granularity,
            rng,
            antithetic: true,
            idiosyncratic_recovery_vol,
        }
    }
}

/// Stochastic path pool-flow source using pre-generated period shocks.
pub(crate) struct StochasticPathFlowSource {
    shocks: Vec<PeriodPoolShock>,
    next_period: usize,
    /// Per-name copula engine. `Some` when the scenario uses finite-pool
    /// per-name default simulation.
    per_name: Option<PerNameDefaultEngine>,
    /// Scratch buffer for per-name default indicators, reused each period to
    /// avoid per-period allocation.
    default_scratch: Vec<bool>,
    /// Scratch buffer for per-name recovery rates, index-aligned with
    /// `default_scratch`. Entry `k` is the recovery the `k`-th performing
    /// asset realizes if it defaults this period.
    recovery_scratch: Vec<f64>,
}

impl StochasticPathFlowSource {
    /// Create a flow source for one scenario path (pool-wide / LHP shocks).
    pub(crate) fn new(shocks: Vec<PeriodPoolShock>) -> Self {
        Self {
            shocks,
            next_period: 0,
            per_name: None,
            default_scratch: Vec::new(),
            recovery_scratch: Vec::new(),
        }
    }

    /// Create a flow source that realizes per-name copula defaults.
    pub(crate) fn with_per_name(
        shocks: Vec<PeriodPoolShock>,
        per_name: PerNameDefaultEngine,
    ) -> Self {
        Self {
            shocks,
            next_period: 0,
            per_name: Some(per_name),
            default_scratch: Vec::new(),
            recovery_scratch: Vec::new(),
        }
    }
}

impl PoolFlowSource for StochasticPathFlowSource {
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows> {
        let shock = self.shocks.get(self.next_period).copied().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "stochastic path has no pool shock for payment period {}",
                self.next_period + 1
            ))
        })?;
        self.next_period += 1;

        // Copula default resolution: when the per-name engine and the
        // period's per-name plan are both present, the copula owns the
        // period's default rate. `PerName` granularity realizes each asset
        // individually (latent variable `Aᵢ`); `LargeHomogeneous` applies the
        // closed-form LHP conditional default probability uniformly — the
        // `N → ∞` limit of the per-name model.
        let copula_outcome = match (self.per_name.as_mut(), shock.per_name) {
            (Some(engine), Some(plan)) => match engine.granularity {
                PoolGranularity::PerName => {
                    // One marginal-PD entry per still-performing asset, in
                    // the pool's intrinsic asset order, so the per-name εᵢ
                    // draws are order-stable.
                    let alive = request
                        .state
                        .pool_state
                        .is_defaulted
                        .iter()
                        .zip(request.state.pool_state.balances.iter())
                        .filter(|(defaulted, balance)| !**defaulted && **balance > 0.0)
                        .count();
                    let marginal = vec![plan.marginal_pd; alive];
                    // Antithetic partners negate their idiosyncratic εᵢ draws
                    // so the copula latent variable is the antithetic variate
                    // of the paired path (the systematic Z is already negated
                    // by `monte_carlo_factor_sets`).
                    if engine.antithetic {
                        engine.simulator.simulate_period_antithetic(
                            plan.systematic_z,
                            &marginal,
                            &mut engine.rng,
                            &mut self.default_scratch,
                        );
                    } else {
                        engine.simulator.simulate_period(
                            plan.systematic_z,
                            &marginal,
                            &mut engine.rng,
                            &mut self.default_scratch,
                        );
                    }

                    // Per-name idiosyncratic recovery dispersion: each name
                    // recovers at its own rate, scattered around the period
                    // systematic recovery `shock.recovery_rate`. A draw is
                    // taken for every name (not only defaulters) so the RNG
                    // stream stays order-stable; the antithetic partner negates
                    // the recovery shock, mirroring the default-shock negation.
                    // When the recovery model has no idiosyncratic volatility
                    // no draw is consumed, so a constant-recovery scenario is
                    // bit-identical to the pre-dispersion engine.
                    self.recovery_scratch.clear();
                    self.recovery_scratch.reserve(self.default_scratch.len());
                    let sigma = engine.idiosyncratic_recovery_vol;
                    for _ in 0..self.default_scratch.len() {
                        let recovery = if sigma > 0.0 {
                            let raw = engine.rng.next_std_normal();
                            let eps = if engine.antithetic { -raw } else { raw };
                            (shock.recovery_rate + sigma * eps).clamp(0.0, 1.0)
                        } else {
                            shock.recovery_rate
                        };
                        self.recovery_scratch.push(recovery);
                    }

                    Some(PeriodDefaultOutcome::PerName {
                        defaults: &self.default_scratch,
                        recoveries: &self.recovery_scratch,
                    })
                }
                PoolGranularity::LargeHomogeneous => {
                    // Closed-form LHP limit: apply E[1{Aᵢ ≤ c} | Z, W] to the
                    // whole pool as a period-level default rate. The simulator
                    // draws the same shared mixing `W` per period as the
                    // per-name path, so this is the genuine `N → ∞` limit.
                    let rate = engine.simulator.conditional_default_prob(
                        plan.systematic_z,
                        plan.marginal_pd,
                        &mut engine.rng,
                    );
                    Some(PeriodDefaultOutcome::PoolWidePeriodRate(rate))
                }
            },
            _ => None,
        };

        calculate_pool_flows_with_rates(RatedPoolFlowRequest {
            state: request.state,
            pay_date: request.pay_date,
            prev_date: request.prev_date,
            months_per_period: request.months_per_period,
            context: request.context,
            rates: PoolFlowRates {
                smm: shock.smm,
                mdr: shock.mdr,
                recovery_rate: shock.recovery_rate,
            },
            copula_outcome,
        })
    }
}

// ============================================================================
// PUBLIC API
// ============================================================================

/// Release the unused spread-account balance to equity at deal end.
///
/// If the deal configures `excess_spread` and the account holds a positive
/// balance after the final period, that balance is distributed to the equity
/// tranche as a residual principal flow — unless a cumulative-loss trap trigger
/// (`trap_loss_pct`) is breached, in which case the balance is retained in the
/// deal (consumed enhancement) and permanently reduces equity. No-op when no
/// `excess_spread` rule is configured (identity).
fn release_spread_account(
    state: &mut SimulationState<'_>,
    instrument: &StructuredCredit,
) -> Result<()> {
    let Some(es) = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.excess_spread.as_ref())
    else {
        return Ok(());
    };

    let balance = state.spread_account;
    if balance.amount() <= 0.0 {
        return Ok(());
    }

    // Retain (do not release) if the cumulative-loss trap trigger is breached.
    if let Some(trap) = es.trap_loss_pct {
        let denom = state.total_pool_balance.amount();
        let loss_fraction = if denom > 0.0 {
            state.cumulative_expected_loss / denom
        } else {
            0.0
        };
        if loss_fraction >= trap {
            state.spread_account = Money::new(0.0, state.base_ccy);
            return Ok(());
        }
    }

    // Release to the equity tranche as a residual principal flow.
    let equity_id = state
        .tranches
        .tranches
        .iter()
        .find(|t| t.seniority == TrancheSeniority::Equity)
        .map(|t| t.id.as_str().to_string());
    if let Some(eq_id) = equity_id {
        let release_date = state.prev_date.unwrap_or(state.closing_date);
        if let Some(res) = state.results.get_mut(&eq_id) {
            res.cashflows.push((release_date, balance));
            res.principal_flows.push((release_date, balance));
            res.total_principal = res.total_principal.checked_add(balance)?;
        }
    }
    state.spread_account = Money::new(0.0, state.base_ccy);
    Ok(())
}

/// Run full cashflow simulation for a structured credit instrument.
///
/// Returns detailed cashflow results for each tranche.
pub(crate) fn run_simulation_with_source<S: PoolFlowSource + ?Sized>(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
    source: &mut S,
) -> Result<HashMap<String, TrancheCashflows>> {
    let pool = &instrument.pool;
    let tranches = &instrument.tranches;

    if pool.total_balance()?.amount() <= 0.0 {
        return Ok(HashMap::default());
    }

    // Validate and extract months per period
    let months_per_period = match instrument.frequency.months() {
        Some(m) => m as f64,
        None => {
            return Err(finstack_quant_core::Error::Validation(
                "Structured credit instruments require month-based payment frequencies".to_string(),
            ));
        }
    };

    // Initialize simulation state
    let mut state = SimulationState::new(
        pool,
        tranches,
        instrument.closing_date,
        instrument.credit_model.recovery_spec.recovery_lag,
    )?;

    // Create the base waterfall, then layer on any declarative rules (e.g.
    // available-funds caps). With no rules this is the identity. The AFC cap
    // rate is the collateral weighted-average coupon, effectively constant over
    // the deal's life, so resolution runs once here rather than per period.
    let base_waterfall = instrument.create_waterfall();
    let waterfall =
        crate::instruments::fixed_income::structured_credit::pricing::resolve::resolve_waterfall(
            &base_waterfall,
            instrument.waterfall_rules.as_ref(),
            instrument.pool.weighted_avg_coupon(),
        );

    // Resolve payment calendar - required for structured credit deals.
    // Silent fallback to weekends-only would shift coupons around holidays,
    // breaking WAC/WAL and OC tests.
    let calendar: &dyn HolidayCalendar = match instrument.payment_calendar_id.as_deref() {
        Some(cal_id) => CalendarRegistry::global()
            .resolve_str(cal_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: format!(
                        "payment_calendar_id:{} (available: {})",
                        cal_id,
                        CalendarRegistry::global().available_ids().join(", ")
                    ),
                })
            })?,
        None => {
            return Err(finstack_quant_core::Error::Validation(
                "Structured credit instruments require a payment_calendar_id for accurate \
                     schedule generation. Specify a valid calendar ID (e.g., 'nyse', 'target2') \
                     to ensure payment dates are adjusted correctly for business days."
                    .to_string(),
            ));
        }
    };

    let convention = instrument
        .payment_bdc
        .unwrap_or(BusinessDayConvention::ModifiedFollowing);

    // Generate the full contractual payment schedule, then filter future dates.
    // Re-anchoring the schedule at `as_of` would shift legal coupon dates for
    // seasoned deals valued between payment dates.
    let schedule = ScheduleBuilder::new(instrument.first_payment_date, instrument.maturity)?
        .frequency(instrument.frequency)
        .stub_rule(StubKind::ShortBack)
        .build()?;

    let mut adjusted_schedule = schedule;
    for date in &mut adjusted_schedule.dates {
        *date = adjust(*date, convention, calendar)?;
    }
    let schedule_dates: Vec<Date> = adjusted_schedule
        .dates
        .into_iter()
        .filter(|date| *date > as_of)
        .collect();

    // Simulate period-by-period
    for pay_date in schedule_dates {
        if state.is_pool_exhausted() {
            break;
        }

        // Clean-up call: if pool factor drops below threshold, redeem tranches.
        //
        // INTEX/Bloomberg convention: when pool factor (current / original total
        // balance) drops below the cleanup threshold (typically 10%), the equity
        // holder may exercise an optional redemption. Redemption pays tranches
        // in seniority order (senior first), bounded by the remaining pool value.
        //
        // A real cleanup-call redemption settles each note at its *full claim*:
        // par balance + accrued interest for the stub period + any deferred
        // (PIK) interest carried forward (+ an optional call premium). Booking
        // the whole amount as principal — as the engine previously did —
        // understates the holder's interest income and overstates principal
        // repayment. The interest portion is recorded as an interest flow and
        // does NOT retire notional; only the principal portion does.
        if let Some(cleanup_threshold) = instrument.cleanup_call_pct {
            let pool_factor = if state.total_pool_balance.amount() > 0.0 {
                state.pool_outstanding.amount() / state.total_pool_balance.amount()
            } else {
                0.0
            };
            if pool_factor < cleanup_threshold && pool_factor > 0.0 {
                // Available cash for redemption = remaining pool outstanding
                // PLUS any pending recoveries in the lag queue.
                //
                // `pool_outstanding` has already been debited by gross defaults
                // each period (line ~2032), so it does NOT include defaulted
                // notional. `recovery_queue.pending_amount()` holds future cash
                // *inflows* — lagged recovery proceeds on already-defaulted
                // collateral that have not yet matured. At the cleanup call the
                // equity holder purchases the remaining pool, realising these
                // recoveries immediately. They are therefore ADDITIVE to the
                // cash available to redeem the notes, not deductive.
                //
                // The previous code subtracted them (using the misleading name
                // `pending_losses`), understating available cash, under-paying
                // senior tranches, and leaving recovery value stranded.
                let pending_recoveries = state.recovery_queue.pending_amount(state.base_ccy);
                // The cleanup call realizes the pending recoveries immediately
                // (they fund the redemption below); drain the queue so the
                // end-of-simulation drain cannot release them a second time.
                let _ = state.recovery_queue.drain_pending();
                let mut available_for_redemption =
                    state.pool_outstanding.amount() + pending_recoveries.amount();

                // Stub-period start for accrued-interest calculation: the last
                // payment date (or closing) up to this cleanup-call date.
                let cleanup_period_start = state.prev_date.unwrap_or(state.closing_date);

                // Pay tranches in seniority order (Senior=0 first, Equity=3 last)
                let mut redemption_order: Vec<usize> = (0..state.tranches.tranches.len()).collect();
                redemption_order.sort_by_key(|&i| state.tranches.tranches[i].seniority);

                for &idx in &redemption_order {
                    if available_for_redemption <= WRITEDOWN_DE_MINIMIS {
                        break;
                    }
                    let tranche = &state.tranches.tranches[idx];
                    let tranche_id_str = tranche.id.as_str();
                    let balance = state
                        .tranche_balances
                        .get(tranche_id_str)
                        .copied()
                        .unwrap_or(Money::new(0.0, state.base_ccy));

                    if balance.amount() <= WRITEDOWN_DE_MINIMIS {
                        continue;
                    }

                    // Accrued interest for the stub period on the current
                    // (post-writedown) balance, using the tranche coupon and
                    // its own day-count convention.
                    let coupon_rate = tranche
                        .coupon
                        .try_current_rate_with_index(pay_date, context)?;
                    let accrual_factor = tranche.day_count.year_fraction(
                        cleanup_period_start,
                        pay_date,
                        DayCountContext::default(),
                    )?;
                    let accrued = balance.amount() * coupon_rate * accrual_factor;

                    // Deferred / PIK interest carried forward into this period.
                    let deferred = state
                        .deferred_interest
                        .get(tranche_id_str)
                        .map(|m| m.amount())
                        .unwrap_or(0.0);

                    // Optional call premium (currently always par-flat; routed
                    // through one helper so a future premium is a one-line
                    // change without touching the StructuredCredit type).
                    let premium = cleanup_call_premium(instrument, balance.amount());

                    // Full redemption claim, bounded by available cash.
                    let total_claim = balance.amount() + accrued + deferred + premium;
                    let redemption_amt = total_claim.min(available_for_redemption);
                    available_for_redemption -= redemption_amt;

                    // Split the bounded redemption: interest (accrued +
                    // deferred + premium) is satisfied first as the senior
                    // claim within the redemption, the remainder retires
                    // principal. Only the principal portion reduces notional.
                    let interest_claim = accrued + deferred + premium;
                    let interest_paid = redemption_amt.min(interest_claim).max(0.0);
                    let principal_paid = (redemption_amt - interest_paid).max(0.0);

                    let redemption = Money::new(redemption_amt, state.base_ccy);
                    let interest_money = Money::new(interest_paid, state.base_ccy);
                    let principal_money = Money::new(principal_paid, state.base_ccy);

                    if let Some(res) = state.results.get_mut(tranche_id_str) {
                        res.cashflows.push((pay_date, redemption));
                        if interest_paid > 0.0 {
                            res.interest_flows.push((pay_date, interest_money));
                            res.total_interest = res.total_interest.checked_add(interest_money)?;
                        }
                        if principal_paid > 0.0 {
                            res.principal_flows.push((pay_date, principal_money));
                            res.total_principal =
                                res.total_principal.checked_add(principal_money)?;
                        }
                    }
                    // Deferred interest cured by this redemption is cleared.
                    if let Some(def) = state.deferred_interest.get_mut(tranche_id_str) {
                        let cured = interest_paid.min(deferred).max(0.0);
                        *def = def
                            .checked_sub(Money::new(cured, state.base_ccy))
                            .unwrap_or(Money::new(0.0, state.base_ccy));
                    }
                    // Only the principal portion retires notional.
                    if let Some(bal) = state.tranche_balances.get_mut(tranche_id_str) {
                        *bal = bal
                            .checked_sub(principal_money)
                            .unwrap_or(Money::new(0.0, state.base_ccy));
                    }
                }
                break; // Terminate simulation after cleanup call
            }
        }

        simulate_period(
            &mut state,
            instrument,
            &waterfall,
            pay_date,
            context,
            months_per_period,
            source,
        )?;
    }

    // Drain lagged recoveries still pending at simulation end. When the
    // simulation terminates via pool exhaustion or the final scheduled date,
    // recoveries from defaults within `recovery_lag` months of the end have
    // not yet matured out of the queue; without this drain that recovery cash
    // is silently dropped and losses are overstated for any deal with
    // defaults near maturity. Mirrors the cleanup-call branch, which already
    // realizes the pending queue when it terminates the deal. Each entry is
    // released at the later of the final simulated date and its own
    // default-date + recovery lag, business-day adjusted.
    drain_pending_recoveries_at_end(&mut state, calendar, convention)?;

    // Release any unused spread-account balance to equity at deal end (unless a
    // cumulative-loss trap trigger retains it as consumed enhancement).
    release_spread_account(&mut state, instrument)?;

    Ok(state.finalize())
}

/// Drain and distribute recoveries still pending in the lag queue when the
/// simulation terminates (pool exhaustion or final scheduled payment date).
///
/// Each pending entry is released on the later of the last simulated payment
/// date and its natural maturity (`default_date + recovery_lag`), adjusted to
/// a business day, and distributed through the same terminal path the
/// cleanup-call branch uses: tranches in seniority order, each tranche's
/// claim being its deferred interest (senior portion) plus its remaining
/// principal balance. No new coupon accrues after the final simulated date,
/// so unlike the mid-life cleanup call there is no stub accrued-interest leg.
fn drain_pending_recoveries_at_end(
    state: &mut SimulationState,
    calendar: &dyn HolidayCalendar,
    convention: BusinessDayConvention,
) -> Result<()> {
    let pending = state.recovery_queue.drain_pending();
    if pending.is_empty() {
        return Ok(());
    }

    let last_date = state.prev_date.unwrap_or(state.closing_date);
    let lag_months = i32::try_from(state.recovery_lag_months).unwrap_or(i32::MAX);

    // Seniority order: Senior=0 first, Equity=3 last (same as cleanup call).
    let mut order: Vec<usize> = (0..state.tranches.tranches.len()).collect();
    order.sort_by_key(|&i| state.tranches.tranches[i].seniority);

    for (default_date, amount) in pending {
        let natural_release = default_date.add_months(lag_months);
        let release_date = adjust(natural_release.max(last_date), convention, calendar)?;

        let mut available = amount.amount();
        for &idx in &order {
            if available <= WRITEDOWN_DE_MINIMIS {
                break;
            }
            let tranche_id_str = state.tranches.tranches[idx].id.as_str();
            let balance = state
                .tranche_balances
                .get(tranche_id_str)
                .map(|m| m.amount())
                .unwrap_or(0.0);
            let deferred = state
                .deferred_interest
                .get(tranche_id_str)
                .map(|m| m.amount())
                .unwrap_or(0.0);
            let claim = balance + deferred;
            if claim <= WRITEDOWN_DE_MINIMIS {
                continue;
            }

            let paid = claim.min(available);
            available -= paid;

            // Deferred interest is the senior claim within the distribution;
            // the remainder retires principal (same split as the cleanup
            // call's terminal redemption).
            let interest_paid = paid.min(deferred).max(0.0);
            let principal_paid = (paid - interest_paid).max(0.0);

            let payment = Money::new(paid, state.base_ccy);
            let interest_money = Money::new(interest_paid, state.base_ccy);
            let principal_money = Money::new(principal_paid, state.base_ccy);

            if let Some(res) = state.results.get_mut(tranche_id_str) {
                res.cashflows.push((release_date, payment));
                if interest_paid > 0.0 {
                    res.interest_flows.push((release_date, interest_money));
                    res.total_interest = res.total_interest.checked_add(interest_money)?;
                }
                if principal_paid > 0.0 {
                    res.principal_flows.push((release_date, principal_money));
                    res.total_principal = res.total_principal.checked_add(principal_money)?;
                }
            }
            if interest_paid > 0.0 {
                if let Some(def) = state.deferred_interest.get_mut(tranche_id_str) {
                    *def = def
                        .checked_sub(interest_money)
                        .unwrap_or(Money::new(0.0, state.base_ccy));
                }
            }
            if principal_paid > 0.0 {
                if let Some(bal) = state.tranche_balances.get_mut(tranche_id_str) {
                    *bal = bal
                        .checked_sub(principal_money)
                        .unwrap_or(Money::new(0.0, state.base_ccy));
                }
            }
        }

        // Any remainder beyond all note claims is the equity holder's
        // residual — book it to the equity tranche (mirroring the standard
        // waterfall's Equity residual recipient and step-5's booking of
        // residual cash as a principal flow), so recovery cash is never
        // silently dropped.
        if available > WRITEDOWN_DE_MINIMIS {
            let equity_idx = order
                .iter()
                .rev()
                .copied()
                .find(|&i| state.tranches.tranches[i].seniority == TrancheSeniority::Equity);
            if let Some(idx) = equity_idx {
                let tranche_id_str = state.tranches.tranches[idx].id.as_str();
                let residual = Money::new(available, state.base_ccy);
                if let Some(res) = state.results.get_mut(tranche_id_str) {
                    res.cashflows.push((release_date, residual));
                    res.principal_flows.push((release_date, residual));
                    res.total_principal = res.total_principal.checked_add(residual)?;
                }
            }
        }
    }

    Ok(())
}

/// Aggregate tranche-level cashflows into one dated cashflow vector.
pub(crate) fn aggregate_tranche_cashflows(
    full_results: &HashMap<String, TrancheCashflows>,
) -> Result<DatedFlows> {
    // Aggregate all tranche cashflows into a single schedule
    let estimated_dates = full_results
        .values()
        .next()
        .map(|r| r.cashflows.len())
        .unwrap_or(0);
    let mut flow_map: HashMap<Date, Money> = {
        let mut m = HashMap::default();
        m.reserve(estimated_dates);
        m
    };

    for result in full_results.values() {
        for (date, amount) in &result.cashflows {
            if let Some(existing) = flow_map.get_mut(date) {
                *existing = existing.checked_add(*amount)?;
            } else {
                flow_map.insert(*date, *amount);
            }
        }
    }

    let mut all_flows: DatedFlows = flow_map.into_iter().collect();
    all_flows.sort_by_key(|(d, _)| *d);

    Ok(all_flows)
}

/// Remove one tranche's cashflows from a full simulation result map.
pub(crate) fn take_tranche_cashflows(
    full_results: &mut HashMap<String, TrancheCashflows>,
    tranche_id: &str,
) -> Result<TrancheCashflows> {
    full_results.remove(tranche_id).ok_or_else(|| {
        finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
            id: format!("tranche:{}", tranche_id),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::{
        AssetPool, DealType, DefaultModelSpec, PoolAsset, PrepaymentModelSpec, RecoveryModelSpec,
        Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_core::currency::Currency;
    use time::Month;

    fn empty_tranche_cashflows(id: &str, currency: Currency) -> TrancheCashflows {
        TrancheCashflows {
            tranche_id: id.to_string(),
            cashflows: Vec::new(),
            detailed_flows: Vec::new(),
            interest_flows: Vec::new(),
            principal_flows: Vec::new(),
            pik_flows: Vec::new(),
            writedown_flows: Vec::new(),
            final_balance: Money::new(0.0, currency),
            total_interest: Money::new(0.0, currency),
            total_principal: Money::new(0.0, currency),
            total_pik: Money::new(0.0, currency),
            total_writedown: Money::new(0.0, currency),
        }
    }

    #[test]
    fn aggregate_tranche_cashflows_errors_on_incompatible_same_date_amounts() {
        let pay_date = Date::from_calendar_date(2026, Month::January, 15).expect("valid date");
        let mut usd = empty_tranche_cashflows("AAA", Currency::USD);
        usd.cashflows
            .push((pay_date, Money::new(100.0, Currency::USD)));

        let mut eur = empty_tranche_cashflows("BBB", Currency::EUR);
        eur.cashflows
            .push((pay_date, Money::new(50.0, Currency::EUR)));

        let mut results = HashMap::default();
        results.insert(usd.tranche_id.clone(), usd);
        results.insert(eur.tranche_id.clone(), eur);

        let err = aggregate_tranche_cashflows(&results)
            .expect_err("currency mismatch should not be silently dropped");

        assert!(matches!(
            err,
            finstack_quant_core::Error::CurrencyMismatch { .. }
        ));
    }

    // ── W-25: cleanup-call redemption must include accrued interest ──────

    fn cleanup_test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    fn cleanup_discount_curve() -> finstack_quant_core::market_data::term_structures::DiscountCurve
    {
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(cleanup_test_date())
            .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.88)])
            .build()
            .expect("curve")
    }

    /// A single-tranche ABS with a high CPR so the pool amortizes below the
    /// 10% cleanup threshold partway through its life, triggering an optional
    /// cleanup-call redemption mid-period.
    fn cleanup_call_deal() -> StructuredCredit {
        let maturity = Date::from_calendar_date(2029, Month::January, 1).expect("valid date");
        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(10_000_000.0, Currency::USD),
            0.06,
            maturity,
            DayCount::Thirty360,
        ));
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::new(10_000_000.0, Currency::USD),
            // Non-zero coupon so the stub-period accrued interest is non-zero.
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let mut instrument = StructuredCredit::new_abs(
            "ABS-CLEANUP",
            pool,
            TrancheStructure::new(vec![tranche]).expect("structure"),
            cleanup_test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse")
        .with_cleanup_call(0.10)
        .expect("cleanup call");
        // 60% CPR: the pool factor crosses below 10% within ~4 years, well
        // before maturity, so a cleanup call genuinely fires mid-life.
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.60);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument
    }

    /// W-25 — a cleanup-call redemption must settle the note at par PLUS
    /// accrued interest for the stub period, not at par balance only.
    ///
    /// The pre-fix code booked the entire redemption as principal and recorded
    /// no interest flow on the cleanup date. This test runs a deal that hits
    /// its cleanup call mid-period and asserts (a) an interest flow IS recorded
    /// on the final (cleanup) date, and (b) the total cashflow on that date
    /// strictly exceeds the principal retired — the difference being accrued
    /// interest.
    #[test]
    fn cleanup_call_redemption_includes_accrued_interest() {
        let instrument = cleanup_call_deal();
        let market = MarketContext::new().insert(cleanup_discount_curve());

        let results = run_simulation_with_source(
            &instrument,
            &market,
            cleanup_test_date(),
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation");

        let tranche = results.get("A").expect("tranche A result");

        // The cleanup call terminates the simulation: the last cashflow date
        // is the cleanup-call date. Identify it.
        let cleanup_date = tranche
            .cashflows
            .iter()
            .map(|(d, _)| *d)
            .max()
            .expect("at least one cashflow");

        // Principal retired on the cleanup date.
        let principal_on_cleanup: f64 = tranche
            .principal_flows
            .iter()
            .filter(|(d, _)| *d == cleanup_date)
            .map(|(_, m)| m.amount())
            .sum();

        // Interest paid on the cleanup date — the defect: this was 0.0.
        let interest_on_cleanup: f64 = tranche
            .interest_flows
            .iter()
            .filter(|(d, _)| *d == cleanup_date)
            .map(|(_, m)| m.amount())
            .sum();

        // Total cash on the cleanup date.
        let total_on_cleanup: f64 = tranche
            .cashflows
            .iter()
            .filter(|(d, _)| *d == cleanup_date)
            .map(|(_, m)| m.amount())
            .sum();

        assert!(
            principal_on_cleanup > WRITEDOWN_DE_MINIMIS,
            "cleanup call should retire principal; got {principal_on_cleanup}"
        );
        assert!(
            interest_on_cleanup > WRITEDOWN_DE_MINIMIS,
            "cleanup-call redemption must record accrued interest on the \
             cleanup date; got {interest_on_cleanup} (pre-fix booked the whole \
             redemption as principal and recorded no interest)"
        );
        // Total redemption cash must exceed the par balance retired by exactly
        // the accrued-interest component.
        assert!(
            total_on_cleanup > principal_on_cleanup + WRITEDOWN_DE_MINIMIS,
            "total cleanup cashflow {total_on_cleanup} must exceed principal \
             {principal_on_cleanup} by the accrued interest"
        );
        assert!(
            (total_on_cleanup - principal_on_cleanup - interest_on_cleanup).abs()
                < WRITEDOWN_DE_MINIMIS,
            "cleanup cashflow must split exactly into principal + interest: \
             total={total_on_cleanup}, principal={principal_on_cleanup}, \
             interest={interest_on_cleanup}"
        );
    }

    // ── Cash-conservation fixes: items 1, 2, 13 ─────────────────────────

    use crate::instruments::fixed_income::structured_credit::types::{
        ReinvestmentCriteria, ReinvestmentPeriod,
    };

    fn cc_test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    fn cc_discount_curve() -> finstack_quant_core::market_data::term_structures::DiscountCurve {
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(cc_test_date())
            .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.88), (10.0, 0.75)])
            .build()
            .expect("curve")
    }

    /// Item 1 — reinvestment-period collateral principal must be recycled into
    /// new collateral (pool held flat net of defaults), NOT silently vanish.
    ///
    /// A CLO with a high CPR and an active reinvestment period. With the
    /// pre-fix engine the collected principal is neither distributed nor
    /// reinvested: the asset balances shrink, the reinvestment-end
    /// reconciliation snaps `pool_outstanding` down to the shrunken sum, and
    /// the cash is lost. The fix recycles it, so the pool keeps generating
    /// interest and the senior tranche collects materially more total cash.
    #[test]
    fn reinvestment_period_principal_is_recycled_not_lost() {
        let maturity = Date::from_calendar_date(2034, Month::January, 1).expect("valid date");
        let reinvest_end = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        // Build a CLO whose pool prepays fast (high CPR). During the 6y
        // reinvestment period, that prepaid principal must be recycled.
        let build_deal = |with_reinvestment: bool| {
            let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
            pool.assets.push(PoolAsset::fixed_rate_bond(
                "L1",
                Money::new(100_000_000.0, Currency::USD),
                0.07,
                maturity,
                DayCount::Thirty360,
            ));
            if with_reinvestment {
                pool.reinvestment_period = Some(ReinvestmentPeriod {
                    end_date: reinvest_end,
                    is_active: true,
                    criteria: ReinvestmentCriteria::default(),
                });
            }
            let tranche = Tranche::new(
                "A",
                0.0,
                100.0,
                TrancheSeniority::Senior,
                Money::new(100_000_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("tranche");
            let mut instrument = StructuredCredit::new_clo(
                "CLO-REINVEST",
                pool,
                TrancheStructure::new(vec![tranche]).expect("structure"),
                cc_test_date(),
                maturity,
                "USD-OIS",
            )
            .with_payment_calendar("nyse");
            instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
            instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
            instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
            instrument
        };

        let market = MarketContext::new().insert(cc_discount_curve());

        let with_reinvest = run_simulation_with_source(
            &build_deal(true),
            &market,
            cc_test_date(),
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation with reinvestment");

        let tranche = with_reinvest.get("A").expect("tranche A");

        // Total interest collected by the senior tranche over the deal life.
        let total_interest: f64 = tranche.interest_flows.iter().map(|(_, m)| m.amount()).sum();
        // Total principal returned.
        let total_principal: f64 = tranche
            .principal_flows
            .iter()
            .map(|(_, m)| m.amount())
            .sum();

        // With recycling, the $100M of notional is held flat through the 6y
        // reinvestment period, so it keeps throwing off 5% coupons. Interest
        // on a flat $100M for ~6y of reinvestment plus amortization after is
        // far above what a vanishing-principal pool would pay.
        // Pre-fix (principal lost): the senior tranche collects only a small
        // fraction of its principal back. Post-fix it must be repaid in full.
        assert!(
            total_principal > 99_000_000.0,
            "with reinvestment recycling the senior tranche must be repaid \
             nearly in full; got principal {total_principal:.0} (pre-fix the \
             reinvested principal vanishes and is never returned)"
        );
        assert!(
            total_interest > 25_000_000.0,
            "a $100M tranche held flat through a 6y reinvestment period at 5% \
             must collect substantial interest; got {total_interest:.0}"
        );
        // Final balance must be (near) zero — the tranche fully amortizes.
        assert!(
            tranche.final_balance.amount() < 1.0,
            "tranche must fully amortize by maturity; final balance {}",
            tranche.final_balance.amount()
        );
    }

    /// Item 2 — loss allocation must not double-count notional already retired
    /// by principal repayment. A tranche's notional is consumed by exactly two
    /// channels — principal repayment and write-down — and the sum can never
    /// exceed its original face.
    ///
    /// The pre-fix engine allocated *cumulative* expected loss against a cap
    /// of `original_balance` every period. A tranche written down early (when
    /// little principal had been repaid) keeps that write-down recorded, while
    /// the waterfall keeps paying it principal in later periods — so
    /// `principal_repaid + write-down` could exceed face. The fix allocates
    /// only this period's incremental net loss and caps each tranche's
    /// incremental write-down at its CURRENT balance.
    ///
    /// The loss-bearing tranche here is a **subordinated** tranche (paid via
    /// bounded `TranchePrincipal`, not the uncapped equity residual), so the
    /// `principal + write-down ≤ face` invariant is the genuine test.
    #[test]
    fn loss_allocation_caps_writedown_by_face_net_of_principal_repaid() {
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
        // High CPR pays the structure down fast; concurrent high CDR writes
        // losses against the (already-amortizing) subordinated tranche.
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "L1",
            Money::new(100_000_000.0, Currency::USD),
            0.07,
            maturity,
            DayCount::Thirty360,
        ));
        // Senior + subordinated + equity. The subordinated tranche absorbs
        // loss after equity and is paid principal via bounded TranchePrincipal.
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "A",
                0.0,
                70.0,
                TrancheSeniority::Senior,
                Money::new(70_000_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("senior"),
            Tranche::new(
                "B",
                70.0,
                92.0,
                TrancheSeniority::Subordinated,
                Money::new(22_000_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.07 },
                maturity,
            )
            .expect("subordinated"),
            Tranche::new(
                "EQ",
                92.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(8_000_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity,
            )
            .expect("equity"),
        ])
        .expect("structure");
        let mut instrument = StructuredCredit::new_clo(
            "CLO-LOSS-CAP",
            pool,
            tranches,
            cc_test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        // Heavy prepayment + heavy default running together.
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.40);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.30);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);

        let market = MarketContext::new().insert(cc_discount_curve());
        let results = run_simulation_with_source(
            &instrument,
            &market,
            cc_test_date(),
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation");

        // The core invariant for every bounded-principal tranche: principal
        // repaid + write-down must never exceed the original face. Pre-fix the
        // cumulative-loss cap ignored principal repaid, so this sum could
        // exceed face (double-counting retired notional as loss-absorbing).
        for (id, face) in [("A", 70_000_000.0_f64), ("B", 22_000_000.0_f64)] {
            let tranche = results.get(id).unwrap_or_else(|| panic!("tranche {id}"));
            let total_writedown = tranche.total_writedown.amount();
            let total_principal = tranche.total_principal.amount();
            assert!(
                total_principal + total_writedown <= face + WRITEDOWN_DE_MINIMIS,
                "{id}: principal repaid ({total_principal:.0}) + write-down \
                 ({total_writedown:.0}) = {:.0} must not exceed original face \
                 ({face:.0}); the pre-fix cap double-counts repaid face",
                total_principal + total_writedown
            );
            // A write-down can never exceed the tranche's own face.
            assert!(
                total_writedown <= face + WRITEDOWN_DE_MINIMIS,
                "{id}: write-down {total_writedown:.0} exceeds face {face:.0}"
            );
        }
    }

    /// Item 13 — per-period cash conservation: every dollar of pool collateral
    /// cashflow must be accounted for as a tranche cashflow, a reserve
    /// movement, or principal recycled during reinvestment. This drives the
    /// in-engine debug assertion on a representative deal and asserts the
    /// deal-level identity holds across the whole simulation.
    #[test]
    fn per_period_cash_conservation_holds_over_full_simulation() {
        let maturity = Date::from_calendar_date(2031, Month::January, 1).expect("valid date");

        let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "L1",
            Money::new(80_000_000.0, Currency::USD),
            0.07,
            maturity,
            DayCount::Thirty360,
        ));
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "L2",
            Money::new(20_000_000.0, Currency::USD),
            0.065,
            maturity,
            DayCount::Thirty360,
        ));
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "A",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                Money::new(80_000_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("senior"),
            Tranche::new(
                "B",
                80.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(20_000_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity,
            )
            .expect("equity"),
        ])
        .expect("structure");
        let mut instrument = StructuredCredit::new_clo(
            "CLO-CONSERVE",
            pool,
            tranches,
            cc_test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.10);
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.05);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 6);

        let market = MarketContext::new().insert(cc_discount_curve());
        // The per-period cash-conservation debug assertion fires inside
        // simulate_period; reaching `finalize` without a panic is the test.
        let results = run_simulation_with_source(
            &instrument,
            &market,
            cc_test_date(),
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation");

        // Deal-level sanity: total cash distributed to tranches is positive
        // and finite (the per-period assertion already proved conservation).
        let total_cash: f64 = results
            .values()
            .flat_map(|r| r.cashflows.iter())
            .map(|(_, m)| m.amount())
            .sum();
        assert!(
            total_cash.is_finite() && total_cash > 0.0,
            "deal must distribute positive finite cash; got {total_cash}"
        );
    }

    /// Item 6 — the contractual level payment must be FROZEN once computed.
    ///
    /// A level-pay loan's scheduled payment is fixed at origination;
    /// prepayments shorten the loan, they do not shrink the scheduled payment.
    /// White-box test: build a `SimulationState` with one amortizing mortgage,
    /// run period 1 (pure scheduled amortization), then simulate an external
    /// prepayment by shrinking the asset balance and run period 2. The frozen
    /// level payment must be unchanged, and period-2 scheduled principal must
    /// equal `frozen_LP − shrunk_balance·period_rate` — strictly LARGER than
    /// the pre-fix recomputed value (which scales the whole payment down with
    /// the balance).
    #[test]
    fn level_pay_scheduled_principal_uses_frozen_contractual_payment() {
        use crate::instruments::fixed_income::structured_credit::types::AssetType;
        use finstack_quant_core::types::InstrumentId;

        let closing = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let maturity = Date::from_calendar_date(2034, Month::January, 1).expect("date");
        let pay1 = Date::from_calendar_date(2024, Month::April, 1).expect("date");
        let pay2 = Date::from_calendar_date(2024, Month::July, 1).expect("date");

        let rate = 0.06_f64;
        let original_balance = 1_000_000.0_f64;

        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset {
            day_count: DayCount::Thirty360,
            id: InstrumentId::new("MTG1"),
            asset_type: AssetType::SingleFamilyMortgage { ltv: None },
            balance: Money::new(original_balance, Currency::USD),
            rate,
            spread_bps: None,
            index_id: None,
            maturity,
            credit_quality: None,
            industry: None,
            obligor_id: None,
            is_defaulted: false,
            recovery_amount: None,
            purchase_price: None,
            acquisition_date: None,
            smm_override: None,
            mdr_override: None,
        });
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::new(original_balance, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("structure");

        let market = MarketContext::new().insert(cc_discount_curve());
        let mut state = SimulationState::new(&pool, &tranches, closing, 0).expect("state");

        let months_per_period = 3.0_f64;
        let rates = PoolFlowRates {
            smm: 0.0,
            mdr: 0.0,
            recovery_rate: 0.0,
        };

        // ── Period 1: pure scheduled amortization (no prepay). ──────────
        let flows1 = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
            state: &mut state,
            pay_date: pay1,
            prev_date: closing,
            months_per_period,
            context: &market,
            rates,
            copula_outcome: None,
        })
        .expect("period 1 flows");
        let level_payment = state.pool_state.level_payments[0]
            .expect("level payment must be frozen after the first amortizing period");
        let sched1 = flows1.scheduled_principal.amount();
        assert!(
            sched1 > 0.0,
            "period 1 must amortize some scheduled principal"
        );

        // ── Simulate an external prepayment: halve the asset balance. ───
        let balance_after_prepay = state.pool_state.balances[0] * 0.5;
        state.pool_state.balances[0] = balance_after_prepay;

        // ── Period 2: the level payment must NOT be recomputed. ─────────
        let flows2 = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
            state: &mut state,
            pay_date: pay2,
            prev_date: pay1,
            months_per_period,
            context: &market,
            rates,
            copula_outcome: None,
        })
        .expect("period 2 flows");

        // The frozen level payment is unchanged.
        assert_eq!(
            state.pool_state.level_payments[0],
            Some(level_payment),
            "the contractual level payment must stay frozen across periods"
        );

        // Period-2 scheduled principal = frozen_LP − interest on the
        // (prepaid-down) balance. With the bug the level payment would be
        // recomputed off the halved balance, roughly halving it.
        //
        // Convention: the level-pay
        // annuity uses the NOMINAL periodic rate `rate × months/12` (US
        // mortgage convention, matching mbs_passthrough/pricer.rs), not the
        // effective-compounding `(1+rate)^(months/12) − 1` previously pinned.
        let period_rate = rate * months_per_period / 12.0;
        let expected_sched2 = level_payment - balance_after_prepay * period_rate;
        let sched2 = flows2.scheduled_principal.amount();
        assert!(
            (sched2 - expected_sched2).abs() < 1.0,
            "period-2 scheduled principal {sched2:.2} must equal frozen \
             level payment {level_payment:.2} minus interest on the \
             prepaid-down balance ({expected_sched2:.2})"
        );

        // The buggy recomputed payment would be ~half the frozen one, so the
        // buggy scheduled principal would be far below the correct value.
        let buggy_recomputed_lp = level_payment * 0.5; // LP ∝ balance
        let buggy_sched2 = (buggy_recomputed_lp - balance_after_prepay * period_rate).max(0.0);
        assert!(
            sched2 > buggy_sched2 + 1_000.0,
            "frozen-payment scheduled principal {sched2:.2} must materially \
             exceed the buggy recomputed-payment value {buggy_sched2:.2}"
        );
    }

    // ── W-26: cleanup-call redemption must INCLUDE pending recoveries ───────

    /// A deal that accumulates a large recovery queue before the cleanup call
    /// fires. This requires a HIGH CDR (many defaults) with a LONG recovery
    /// lag (24 months), and a cleanup threshold that is crossed purely by
    /// defaults (no CPR). With CDR ≈ 30% annual the pool drops to ~10% of
    /// original balance in ~77 months (~6.5 years), at which point the
    /// recovery queue holds roughly 3.6 M in pending inflows. The pool
    /// outstanding at cleanup is only ~1 M, so:
    ///
    ///   buggy  : available = 1M − 3.6M → clamped to 0 → tranche paid nothing
    ///   correct: available = 1M + 3.6M = 4.6M → tranche fully redeemed
    ///
    /// The tranche's remaining balance at cleanup is ~4.6 M (original − write-
    /// downs), so the correct code fully retires it while the buggy code leaves
    /// it unpaid (final_balance ≫ 0).
    fn cleanup_with_large_recovery_queue_deal() -> StructuredCredit {
        let start = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        // Long maturity so the deal runs to cleanup before expiring.
        let maturity = Date::from_calendar_date(2035, Month::January, 1).expect("valid date");
        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(10_000_000.0, Currency::USD),
            0.06,
            maturity,
            DayCount::Thirty360,
        ));
        // Single senior tranche; the equity shortfall is equity's problem.
        let senior = Tranche::new(
            "A",
            0.0,
            80.0, // 80% of original balance = 8 M face
            TrancheSeniority::Senior,
            Money::new(8_000_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("senior tranche");
        let equity = Tranche::new(
            "E",
            80.0,
            100.0, // junior 20% = 2 M face
            TrancheSeniority::Equity,
            Money::new(2_000_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("equity tranche");
        let mut instrument = StructuredCredit::new_abs(
            "ABS-BIG-RECOVERY-QUEUE",
            pool,
            TrancheStructure::new(vec![senior, equity]).expect("structure"),
            start,
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse")
        .with_cleanup_call(0.10)
        .expect("cleanup call");
        // CDR=30%: heavy defaults drive the pool below 10% factor in ~77 months.
        // No prepayments: the cleanup is triggered purely by defaults.
        // Recovery_lag=24 months: at the cleanup date the entire recovery queue
        // (from up to 77 prior default periods) is still pending.
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.30);
        instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 24);
        instrument
    }

    /// W-26 — cleanup-call redemption must ADD pending recoveries to available
    /// cash, not subtract them.
    ///
    /// `RecoveryQueue::pending_amount()` represents future cash *inflows* —
    /// recovery proceeds from already-defaulted collateral that have not yet
    /// matured through the lag period. At the cleanup call, the equity holder
    /// purchases the remaining pool (including distressed assets / recovery
    /// rights), so these inflows are realised immediately. They must be ADDED
    /// to `pool_outstanding` when computing `available_for_redemption`.
    ///
    /// The buggy code subtracted them:
    ///   `available = pool_outstanding − pending_recoveries`
    /// causing `available` to clamp to zero when pending_recoveries > pool_outstanding,
    /// so the senior tranche is paid nothing at the cleanup call and retains
    /// a large unpaid final balance.
    ///
    /// The fix adds them:
    ///   `available = pool_outstanding + pending_recoveries`
    /// which fully funds the remaining tranche claims.
    ///
    /// Assertion: after the fix, the senior tranche's final_balance must be
    /// zero (fully redeemed). With the bug, it would be ~4 M (the pending
    /// recoveries that were silently subtracted and then lost).
    #[test]
    fn cleanup_call_redemption_includes_pending_recovery_queue() {
        let instrument = cleanup_with_large_recovery_queue_deal();
        let start = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let market = MarketContext::new().insert(
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(start)
                .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.88), (15.0, 0.70)])
                .build()
                .expect("curve"),
        );

        let results = run_simulation_with_source(
            &instrument,
            &market,
            start,
            &mut DeterministicPoolFlowSource,
        )
        .expect("simulation");

        let tranche = results.get("A").expect("tranche A result");

        // Identify the cleanup-call date (simulation terminates there).
        let cleanup_date = tranche
            .cashflows
            .iter()
            .map(|(d, _)| *d)
            .max()
            .expect("at least one cashflow");

        // Principal retired at the cleanup call on the senior tranche.
        let principal_on_cleanup: f64 = tranche
            .principal_flows
            .iter()
            .filter(|(d, _)| *d == cleanup_date)
            .map(|(_, m)| m.amount())
            .sum();

        let final_balance = tranche.final_balance.amount();

        // With CDR=30% and a 24-month recovery lag, the recovery queue at
        // cleanup holds ~3.6 M in pending inflows while pool_outstanding ≈ 1 M.
        //
        // Buggy (subtract): available = max(0, 1M − 3.6M) = 0 → senior gets
        // $0 principal at the cleanup call → final_balance stays at ~4 M.
        //
        // Correct (add): available = 1M + 3.6M = 4.6M → senior fully redeemed
        // → final_balance = 0.
        //
        // The senior tranche's final balance must be zero after the fix.
        assert!(
            final_balance < 1.0,
            "senior tranche must be fully redeemed at the cleanup call; \
             final_balance={final_balance:.2} (non-zero means pending recoveries \
             were subtracted instead of added, leaving the tranche unredeemed)"
        );

        // The cleanup call must have retired some principal on the senior tranche.
        assert!(
            principal_on_cleanup > WRITEDOWN_DE_MINIMIS,
            "cleanup call must retire senior principal; got {principal_on_cleanup:.2} \
             (under the buggy subtraction available_for_redemption clamps to 0)"
        );
    }

    /// Item 12 — interest must stop accruing when an asset defaults.
    ///
    /// A defaulting asset must not accrue the full period's interest on its
    /// full pre-default balance. With defaults assumed uniformly distributed
    /// over the period, the defaulting fraction `period_mdr` accrues on
    /// average HALF the period's interest, so total interest is scaled by
    /// `(1 − 0.5·period_mdr)`.
    ///
    /// White-box test: a single non-amortizing asset, two runs at the same
    /// balance/rate — one with zero default, one with a high MDR. The
    /// high-MDR run's interest must equal the zero-default interest times
    /// `(1 − 0.5·period_mdr)`, strictly less than the full accrual.
    #[test]
    fn interest_accrual_stops_at_default() {
        use crate::instruments::fixed_income::structured_credit::types::AssetType;
        use finstack_quant_core::types::InstrumentId;

        let closing = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let maturity = Date::from_calendar_date(2034, Month::January, 1).expect("date");
        let pay1 = Date::from_calendar_date(2024, Month::April, 1).expect("date");

        let rate = 0.08_f64;
        let balance = 1_000_000.0_f64;
        let months_per_period = 3.0_f64;
        let market = MarketContext::new().insert(cc_discount_curve());

        // Build a single non-amortizing bond pool with an optional MDR override.
        let build_pool = |mdr_override: Option<f64>| {
            let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
            pool.assets.push(PoolAsset {
                day_count: DayCount::Thirty360,
                id: InstrumentId::new("BND1"),
                asset_type: AssetType::HighYieldBond { industry: None },
                balance: Money::new(balance, Currency::USD),
                rate,
                spread_bps: None,
                index_id: None,
                maturity,
                credit_quality: None,
                industry: None,
                obligor_id: None,
                is_defaulted: false,
                recovery_amount: None,
                purchase_price: None,
                acquisition_date: None,
                smm_override: None,
                mdr_override,
            });
            pool
        };
        let make_tranches = || {
            let tranche = Tranche::new(
                "A",
                0.0,
                100.0,
                TrancheSeniority::Senior,
                Money::new(balance, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity,
            )
            .expect("tranche");
            TrancheStructure::new(vec![tranche]).expect("structure")
        };

        let one_period_interest = |pool: &AssetPool, tranches: &TrancheStructure| -> f64 {
            let mut state = SimulationState::new(pool, tranches, closing, 0).expect("state");
            let flows = calculate_pool_flows_with_rates(RatedPoolFlowRequest {
                state: &mut state,
                pay_date: pay1,
                prev_date: closing,
                months_per_period,
                context: &market,
                rates: PoolFlowRates {
                    smm: 0.0,
                    mdr: 0.0,
                    recovery_rate: 0.40,
                },
                copula_outcome: None,
            })
            .expect("flows");
            flows.interest.amount()
        };

        // No default: full period accrual.
        let pool_no_default = build_pool(None);
        let tranches_a = make_tranches();
        let interest_no_default = one_period_interest(&pool_no_default, &tranches_a);
        assert!(
            interest_no_default > 0.0,
            "baseline interest must be positive"
        );

        // High monthly MDR ⇒ large period default fraction.
        let monthly_mdr = 0.10_f64;
        let pool_with_default = build_pool(Some(monthly_mdr));
        let tranches_b = make_tranches();
        let interest_with_default = one_period_interest(&pool_with_default, &tranches_b);

        // Expected: period_mdr = 1 − (1 − monthly_mdr)^months; interest scaled
        // by (1 − 0.5·period_mdr).
        let period_mdr = 1.0 - (1.0 - monthly_mdr).powf(months_per_period);
        let expected = interest_no_default * (1.0 - 0.5 * period_mdr);
        assert!(
            (interest_with_default - expected).abs() < 1.0,
            "defaulting-asset interest {interest_with_default:.2} must be the \
             full-accrual interest {interest_no_default:.2} scaled by \
             (1 − 0.5·period_mdr) = {expected:.2}"
        );
        // And it must be strictly below the un-haircut full accrual.
        assert!(
            interest_with_default < interest_no_default - 1.0,
            "defaulting-asset interest must be strictly below the full \
             pre-default accrual ({interest_with_default:.2} vs \
             {interest_no_default:.2})"
        );
    }

    /// M15 — lagged recoveries still pending at simulation end must be
    /// drained and distributed, not silently dropped by `finalize()`.
    ///
    /// With a 12-month recovery lag and defaults occurring in every period
    /// (including the final year), the recovery queue is non-empty when the
    /// final scheduled payment date is reached. Pre-fix, that pending
    /// recovery cash was dropped — losses overstated for any deal with
    /// defaults near maturity. The cleanup-call branch already realized the
    /// pending queue; the two normal termination paths now do too.
    #[test]
    fn pending_recoveries_at_simulation_end_are_drained_not_dropped() {
        let closing = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("date");
        let face_senior = 90_000_000.0_f64;

        let build_deal = |lag_months: u32| {
            let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
            pool.assets.push(PoolAsset::fixed_rate_bond(
                "A1",
                Money::new(100_000_000.0, Currency::USD),
                0.06,
                maturity,
                DayCount::Thirty360,
            ));
            let tranches = TrancheStructure::new(vec![
                Tranche::new(
                    "A",
                    0.0,
                    90.0,
                    TrancheSeniority::Senior,
                    Money::new(face_senior, Currency::USD),
                    TrancheCoupon::Fixed { rate: 0.05 },
                    maturity,
                )
                .expect("senior"),
                Tranche::new(
                    "EQ",
                    90.0,
                    100.0,
                    TrancheSeniority::Equity,
                    Money::new(10_000_000.0, Currency::USD),
                    TrancheCoupon::Fixed { rate: 0.0 },
                    maturity,
                )
                .expect("equity"),
            ])
            .expect("structure");
            let mut instrument = StructuredCredit::new_abs(
                "ABS-TAIL-RECOVERY",
                pool,
                tranches,
                closing,
                maturity,
                "USD-OIS",
            )
            .with_payment_calendar("nyse");
            // CDR 20%: defaults occur in every period including the final
            // year, so with a 12-month lag the queue is non-empty at the
            // final scheduled date. No prepayments, 40% recovery.
            instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.20);
            instrument.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
            instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, lag_months);
            instrument
        };

        let market = MarketContext::new().insert(cc_discount_curve());
        let run = |lag: u32| {
            run_simulation_with_source(
                &build_deal(lag),
                &market,
                closing,
                &mut DeterministicPoolFlowSource,
            )
            .expect("simulation")
        };
        let lagged = run(12);
        let immediate = run(0);

        // (a) No dropped recovery cash on the senior note: its face must be
        // fully consumed by principal repaid + net-loss write-downs, and its
        // final balance must be zero. Pre-fix, the pending tail recoveries
        // were dropped, leaving the senior under-repaid by exactly the
        // pending queue (defaulted × recovery rate from the final lag
        // window).
        let senior = lagged.get("A").expect("senior tranche");
        let retired = senior.total_principal.amount() + senior.total_writedown.amount();
        assert!(
            (retired - face_senior).abs() < 1.0,
            "senior principal ({:.2}) + write-down ({:.2}) must equal face \
             {face_senior:.0}; a shortfall means tail recoveries were dropped",
            senior.total_principal.amount(),
            senior.total_writedown.amount(),
        );
        assert!(
            senior.final_balance.amount() < 1.0,
            "senior tranche must be fully retired once pending recoveries \
             are drained; final_balance={}",
            senior.final_balance.amount()
        );

        // (b) Conservation against a lag-0 run: the recovery lag shifts cash
        // TIMING only — pool interest, defaults, and recoveries are identical
        // — so total cash distributed across the structure must match.
        let total_cash = |res: &HashMap<String, TrancheCashflows>| -> f64 {
            res.values()
                .flat_map(|r| r.cashflows.iter())
                .map(|(_, m)| m.amount())
                .sum()
        };
        let cash_lagged = total_cash(&lagged);
        let cash_immediate = total_cash(&immediate);
        assert!(
            (cash_lagged - cash_immediate).abs() < 1.0,
            "total cash with a 12m recovery lag ({cash_lagged:.2}) must equal \
             the lag-0 total ({cash_immediate:.2}); a shortfall is dropped \
             tail-recovery cash"
        );

        // (c) The drained flows must land at-or-after the final scheduled
        // date (released at the later of the final date and default + lag).
        let last_flow = lagged
            .values()
            .flat_map(|r| r.cashflows.iter())
            .map(|(d, _)| *d)
            .max()
            .expect("flows");
        assert!(
            last_flow >= maturity,
            "drained recovery flows must release on/after the final date; \
             last flow {last_flow}"
        );
    }
}

// ============================================================================
// SIMULATION STATE
// ============================================================================

/// De minimis threshold for write-down recording (avoids noise from fp rounding).
const WRITEDOWN_DE_MINIMIS: f64 = 0.01;

/// Optional cleanup-call premium paid on top of par + accrued interest.
///
/// Real cleanup calls sometimes carry a small make-whole-style premium that
/// senior holders are effectively short. The [`StructuredCredit`] type carries
/// no call-premium field today, so this helper returns `0.0` (redeem at par).
/// Routing the premium through a single function keeps a future premium a
/// one-line change here, without threading a new field through the public
/// instrument type.
fn cleanup_call_premium(_instrument: &StructuredCredit, _tranche_balance: f64) -> f64 {
    0.0
}

/// Internal state for period-by-period simulation.
pub(crate) struct SimulationState<'a> {
    /// AssetPool state (SoA layout)
    pool_state: PoolState,
    /// Total pool outstanding (sum of balances)
    pool_outstanding: Money,
    recovery_queue: RecoveryQueue,
    tranche_balances: HashMap<String, Money>,
    /// Deferred (PIK) interest per tranche, carried forward to next period.
    deferred_interest: HashMap<String, Money>,
    results: HashMap<String, TrancheCashflows>,
    prev_date: Option<Date>,
    base_ccy: Currency,
    recovery_lag_months: u32,
    pool: &'a AssetPool,
    tranches: &'a TrancheStructure,
    closing_date: Date,
    pool_balance_cleanup_threshold: f64,
    tranche_recipient_keys: Vec<RecipientType>,
    /// Whether reinvestment was active in the previous period.
    /// Used to detect the reinvestment-end transition and reconcile pool_outstanding.
    was_reinvestment_active: bool,
    /// Cumulative expected net losses (default_amount * (1 - recovery_rate)).
    ///
    /// Uses the expected recovery at the point of default rather than lagged
    /// realized recoveries. This is the INTEX/Moody's Analytics convention:
    /// loss allocation should reflect economic loss at default, not cash-timing
    /// of recovery receipts. Lagged recoveries affect waterfall cash, not
    /// loss allocation.
    cumulative_expected_loss: f64,
    /// Cumulative net loss that exceeds the structure's total absorbable
    /// notional (every tranche fully written down). Surfaced rather than
    /// silently dropped by the loss-allocation `min(...)` cap, so the
    /// per-period cash-conservation check can account for it.
    cumulative_loss_unallocated: f64,
    /// Total pool balance at simulation start (including defaulted assets).
    /// Used for cleanup call pool factor calculation.
    total_pool_balance: Money,
    /// Performing pool balance at simulation start (excluding pre-defaulted assets).
    /// Used as denominator for loss allocation percentage.
    performing_pool_balance: Money,
    /// Pre-computed tranche indices sorted by loss allocation order:
    /// equity (first loss) → subordinated → mezzanine → senior.
    /// Computed once, reused every period.
    loss_alloc_order: Vec<usize>,
    /// Balance-weighted average collateral age (WALA) in months at closing,
    /// derived from each asset's `acquisition_date`. PSA/SDA seasoning ramps
    /// are keyed off LOAN age, not deal age, so seasoned collateral must
    /// start partway up the ramp. Assets without an `acquisition_date`
    /// contribute zero age (collateral assumed new at closing).
    pool_wala_months: u32,
    /// Current reserve account balance.
    reserve_balance: Money,
    /// Current excess-spread (spread-account) balance. Carries period to period:
    /// funded from captured residual interest and drawn to cover debt interest
    /// shortfalls. See `ExcessSpreadSpec`.
    spread_account: Money,
}

impl<'a> SimulationState<'a> {
    fn new(
        pool: &'a AssetPool,
        tranches: &'a TrancheStructure,
        closing_date: Date,
        recovery_lag_months: u32,
    ) -> Result<Self> {
        let base_ccy = pool.base_currency();
        let pool_balance_cleanup_threshold = embedded_registry()?.pool_balance_cleanup_threshold();

        // Initialize results map for each tranche
        let results: HashMap<String, TrancheCashflows> = tranches
            .tranches
            .iter()
            .map(|t| {
                (
                    t.id.to_string(),
                    TrancheCashflows {
                        tranche_id: t.id.to_string(),
                        cashflows: Vec::new(),
                        detailed_flows: Vec::new(),
                        interest_flows: Vec::new(),
                        principal_flows: Vec::new(),
                        pik_flows: Vec::new(),
                        writedown_flows: Vec::new(),
                        final_balance: t.current_balance,
                        total_interest: Money::new(0.0, base_ccy),
                        total_principal: Money::new(0.0, base_ccy),
                        total_pik: Money::new(0.0, base_ccy),
                        total_writedown: Money::new(0.0, base_ccy),
                    },
                )
            })
            .collect();

        let tranche_balances: HashMap<String, Money> = tranches
            .tranches
            .iter()
            .map(|t| (t.id.to_string(), t.current_balance))
            .collect();

        // Map each tranche to its waterfall distribution key.
        // Equity tranches receive residual via RecipientType::Equity in the
        // standard waterfall, so their key must match that variant.
        let tranche_recipient_keys: Vec<RecipientType> = tranches
            .tranches
            .iter()
            .map(|t| {
                if t.seniority == TrancheSeniority::Equity {
                    RecipientType::Equity
                } else {
                    RecipientType::Tranche(t.id.to_string())
                }
            })
            .collect();

        // Initialize PoolState
        // Note: For now we convert the full asset list to PoolState.
        // Future optimization: Support RepLine conversion to PoolState.
        let pool_state = PoolState::from_pool(pool);

        let deferred_interest: HashMap<String, Money> = tranches
            .tranches
            .iter()
            .map(|t| (t.id.to_string(), Money::new(0.0, base_ccy)))
            .collect();

        // Determine if reinvestment is initially active
        let initial_reinvestment_active = pool
            .reinvestment_period
            .as_ref()
            .is_some_and(|period| closing_date <= period.end_date);

        let total_pool_balance = pool.total_balance().unwrap_or(Money::new(0.0, base_ccy));

        // Performing balance excludes pre-defaulted assets. Used as denominator
        // for loss allocation — pre-defaulted assets are already priced into the
        // deal structure and should not trigger additional write-downs.
        let performing_pool_balance = pool.performing_balance().unwrap_or(total_pool_balance);

        // Pre-compute loss allocation order once: equity first → senior last.
        // TrancheSeniority enum: Senior=0, Mezzanine=1, Subordinated=2, Equity=3.
        // Sort descending so Equity (3) comes first.
        let mut loss_alloc_order: Vec<usize> = (0..tranches.tranches.len()).collect();
        loss_alloc_order.sort_by(|&a, &b| {
            tranches.tranches[b]
                .seniority
                .cmp(&tranches.tranches[a].seniority)
        });

        // Balance-weighted average collateral age (WALA) at closing. The
        // `acquisition_date` carried by each pool asset (the origination /
        // issue date for assets built from bonds) is the closest available
        // proxy for loan origination; assets without one contribute zero age.
        let mut weighted_age = 0.0_f64;
        let mut total_weight = 0.0_f64;
        for asset in &pool.assets {
            let weight = asset.balance.amount().max(0.0);
            if weight <= 0.0 {
                continue;
            }
            total_weight += weight;
            if let Some(acq_date) = asset.acquisition_date {
                if closing_date > acq_date {
                    weighted_age += f64::from(acq_date.months_until(closing_date)) * weight;
                }
            }
        }
        let pool_wala_months = if total_weight > 0.0 {
            (weighted_age / total_weight).round() as u32
        } else {
            0
        };

        Ok(Self {
            pool_state,
            pool_outstanding: total_pool_balance,
            recovery_queue: RecoveryQueue::new(),
            tranche_balances,
            deferred_interest,
            results,
            prev_date: Some(closing_date),
            base_ccy,
            recovery_lag_months,
            pool,
            tranches,
            closing_date,
            pool_balance_cleanup_threshold,
            tranche_recipient_keys,
            was_reinvestment_active: initial_reinvestment_active,
            cumulative_expected_loss: 0.0,
            cumulative_loss_unallocated: 0.0,
            total_pool_balance,
            performing_pool_balance,
            loss_alloc_order,
            pool_wala_months,
            reserve_balance: pool.reserve_account,
            spread_account: Money::new(0.0, base_ccy),
        })
    }

    fn is_pool_exhausted(&self) -> bool {
        self.pool_outstanding.amount() <= self.pool_balance_cleanup_threshold
    }

    fn finalize(mut self) -> HashMap<String, TrancheCashflows> {
        for (tranche_id, res) in self.results.iter_mut() {
            let mut final_balance = self
                .tranche_balances
                .get(tranche_id)
                .copied()
                .unwrap_or(Money::new(0.0, self.base_ccy));
            if final_balance.amount() < 0.0 && final_balance.amount().abs() <= WRITEDOWN_DE_MINIMIS
            {
                final_balance = Money::new(0.0, self.base_ccy);
            }
            res.final_balance = final_balance;

            for (date, amount) in &res.interest_flows {
                if amount.amount() > 0.0 {
                    res.detailed_flows.push(CashFlow {
                        date: *date,
                        reset_date: None,
                        amount: *amount,
                        kind: CFKind::Fixed,
                        accrual_factor: 0.0,
                        rate: None,
                    });
                }
            }
            for (date, amount) in &res.principal_flows {
                if amount.amount() > 0.0 {
                    res.detailed_flows.push(CashFlow {
                        date: *date,
                        reset_date: None,
                        amount: *amount,
                        kind: CFKind::Amortization,
                        accrual_factor: 0.0,
                        rate: None,
                    });
                }
            }
            // Include write-down flows in detailed_flows so NPV and
            // risk analytics capture the full economic picture.
            // Write-downs represent permanent loss of notional and are
            // classified as DefaultedNotional (negative = loss to holder).
            for (date, amount) in &res.writedown_flows {
                if amount.amount() > 0.0 {
                    res.detailed_flows.push(CashFlow {
                        date: *date,
                        reset_date: None,
                        amount: Money::new(-amount.amount(), amount.currency()),
                        kind: CFKind::DefaultedNotional,
                        accrual_factor: 0.0,
                        rate: None,
                    });
                }
            }
        }

        self.results
    }
}

// ============================================================================
// PERIOD SIMULATION
// ============================================================================

/// Simulate a single payment period.
///
/// Period execution order matches INTEX/Bloomberg convention:
///   1. Calculate pool cashflows (interest, principal, default, recovery)
///   2. Allocate losses through capital structure (using expected loss at default)
///   3. Execute waterfall on post-loss tranche balances
///   4. Record cashflows and update tranche balances
///   5. Update pool balance
///
/// Loss allocation uses **expected net loss** = default * (1 - recovery_rate),
/// applied at the point of default. This decouples loss recognition from cash
/// timing of recovery receipts (which are lagged). Recoveries still flow through
/// the waterfall as cash when they mature from the recovery queue.
fn simulate_period(
    state: &mut SimulationState,
    instrument: &StructuredCredit,
    waterfall: &Waterfall,
    pay_date: Date,
    context: &MarketContext,
    months_per_period: f64,
    source: &mut (impl PoolFlowSource + ?Sized),
) -> Result<()> {
    // Seasoning for PSA/SDA ramps = collateral age, not deal age: the
    // pool's balance-weighted average loan age at closing (WALA, derived
    // from asset acquisition dates) plus the months elapsed since closing.
    // Seasoned collateral therefore enters the ramp partway up instead of
    // restarting at month zero on the deal closing date.
    let seasoning_months = state.pool_wala_months + state.closing_date.months_until(pay_date);

    // Capture period start before updating prev_date (for accrual calculations)
    let period_start = state.prev_date.unwrap_or(state.closing_date);

    // Reinvestment logic -- determined before pool flows so reconciliation
    // can snap pool_outstanding to the correct pre-flow asset balances.
    let is_reinvestment_active = state
        .pool
        .reinvestment_period
        .as_ref()
        .is_some_and(|period| pay_date <= period.end_date);

    // Reconciliation: When reinvestment transitions from active → inactive,
    // snap pool_outstanding to the actual sum of asset balances BEFORE this
    // period's flows are applied. During the reinvestment period,
    // pool_outstanding is reduced only by defaults (gross), which can cause
    // it to diverge from the true sum of asset-level balances (e.g. due to
    // matured assets, rounding, or partial defaults). This one-time
    // reconciliation eliminates the phantom balance at the transition point.
    //
    // Must happen before calculate_pool_flows so that Step 4's normal
    // subtraction of this period's flows is applied to the correct base.
    if state.was_reinvestment_active && !is_reinvestment_active {
        let actual_sum: f64 = state.pool_state.balances.iter().sum();
        state.pool_outstanding = Money::new(actual_sum.max(0.0), state.base_ccy);
    }
    state.was_reinvestment_active = is_reinvestment_active;

    // ── Step 1: Calculate pool cashflows for the period ──────────────
    let pool_flows = source.calculate_pool_flows(PoolFlowRequest {
        state,
        instrument,
        pay_date,
        prev_date: period_start,
        seasoning_months,
        months_per_period,
        context,
    })?;

    state.prev_date = Some(pay_date);

    // ── Reinvestment recycling ───────────────────────────────────────
    // During the reinvestment period, collected principal (scheduled
    // amortization + prepayments) is NOT distributed to the tranches — it is
    // recycled by the manager into new collateral. Without this recycle the
    // asset-level balances shrink every period (calculate_pool_flows debits
    // scheduled principal and prepayments), the collected principal is never
    // distributed (Step 3 excludes it from the waterfall), and at the
    // reinvestment-end reconciliation `pool_outstanding` is snapped DOWN to
    // the shrunken asset sum — so the recycled principal silently vanishes
    // and never generates future cashflows.
    //
    // Recycle by crediting the collected principal back onto the surviving
    // performing assets (pro-rata to their post-flow balances). This holds
    // the pool balance flat net of defaults, so the recycled cash continues
    // to throw off interest, principal and defaults in later periods.
    // Recoveries are CASH and are never recycled — they flow to the waterfall.
    if is_reinvestment_active {
        let recyclable = pool_flows.scheduled_principal.amount().max(0.0)
            + pool_flows.prepayment.amount().max(0.0);
        if recyclable.is_finite() && recyclable > 0.0 {
            recycle_reinvestment_principal(state, recyclable);
        }
    }

    // Add new recoveries to the lag queue
    state
        .recovery_queue
        .add_recovery(pay_date, pool_flows.recovery);

    // Release matured recoveries (these become cash for waterfall distribution)
    let released_recoveries = state.recovery_queue.release_matured(
        pay_date,
        state.recovery_lag_months,
        state.base_ccy,
    )?;

    // ── Step 2: Loss allocation through capital structure ────────────
    //
    // INTEX/Moody's Analytics convention: allocate expected net loss at the
    // point of default, NOT when lagged recoveries arrive. This ensures:
    //   - Tranche balances reflect economic reality before the waterfall runs
    //   - Interest accrues only on non-impaired notional
    //   - OC/IC coverage tests see correct post-loss balances
    //   - No risk of paying interest on subsequently written-down principal
    //
    // Net loss = defaulted principal − realized recovery. Using the actual
    // recovered amount (rather than `default × (1 − mean_recovery)`) makes the
    // write-down reflect per-name recovery dispersion; it reduces to the old
    // formula when every default recovers at the period systematic rate.
    // This is a permanent, irreversible write-down.
    let period_expected_loss =
        (pool_flows.default.amount() - pool_flows.recovery.amount()).max(0.0);
    state.cumulative_expected_loss += period_expected_loss;

    if state.cumulative_expected_loss > WRITEDOWN_DE_MINIMIS
        && state.performing_pool_balance.amount() > 0.0
    {
        // Allocate the *period's* expected net loss bottom-up using the
        // pre-computed order.
        //
        // A tranche's notional is consumed by exactly two channels: principal
        // repayment (cash retiring face) and write-down (loss retiring face).
        // The invariant is `principal_repaid + write-down ≤ original_balance`.
        //
        // The previous engine allocated *cumulative* expected loss against a
        // cap of `original_balance` every period. That double-counts retired
        // face: a tranche written down in an early period (when little
        // principal had been repaid) keeps that write-down recorded, while the
        // waterfall continues paying it principal in later periods — so
        // `principal_repaid + write-down` could exceed face. The robust fix is
        // to (a) allocate only this period's *incremental* net loss, and
        // (b) cap each tranche's incremental write-down at its CURRENT balance
        // — a tranche can never be written down below zero, and any loss it
        // cannot absorb cascades up to the next-most-senior tranche.
        let already_allocated: f64 = state
            .results
            .values()
            .map(|r| r.total_writedown.amount())
            .sum();
        let mut remaining_loss = (state.cumulative_expected_loss - already_allocated).max(0.0);
        // Clone loss_alloc_order to avoid borrow conflict with state
        let loss_order = state.loss_alloc_order.clone();
        for &idx in &loss_order {
            if remaining_loss <= WRITEDOWN_DE_MINIMIS {
                break;
            }
            let tranche_id_str = state.tranches.tranches[idx].id.as_str();

            // The tranche can absorb at most its current outstanding balance.
            // `tranche_balances` already nets out every prior principal
            // payment AND prior write-down, so capping the incremental
            // write-down here keeps `principal_repaid + write-down ≤ face`.
            let current = state
                .tranche_balances
                .get(tranche_id_str)
                .map(|m| m.amount())
                .unwrap_or(0.0);
            if current <= WRITEDOWN_DE_MINIMIS {
                continue;
            }

            let incremental = remaining_loss.min(current);
            remaining_loss -= incremental;
            if incremental > WRITEDOWN_DE_MINIMIS {
                // Reduce tranche balance BEFORE waterfall execution.
                if let Some(current_balance) = state.tranche_balances.get_mut(tranche_id_str) {
                    let new_balance = (current_balance.amount() - incremental).max(0.0);
                    *current_balance = Money::new(new_balance, state.base_ccy);
                }

                let writedown = Money::new(incremental, state.base_ccy);
                if let Some(res) = state.results.get_mut(tranche_id_str) {
                    res.writedown_flows.push((pay_date, writedown));
                    res.total_writedown = res.total_writedown.checked_add(writedown)?;
                }
            }
        }

        // Cumulative net loss above the structure's total absorbable notional
        // cannot be written down further (every tranche is fully impaired).
        // Surface it rather than silently dropping it: the engine accumulates
        // it on `cumulative_loss_unallocated` so it is observable and the
        // post-loss invariant below can be checked. `remaining_loss` here is
        // the portion of THIS period's incremental net loss that no tranche
        // could absorb, so it is accumulated, not overwritten.
        if remaining_loss > WRITEDOWN_DE_MINIMIS {
            state.cumulative_loss_unallocated += remaining_loss;
        }

        // Invariant: unallocated loss can only be non-zero once every tranche
        // is fully written down (no notional left to absorb it). Debug-only —
        // compiled out in release builds.
        if cfg!(debug_assertions) && state.cumulative_loss_unallocated > WRITEDOWN_DE_MINIMIS {
            let total_face: f64 = state
                .tranches
                .tranches
                .iter()
                .map(|t| t.original_balance.amount())
                .sum();
            let total_writedown: f64 = state
                .results
                .values()
                .map(|r| r.total_writedown.amount())
                .sum();
            let total_principal: f64 = state
                .results
                .values()
                .map(|r| r.total_principal.amount())
                .sum();
            debug_assert!(
                total_writedown + total_principal >= total_face - WRITEDOWN_DE_MINIMIS,
                "unallocated loss {} surfaced but structure is not fully \
                 retired: face={total_face}, writedown={total_writedown}, \
                 principal={total_principal}",
                state.cumulative_loss_unallocated,
            );
        }
    }

    // ── Step 3: Prepare waterfall inputs ─────────────────────────────
    // Total principal from pool (scheduled + prepayment)
    let total_principal_from_pool = pool_flows
        .scheduled_principal
        .checked_add(pool_flows.prepayment)?;

    // During reinvestment, principal collections are reinvested into new assets.
    // Recoveries are CASH and always flow through the waterfall.
    let principal_available_for_waterfall = if is_reinvestment_active {
        released_recoveries
    } else {
        total_principal_from_pool.checked_add(released_recoveries)?
    };

    let mut total_cash_for_waterfall = pool_flows
        .interest
        .checked_add(principal_available_for_waterfall)?;

    // Excess-spread (spread-account) capture/draw, applied to the cash entering
    // the waterfall. Capturing *here* — before the single sequential waterfall
    // can sweep surplus interest into senior principal — is what lets the
    // account fund from excess interest mid-deal and later draw to cover debt
    // interest shortfalls. No-op (identity) when no `excess_spread` is set.
    // `spread_net_capture` is the net cash diverted into the account this period
    // (negative when drawing), reconciled by the cash-conservation check.
    let mut spread_net_capture = 0.0_f64;
    if let Some(es) = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.excess_spread.as_ref())
    {
        // Current-period interest due across all debt (non-equity) tranches.
        let mut debt_interest_due = 0.0_f64;
        for tranche in &state.tranches.tranches {
            if tranche.seniority == TrancheSeniority::Equity {
                continue;
            }
            let bal = state
                .tranche_balances
                .get(tranche.id.as_str())
                .map_or(0.0, Money::amount);
            let rate = tranche
                .coupon
                .try_current_rate_with_index(pay_date, context)?;
            let accrual = tranche.day_count.year_fraction(
                period_start,
                pay_date,
                DayCountContext::default(),
            )?;
            debt_interest_due += bal * rate * accrual;
        }

        let interest_avail = pool_flows.interest.amount();
        if interest_avail > debt_interest_due {
            // Capture surplus interest into the account, up to the target.
            let room = (es.target_balance.amount() - state.spread_account.amount()).max(0.0);
            let capture = (interest_avail - debt_interest_due).min(room).max(0.0);
            state.spread_account =
                Money::new(state.spread_account.amount() + capture, state.base_ccy);
            total_cash_for_waterfall = Money::new(
                (total_cash_for_waterfall.amount() - capture).max(0.0),
                state.base_ccy,
            );
            spread_net_capture = capture;
        } else {
            // Draw from the account to cover the interest shortfall.
            let draw = (debt_interest_due - interest_avail)
                .min(state.spread_account.amount())
                .max(0.0);
            state.spread_account = Money::new(state.spread_account.amount() - draw, state.base_ccy);
            total_cash_for_waterfall =
                Money::new(total_cash_for_waterfall.amount() + draw, state.base_ccy);
            spread_net_capture = -draw;
        }
    }

    // ── Step 4: Execute Waterfall on post-loss balances ──────────────
    // Per-period step-down: switch principal to pro-rata once the deal has
    // seasoned past the step-down date with cumulative losses below the trigger.
    // Borrowed (zero-cost) when no step-down rule applies.
    let cumulative_loss_fraction = if state.total_pool_balance.amount() > 0.0 {
        state.cumulative_expected_loss / state.total_pool_balance.amount()
    } else {
        0.0
    };
    let period_waterfall =
        crate::instruments::fixed_income::structured_credit::pricing::resolve::apply_step_down(
            waterfall,
            instrument.waterfall_rules.as_ref(),
            pay_date,
            cumulative_loss_fraction,
        );

    let waterfall_context =
        crate::instruments::fixed_income::structured_credit::pricing::waterfall::WaterfallContext {
            available_cash: total_cash_for_waterfall,
            interest_collections: pool_flows.interest,
            principal_collections: principal_available_for_waterfall,
            payment_date: pay_date,
            period_start,
            pool_balance: state.pool_outstanding,
            market: context,
            tranche_balances: Some(&state.tranche_balances),
            deferred_interest: Some(&state.deferred_interest),
            reserve_balance: state.reserve_balance,
            recovery_proceeds: released_recoveries,
        };

    let waterfall_result =
        crate::instruments::fixed_income::structured_credit::pricing::waterfall::execute_waterfall(
            &period_waterfall,
            state.tranches,
            state.pool,
            waterfall_context,
        )?;

    // Update reserve balance from waterfall distributions to ReserveAccount recipients.
    for (recipient, amount) in &waterfall_result.distributions {
        if let RecipientType::ReserveAccount(_) = recipient {
            state.reserve_balance = state.reserve_balance.checked_add(*amount)?;
        }
    }

    // Available-funds cap: for AFC tranches, cap the recorded coupon at the
    // collateral weighted-average coupon. Combined with the capped waterfall
    // allocation (`resolve_waterfall`), this caps both the interest *due*
    // (recorded below) and the cash routed to the interest recipient, so the
    // excess spread above the cap flows on through the waterfall rather than
    // inflating this tranche's interest.
    let afc_spec = instrument
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.afc.as_ref());
    let afc_cap_rate = if afc_spec.is_some() {
        instrument.pool.weighted_avg_coupon()
    } else {
        0.0
    };

    // ── Step 5: Record flows and update balances ─────────────────────
    for (idx, tranche) in state.tranches.tranches.iter().enumerate() {
        let recipient_key = &state.tranche_recipient_keys[idx];
        let tranche_id_str = tranche.id.as_str();

        let current_balance = state
            .tranche_balances
            .get(tranche_id_str)
            .copied()
            .unwrap_or(Money::new(0.0, state.base_ccy));
        let coupon_rate = {
            let raw = tranche
                .coupon
                .try_current_rate_with_index(pay_date, context)?;
            match afc_spec {
                Some(spec) if spec.capped_tranches.iter().any(|t| t == tranche_id_str) => {
                    raw.min(afc_cap_rate)
                }
                _ => raw,
            }
        };

        // Use tranche's day-count convention for proper accrual calculation
        let accrual_factor =
            tranche
                .day_count
                .year_fraction(period_start, pay_date, DayCountContext::default())?;

        let existing_deferred = state
            .deferred_interest
            .get(tranche_id_str)
            .copied()
            .unwrap_or(Money::new(0.0, state.base_ccy));

        // Current-period interest due on post-writedown balance. Non-PIK
        // deferred interest is a separate senior interest claim and is passed
        // into the waterfall context before execution.
        let current_interest_due = Money::new(
            current_balance.amount() * coupon_rate * accrual_factor,
            state.base_ccy,
        );
        let total_interest_claim = if tranche.pik_enabled {
            current_interest_due
        } else {
            existing_deferred.checked_add(current_interest_due)?
        };

        let payment_received = waterfall_result
            .distributions
            .get(recipient_key)
            .copied()
            .unwrap_or(Money::new(0.0, state.base_ccy));

        // Determine how much of this tranche's distribution cures interest
        // claims before any remainder is classified as principal.
        let interest_paid = if payment_received.amount() >= total_interest_claim.amount() {
            total_interest_claim
        } else {
            payment_received
        };
        let deferred_repaid = Money::new(
            interest_paid
                .amount()
                .min(existing_deferred.amount())
                .max(0.0),
            state.base_ccy,
        );
        let current_interest_paid = interest_paid
            .checked_sub(deferred_repaid)
            .unwrap_or(Money::new(0.0, state.base_ccy));
        let current_interest_shortfall = Money::new(
            (current_interest_due.amount() - current_interest_paid.amount()).max(0.0),
            state.base_ccy,
        );

        let principal_payment = payment_received
            .checked_sub(interest_paid)
            .unwrap_or(Money::new(0.0, state.base_ccy));

        if let Some(res) = state.results.get_mut(tranche_id_str) {
            if payment_received.amount() > 0.0 {
                res.cashflows.push((pay_date, payment_received));
            }
            if interest_paid.amount() > 0.0 {
                res.interest_flows.push((pay_date, interest_paid));
                res.total_interest = res.total_interest.checked_add(interest_paid)?;
            }
            if principal_payment.amount() > 0.0 {
                res.principal_flows.push((pay_date, principal_payment));
                res.total_principal = res.total_principal.checked_add(principal_payment)?;
            }
            // Record PIK (interest shortfall deferred to future periods)
            if current_interest_shortfall.amount() > 0.0 {
                res.pik_flows.push((pay_date, current_interest_shortfall));
                res.total_pik = res.total_pik.checked_add(current_interest_shortfall)?;
            }
        }

        let remaining_deferred = if tranche.pik_enabled {
            Money::new(0.0, state.base_ccy)
        } else {
            existing_deferred
                .checked_sub(deferred_repaid)
                .unwrap_or(Money::new(0.0, state.base_ccy))
                .checked_add(current_interest_shortfall)?
        };
        state
            .deferred_interest
            .insert(tranche_id_str.to_string(), remaining_deferred);

        // Update tranche balance:
        // - Always reduce by principal payment
        // - Only accrete shortfall if PIK is explicitly enabled for this tranche
        //
        // Standard CLO/ABS indenture: shortfalls are tracked as deferred interest
        // and paid from future interest collections, NOT capitalized into balance.
        // PIK accretion (capitalizing shortfall) is an explicit structural feature
        // that must be opted into per tranche.
        if let Some(current) = state.tranche_balances.get_mut(tranche_id_str) {
            let after_principal = current.checked_sub(principal_payment).unwrap_or(*current);
            // The waterfall nets in-period principal against the period-start
            // balance snapshot, so TranchePrincipal payments cannot exceed the
            // remaining balance. Residual/equity distributions, however, are
            // booked here as "principal" against a zero balance — floor at
            // zero so a negative balance never propagates into later periods'
            // interest accrual and coverage tests.
            let after_principal = if after_principal.amount() < 0.0 {
                Money::new(0.0, state.base_ccy)
            } else {
                after_principal
            };
            if tranche.pik_enabled && current_interest_shortfall.amount() > 0.0 {
                *current = after_principal.checked_add(current_interest_shortfall)?;
            } else {
                *current = after_principal;
            }
        }
    }

    // ── Step 6: Update pool balance ──────────────────────────────────
    if is_reinvestment_active {
        // During reinvestment, principal is recycled into new assets.
        // AssetPool balance drops only by defaults (gross).
        state.pool_outstanding = state.pool_outstanding.checked_sub(pool_flows.default)?;
    } else {
        // After reinvestment, all principal reductions hit pool balance.
        state.pool_outstanding = state
            .pool_outstanding
            .checked_sub(total_principal_from_pool)?
            .checked_sub(pool_flows.default)?;
    }

    // Numerical cleanup: avoid tiny negative residual balances like -0.00
    // after repeated principal/default arithmetic.
    if state.pool_outstanding.amount() < 0.0
        && state.pool_outstanding.amount().abs() <= WRITEDOWN_DE_MINIMIS
    {
        state.pool_outstanding = Money::new(0.0, state.base_ccy);
    }

    // ── Item 13: per-period cash-conservation invariant ──────────────
    // Every dollar of pool cash routed into the waterfall must come out as a
    // tranche/recipient distribution or be left as residual cash. This is a
    // debug/test-only assertion (zero release-build cost) that catches
    // accounting regressions in the waterfall and the engine's pool-flow
    // aggregation.
    debug_assert_cash_conserved(
        total_cash_for_waterfall,
        &pool_flows,
        released_recoveries,
        is_reinvestment_active,
        &waterfall_result,
        spread_net_capture,
    );

    Ok(())
}

/// Per-period cash-conservation invariant (debug/test builds only).
///
/// Verifies two identities for one payment period:
///
/// 1. **Input identity** — the cash handed to the waterfall equals the pool
///    cash that is actually distributable this period:
///    `total_cash_for_waterfall = interest + released_recoveries`
///    (`+ scheduled_principal + prepayment` when reinvestment is inactive;
///    during reinvestment that principal is recycled into collateral, not
///    distributed).
///
/// 2. **Output identity** — the waterfall conserves cash:
///    `Σ distributions + remaining_cash = total_available`.
///
/// Compiled out entirely in release builds (`debug_assert!`), so there is no
/// hot-path cost; it exists to fail loudly in tests and debug runs if a future
/// change breaks the engine's cash accounting.
#[inline]
fn debug_assert_cash_conserved(
    total_cash_for_waterfall: Money,
    pool_flows: &PoolFlows,
    released_recoveries: Money,
    is_reinvestment_active: bool,
    waterfall_result: &WaterfallDistribution,
    spread_net_capture: f64,
) {
    if !cfg!(debug_assertions) {
        return;
    }

    // Tolerance scales with deal size: penny-safe pro-rata allocation in the
    // waterfall rounds to the currency's smallest unit per recipient.
    let tol = (total_cash_for_waterfall.amount().abs() * 1e-9).max(1.0);

    // Identity 1: input to the waterfall == distributable pool cash, net of any
    // cash diverted into (or supplied from) the excess-spread account.
    let expected_input = if is_reinvestment_active {
        pool_flows.interest.amount() + released_recoveries.amount()
    } else {
        pool_flows.interest.amount()
            + pool_flows.scheduled_principal.amount()
            + pool_flows.prepayment.amount()
            + released_recoveries.amount()
    } - spread_net_capture;
    debug_assert!(
        (total_cash_for_waterfall.amount() - expected_input).abs() <= tol,
        "cash-conservation (input): waterfall received {} but distributable \
         pool cash is {} (interest={}, scheduled={}, prepay={}, recoveries={}, \
         reinvesting={})",
        total_cash_for_waterfall.amount(),
        expected_input,
        pool_flows.interest.amount(),
        pool_flows.scheduled_principal.amount(),
        pool_flows.prepayment.amount(),
        released_recoveries.amount(),
        is_reinvestment_active,
    );

    // Identity 2: the waterfall neither creates nor destroys cash.
    let distributed: f64 = waterfall_result
        .distributions
        .values()
        .map(|m| m.amount())
        .sum();
    let accounted = distributed + waterfall_result.remaining_cash.amount();
    debug_assert!(
        (accounted - waterfall_result.total_available.amount()).abs() <= tol,
        "cash-conservation (output): waterfall distributed {} + residual {} = \
         {} but had {} available",
        distributed,
        waterfall_result.remaining_cash.amount(),
        accounted,
        waterfall_result.total_available.amount(),
    );
}

/// Recycle reinvestment-period principal back into the surviving pool.
///
/// During the reinvestment period, collected scheduled principal and
/// prepayments are reinvested by the manager into new collateral rather than
/// distributed to the tranches. This helper models that by crediting the
/// `recyclable` cash onto the still-performing assets (those that are not
/// defaulted and carry a positive balance), pro-rata to their current
/// balances. The net effect is that the pool balance stays flat net of
/// defaults, so the recycled principal continues to generate interest,
/// scheduled principal and defaults in subsequent periods instead of silently
/// vanishing at the reinvestment-end reconciliation.
///
/// If no performing assets remain (the whole pool has defaulted/amortized),
/// the cash cannot be placed into new collateral and the recycle is a no-op;
/// the deal is structurally at its end and the cleanup/exhaustion logic takes
/// over.
fn recycle_reinvestment_principal(state: &mut SimulationState, recyclable: f64) {
    let performing_total: f64 = state
        .pool_state
        .is_defaulted
        .iter()
        .zip(state.pool_state.balances.iter())
        .filter(|(defaulted, balance)| !**defaulted && **balance > 0.0)
        .map(|(_, balance)| *balance)
        .sum();

    if performing_total <= 0.0 {
        // No surviving collateral to reinvest into — recycle is a no-op.
        return;
    }

    let n = state.pool_state.len();
    for i in 0..n {
        if state.pool_state.is_defaulted[i] {
            continue;
        }
        let balance = state.pool_state.balances[i];
        if balance <= 0.0 {
            continue;
        }
        let share = balance / performing_total;
        state.pool_state.balances[i] = balance + recyclable * share;
    }
}

// ============================================================================
// CALCULATION HELPERS
// ============================================================================

/// AssetPool flow results for a single period.
pub(crate) struct PoolFlows {
    interest: Money,
    scheduled_principal: Money,
    prepayment: Money,
    default: Money,
    recovery: Money,
}

/// Calculate all pool flows for the period.
///
/// Implements:
/// - M1: Scheduled amortization for amortizing assets (mortgages, auto, etc.)
/// - M3: Maturity/balloon payment when an asset reaches maturity
/// - m2: Sequential default → scheduled principal → prepay application
///   (Intex/Moody's Analytics & SIFMA convention: MDR on the BOP balance,
///   scheduled principal on the survivor, SMM on the remainder)
#[derive(Debug, Clone, Copy)]
struct PoolFlowRates {
    smm: f64,
    mdr: f64,
    recovery_rate: f64,
}

/// Copula-resolved default outcome for one payment period.
///
/// Present only when the scenario default model is a copula; otherwise the
/// engine uses the legacy monthly-equivalent `PoolFlowRates::mdr`.
enum PeriodDefaultOutcome<'a> {
    /// Per-name finite-pool simulation. Entry `k` of each slice describes the
    /// `k`-th still-performing asset (`!is_defaulted && balance > 0`) in the
    /// pool's intrinsic asset order.
    PerName {
        /// `true` ⇒ the asset defaults in full this period.
        defaults: &'a [bool],
        /// The recovery rate the asset realizes if it defaults this period,
        /// scattered idiosyncratically around the period systematic recovery.
        recoveries: &'a [f64],
    },

    /// LHP fast-path: a single **period-level** default rate (already
    /// aggregated over the period — *not* a monthly-equivalent rate) applied
    /// uniformly to every performing asset.
    PoolWidePeriodRate(f64),
}

struct RatedPoolFlowRequest<'a, 's> {
    state: &'a mut SimulationState<'s>,
    pay_date: Date,
    prev_date: Date,
    months_per_period: f64,
    context: &'a MarketContext,
    rates: PoolFlowRates,
    /// `Some` when the scenario default model is a copula (per-name or LHP);
    /// `None` for the legacy pool-wide MDR / deterministic path.
    copula_outcome: Option<PeriodDefaultOutcome<'a>>,
}

fn calculate_pool_flows_with_rates(request: RatedPoolFlowRequest<'_, '_>) -> Result<PoolFlows> {
    let state = request.state;
    let base_ccy = state.base_ccy;
    let mut total_interest = Money::new(0.0, base_ccy);
    let mut total_scheduled = Money::new(0.0, base_ccy);
    let mut total_prepay = Money::new(0.0, base_ccy);
    let mut total_default = Money::new(0.0, base_ccy);
    let mut total_recovery = Money::new(0.0, base_ccy);

    // Period-rate approximation for non-monthly payment frequencies: the
    // monthly SMM/MDR are sourced once per payment period at END-of-period
    // seasoning and compounded across the whole period. During a seasoning
    // ramp (PSA/SDA) the end-of-period rate is the highest within the period,
    // so ramp-phase speeds are slightly overstated for quarterly/semi-annual
    // deals (exact for monthly pay and for seasoning past the ramp).
    // Averaging the monthly rates within the period would remove the bias.
    let global_period_smm = 1.0 - (1.0 - request.rates.smm).powf(request.months_per_period);
    let global_period_mdr = 1.0 - (1.0 - request.rates.mdr).powf(request.months_per_period);

    // Pre-resolve all curves
    let mut resolved_rates = Vec::with_capacity(state.pool_state.unique_curves.len());
    for idx_str in &state.pool_state.unique_curves {
        let fwd = request.context.get_forward(idx_str)?;
        let base = fwd.base_date();
        let dc = fwd.day_count();
        let t2 = dc.year_fraction(base, request.pay_date, DayCountContext::default())?;
        let tenor = fwd.tenor();
        let t1 = (t2 - tenor).max(0.0);
        let r = if t2 > 0.0 && t1 < t2 {
            fwd.rate_period(t1, t2)
        } else {
            fwd.rate(0.0)
        };
        resolved_rates.push(r);
    }

    // Copula default resolution. For `PerName`, `per_name_mask[k]` is the
    // realized default outcome of the k-th still-performing asset (in pool
    // order); `alive_idx` advances for every asset that passes the
    // performing-asset gate below, so the indicator slice stays
    // index-aligned with the simulator's draw order.
    // For the LHP fast-path, `lhp_period_rate` is a single period-level rate
    // applied to every performing asset.
    let (per_name_outcome, lhp_period_rate) = match &request.copula_outcome {
        Some(PeriodDefaultOutcome::PerName {
            defaults,
            recoveries,
        }) => (Some((*defaults, *recoveries)), None),
        Some(PeriodDefaultOutcome::PoolWidePeriodRate(rate)) => (None, Some(*rate)),
        None => (None, None),
    };
    let mut alive_idx = 0usize;

    let n = state.pool_state.len();

    // Item 3 — mask/asset-loop alignment guard.
    //
    // The per-name default mask is sized by the *builder* (StochasticPathFlowSource)
    // from the count of still-performing assets (`!is_defaulted && balance > 0`)
    // at period start. The asset loop below claims one mask entry per asset
    // that passes the *same* performing-asset gate, in the *same* pool-index
    // order, so the k-th claim lines up with the k-th drawn idiosyncratic
    // shock. The asset loop mutates `is_defaulted`/`balances`, but only ever
    // for the asset it is currently processing (index `i`) and only after
    // that asset has claimed its slot — a later asset's gate is never
    // affected. The two counts are therefore equal by construction.
    //
    // A silent `unwrap_or(false)` on an out-of-bounds claim would turn any
    // future regression that breaks this alignment into a wrong-but-quiet
    // default realization. Instead, validate the mask length up-front and
    // fail loudly: this makes the deterministic alignment a checked invariant
    // rather than an implicit assumption.
    if let Some((mask, recoveries)) = per_name_outcome {
        let performing = (0..n)
            .filter(|&i| state.pool_state.balances[i] > 0.0 && !state.pool_state.is_defaulted[i])
            .count();
        if mask.len() != performing {
            return Err(finstack_quant_core::Error::Validation(format!(
                "per-name copula default mask is misaligned with the asset \
                 loop: mask carries {} entries but {} assets are performing \
                 at period start (pay_date {})",
                mask.len(),
                performing,
                request.pay_date,
            )));
        }
        // The recovery slice is built name-aligned with the default mask in
        // the same period; guard the invariant so a future regression cannot
        // silently mis-pair recoveries with defaults.
        if recoveries.len() != mask.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "per-name recovery slice ({} entries) is misaligned with the \
                 default mask ({} entries) at pay_date {}",
                recoveries.len(),
                mask.len(),
                request.pay_date,
            )));
        }
    }

    for i in 0..n {
        let balance = state.pool_state.balances[i];
        if balance <= 0.0 {
            continue;
        }

        // Skip already-defaulted assets: prevents pre-existing defaulted assets
        // (e.g. assets that entered the pool in workout) from accruing interest,
        // defaulting again, or prepaying. Also guards against assets marked as
        // fully defaulted during simulation.
        if state.pool_state.is_defaulted[i] {
            continue;
        }

        // This asset is performing at period start; claim its per-name
        // default indicator and idiosyncratic recovery. The pre-loop length
        // guard proves `alive_idx` is always in bounds here, so the claim is
        // exact and order-stable.
        let per_name_claim = per_name_outcome.map(|(mask, recoveries)| {
            let defaulted = mask.get(alive_idx).copied().unwrap_or(false);
            let recovery = recoveries
                .get(alive_idx)
                .copied()
                .unwrap_or(request.rates.recovery_rate);
            alive_idx += 1;
            (defaulted, recovery)
        });

        // Resolve this period's default rate up-front — it is needed both for
        // the mid-period interest-accrual haircut below and for the principal
        // default amount further down. The rate depends only on the asset's
        // MDR override, the per-name copula realization, the LHP period rate,
        // or the legacy pool-wide MDR — none of which depend on the scheduled
        // amortization computed later.
        //
        // Default-rate precedence:
        //   1. Per-asset `mdr_override` (explicit user input) — always wins.
        //   2. Per-name copula realization — full default (1.0) or none (0.0).
        //   3. LHP fast-path period rate — the closed-form `N → ∞` limit.
        //   4. Legacy pool-wide MDR (`global_period_mdr`).
        let period_mdr = if let Some(mdr) = state.pool_state.mdr_overrides[i] {
            1.0 - (1.0 - mdr).powf(request.months_per_period)
        } else if let Some((defaulted, _)) = per_name_claim {
            if defaulted {
                1.0
            } else {
                0.0
            }
        } else if let Some(rate) = lhp_period_rate {
            rate.clamp(0.0, 1.0)
        } else {
            global_period_mdr
        };

        // 1. Interest -- computed first so matured assets still pay their final coupon
        let rate = if let Some(curve_idx) = state.pool_state.curve_indices[i] {
            let base_rate = resolved_rates[curve_idx];
            base_rate + (state.pool_state.spread_bps[i].unwrap_or(0.0).max(0.0) / 10_000.0)
        } else {
            state.pool_state.rates[i]
        };

        // m-FINAL-1: Cap interest accrual at asset maturity for mid-period maturities.
        // If the asset matures between prev_date and pay_date, accrue interest only up
        // to the maturity date, not the full period end.
        let interest_end = state.pool_state.maturities[i].min(request.pay_date);

        let accrual_factor = state.pool_state.day_counts[i]
            .unwrap_or(DayCount::Act360)
            .year_fraction(request.prev_date, interest_end, DayCountContext::default())?;

        // Item 12 — interest must stop accruing when an asset defaults.
        // Defaults in a period are modeled as a rate `period_mdr` (a fraction
        // of the balance), with no explicit intra-period default date. Under
        // the standard market convention defaults are assumed uniformly
        // distributed over the period, so the defaulting fraction accrues, on
        // average, HALF the period's interest. The non-defaulting fraction
        // accrues the full period. Net interest is therefore scaled by
        // `(1 − 0.5·period_mdr)` rather than accruing the full pre-default
        // balance for the whole period.
        let default_accrual_haircut = 1.0 - 0.5 * period_mdr.clamp(0.0, 1.0);
        let interest = Money::new(
            balance * rate * accrual_factor * default_accrual_haircut,
            base_ccy,
        );
        total_interest = total_interest.checked_add(interest)?;

        // ── Default FIRST, on the beginning-of-period balance ────────────
        //
        // Market convention (Intex/Moody's Analytics; SIFMA standard MBS
        // cashflow methodology): the period default rate (MDR) is applied to
        // the BEGINNING-of-period balance, scheduled principal is then
        // computed on the surviving (post-default) balance, and the SMM is
        // applied to the survivor after scheduled principal. This also
        // reconciles with the mid-period interest-accrual haircut above,
        // which already assumes the defaulting fraction comes out of the
        // pre-scheduled (BOP) balance.
        let default_amt = balance * period_mdr;
        let balance_after_default = balance - default_amt;

        // Per-name defaults recover at their own idiosyncratically-dispersed
        // rate; the LHP and legacy paths use the period systematic recovery.
        let asset_recovery_rate = match per_name_claim {
            Some((_, recovery)) => recovery,
            None => request.rates.recovery_rate,
        };
        let recovery_amt = default_amt * asset_recovery_rate;
        total_default = total_default.checked_add(Money::new(default_amt, base_ccy))?;
        total_recovery = total_recovery.checked_add(Money::new(recovery_amt, base_ccy))?;

        // Mark asset as fully defaulted if default consumed (nearly) all the
        // BOP balance. Relative tolerance 1 - 1e-10 catches floating-point
        // imprecision when the MDR is effectively 100% (e.g. a per-name
        // copula full default) without false positives from small balances.
        if default_amt >= balance * (1.0 - 1e-10) {
            state.pool_state.is_defaulted[i] = true;
            state.pool_state.balances[i] = 0.0;
            continue;
        }

        // Check maturity -- if asset has matured, return the surviving
        // (post-default) balance as a balloon payment and zero out the asset.
        // Interest was already computed above (capped at maturity date, with
        // the default haircut applied).
        if request.pay_date >= state.pool_state.maturities[i] {
            let balloon = Money::new(balance_after_default, base_ccy);
            total_scheduled = total_scheduled.checked_add(balloon)?;
            state.pool_state.balances[i] = 0.0;
            continue;
        }

        // Scheduled amortization for amortizing assets, computed on the
        // SURVIVING (post-default) balance.
        //
        // Item 6 — a level-pay loan's scheduled payment is FIXED at
        // origination. Prepayments shorten the loan; they do not reduce the
        // scheduled payment. The previous engine recomputed the level payment
        // every period from the current (post-prepayment) balance and
        // remaining term, so the payment — and hence scheduled principal —
        // shrank after every prepayment. The fix freezes the contractual
        // level payment on first sight and reuses it. Defaulted loans'
        // contractual payments terminate, so the frozen aggregate payment IS
        // scaled by each period's survival fraction `(1 − period_mdr)` below.
        let scheduled_principal = if state.pool_state.is_amortizing[i] && rate > 0.0 {
            if !rate.is_finite() || rate <= -1.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "invalid amortization rate for pool asset '{}': {rate}",
                    state.pool_state.ids[i]
                )));
            }
            // period_rate = NOMINAL periodic rate: annual rate × months in the
            // payment period / 12. This is the US mortgage market convention
            // (e.g. a 6% 30-year mortgage pays 0.5% per month, not
            // 1.06^(1/12) − 1 ≈ 0.487%); the effective-compounding formula
            // previously used here understated the level payment by ~2.6%
            // relative error at typical coupons. Matches the MBS passthrough
            // pricer (mbs_passthrough/pricer.rs, `wac / 12.0`).
            let period_rate = rate * request.months_per_period / 12.0;
            if !period_rate.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "invalid amortization math for pool asset '{}': rate={rate}, period_rate={period_rate}",
                    state.pool_state.ids[i]
                )));
            }

            // Resolve the frozen contractual level payment, computing it once
            // on the first period the asset amortizes (period-native math:
            // level_payment = P * r_p / (1 − (1+r_p)^−n_p)).
            let level_payment = match state.pool_state.level_payments[i] {
                Some(lp) => lp,
                None => {
                    let remaining_days = (state.pool_state.maturities[i] - request.pay_date)
                        .whole_days()
                        .max(1) as f64;
                    let remaining_months = (remaining_days / 30.44).round().max(1.0);
                    let remaining_periods_f64 = remaining_months / request.months_per_period;
                    let denom = 1.0 - (1.0 + period_rate).powf(-remaining_periods_f64);
                    if !remaining_periods_f64.is_finite() || !denom.is_finite() {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "invalid amortization math for pool asset '{}': rate={rate}, period_rate={period_rate}",
                            state.pool_state.ids[i]
                        )));
                    }
                    let lp = if denom.abs() > 1e-12 && remaining_periods_f64 > 0.0 {
                        balance * period_rate / denom
                    } else {
                        // Denominator ~0 (very short term): pay the full balance.
                        balance
                    };
                    if !lp.is_finite() {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "invalid level payment for pool asset '{}': {lp}",
                            state.pool_state.ids[i]
                        )));
                    }
                    state.pool_state.level_payments[i] = Some(lp);
                    lp
                }
            };

            // Defaulted loans' contractual payments terminate. Defaults are
            // applied pro-rata across the (rep-line) asset, so the surviving
            // pool's aggregate level payment scales by this period's survival
            // fraction. Persist the scaled payment so future periods amortize
            // off the survivors' contractual payment.
            let surviving_payment = level_payment * (1.0 - period_mdr);
            state.pool_state.level_payments[i] = Some(surviving_payment);

            // Scheduled principal = survivors' level payment − this period's
            // interest on the surviving balance (interest + scheduled
            // principal = level payment under the same nominal-rate
            // convention). As the balance amortizes the interest portion
            // shrinks and the principal portion grows — the correct level-pay
            // profile. Bounded by the surviving balance so the loan never
            // over-amortizes.
            (surviving_payment - balance_after_default * period_rate)
                .max(0.0)
                .min(balance_after_default)
        } else {
            0.0
        };

        total_scheduled = total_scheduled.checked_add(Money::new(scheduled_principal, base_ccy))?;

        // Balance after default and scheduled amortization
        let balance_after_sched = balance_after_default - scheduled_principal;

        // Prepayment LAST: SMM applies to the survivor balance after
        // scheduled principal (Intex/Moody's Analytics & SIFMA standard
        // ordering: default on BOP balance → scheduled principal on the
        // survivor → prepayment on the remainder).
        let period_smm = if let Some(smm) = state.pool_state.smm_overrides[i] {
            1.0 - (1.0 - smm).powf(request.months_per_period)
        } else {
            global_period_smm
        };
        let prepay_amt = balance_after_sched * period_smm;
        total_prepay = total_prepay.checked_add(Money::new(prepay_amt, base_ccy))?;

        // Update balance
        let new_balance = balance_after_sched - prepay_amt;
        state.pool_state.balances[i] = new_balance.max(0.0);
    }

    Ok(PoolFlows {
        interest: total_interest,
        scheduled_principal: total_scheduled,
        prepayment: total_prepay,
        default: total_default,
        recovery: total_recovery,
    })
}
