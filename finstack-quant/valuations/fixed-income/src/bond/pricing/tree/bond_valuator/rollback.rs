//! Backward-induction rollback, exercise decisions, and deterministic/tree pricing passes.

use super::BondValuator;
use crate::rates_credit::continuous_frp_weight;
use finstack_quant_core::Result;
use finstack_quant_core::math::Compounding;
use finstack_quant_models::trees::hull_white_tree::HullWhiteTree;
use finstack_quant_models::trees::kink_smoothing::{smoothed_max, smoothed_min};
use finstack_quant_models::trees::tree_framework::{
    CallCap, NodeClaim, NodeState, TreeDiscounting, TreeValuator,
};
use finstack_quant_models::trees::two_factor_rates_credit::RatesCreditTree;

impl BondValuator {
    /// Apply holder put and issuer call rights to a value that excludes current
    /// contractual payments. Call friction raises only the issuer's exercise
    /// threshold; it never changes holder proceeds.
    #[inline]
    pub(super) fn apply_exercise(&self, step: usize, risky_continuation: f64) -> f64 {
        let mut exercised_or_held = risky_continuation;
        if let Some(put_price) = self.put_at(step) {
            exercised_or_held = exercised_or_held.max(put_price);
        }
        if let Some(call_price) = self.call_at(step) {
            let outstanding = self.outstanding_principal_at(step);
            let friction_amount = outstanding * (self.call_friction_cents / 10_000.0);
            if exercised_or_held > call_price + friction_amount {
                exercised_or_held = call_price;
            }
        }
        exercised_or_held
    }

    /// Apply holder put and issuer call rights across one lattice level whose
    /// values exclude current contractual payments.
    ///
    /// This is the level-wide form of [`Self::apply_exercise`]: the put floor
    /// and a frictionless call cap are kink-smoothed across neighbouring
    /// nodes, so the model price does not ripple with the rate level at the
    /// node spacing. A call with friction is a jump in value at the exercise
    /// threshold rather than a kink, and keeps the node-by-node rule.
    fn apply_exercise_level(&self, step: usize, values: &mut [f64], scratch: &mut Vec<f64>) {
        if let Some(put_price) = self.put_at(step) {
            scratch.clear();
            scratch.resize(values.len(), put_price);
            smoothed_max(values, scratch);
        }
        if let Some(call_price) = self.call_at(step) {
            let friction_amount =
                self.outstanding_principal_at(step) * (self.call_friction_cents / 10_000.0);
            if friction_amount == 0.0 {
                scratch.clear();
                scratch.resize(values.len(), call_price);
                smoothed_min(values, scratch);
            } else {
                for value in values.iter_mut() {
                    if *value > call_price + friction_amount {
                        *value = call_price;
                    }
                }
            }
        }
    }

    /// Terminal contractual payment with maturity exercise applied to the
    /// redemption component only. Coupons and scheduled amortization remain
    /// payable; the final notional repayment is the hold alternative.
    #[inline]
    fn terminal_value(&self, step: usize, oas_rate: f64) -> f64 {
        let current_payments = self.cashflow_at_oas(step, oas_rate);
        let contractual_redemption = self.redemption_at_oas(step, oas_rate);
        let other_payments = current_payments - contractual_redemption;
        other_payments + self.apply_exercise(step, contractual_redemption)
    }

    /// Price the bond using a calibrated Hull-White trinomial tree with OAS.
    ///
    /// Uses `HullWhiteTree::backward_induction` with the bond's cashflow and
    /// call/put schedules applied at each node. The OAS is added to each
    /// node's short rate on `oas_compounding` when discounting.
    ///
    /// # Arguments
    ///
    /// * `hw_tree` - Calibrated Hull-White tree
    /// * `oas_bp` - Option-adjusted spread in basis points
    ///
    /// # Returns
    ///
    /// Model dirty price of the bond.
    ///
    /// # Errors
    ///
    /// Propagates tree backward-induction validation failures.
    pub(crate) fn price_with_hw_tree(
        &self,
        hw_tree: &HullWhiteTree,
        oas_bp: f64,
        oas_compounding: Compounding,
    ) -> Result<f64> {
        let final_step = hw_tree.num_steps();
        let comp = hw_tree.config().compounding;
        let oas_rate = oas_bp / 10_000.0;

        if oas_compounding == Compounding::Continuous {
            let terminal_cf = self.terminal_value(final_step, oas_rate);
            let terminal_values = vec![terminal_cf; hw_tree.num_nodes(final_step)];
            let mut scratch = Vec::with_capacity(hw_tree.num_nodes(final_step));

            return hw_tree.backward_induction(&terminal_values, |step, values| {
                // The HW tree's backward_induction already discounts by the
                // short rate r(step, node). Apply the OAS as additional
                // discounting over this step's (possibly non-uniform) interval.
                let oas_df = comp.tree_df(oas_rate, hw_tree.dt_at_step(step));
                for value in values.iter_mut() {
                    *value *= oas_df;
                }
                self.apply_exercise_level(step, values, &mut scratch);
                let cashflow = self.cashflow_at_oas(step, oas_rate);
                for value in values.iter_mut() {
                    *value += cashflow;
                }
            });
        }

        // A periodic OAS is added to each node's short rate on its own
        // compounding, so the continuous shift differs node by node.
        let mut node_shifts = Vec::with_capacity(final_step + 1);
        for step in 0..=final_step {
            let dt = hw_tree.dt_at_step(step.min(final_step.saturating_sub(1)));
            let num_nodes = hw_tree.num_nodes(step);
            let mut row = Vec::with_capacity(num_nodes);
            for node_idx in 0..num_nodes {
                let continuous = comp.tree_to_continuous(hw_tree.rate_at_node(step, node_idx), dt);
                row.push(oas_compounding.continuous_spread_shift(continuous, oas_rate)?);
            }
            node_shifts.push(row);
        }
        let terminal_values: Vec<f64> = (0..hw_tree.num_nodes(final_step))
            .map(|node_idx| self.terminal_value(final_step, node_shifts[final_step][node_idx]))
            .collect();
        let mut scratch = Vec::with_capacity(hw_tree.num_nodes(final_step));
        hw_tree.backward_induction(&terminal_values, |step, values| {
            let dt = hw_tree.dt_at_step(step);
            for (value, shift) in values.iter_mut().zip(&node_shifts[step]) {
                *value *= (-shift * dt).exp();
            }
            self.apply_exercise_level(step, values, &mut scratch);
            for (value, shift) in values.iter_mut().zip(&node_shifts[step]) {
                *value += self.cashflow_at_oas(step, *shift);
            }
        })
    }

    /// Price a calibrated rates-credit tree whose rate and hazard factors are
    /// both deterministic. The collapsed lattice has one economically unique
    /// state per slice, so a scalar reverse pass avoids the cubic joint-node
    /// rollback required for stochastic factors while preserving the same
    /// risky-continuation, FRP recovery, exercise, and payment ordering.
    ///
    /// `oas_bp` is a spread in basis points on `oas_compounding`, added to
    /// each interval's short rate on that basis.
    pub(crate) fn price_deterministic_rates_credit(
        &self,
        tree: &RatesCreditTree,
        oas_bp: f64,
        oas_compounding: Compounding,
    ) -> Result<f64> {
        if tree.get_config().rate_vol != 0.0 || tree.get_config().hazard_vol != 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "deterministic rates-credit bond rollback requires zero rate_vol and hazard_vol, got {} and {}",
                tree.get_config().rate_vol,
                tree.get_config().hazard_vol
            )));
        }
        if !oas_bp.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "deterministic rates-credit bond OAS must be finite, got {oas_bp}"
            )));
        }

        let tree_times = tree.time_grid()?;
        if tree_times.len() != self.time_steps.len()
            || tree_times
                .iter()
                .zip(&self.time_steps)
                .any(|(tree_time, value_time)| {
                    let tolerance = 1.0e-12_f64.max(tree_time.abs() * 1.0e-10);
                    (tree_time - value_time).abs() > tolerance
                })
        {
            return Err(finstack_quant_core::Error::Validation(
                "deterministic rates-credit tree grid does not match BondValuator grid".to_string(),
            ));
        }

        let last = tree_times.len() - 1;
        let oas_rate = oas_bp / 10_000.0;
        let recovery_rate = tree.recovery_rate().clamp(0.0, 1.0);
        let terminal_rate = if last > 0 {
            tree.rate_at_node(last - 1, 0)?
        } else {
            0.0
        };
        let mut value = self.terminal_value(
            last,
            oas_compounding.continuous_spread_shift(terminal_rate, oas_rate)?,
        );

        for step in (0..last).rev() {
            let dt = tree_times[step + 1] - tree_times[step];
            let short_rate = tree.rate_at_node(step, 0)?;
            let oas_shift = oas_compounding.continuous_spread_shift(short_rate, oas_rate)?;
            let hazard = tree.hazard_at_node(step, 0)?.max(0.0);
            let interval_discount = (-(short_rate + oas_shift) * dt).exp();
            let survival = (-hazard * dt).exp();
            let recovery_weight = continuous_frp_weight(interval_discount, survival)?;
            let recovery = recovery_rate * self.outstanding_principal_at(step) * recovery_weight;
            let risky_continuation = survival * interval_discount * value + recovery;
            value = self.cashflow_at_oas(step, oas_shift)
                + self.apply_exercise(step, risky_continuation);
        }

        Ok(value)
    }

    /// Price embedded options against the deterministic discount curve used to
    /// build this valuator. This is the zero-short-rate-volatility limit of the
    /// risk-free tree and avoids inventing volatility merely to satisfy a
    /// stochastic tree constructor.
    ///
    /// `oas_bp` is a spread in basis points on `oas_compounding`, added to
    /// each step's forward rate on that basis.
    pub(crate) fn price_deterministic_discount_curve(
        &self,
        oas_bp: f64,
        oas_compounding: Compounding,
    ) -> Result<f64> {
        if !oas_bp.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "deterministic bond OAS must be finite, got {oas_bp}"
            )));
        }
        if self.step_discount_factors.len() != self.time_steps.len() {
            return Err(finstack_quant_core::Error::Internal(
                "BondValuator discount-factor and time grids differ in length".into(),
            ));
        }

        let last = self.time_steps.len() - 1;
        let oas_rate = oas_bp / 10_000.0;
        // Continuous OAS shift over the step starting at `step`: the spread is
        // added to that step's forward rate on the quote compounding.
        let step_shift = |step: usize| -> Result<(f64, f64, f64)> {
            let current_df = self.step_discount_factors[step];
            let next_df = self.step_discount_factors[step + 1];
            if !current_df.is_finite()
                || !next_df.is_finite()
                || current_df <= 0.0
                || next_df <= 0.0
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "BondValuator deterministic discount factors must be positive and finite at step {step}"
                )));
            }
            let dt = self.time_steps[step + 1] - self.time_steps[step];
            let step_df = next_df / current_df;
            let shift = if oas_compounding == Compounding::Continuous || dt <= 0.0 {
                oas_rate
            } else {
                oas_compounding.continuous_spread_shift(-step_df.ln() / dt, oas_rate)?
            };
            Ok((step_df, dt, shift))
        };
        let terminal_shift = if last > 0 {
            step_shift(last - 1)?.2
        } else {
            oas_compounding.continuous_spread_shift(0.0, oas_rate)?
        };
        let mut value = self.terminal_value(last, terminal_shift);
        for step in (0..last).rev() {
            let (step_df, dt, shift) = step_shift(step)?;
            let interval_df = step_df * (-shift * dt).exp();
            value =
                self.cashflow_at_oas(step, shift) + self.apply_exercise(step, interval_df * value);
        }
        Ok(value)
    }
}

impl TreeValuator for BondValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        let final_step = self.time_steps.len() - 1;
        let oas_rate = state.oas_bp / 10_000.0;
        Ok(self.terminal_value(final_step, oas_rate))
    }

    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64> {
        Ok(self.claim_at_node(state, continuation_value, dt)?.value())
    }

    fn claim_at_node(
        &self,
        state: &NodeState,
        continuation_value: f64,
        dt: f64,
    ) -> Result<NodeClaim> {
        let step = state.step;
        let oas_rate = state.oas_bp / 10_000.0;
        // The tree has already rate/OAS-discounted `continuation_value` from
        // the child slice to this node. Apply only survival to that value, and
        // add continuously paid FRP recovery with the same interval discount.
        // Current contractual payments are at this node, so they are neither
        // survival weighted over the next interval nor included in recovery.
        let risky_continuation = if let Some(hazard) = state.hazard_rate {
            let hazard = hazard.max(0.0);
            let survival = (-hazard * dt).exp();
            let interval_df = state.df.unwrap_or_else(|| {
                let rate = state.interest_rate.unwrap_or(0.0) + oas_rate;
                (-rate * dt).exp()
            });
            let recovery_weight = continuous_frp_weight(interval_df, survival)?;
            let outstanding = self.outstanding_principal_at(step);
            let recovery = self
                .recovery_rate
                .map(|rr| rr.clamp(0.0, 1.0) * outstanding * recovery_weight)
                .unwrap_or(0.0);
            survival * continuation_value + recovery
        } else {
            continuation_value
        };

        // Same rights as `apply_exercise`, left unresolved so the lattice can
        // take the decision across the whole level.
        Ok(NodeClaim {
            hold: risky_continuation,
            floor: self.put_at(step),
            cap: self.call_at(step).map(|price| CallCap {
                price,
                friction: self.outstanding_principal_at(step)
                    * (self.call_friction_cents / 10_000.0),
            }),
            cashflow: self.cashflow_at_oas(step, oas_rate),
        })
    }
}
