//! Tree-based pricing engine for callable term loans.
//!
//! This module provides market-style optionality pricing for term loans with borrower
//! call schedules, using backward induction on a tree and a frictional exercise rule.
//!
//! # Model routing
//!
//! A loan with an explicit `credit_curve_id` prices on the two-factor
//! rates-credit lattice; without one it prices on the risk-free short-rate
//! tree. Nothing is inferred from curve naming conventions.
//!
//! On the rates-credit path every model input comes from
//! [`resolve_rates_credit_config`], so the four volatility regimes
//! (deterministic/stochastic rates × deterministic/stochastic credit) are
//! selected purely by `ModelConfig` — `hw1f_sigma`, `hazard_volatility`, the
//! two mean reversions, and `rate_credit_correlation`. There are no
//! engine-side volatility defaults: an unset volatility means a deterministic
//! factor, not a hidden regime. Hazard inputs on a loan with no credit curve
//! are rejected rather than ignored, since the short-rate tree has no hazard
//! factor to apply them to.
//!
//! A floating-rate loan on the rates-only tree is rejected when short-rate
//! volatility is positive: that tree can only book today's projected coupons.
//! Set `hw1f_sigma` to zero for a frozen projection, or supply
//! `credit_curve_id` so the rates-credit lattice can replay future resets.
//!
//! `hazard_volatility` is an **absolute** hazard-rate volatility, not a
//! relative credit-spread volatility; see
//! [`models::credit::market_anchored`](finstack_quant_models::credit::market_anchored)
//! for the conversion from a market-quoted fractional spread vol.
//!
//! With a positive `hw1f_sigma`, future floating resets re-fix off the rate
//! node (see [`RatesCreditTree::price_with_node_coupons`]): the
//! deterministic projection stays booked unchanged, the node-dependent
//! increment folds at each reset slice, and the standing call provision is
//! discretized to the floating periods' reset/payment slices. Future
//! floating PIK is rejected in that mode rather than misstating
//! path-dependent principal.

use crate::instruments::common_impl::pricing::rates_credit::{
    build_rates_credit_targets, continuous_frp_weight,
};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::term_loan::{RateSpec, TermLoan};
use crate::instruments::pricing_overrides::resolve_rates_credit_config;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use finstack_quant_models::trees::two_factor_rates_credit::{NodeCoupon, RatesCreditTree};
use finstack_quant_models::{
    short_rate_keys, NodeState, ShortRateTree, ShortRateTreeConfig, TreeModel, TreeValuator,
};

/// Reject hazard-model inputs on a loan that never reaches the rates-credit
/// lattice.
///
/// Without a `credit_curve_id` the callable loan prices on the short-rate
/// tree, which has no hazard factor. Silently ignoring a configured hazard
/// volatility or rate/credit correlation would leave the user believing they
/// selected a credit regime that was never applied.
fn reject_inert_hazard_inputs(loan: &TermLoan) -> Result<()> {
    let model = &loan.instrument_pricing_overrides.model_config;
    let configured = [
        ("hazard_volatility", model.hazard_volatility),
        ("hazard_mean_reversion", model.hazard_mean_reversion),
        ("rate_credit_correlation", model.rate_credit_correlation),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.map(|_| label))
    .collect::<Vec<_>>();
    if configured.is_empty() {
        return Ok(());
    }
    Err(finstack_quant_core::Error::Validation(format!(
        "TermLoan '{}' sets {} but has no credit_curve_id, so it prices on the \
         risk-free short-rate tree where the hazard factor does not exist. Set \
         credit_curve_id to opt into the rates-credit lattice, or remove the \
         hazard inputs.",
        loan.id,
        configured.join(", ")
    )))
}

/// Reject floating coupons on a stochastic rates-only tree.
fn reject_stochastic_short_rate_floating(loan: &TermLoan, rate_volatility: f64) -> Result<()> {
    if rate_volatility <= 0.0 || !matches!(loan.rate, RateSpec::Floating(_)) {
        return Ok(());
    }
    Err(finstack_quant_core::Error::Validation(format!(
        "TermLoan '{}' selects stochastic rates-only tree pricing for a floating \
         coupon, but that tree preprojects coupons. Use a credit_curve_id so the \
         rates-credit lattice can replay future resets, or set hw1f_sigma to zero.",
        loan.id
    )))
}

/// Select the calendar date nearest each tree time on the curve's day-count axis.
fn dates_on_time_grid(
    origin: Date,
    maturity: Date,
    day_count: DayCount,
    time_steps: &[f64],
) -> Result<Vec<Date>> {
    if origin >= maturity {
        return Ok(vec![origin; time_steps.len()]);
    }

    let mut dated_times = Vec::with_capacity((maturity - origin).whole_days() as usize + 1);
    let mut date = origin;
    loop {
        let time = day_count.year_fraction(origin, date, DayCountContext::default())?;
        dated_times.push((date, time));
        if date == maturity {
            break;
        }
        date = date.next_day().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "cannot advance tree grid date beyond {date}"
            ))
        })?;
    }

    Ok(time_steps
        .iter()
        .map(|target| {
            let upper = dated_times.partition_point(|(_, time)| *time <= *target);
            match upper {
                0 => dated_times[0].0,
                index if index == dated_times.len() => dated_times[index - 1].0,
                index => {
                    let lower = dated_times[index - 1];
                    let upper = dated_times[index];
                    if (upper.1 - target).abs() <= (target - lower.1).abs() {
                        upper.0
                    } else {
                        lower.0
                    }
                }
            }
        })
        .collect())
}

/// Replay outstanding events once to capture balances immediately before and
/// after payments at each tree time.
fn outstanding_on_time_grid(
    initial_outstanding: f64,
    outstanding_events: &[(f64, f64)],
    time_steps: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let mut call_outstanding = Vec::with_capacity(time_steps.len());
    let mut recovery_outstanding = Vec::with_capacity(time_steps.len());
    let mut outstanding = initial_outstanding;
    let mut event_index = 0;

    for step_time in time_steps {
        while event_index < outstanding_events.len()
            && outstanding_events[event_index].0 < *step_time
        {
            outstanding = outstanding_events[event_index].1;
            event_index += 1;
        }
        call_outstanding.push(outstanding.max(0.0));

        while event_index < outstanding_events.len()
            && outstanding_events[event_index].0 <= *step_time
        {
            outstanding = outstanding_events[event_index].1;
            event_index += 1;
        }
        recovery_outstanding.push(outstanding.max(0.0));
    }

    (call_outstanding, recovery_outstanding)
}

/// Configuration for tree-based term loan pricing (callable PV, OAS).
#[derive(Debug, Clone)]
pub(crate) struct TermLoanTreePricerConfig {
    pub(crate) tree_steps: usize,
    /// Short-rate volatility used **only** by the risk-free short-rate tree
    /// (no credit curve). The rates-credit path takes its volatilities from
    /// [`resolve_rates_credit_config`], never from here.
    pub(crate) rate_volatility: f64,
    pub(crate) tolerance: f64,
    pub(crate) max_iterations: usize,
    pub(crate) initial_bracket_size_bp: Option<f64>,
}

impl Default for TermLoanTreePricerConfig {
    fn default() -> Self {
        Self {
            tree_steps: 100,
            rate_volatility: 0.01,
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
        }
    }
}

/// Calibrated tree plus valuator shared by direct PV and OAS solves.
enum PreparedTree {
    /// Joint rates-credit lattice with optional node-dependent coupons.
    RatesCredit {
        tree: RatesCreditTree,
        valuator: TermLoanValuator,
        node_coupons: Vec<NodeCoupon>,
        time_to_maturity: f64,
    },
    /// Risk-free short-rate tree.
    ShortRate {
        tree: ShortRateTree,
        initial_rate: f64,
        valuator: TermLoanValuator,
        time_to_maturity: f64,
    },
}

/// Term loan valuator for tree-based callable pricing.
///
/// Implements `TreeValuator` by mapping dated loan cashflows and call schedules into
/// step-indexed vectors and applying borrower call exercise with friction costs.
/// Call redemption is dirty: clean strike plus cash accrued at the step date.
struct TermLoanValuator {
    loan: TermLoan,
    /// Coupon + fee cashflows by step (paid regardless of call decision).
    coupon_fee_vec: Vec<f64>,
    /// Coupon/fee pieces paired with true-time offsets from their assigned step.
    coupon_fee_components: Vec<Vec<(f64, f64)>>,
    /// Scheduled principal cashflows by step (only received if not called).
    principal_vec: Vec<f64>,
    /// Principal pieces paired with true-time offsets from their assigned step.
    principal_components: Vec<Vec<(f64, f64)>>,
    /// Dirty call redemption by step (pre-exercise outstanding × clean price
    /// plus cash accrued, DF-timed onto the step).
    call_vec: Vec<Option<f64>>,
    /// True-time offset from the assigned step for each call redemption.
    call_time_offset_vec: Vec<Option<f64>>,
    /// Outstanding principal (pre-exercise) corresponding to `call_vec` steps.
    ///
    /// This is used to compute exercise friction consistently with the call redemption.
    call_outstanding_vec: Vec<Option<f64>>,
    /// Outstanding principal at start of step (used for friction and call strike).
    outstanding_vec: Vec<f64>,
    /// Outstanding after scheduled payments at this step (recovery over the
    /// next interval if the loan is held).
    recovery_outstanding_vec: Vec<f64>,
    /// Optional recovery rate from hazard curve.
    recovery_rate: Option<f64>,
    /// Call friction in cents per 100 of outstanding.
    call_friction_cents: f64,
    /// Uniform tree time grid, kept for node-coupon descriptor
    /// construction on the stochastic-rate rates-credit path.
    time_steps: Vec<f64>,
    /// Valuation date the pricing schedule was built with.
    as_of: Date,
    /// Settlement origin — the tree's `t = 0`.
    origin: Date,
}

impl TermLoanValuator {
    fn new(
        loan: TermLoan,
        market: &MarketContext,
        as_of: Date,
        origin: Date,
        time_to_maturity: f64,
        tree_steps: usize,
    ) -> Result<Self> {
        use crate::cashflow::primitives::CFKind;
        let dt = time_to_maturity / tree_steps as f64;
        let time_steps: Vec<f64> = (0..=tree_steps).map(|i| i as f64 * dt).collect();
        let num_steps = tree_steps + 1;

        let disc = market.get_discount(&loan.discount_curve_id)?;
        let dc_curve = disc.day_count();
        let step_dates = dates_on_time_grid(origin, loan.maturity, dc_curve, &time_steps)?;

        // DF timing correction for cashflows mapped onto the tree grid
        // (same correction the bond valuator's `value_at_step_time` applies):
        // a piece booked at `step_time` is scaled by `DF(event_time) /
        // DF(step_time)` so that, once the tree discounts it from
        // `step_time`, its PV equals the cashflow's PV at its true time.
        // Without this the floor/ceil linear split silently mis-times
        // discounting.
        let value_at_step_time = |amount: f64, event_time: f64, step_time: f64| -> f64 {
            let step_df = disc.df(step_time);
            if step_df <= f64::EPSILON {
                return amount;
            }
            amount * disc.df(event_time) / step_df
        };

        let schedule =
            super::discounting::TermLoanDiscountingPricer::pricing_schedule(&loan, market, as_of)?;
        let accrual_index =
            crate::cashflow::accrual::AccrualIndex::build(&schedule, &loan.accrual_config())?;
        let out_path = schedule.outstanding_by_date()?;

        // Helper: outstanding BEFORE a target date (pre-exercise).
        // Initialise with the schedule's actually funded balance. The facility
        // limit includes undrawn DDTL commitment and is not current principal.
        let initial_funded = out_path
            .iter()
            .take_while(|(date, _)| *date <= origin)
            .map(|(_, amount)| amount.amount())
            .last()
            .unwrap_or_else(|| schedule.get_notional().initial.amount());
        let outstanding_before = |target: Date| -> f64 {
            let mut last = initial_funded;
            for (d, amt) in &out_path {
                if *d < target {
                    last = amt.amount();
                } else {
                    break;
                }
            }
            last
        };

        // Build coupon/fee and principal flow vectors.
        let mut coupon_fee_vec = vec![0.0; num_steps];
        let mut coupon_fee_components = vec![Vec::new(); num_steps];
        let mut principal_vec = vec![0.0; num_steps];
        let mut principal_components = vec![Vec::new(); num_steps];

        // Identify exercise dates for snapping cashflows to exercise steps.
        let mut exercise_dates = std::collections::HashSet::new();
        if let Some(ref cs) = loan.call_schedule {
            for c in &cs.calls {
                if c.date >= origin && c.date <= loan.maturity {
                    exercise_dates.insert(c.date);
                }
            }
        }

        // Book a cashflow onto the grid: exercise cashflows snap to their
        // (ceil) step, others are distributed between floor/ceil steps —
        // matching the bond convention — with each piece carrying the DF
        // timing correction to its destination step time.
        let book = |vec: &mut Vec<f64>,
                    components: &mut Vec<Vec<(f64, f64)>>,
                    amount: f64,
                    t: f64,
                    raw_clamped: f64,
                    snap: bool| {
            if snap {
                let step = (raw_clamped.ceil() as usize).clamp(0, num_steps - 1);
                let adjusted = value_at_step_time(amount, t, time_steps[step]);
                vec[step] += adjusted;
                components[step].push((adjusted, t - time_steps[step]));
            } else {
                let lo = raw_clamped.floor() as usize;
                let weight = raw_clamped - lo as f64;
                if lo < num_steps {
                    let adjusted = value_at_step_time(amount * (1.0 - weight), t, time_steps[lo]);
                    vec[lo] += adjusted;
                    components[lo].push((adjusted, t - time_steps[lo]));
                }
                if lo + 1 < num_steps {
                    let adjusted = value_at_step_time(amount * weight, t, time_steps[lo + 1]);
                    vec[lo + 1] += adjusted;
                    components[lo + 1].push((adjusted, t - time_steps[lo + 1]));
                }
            }
        };

        for cf in schedule.get_flows() {
            // The tree is valued immediately after settlement. Contractual
            // funding and other flows on the settlement date have already
            // exchanged and must not enter the holder's forward value.
            if cf.date <= origin {
                continue;
            }
            let t = dc_curve.year_fraction(origin, cf.date, DayCountContext::default())?;
            let raw = (t / time_to_maturity) * tree_steps as f64;
            let raw_clamped = raw.clamp(0.0, tree_steps as f64);

            let is_exercise = exercise_dates.contains(&cf.date);

            match cf.kind {
                CFKind::Fixed
                | CFKind::FloatReset
                | CFKind::Stub
                | CFKind::Fee
                | CFKind::CommitmentFee
                | CFKind::UsageFee
                | CFKind::FacilityFee => {
                    book(
                        &mut coupon_fee_vec,
                        &mut coupon_fee_components,
                        cf.amount.amount(),
                        t,
                        raw_clamped,
                        is_exercise,
                    );
                }
                // Signed principal cashflows: positive repayments/redemption
                // and negative future DDTL funding legs.
                CFKind::Amortization | CFKind::Notional => {
                    book(
                        &mut principal_vec,
                        &mut principal_components,
                        cf.amount.amount(),
                        t,
                        raw_clamped,
                        is_exercise,
                    );
                }
                _ => {}
            }
        }

        // Replay the dated balance path in tree time order. Call exercise uses
        // the balance strictly before events at the step; recovery over the
        // following interval uses the balance after events at that step.
        let mut outstanding_events: Vec<(f64, f64)> = out_path
            .iter()
            .filter(|(d, _)| *d >= origin && *d <= loan.maturity)
            .filter_map(|(d, amt)| {
                dc_curve
                    .year_fraction(origin, *d, DayCountContext::default())
                    .ok()
                    .map(|t| (t, amt.amount()))
            })
            .collect();
        outstanding_events
            .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let (outstanding_vec, recovery_outstanding_vec) =
            outstanding_on_time_grid(outstanding_before(origin), &outstanding_events, &time_steps);

        // Dirty call redemption (pre-exercise outstanding × call price + accrued).
        //
        // Call type semantics:
        // - Hard/Soft: borrower exercises at `price_pct_of_par` × outstanding.
        //   Soft calls behave identically to Hard in pricing; the premium is
        //   already captured in `price_pct_of_par`.
        // - MakeWhole: borrower pays PV of remaining flows at Treasury + spread,
        //   which by design equals or exceeds the continuation value. The option
        //   is therefore non-economic and skipped in the tree to avoid mispricing.
        let mut call_vec: Vec<Option<f64>> = vec![None; num_steps];
        let mut call_time_offset_vec: Vec<Option<f64>> = vec![None; num_steps];
        let mut call_outstanding_vec: Vec<Option<f64>> = vec![None; num_steps];
        if let Some(ref cs) = loan.call_schedule {
            let mut call_boundaries = Vec::with_capacity(cs.calls.len());
            for call in &cs.calls {
                if call.date > loan.maturity {
                    continue;
                }
                let start_step = if call.date <= origin {
                    0
                } else {
                    let t =
                        dc_curve.year_fraction(origin, call.date, DayCountContext::default())?;
                    let raw = (t / time_to_maturity) * tree_steps as f64;
                    (raw.clamp(0.0, tree_steps as f64).ceil() as usize).clamp(0, num_steps - 1)
                };
                call_boundaries.push((start_step, call));
            }

            // Each call entry is an effective-dated provision. It remains active
            // at every subsequent exercise step until the next entry replaces it.
            // Make-whole entries still act as boundaries, but are not represented
            // as an economic option in this tree.
            let mut boundary_index = 0usize;
            let mut active_call = None;
            for step in 0..num_steps {
                while boundary_index < call_boundaries.len()
                    && call_boundaries[boundary_index].0 <= step
                {
                    active_call = Some(call_boundaries[boundary_index].1);
                    boundary_index += 1;
                }
                let Some(call) = active_call else {
                    continue;
                };
                if matches!(
                    call.call_type,
                    crate::instruments::fixed_income::term_loan::LoanCallType::MakeWhole { .. }
                ) {
                    continue;
                }

                let out = outstanding_vec[step].max(0.0);
                let step_date = step_dates[step];
                let accrued = accrual_index.accrued_at(step_date)?;
                let clean = out * (call.price_pct_of_par / 100.0);
                let event_time =
                    dc_curve.year_fraction(origin, step_date, DayCountContext::default())?;
                call_vec[step] = Some(value_at_step_time(
                    clean + accrued,
                    event_time,
                    time_steps[step],
                ));
                call_time_offset_vec[step] = Some(event_time - time_steps[step]);
                call_outstanding_vec[step] = Some(out);
            }
        }

        // Recovery rate (if hazard curve present). Precedence mirrors other credit-aware pricers:
        // 1) credit_curve_id (if set)
        // 2) discount_curve_id
        // 3) "{discount_curve_id}-CREDIT"
        let recovery_rate = {
            if let Some(ref credit_id) = loan.credit_curve_id {
                market
                    .get_hazard(credit_id.as_str())
                    .ok()
                    .map(|hc| hc.recovery_rate())
            } else {
                market
                    .get_hazard(loan.discount_curve_id.as_str())
                    .ok()
                    .or_else(|| {
                        market
                            .get_hazard(format!("{}-CREDIT", loan.discount_curve_id.as_str()))
                            .ok()
                    })
                    .map(|hc| hc.recovery_rate())
            }
        };

        let call_friction_cents = loan
            .instrument_pricing_overrides
            .model_config
            .call_friction_cents
            .unwrap_or(0.0);

        Ok(Self {
            loan,
            coupon_fee_vec,
            coupon_fee_components,
            principal_vec,
            principal_components,
            call_vec,
            call_time_offset_vec,
            call_outstanding_vec,
            outstanding_vec,
            recovery_outstanding_vec,
            recovery_rate,
            call_friction_cents,
            time_steps,
            as_of,
            origin,
        })
    }

    /// Build node-coupon descriptors for stochastic future floating resets.
    fn stochastic_node_coupons(&self, market: &MarketContext) -> Result<Vec<NodeCoupon>> {
        use crate::instruments::common_impl::pricing::floating_reset_descriptors::{
            build_node_coupons, has_future_pik, params_from_spec, strips_index_constraints,
            NodeCouponBuildInputs,
        };

        let RateSpec::Floating(ref float_spec) = self.loan.rate else {
            return Ok(Vec::new());
        };
        let schedule = super::discounting::TermLoanDiscountingPricer::pricing_schedule(
            &self.loan, market, self.as_of,
        )?;
        let disc = market.get_discount(&self.loan.discount_curve_id)?;

        // PIK first: a 100% PIK floating leg emits no cash FloatReset flows
        // at all, so an empty descriptor list must not be read as "nothing
        // stochastic here" — the capitalized amounts themselves are
        // node-dependent.
        if has_future_pik(&schedule, self.origin) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "TermLoan '{}' capitalizes coupons (PIK) after settlement while \
                 pricing floating resets under stochastic rates. A future \
                 floating PIK coupon makes outstanding principal \
                 path-dependent, which the recombining rates-credit lattice \
                 cannot represent. Price with deterministic rates (hw1f_sigma \
                 unset) or remove the PIK feature.",
                self.loan.id
            )));
        }

        build_node_coupons(
            &NodeCouponBuildInputs {
                schedule: &schedule,
                params: params_from_spec(float_spec),
                grid_origin: self.origin,
                time_steps: &self.time_steps,
                day_count: disc.day_count(),
                discount: disc.as_ref(),
                strip_index_constraints: strips_index_constraints(float_spec),
            },
            |step| self.outstanding_at(step),
        )
    }

    /// Restrict the standing call provision to reset/payment boundaries
    /// inside node-dependent coupon periods.
    ///
    /// A term-loan call entry is an effective-dated **standing** provision:
    /// the deterministic engine evaluates exercise at every tree step. With
    /// node-dependent coupons, an exercise strictly inside a future
    /// floating period would need the path-dependent fixing state the
    /// recombining lattice does not carry, so in stochastic-rate mode the
    /// exercise opportunity set is discretized to the periods' reset and
    /// payment slices — the market-standard convention of evaluating loan
    /// prepayment on coupon dates. Steps outside node-dependent periods
    /// (in particular the current, already-fixed period) keep every-step
    /// exercise, matching the deterministic engine.
    fn restrict_exercise_to_reset_boundaries(&mut self, coupons: &[NodeCoupon]) {
        if coupons.is_empty() {
            return;
        }
        for step in 0..self.call_vec.len() {
            let interior = coupons
                .iter()
                .any(|c| step > c.reset_step && step < c.payment_step);
            if interior {
                self.call_vec[step] = None;
                self.call_time_offset_vec[step] = None;
                self.call_outstanding_vec[step] = None;
            }
        }
    }

    #[inline]
    fn value_at_oas(
        base: &[f64],
        components: &[Vec<(f64, f64)>],
        step: usize,
        oas_rate: f64,
    ) -> f64 {
        if oas_rate == 0.0 {
            return base.get(step).copied().unwrap_or(0.0);
        }
        components.get(step).map_or(0.0, |pieces| {
            pieces
                .iter()
                .map(|(amount, offset)| amount * (-oas_rate * offset).exp())
                .sum()
        })
    }

    #[inline]
    fn coupon_fee_at(&self, step: usize, oas_rate: f64) -> f64 {
        Self::value_at_oas(
            &self.coupon_fee_vec,
            &self.coupon_fee_components,
            step,
            oas_rate,
        )
    }

    #[inline]
    fn principal_cf_at(&self, step: usize, oas_rate: f64) -> f64 {
        Self::value_at_oas(
            &self.principal_vec,
            &self.principal_components,
            step,
            oas_rate,
        )
    }

    #[inline]
    fn call_at(&self, step: usize, oas_rate: f64) -> Option<f64> {
        self.call_vec.get(step).copied().flatten().map(|amount| {
            let offset = self
                .call_time_offset_vec
                .get(step)
                .copied()
                .flatten()
                .unwrap_or(0.0);
            amount * (-oas_rate * offset).exp()
        })
    }

    #[inline]
    fn call_outstanding_at(&self, step: usize) -> Option<f64> {
        self.call_outstanding_vec.get(step).copied().flatten()
    }

    #[inline]
    fn outstanding_at(&self, step: usize) -> f64 {
        self.outstanding_vec
            .get(step)
            .copied()
            .unwrap_or(self.loan.notional_limit.amount())
    }

    #[inline]
    fn recovery_outstanding_at(&self, step: usize) -> f64 {
        self.recovery_outstanding_vec
            .get(step)
            .copied()
            .unwrap_or_else(|| self.outstanding_at(step))
    }

    /// Replace scheduled principal plus risky continuation with dirty call
    /// proceeds when the borrower exercises. Friction raises only the
    /// exercise threshold.
    #[inline]
    fn apply_call(&self, step: usize, hold_principal: f64, oas_rate: f64) -> f64 {
        let Some(call_price) = self.call_at(step, oas_rate) else {
            return hold_principal;
        };
        let outstanding = self
            .call_outstanding_at(step)
            .unwrap_or_else(|| self.outstanding_at(step));
        let friction_amount = outstanding * (self.call_friction_cents / 10_000.0);
        if hold_principal > call_price + friction_amount {
            call_price
        } else {
            hold_principal
        }
    }
}

impl TreeValuator for TermLoanValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        let step = state.step;
        let oas_rate = state.get_var_or(short_rate_keys::OAS, 0.0) / 10_000.0;
        Ok(self.coupon_fee_at(step, oas_rate) + self.principal_cf_at(step, oas_rate))
    }

    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64> {
        let step = state.step;
        let oas_rate = state.get_var_or(short_rate_keys::OAS, 0.0) / 10_000.0;
        let coupon_fee = self.coupon_fee_at(step, oas_rate);
        let principal_cf = self.principal_cf_at(step, oas_rate);

        // The tree has already rate/OAS-discounted `continuation_value`. Apply
        // survival and FRP recovery only to that hold value. Current coupon
        // and (if exercised) call proceeds are cash at this node and must not
        // be survival-weighted over the next interval.
        let risky_continuation = if let Some(hazard) = state.hazard_rate {
            let hazard = hazard.max(0.0);
            let survival = (-hazard * dt).exp();
            let interval_df = state.discount_factor().unwrap_or_else(|| {
                let rate = state.interest_rate().unwrap_or(0.0);
                (-rate * dt).exp()
            });
            let recovery_weight = continuous_frp_weight(interval_df, survival)?;
            let recovery = self
                .recovery_rate
                .map(|rr| rr.clamp(0.0, 1.0) * self.recovery_outstanding_at(step) * recovery_weight)
                .unwrap_or(0.0);
            survival * continuation_value + recovery
        } else {
            continuation_value
        };

        Ok(coupon_fee + self.apply_call(step, risky_continuation + principal_cf, oas_rate))
    }
}

/// Tree-based pricer for callable term loans.
#[derive(Debug, Clone)]
pub struct TermLoanTreePricer {
    config: TermLoanTreePricerConfig,
}

impl Default for TermLoanTreePricer {
    fn default() -> Self {
        Self::new()
    }
}

impl TermLoanTreePricer {
    /// Create a tree pricer with default configuration.
    pub fn new() -> Self {
        Self {
            config: TermLoanTreePricerConfig::default(),
        }
    }

    fn resolved_config(&self, loan: &TermLoan) -> TermLoanTreePricerConfig {
        let configured_rate_volatility = loan.instrument_pricing_overrides.model_config.hw1f_sigma;
        let rate_volatility = configured_rate_volatility.unwrap_or_else(|| {
            if loan.credit_curve_id.is_none() && matches!(loan.rate, RateSpec::Floating(_)) {
                0.0
            } else {
                self.config.rate_volatility
            }
        });
        TermLoanTreePricerConfig {
            tree_steps: loan
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(self.config.tree_steps),
            rate_volatility,
            tolerance: self.config.tolerance,
            max_iterations: self.config.max_iterations,
            initial_bracket_size_bp: self.config.initial_bracket_size_bp,
        }
    }

    fn quoted_oas_bp(loan: &TermLoan) -> f64 {
        loan.instrument_pricing_overrides
            .market_quotes
            .quoted_oas
            .unwrap_or(0.0)
            * 10_000.0
    }

    fn prepare(
        &self,
        loan: &TermLoan,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Option<PreparedTree>> {
        let origin = loan.settlement_date(as_of)?;
        if origin >= loan.maturity {
            return Ok(None);
        }

        let disc = market.get_discount(&loan.discount_curve_id)?;
        let time_to_maturity =
            disc.day_count()
                .year_fraction(origin, loan.maturity, DayCountContext::default())?;
        if time_to_maturity <= 0.0 {
            return Ok(None);
        }

        let cfg = self.resolved_config(loan);
        let steps = cfg.tree_steps;
        let rate_volatility = cfg.rate_volatility;
        let hazard_curve = loan
            .credit_curve_id
            .as_ref()
            .map(|id| market.get_hazard(id.as_str()))
            .transpose()?;

        if hazard_curve.is_none() {
            reject_inert_hazard_inputs(loan)?;
            reject_stochastic_short_rate_floating(loan, rate_volatility)?;
        }

        let mut valuator =
            TermLoanValuator::new(loan.clone(), market, as_of, origin, time_to_maturity, steps)?;

        if let Some(hc) = hazard_curve.as_ref() {
            let tree_cfg = resolve_rates_credit_config(&loan.instrument_pricing_overrides, steps)?;
            let mut tree = RatesCreditTree::new(tree_cfg);
            let targets = build_rates_credit_targets(
                disc.as_ref(),
                hc.as_ref(),
                origin,
                loan.maturity,
                time_to_maturity,
                steps,
            )?;
            tree.calibrate(&targets)?;
            let node_coupons = if tree.config.rate_vol > 0.0 {
                let coupons = valuator.stochastic_node_coupons(market)?;
                valuator.restrict_exercise_to_reset_boundaries(&coupons);
                coupons
            } else {
                Vec::new()
            };
            return Ok(Some(PreparedTree::RatesCredit {
                tree,
                valuator,
                node_coupons,
                time_to_maturity,
            }));
        }

        let mut tree = ShortRateTree::new(ShortRateTreeConfig {
            steps,
            volatility: rate_volatility,
            ..Default::default()
        });
        tree.calibrate(disc.as_ref(), time_to_maturity)?;
        let initial_rate = tree.rate_at_node(0, 0)?;
        Ok(Some(PreparedTree::ShortRate {
            tree,
            initial_rate,
            valuator,
            time_to_maturity,
        }))
    }

    fn price_on_tree(prepared: &PreparedTree, market: &MarketContext, oas_bp: f64) -> Result<f64> {
        match prepared {
            PreparedTree::RatesCredit {
                tree,
                valuator,
                node_coupons,
                time_to_maturity,
            } => {
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::OAS, oas_bp);
                tree.price_with_node_coupons(
                    vars,
                    *time_to_maturity,
                    market,
                    valuator,
                    node_coupons,
                )
            }
            PreparedTree::ShortRate {
                tree,
                initial_rate,
                valuator,
                time_to_maturity,
            } => {
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::SHORT_RATE, *initial_rate);
                vars.insert(short_rate_keys::OAS, oas_bp);
                tree.price(vars, *time_to_maturity, market, valuator)
            }
        }
    }

    /// Price a callable term loan using tree-based backward induction.
    ///
    /// Uses `quoted_oas` (decimal) when present; otherwise prices at a zero OAS.
    ///
    /// # Arguments
    ///
    /// * `loan` - Callable or credit-risky term loan to value.
    /// * `market` - Discount curve and, when `credit_curve_id` is set, hazard curve.
    /// * `as_of` - Trade/valuation date used to resolve settlement and the schedule.
    pub fn price_callable(
        &self,
        loan: &TermLoan,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        self.price_at_oas(loan, market, as_of, Self::quoted_oas_bp(loan))
    }

    /// Price the loan on the prepared tree at a fixed OAS in basis points.
    ///
    /// # Arguments
    ///
    /// * `loan` - Term loan to value, including any call schedule and model overrides.
    /// * `market` - Discount curve and optional hazard curve named by the loan.
    /// * `as_of` - Trade/valuation date used to resolve settlement and the schedule.
    /// * `oas_bp` - Continuously compounded option-adjusted spread in basis points
    ///   applied as a parallel shift to the calibrated short-rate lattice.
    pub(crate) fn price_at_oas(
        &self,
        loan: &TermLoan,
        market: &MarketContext,
        as_of: Date,
        oas_bp: f64,
    ) -> Result<Money> {
        if !oas_bp.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "TermLoan '{}' OAS must be finite, got {oas_bp}",
                loan.id
            )));
        }
        match self.prepare(loan, market, as_of)? {
            None => Ok(Money::from((0_i64, loan.currency))),
            Some(prepared) => Ok(Money::new(
                Self::price_on_tree(&prepared, market, oas_bp)?,
                loan.currency,
            )?),
        }
    }

    /// Calculate OAS (in bp) for a callable term loan given a market clean price (% of par).
    ///
    /// Mirrors bond OAS: solves for the constant spread that matches market dirty price.
    ///
    /// # OAS Convention
    ///
    /// OAS is a **parallel shift to the calibrated risk-free short rate lattice**.
    /// When the rates+credit two-factor tree is used (hazard curve present), the
    /// hazard tree captures credit spread independently, so OAS represents the
    /// option-adjusted spread **over the risk-free curve** — consistent with
    /// Bloomberg OAS convention.
    ///
    /// # Arguments
    ///
    /// * `loan` - Term loan whose tree model is calibrated once and then shifted.
    /// * `market` - Discount curve and optional hazard curve used for that model.
    /// * `as_of` - Trade/valuation date used to convert the clean quote to settlement dirty.
    /// * `clean_price_pct_of_par` - Market clean price as a percent of funded outstanding.
    pub fn calculate_oas(
        &self,
        loan: &TermLoan,
        market: &MarketContext,
        as_of: Date,
        clean_price_pct_of_par: f64,
    ) -> Result<f64> {
        let Some(prepared) = self.prepare(loan, market, as_of)? else {
            return Ok(0.0);
        };

        let quote_schedule =
            super::discounting::TermLoanDiscountingPricer::pricing_schedule(loan, market, as_of)?;
        let dirty_target = crate::instruments::fixed_income::term_loan::metrics::irr_helpers::quoted_dirty_from_clean_px(
            loan,
            &quote_schedule,
            as_of,
            clean_price_pct_of_par,
        )?
        .amount();

        let pricing_error: std::cell::RefCell<Option<finstack_quant_core::Error>> =
            std::cell::RefCell::new(None);
        let record_error = |e: finstack_quant_core::Error| -> f64 {
            let mut slot = pricing_error.borrow_mut();
            if slot.is_none() {
                *slot = Some(e);
            }
            1.0e12
        };

        let objective_fn = |oas_bp: f64| -> f64 {
            match Self::price_on_tree(&prepared, market, oas_bp) {
                Ok(model_price) => model_price - dirty_target,
                Err(e) => record_error(e),
            }
        };

        let cfg = self.resolved_config(loan);
        let mut solver = BrentSolver::new()
            .tolerance(cfg.tolerance)
            .initial_bracket_size(cfg.initial_bracket_size_bp);
        solver.max_iterations = cfg.max_iterations;
        solver
            .solve(objective_fn, 0.0)
            .map_err(|e| match pricing_error.borrow_mut().take() {
                Some(tree_err) => finstack_quant_core::Error::Validation(format!(
                    "TermLoan OAS tree solve failed: {e}; first underlying \
                     tree-pricing error: {tree_err}"
                )),
                None => e,
            })
    }
}

impl Pricer for TermLoanTreePricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::TermLoan, ModelKey::Tree)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let loan = instrument
            .as_any()
            .downcast_ref::<TermLoan>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::TermLoan, instrument.key())
            })?;

        let pv = self.price_callable(loan, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(loan.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;

    use finstack_quant_core::money::Money;
    use finstack_quant_models::{state_keys, NodeState};
    use time::macros::date;

    fn dummy_loan() -> TermLoan {
        let mut loan = TermLoan::example().expect("example loan");
        loan.notional_limit = Money::from((100_i64, Currency::USD));
        loan
    }

    fn node_test_valuator(
        coupon_fee: f64,
        principal: [f64; 2],
        call: [Option<f64>; 2],
        outstanding: f64,
        recovery_outstanding: f64,
        recovery_rate: Option<f64>,
        call_friction_cents: f64,
    ) -> TermLoanValuator {
        let loan = dummy_loan();
        TermLoanValuator {
            loan,
            coupon_fee_vec: vec![coupon_fee, coupon_fee],
            coupon_fee_components: vec![vec![(coupon_fee, 0.0)]; 2],
            principal_vec: principal.to_vec(),
            principal_components: principal
                .iter()
                .map(|amount| vec![(*amount, 0.0)])
                .collect(),
            call_vec: call.to_vec(),
            call_time_offset_vec: call.iter().map(|price| price.map(|_| 0.0)).collect(),
            call_outstanding_vec: vec![Some(outstanding), None],
            outstanding_vec: vec![outstanding, 0.0],
            recovery_outstanding_vec: vec![recovery_outstanding, 0.0],
            recovery_rate,
            call_friction_cents,
            time_steps: vec![0.0, 1.0],
            as_of: date!(2025 - 01 - 01),
            origin: date!(2025 - 01 - 01),
        }
    }

    #[test]
    fn step_dates_follow_the_tree_day_count_axis() {
        let origin = date!(2025 - 01 - 31);
        let maturity = date!(2025 - 03 - 31);
        let day_count = DayCount::Thirty360;
        let time_to_maturity = day_count
            .year_fraction(origin, maturity, DayCountContext::default())
            .expect("maturity time");
        let time_steps = [0.0, time_to_maturity / 2.0, time_to_maturity];

        let dates =
            dates_on_time_grid(origin, maturity, day_count, &time_steps).expect("step dates");

        assert_eq!(dates, [origin, date!(2025 - 03 - 01), maturity]);
    }

    #[test]
    fn outstanding_grid_keeps_pre_and_post_payment_balances() {
        let time_steps = [0.0, 0.5, 1.0];
        let outstanding_events = [(0.5, 80.0), (1.0, 0.0)];

        let (call_outstanding, recovery_outstanding) =
            outstanding_on_time_grid(100.0, &outstanding_events, &time_steps);

        assert_eq!(call_outstanding, [100.0, 100.0, 80.0]);
        assert_eq!(recovery_outstanding, [100.0, 80.0, 0.0]);
    }

    #[test]
    fn risky_hold_weights_continuation_not_current_coupon() {
        let valuator =
            node_test_valuator(5.0, [0.0, 100.0], [None; 2], 100.0, 100.0, Some(0.4), 0.0);
        let market = MarketContext::new();
        let rate = 0.03_f64;
        let hazard = 0.10_f64;
        let dt = 1.0_f64;
        let interval_df = (-rate * dt).exp();
        let mut vars = HashMap::default();
        vars.insert(state_keys::INTEREST_RATE, rate);
        vars.insert(state_keys::HAZARD_RATE, hazard);
        vars.insert(state_keys::DF, interval_df);
        let state = NodeState::new(0, 0.0, &vars, &market);
        let discounted_continuation = 100.0 * interval_df;

        let actual = valuator
            .value_at_node(&state, discounted_continuation, dt)
            .expect("node value");
        let survival = (-hazard * dt).exp();
        let recovery_weight =
            continuous_frp_weight(interval_df, survival).expect("recovery weight");
        let expected = 5.0 + survival * discounted_continuation + 0.4 * 100.0 * recovery_weight;
        assert!(
            (actual - expected).abs() < 1e-12,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn recovery_uses_post_payment_outstanding() {
        let valuator =
            node_test_valuator(5.0, [20.0, 80.0], [None; 2], 100.0, 80.0, Some(0.4), 0.0);
        let market = MarketContext::new();
        let rate = 0.03_f64;
        let hazard = 0.10_f64;
        let dt = 1.0_f64;
        let interval_df = (-rate * dt).exp();
        let mut vars = HashMap::default();
        vars.insert(state_keys::INTEREST_RATE, rate);
        vars.insert(state_keys::HAZARD_RATE, hazard);
        vars.insert(state_keys::DF, interval_df);
        let state = NodeState::new(0, 0.0, &vars, &market);
        let discounted_continuation = 80.0 * interval_df;

        let actual = valuator
            .value_at_node(&state, discounted_continuation, dt)
            .expect("node value");
        let survival = (-hazard * dt).exp();
        let recovery_weight =
            continuous_frp_weight(interval_df, survival).expect("recovery weight");
        let expected =
            5.0 + 20.0 + survival * discounted_continuation + 0.4 * 80.0 * recovery_weight;
        assert!(
            (actual - expected).abs() < 1e-12,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn immediate_call_is_not_survival_weighted() {
        let valuator = node_test_valuator(
            5.0,
            [0.0, 100.0],
            [Some(101.0), None],
            100.0,
            100.0,
            Some(0.4),
            0.0,
        );
        let market = MarketContext::new();
        let mut vars = HashMap::default();
        vars.insert(state_keys::INTEREST_RATE, 0.0);
        vars.insert(state_keys::HAZARD_RATE, 1.0);
        vars.insert(state_keys::DF, 1.0);
        let state = NodeState::new(0, 0.0, &vars, &market);

        let actual = valuator
            .value_at_node(&state, 1_000.0, 1.0)
            .expect("node value");
        assert_eq!(actual, 106.0);
    }
}
