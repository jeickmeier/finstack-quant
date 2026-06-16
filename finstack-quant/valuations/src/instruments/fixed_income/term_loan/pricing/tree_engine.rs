//! Tree-based pricing engine for callable term loans.
//!
//! This module provides market-style optionality pricing for term loans with borrower
//! call schedules, using backward induction on a tree and a frictional exercise rule.
//!
//! Design goals:
//! - Use the shared tree framework (`TreeModel` + `TreeValuator`)
//! - Support deterministic discounting (via the calibrated rate tree) and an optional
//!   credit-spread tree path
//! - Apply `PricingOverrides::call_friction_cents` as an exercise threshold uplift

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::term_loan::cashflows::generate_cashflows;
use crate::instruments::fixed_income::term_loan::TermLoan;
use crate::models::trees::two_factor_rates_credit::{RatesCreditConfig, RatesCreditTree};
use crate::models::{
    short_rate_keys, NodeState, ShortRateTree, ShortRateTreeConfig, TreeModel, TreeValuator,
};
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

/// Configuration for tree-based term loan pricing (callable PV, OAS).
#[derive(Debug, Clone)]
pub(crate) struct TermLoanTreePricerConfig {
    pub(crate) tree_steps: usize,
    pub(crate) volatility: f64,
    pub(crate) tolerance: f64,
    pub(crate) max_iterations: usize,
    pub(crate) initial_bracket_size_bp: Option<f64>,
}

impl Default for TermLoanTreePricerConfig {
    fn default() -> Self {
        Self {
            tree_steps: 100,
            volatility: 0.01,
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

        let schedule = generate_cashflows(&loan, market, as_of)?;
        let out_path = schedule.outstanding_by_date()?;

        // Helper: outstanding BEFORE a target date (pre-exercise).
        // Initialise with the loan's initial notional so that calls at or before the
        // first outstanding entry still see the correct starting balance.
        let outstanding_before = |target: Date| -> f64 {
            let mut last = loan.notional_limit.amount();
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

        for cf in &schedule.flows {
            if cf.date < origin {
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
                // Principal repayment / redemption (positive to holder only;
                // negative notionals are funding legs and excluded).
                CFKind::Amortization | CFKind::Notional => {
                    if cf.amount.amount() > 0.0 {
                        book(
                            &mut principal_vec,
                            cf.amount.amount(),
                            t,
                            raw_clamped,
                            is_exercise,
                        );
                    }
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
            for c in &cs.calls {
                if c.date < origin || c.date > loan.maturity {
                    continue;
                }
                if matches!(
                    c.call_type,
                    crate::instruments::fixed_income::term_loan::LoanCallType::MakeWhole { .. }
                ) {
                    continue;
                }
                let t = dc_curve.year_fraction(
                    origin,
                    c.date,
                    finstack_quant_core::dates::DayCountContext::default(),
                )?;
                let raw = (t / time_to_maturity) * tree_steps as f64;
                let step =
                    (raw.clamp(0.0, tree_steps as f64).ceil() as usize).clamp(0, num_steps - 1);

                let out = outstanding_before(c.date).max(0.0);
                let redemption = out * (c.price_pct_of_par / 100.0);

                // If multiple calls map to the same step, use the minimum redemption (most issuer-friendly),
                // which is conservative for the lender (lower PV).
                match call_vec[step] {
                    Some(existing) => {
                        if redemption < existing {
                            call_vec[step] = Some(redemption);
                            call_outstanding_vec[step] = Some(out);
                        }
                    }
                    None => {
                        call_vec[step] = Some(redemption);
                        call_outstanding_vec[step] = Some(out);
                    }
                }
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
            .pricing_overrides
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
        })
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
                .pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(self.config.tree_steps),
            volatility: loan
                .pricing_overrides
                .market_quotes
                .implied_volatility
                .unwrap_or(self.config.volatility),
            tolerance: self.config.tolerance,
            max_iterations: self.config.max_iterations,
            initial_bracket_size_bp: self.config.initial_bracket_size_bp,
        };
        let steps = cfg.tree_steps;
        let vol = cfg.volatility;

        // Choose model: if hazard curve is available, use the rates+credit tree; otherwise short-rate.
        // Precedence mirrors TermLoan's `credit_curve_id` semantics.
        let hazard_curve = if let Some(ref id) = loan.credit_curve_id {
            market.get_hazard(id.as_str()).ok()
        } else {
            market
                .get_hazard(loan.discount_curve_id.as_str())
                .ok()
                .or_else(|| {
                    market
                        .get_hazard(format!("{}-CREDIT", loan.discount_curve_id.as_str()))
                        .ok()
                })
        };

        let valuator =
            TermLoanValuator::new(loan.clone(), market, as_of, origin, time_to_maturity, steps)?;

        let price_amount = if let Some(hc) = hazard_curve.as_ref() {
            // Credit-spread tree path: calibrated to discount + hazard curves.
            // Rate vol 0 → deterministic forward rates; stochastic hazard.
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps,
                rate_vol: 0.0,
                hazard_vol: vol,
                ..Default::default()
            });
            tree.calibrate(disc.as_ref(), hc.as_ref(), time_to_maturity)?;

            let vars = HashMap::<&'static str, f64>::default();
            tree.price(vars, time_to_maturity, market, &valuator)?
        } else {
            // Short-rate tree calibrated to the discount curve.
            let mut tree = ShortRateTree::new(ShortRateTreeConfig {
                steps,
                volatility: vol,
                ..Default::default()
            });
            tree.calibrate(&loan.discount_curve_id, disc.as_ref(), time_to_maturity)?;

            let initial_rate = tree.rate_at_node(0, 0).unwrap_or_else(|_| disc.zero(0.0));
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

        // Target dirty price in currency.
        let notional = loan.notional_limit.amount();
        let dirty_target = clean_price_pct_of_par * notional / 100.0;

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
                .pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(self.config.tree_steps),
            volatility: loan
                .pricing_overrides
                .market_quotes
                .implied_volatility
                .unwrap_or(self.config.volatility),
            tolerance: self.config.tolerance,
            max_iterations: self.config.max_iterations,
            initial_bracket_size_bp: self.config.initial_bracket_size_bp,
        };
        let steps = cfg.tree_steps;
        let vol = cfg.volatility;

        // Choose model based on hazard availability.
        let hazard_curve = if let Some(ref id) = loan.credit_curve_id {
            market.get_hazard(id.as_str()).ok()
        } else {
            market
                .get_hazard(loan.discount_curve_id.as_str())
                .ok()
                .or_else(|| {
                    market
                        .get_hazard(format!("{}-CREDIT", loan.discount_curve_id.as_str()))
                        .ok()
                })
        };

        // Build valuator once.
        let valuator =
            TermLoanValuator::new(loan.clone(), market, as_of, origin, time_to_maturity, steps)?;

        // Pre-calibrate the credit tree once (it stays fixed; OAS is passed via vars).
        let rc_tree = if let Some(hc) = hazard_curve.as_ref() {
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps,
                rate_vol: 0.0,
                hazard_vol: vol,
                ..Default::default()
            });
            if tree
                .calibrate(disc.as_ref(), hc.as_ref(), time_to_maturity)
                .is_err()
            {
                None
            } else {
                Some(tree)
            }
        } else {
            None
        };

        // Pre-calibrate the short-rate tree once when the credit tree is absent.
        // OAS is passed via state variables on each Brent iteration, so the rate
        // tree itself never needs re-calibration — calibrating it once outside
        // the solver loop saves ~50 tree builds per OAS solve (matching the
        // approach already used above for the credit tree).
        let sr_tree_and_initial: Option<(ShortRateTree, f64)> = if rc_tree.is_some() {
            None
        } else {
            let mut tree = ShortRateTree::new(ShortRateTreeConfig {
                steps,
                volatility: vol,
                ..Default::default()
            });
            tree.calibrate(&loan.discount_curve_id, disc.as_ref(), time_to_maturity)
                .map_err(|e| {
                    finstack_quant_core::Error::Validation(format!(
                        "TermLoan OAS short-rate tree calibration failed: {e}"
                    ))
                })?;
            let initial_rate = tree.rate_at_node(0, 0).unwrap_or_else(|_| disc.zero(0.0));
            Some((tree, initial_rate))
        };

        let objective_fn = |oas_bp: f64| -> f64 {
            if let Some(tree) = rc_tree.as_ref() {
                // Calibrated credit tree: OAS as a parallel shift to calibrated rates.
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert("oas", oas_bp);
                match tree.price(vars, time_to_maturity, market, &valuator) {
                    Ok(model_price) => model_price - dirty_target,
                    Err(_) => 1.0e6,
                }
            } else if let Some((tree, initial_rate)) = sr_tree_and_initial.as_ref() {
                // Short-rate tree: pre-calibrated; OAS is a parallel shift via state.
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::SHORT_RATE, *initial_rate);
                vars.insert(short_rate_keys::OAS, oas_bp);
                match tree.price(vars, time_to_maturity, market, &valuator) {
                    Ok(model_price) => model_price - dirty_target,
                    Err(_) => {
                        if oas_bp > 0.0 {
                            1.0e6
                        } else {
                            -1.0e6
                        }
                    }
                }
            } else {
                // Unreachable by construction: if rc_tree is None we always
                // populate sr_tree_and_initial above. Returning a large residual
                // keeps the solver well-behaved if this contract is ever broken.
                1.0e6
            }
        };

        let mut solver = BrentSolver::new()
            .tolerance(cfg.tolerance)
            .initial_bracket_size(cfg.initial_bracket_size_bp);
        solver.max_iterations = cfg.max_iterations;
        solver.solve(objective_fn, 0.0)
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
