//! Tree-based pricing engine for callable term loans.
//!
//! This module provides market-style optionality pricing for term loans with borrower
//! call schedules, using backward induction on a tree and a frictional exercise rule.
//!
//! Design goals:
//! - Use the shared tree framework (`TreeModel` + `TreeValuator`)
//! - Apply `InstrumentPricingOverrides::call_friction_cents` as an exercise threshold uplift
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

use crate::instruments::common_impl::pricing::rates_credit::build_rates_credit_targets;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::term_loan::TermLoan;
use crate::instruments::pricing_overrides::resolve_rates_credit_config;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use finstack_quant_models::trees::two_factor_rates_credit::RatesCreditTree;
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

/// Term loan valuator for tree-based callable pricing.
///
/// Implements `TreeValuator` by mapping dated loan cashflows and call schedules into
/// step-indexed vectors and applying borrower call exercise with friction costs.
struct TermLoanValuator {
    loan: TermLoan,
    /// Coupon + fee cashflows by step (paid regardless of call decision).
    coupon_fee_vec: Vec<f64>,
    /// Scheduled principal cashflows by step (only received if not called).
    principal_vec: Vec<f64>,
    /// Call redemption by step (principal-only, based on pre-exercise outstanding).
    call_vec: Vec<Option<f64>>,
    /// Outstanding principal (pre-exercise) corresponding to `call_vec` steps.
    ///
    /// This is used to compute exercise friction consistently with the call redemption.
    call_outstanding_vec: Vec<Option<f64>>,
    /// Outstanding principal at start of step (used for friction and recovery).
    outstanding_vec: Vec<f64>,
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
        let mut principal_vec = vec![0.0; num_steps];

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
        let book = |vec: &mut Vec<f64>, amount: f64, t: f64, raw_clamped: f64, snap: bool| {
            if snap {
                let step = (raw_clamped.ceil() as usize).clamp(0, num_steps - 1);
                vec[step] += value_at_step_time(amount, t, time_steps[step]);
            } else {
                let lo = raw_clamped.floor() as usize;
                let weight = raw_clamped - lo as f64;
                if lo < num_steps {
                    vec[lo] += value_at_step_time(amount * (1.0 - weight), t, time_steps[lo]);
                }
                if lo + 1 < num_steps {
                    vec[lo + 1] += value_at_step_time(amount * weight, t, time_steps[lo + 1]);
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
            let t = dc_curve.year_fraction(
                origin,
                cf.date,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
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
                        cf.amount.amount(),
                        t,
                        raw_clamped,
                        is_exercise,
                    );
                }
                _ => {}
            }
        }

        // Outstanding principal by step: use the last outstanding level strictly before the
        // calendar date implied by the step time. We approximate by mapping event times.
        let mut outstanding_events: Vec<(f64, f64)> = out_path
            .iter()
            .filter(|(d, _)| *d >= origin && *d <= loan.maturity)
            .filter_map(|(d, amt)| {
                dc_curve
                    .year_fraction(
                        origin,
                        *d,
                        finstack_quant_core::dates::DayCountContext::default(),
                    )
                    .ok()
                    .map(|t| (t, amt.amount()))
            })
            .collect();
        outstanding_events
            .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut outstanding_vec = vec![0.0; num_steps];
        let mut last_out = outstanding_before(origin);
        let mut ev_idx = 0usize;
        for step in 0..num_steps {
            let st = time_steps[step];
            while ev_idx < outstanding_events.len() && outstanding_events[ev_idx].0 < st {
                last_out = outstanding_events[ev_idx].1;
                ev_idx += 1;
            }
            outstanding_vec[step] = last_out.max(0.0);
        }

        // Call redemption vector (pre-exercise outstanding × call price).
        //
        // Call type semantics:
        // - Hard/Soft: borrower exercises at `price_pct_of_par` × outstanding.
        //   Soft calls behave identically to Hard in pricing; the premium is
        //   already captured in `price_pct_of_par`.
        // - MakeWhole: borrower pays PV of remaining flows at Treasury + spread,
        //   which by design equals or exceeds the continuation value. The option
        //   is therefore non-economic and skipped in the tree to avoid mispricing.
        let mut call_vec: Vec<Option<f64>> = vec![None; num_steps];
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
                    let t = dc_curve.year_fraction(
                        origin,
                        call.date,
                        finstack_quant_core::dates::DayCountContext::default(),
                    )?;
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
                call_vec[step] = Some(out * (call.price_pct_of_par / 100.0));
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
            principal_vec,
            call_vec,
            call_outstanding_vec,
            outstanding_vec,
            recovery_rate,
            call_friction_cents,
            time_steps,
            as_of,
            origin,
        })
    }

    /// Build node-coupon descriptors for future floating resets and reject
    /// schedules the stochastic-rate lattice cannot represent.
    ///
    /// Called by the engine **only** on the rates-credit path with a
    /// positive short-rate volatility; deterministic-rate pricing never
    /// invokes it. The deterministic projection of every coupon stays in
    /// `coupon_fee_vec`; descriptors carry only the node-dependent
    /// increment.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when a future
    /// floating coupon's reset and payment collapse onto one tree slice
    /// (grid too coarse) or the schedule capitalizes (PIK) after the
    /// settlement origin — a future floating PIK coupon makes outstanding
    /// principal path-dependent.
    fn stochastic_node_coupons(
        &self,
        market: &MarketContext,
    ) -> Result<Vec<finstack_quant_models::trees::two_factor_rates_credit::NodeCoupon>> {
        use crate::instruments::common_impl::pricing::floating_reset_descriptors::{
            build_node_coupons, has_future_pik, params_from_spec, strips_index_constraints,
            NodeCouponBuildInputs,
        };
        use crate::instruments::fixed_income::term_loan::RateSpec;

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
    fn restrict_exercise_to_reset_boundaries(
        &mut self,
        coupons: &[finstack_quant_models::trees::two_factor_rates_credit::NodeCoupon],
    ) {
        if coupons.is_empty() {
            return;
        }
        for step in 0..self.call_vec.len() {
            let interior = coupons
                .iter()
                .any(|c| step > c.reset_step && step < c.payment_step);
            if interior {
                self.call_vec[step] = None;
                self.call_outstanding_vec[step] = None;
            }
        }
    }

    #[inline]
    fn coupon_fee_at(&self, step: usize) -> f64 {
        self.coupon_fee_vec.get(step).copied().unwrap_or(0.0)
    }

    #[inline]
    fn principal_cf_at(&self, step: usize) -> f64 {
        self.principal_vec.get(step).copied().unwrap_or(0.0)
    }

    #[inline]
    fn call_at(&self, step: usize) -> Option<f64> {
        self.call_vec.get(step).copied().flatten()
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
}

impl TreeValuator for TermLoanValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        let step = state.step;
        // At maturity, scheduled principal repayment is already in principal_vec.
        Ok(self.coupon_fee_at(step) + self.principal_cf_at(step))
    }

    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64> {
        let step = state.step;

        let coupon_fee = self.coupon_fee_at(step);
        let principal_cf = self.principal_cf_at(step);

        // Baseline (no call): receive scheduled principal cashflow then continue.
        let mut principal_value = continuation_value + principal_cf;

        // Borrower call: borrower can redeem at call price if continuation sufficiently high,
        // subject to friction threshold.
        if let Some(call_price) = self.call_at(step) {
            let outstanding = self
                .call_outstanding_at(step)
                .unwrap_or_else(|| self.outstanding_at(step));
            let friction_amount = outstanding * (self.call_friction_cents / 10_000.0);
            let threshold = call_price + friction_amount;
            if principal_value > threshold {
                // If called, redemption replaces scheduled principal cashflow on this date.
                principal_value = call_price;
            }
        }

        let alive_value = coupon_fee + principal_value;

        // Default handling when hazard rate is provided by the tree state.
        //
        // Recovery convention: recovery is received at the *current* node upon
        // default (standard Hull/Brigo-Mercurio convention). No additional one-
        // period discounting is applied to recovery — `alive_value` and `recovery`
        // are both in PV-at-this-node terms.
        if let Some(hazard) = state.hazard_rate {
            let p_surv = (-hazard.max(0.0) * dt).exp();
            let default_prob = (1.0 - p_surv).clamp(0.0, 1.0);
            let outstanding = self.outstanding_at(step);
            let recovery = self
                .recovery_rate
                .map(|rr| rr.clamp(0.0, 1.0) * outstanding)
                .unwrap_or(0.0);
            Ok(p_surv * alive_value + default_prob * recovery)
        } else {
            Ok(alive_value)
        }
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

    /// Price a callable term loan using tree-based backward induction.
    pub fn price_callable(
        &self,
        loan: &TermLoan,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        let origin = loan.settlement_date(as_of)?;
        if origin >= loan.maturity {
            return Ok(Money::new(0.0, loan.currency));
        }

        let disc = market.get_discount(&loan.discount_curve_id)?;
        let dc_curve = disc.day_count();
        let time_to_maturity = dc_curve.year_fraction(
            origin,
            loan.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if time_to_maturity <= 0.0 {
            return Ok(Money::new(0.0, loan.currency));
        }

        let cfg = TermLoanTreePricerConfig {
            tree_steps: loan
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(self.config.tree_steps),
            rate_volatility: loan
                .instrument_pricing_overrides
                .model_config
                .hw1f_sigma
                .unwrap_or(self.config.rate_volatility),
            tolerance: self.config.tolerance,
            max_iterations: self.config.max_iterations,
            initial_bracket_size_bp: self.config.initial_bracket_size_bp,
        };
        let steps = cfg.tree_steps;
        let rate_volatility = cfg.rate_volatility;

        // Choose model: if hazard curve is available, use the rates+credit tree; otherwise short-rate.
        // Precedence mirrors TermLoan's `credit_curve_id` semantics.
        let hazard_curve = loan
            .credit_curve_id
            .as_ref()
            .map(|id| market.get_hazard(id.as_str()))
            .transpose()?;

        let mut valuator =
            TermLoanValuator::new(loan.clone(), market, as_of, origin, time_to_maturity, steps)?;

        let price_amount = if let Some(hc) = hazard_curve.as_ref() {
            // Rates-credit lattice: both factor volatilities come from the
            // shared resolver, so the regime is whatever the instrument's
            // ModelConfig declares rather than a hard-coded pair.
            let cfg = resolve_rates_credit_config(&loan.instrument_pricing_overrides, steps)?;
            let mut tree = RatesCreditTree::new(cfg);
            let targets = build_rates_credit_targets(
                disc.as_ref(),
                hc.as_ref(),
                origin,
                loan.maturity,
                time_to_maturity,
                steps,
            )?;
            tree.calibrate(&targets)?;

            // Future floating resets re-fix off the rate node only when the
            // rate factor diffuses; deterministic-rate pricing keeps today's
            // projected coupons and every-step exercise unchanged.
            let node_coupons = if tree.config.rate_vol > 0.0 {
                let coupons = valuator.stochastic_node_coupons(market)?;
                valuator.restrict_exercise_to_reset_boundaries(&coupons);
                coupons
            } else {
                Vec::new()
            };

            let vars = HashMap::<&'static str, f64>::default();
            tree.price_with_node_coupons(vars, time_to_maturity, market, &valuator, &node_coupons)?
        } else {
            reject_inert_hazard_inputs(loan)?;
            // Short-rate tree calibrated to the discount curve.
            let mut tree = ShortRateTree::new(ShortRateTreeConfig {
                steps,
                volatility: rate_volatility,
                ..Default::default()
            });
            tree.calibrate(disc.as_ref(), time_to_maturity)?;

            let initial_rate = tree.rate_at_node(0, 0)?;
            let mut vars = HashMap::<&'static str, f64>::default();
            vars.insert(short_rate_keys::SHORT_RATE, initial_rate);
            vars.insert(short_rate_keys::OAS, 0.0);
            tree.price(vars, time_to_maturity, market, &valuator)?
        };

        Ok(Money::new(price_amount, loan.currency))
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
    pub fn calculate_oas(
        &self,
        loan: &TermLoan,
        market: &MarketContext,
        as_of: Date,
        clean_price_pct_of_par: f64,
    ) -> Result<f64> {
        let origin = loan.settlement_date(as_of)?;
        if origin >= loan.maturity {
            return Ok(0.0);
        }

        // Target dirty settlement amount using funded outstanding and accrued interest.
        let quote_schedule =
            super::discounting::TermLoanDiscountingPricer::pricing_schedule(loan, market, as_of)?;
        let dirty_target = crate::instruments::fixed_income::term_loan::metrics::irr_helpers::quoted_dirty_from_clean_px(
            loan,
            &quote_schedule,
            as_of,
            clean_price_pct_of_par,
        )?
        .amount();

        let disc = market.get_discount(&loan.discount_curve_id)?;
        let dc_curve = disc.day_count();
        let time_to_maturity = dc_curve.year_fraction(
            origin,
            loan.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if time_to_maturity <= 0.0 {
            return Ok(0.0);
        }

        let cfg = TermLoanTreePricerConfig {
            tree_steps: loan
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(self.config.tree_steps),
            rate_volatility: loan
                .instrument_pricing_overrides
                .model_config
                .hw1f_sigma
                .unwrap_or(self.config.rate_volatility),
            tolerance: self.config.tolerance,
            max_iterations: self.config.max_iterations,
            initial_bracket_size_bp: self.config.initial_bracket_size_bp,
        };
        let steps = cfg.tree_steps;
        let rate_volatility = cfg.rate_volatility;

        // Choose model based on hazard availability.
        let hazard_curve = loan
            .credit_curve_id
            .as_ref()
            .map(|id| market.get_hazard(id.as_str()))
            .transpose()?;

        let mut valuator =
            TermLoanValuator::new(loan.clone(), market, as_of, origin, time_to_maturity, steps)?;

        // Pre-calibrate the credit tree once (it stays fixed; OAS is passed via vars).
        let rc_tree = if let Some(hc) = hazard_curve.as_ref() {
            let cfg = resolve_rates_credit_config(&loan.instrument_pricing_overrides, steps)?;
            let mut tree = RatesCreditTree::new(cfg);
            let targets = build_rates_credit_targets(
                disc.as_ref(),
                hc.as_ref(),
                origin,
                loan.maturity,
                time_to_maturity,
                steps,
            )?;
            tree.calibrate(&targets)?;
            Some(tree)
        } else {
            None
        };

        // Node-dependent floating resets, active only when the rates-credit
        // rate factor diffuses. Descriptors are OAS-independent, so they are
        // built (and the standing call discretized to reset/payment
        // boundaries) once before the solve; the per-OAS folding happens
        // inside the tree.
        let rc_node_coupons = match rc_tree.as_ref() {
            Some(tree) if tree.config.rate_vol > 0.0 => {
                let coupons = valuator.stochastic_node_coupons(market)?;
                valuator.restrict_exercise_to_reset_boundaries(&coupons);
                coupons
            }
            _ => Vec::new(),
        };
        let valuator = valuator;

        // Pre-calibrate the short-rate tree once when the credit tree is absent.
        // OAS is passed via state variables on each Brent iteration, so the rate
        // tree itself never needs re-calibration — calibrating it once outside
        // the solver loop saves ~50 tree builds per OAS solve (matching the
        // approach already used above for the credit tree).
        let sr_tree_and_initial: Option<(ShortRateTree, f64)> = if rc_tree.is_some() {
            None
        } else {
            reject_inert_hazard_inputs(loan)?;
            let mut tree = ShortRateTree::new(ShortRateTreeConfig {
                steps,
                volatility: rate_volatility,
                ..Default::default()
            });
            tree.calibrate(disc.as_ref(), time_to_maturity)
                .map_err(|e| {
                    finstack_quant_core::Error::Validation(format!(
                        "TermLoan OAS short-rate tree calibration failed: {e}"
                    ))
                })?;
            let initial_rate = tree.rate_at_node(0, 0)?;
            Some((tree, initial_rate))
        };

        // Capture the first in-iteration tree-pricing error so a solver failure
        // reports the underlying cause instead of a bare convergence message.
        // `BrentSolver` takes an `FnMut(f64) -> f64` with no error channel, so
        // the residual has to stand in for the failure; a flat large positive
        // value is correct here because the model price falls monotonically in
        // OAS and pricing only fails in the divergent deeply-negative regime
        // where the true price tends to +infinity. A `±1e6` keyed to
        // `sign(oas)` would flip at oas = 0 and hand Brent a fabricated bracket
        // around a non-root.
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
            if let Some(tree) = rc_tree.as_ref() {
                // Calibrated credit tree: OAS as a parallel shift to calibrated rates.
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::OAS, oas_bp);
                match tree.price_with_node_coupons(
                    vars,
                    time_to_maturity,
                    market,
                    &valuator,
                    &rc_node_coupons,
                ) {
                    Ok(model_price) => model_price - dirty_target,
                    Err(e) => record_error(e),
                }
            } else if let Some((tree, initial_rate)) = sr_tree_and_initial.as_ref() {
                // Short-rate tree: pre-calibrated; OAS is a parallel shift via state.
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::SHORT_RATE, *initial_rate);
                vars.insert(short_rate_keys::OAS, oas_bp);
                match tree.price(vars, time_to_maturity, market, &valuator) {
                    Ok(model_price) => model_price - dirty_target,
                    Err(e) => record_error(e),
                }
            } else {
                record_error(finstack_quant_core::Error::internal(
                    "term-loan OAS solve invoked without a calibrated tree",
                ))
            }
        };

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
