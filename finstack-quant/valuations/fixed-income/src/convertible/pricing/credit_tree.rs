//! Convertible binomial lattice engine supporting both defaultable-equity
//! (jump-to-default with explicit principal recovery) and canonical
//! Tsiveriotis-Fernandes (1998) two-component `(total, cash)` discounting.
//!
//! # References
//!
//! - Tsiveriotis, K., & Fernandes, C. (1998). "Valuing Convertible Bonds with
//!   Credit Risk." `docs/REFERENCES.md#tsiveriotis-fernandes-1998`

use finstack_quant_core::{Error, Result};

use finstack_quant_models::trees::tree_framework::EvolutionParams;

use super::engine::ConvertibleModel;
use super::valuator::ConvertibleBondValuator;

/// Resolve the terminal exercise choice and its cash component.
///
/// The represented contract always retains the scheduled coupon on conversion.
/// Forced conversion takes precedence over cash redemption and holder puts.
/// For optional conversion, a positive `log_spacing` smooths only the cash/equity
/// split over a terminal lattice cell; exact-date valuation passes zero.
pub(super) fn terminal_payoff(
    conversion_value: f64,
    cash_level: f64,
    coupon: f64,
    can_convert: bool,
    mandatory: bool,
    log_spacing: f64,
) -> (f64, f64) {
    let (total, cash) = if can_convert && mandatory {
        (conversion_value, 0.0)
    } else if can_convert {
        // The cash/equity split jumps where conversion equals redemption.
        // Smooth the split over the node's log-spot cell to avoid spurious
        // finite-difference Greeks when a node lies exactly on that boundary.
        let equity_weight = if log_spacing > 0.0 && conversion_value > 0.0 {
            let log_distance = (conversion_value / cash_level).ln();
            (0.5 + log_distance / log_spacing).clamp(0.0, 1.0)
        } else if conversion_value > cash_level {
            1.0
        } else {
            0.0
        };
        (
            conversion_value.max(cash_level),
            (1.0 - equity_weight) * cash_level,
        )
    } else {
        (cash_level, cash_level)
    };
    (total + coupon, cash + coupon)
}

/// Equity market state for a jump-to-zero default model.
///
/// Conditional on survival the stock grows at `r - q + lambda`; at default
/// it becomes zero and the bond pays recovery times outstanding principal.
/// See Milanov and Kounchev (2012), https://arxiv.org/abs/1206.1400.
#[derive(Debug, Clone, Copy)]
pub(super) struct CreditTreeMarketInputs {
    /// Underlying share price in the bond's quote currency.
    pub(super) spot: f64,
    /// Annualized equity volatility (decimal).
    pub(super) volatility: f64,
    /// Short risk-free rate (continuously compounded, decimal) used only for
    /// the lattice's constant up/down factors.
    pub(super) risk_free_rate: f64,
    /// Continuous dividend yield (decimal).
    pub(super) dividend_yield: f64,
}

pub(super) struct CreditTreeEngine<'a> {
    pub(super) valuator: &'a ConvertibleBondValuator,
    pub(super) steps: usize,
    pub(super) time_to_maturity: f64,
    pub(super) model: ConvertibleModel,
    /// Recovery rate as a decimal fraction of principal, in `[0, 1]`
    /// (used by Tsiveriotis-Fernandes to map hazard survival to credit spread).
    pub(super) recovery_rate: f64,
}

impl<'a> CreditTreeEngine<'a> {
    /// Per-step credit discount factor `exp(-s_i * dt) = Q_i^(1 - R)` for TF98.
    fn tf_credit_step_dfs(&self) -> Result<Vec<f64>> {
        let loss = 1.0 - self.recovery_rate;
        self.valuator
            .survival_factors
            .iter()
            .map(|&survival| {
                if survival > 0.0 && survival.is_finite() {
                    Ok(survival.powf(loss))
                } else {
                    Err(Error::Validation(format!(
                        "Tsiveriotis-Fernandes tree requires positive per-step survival, got {survival}"
                    )))
                }
            })
            .collect()
    }

    pub(super) fn price(&self, market: CreditTreeMarketInputs) -> Result<(f64, f64)> {
        let CreditTreeMarketInputs {
            spot,
            volatility,
            risk_free_rate,
            dividend_yield,
        } = market;

        let dt = self.time_to_maturity / self.steps as f64;
        let tf_credit_dfs = match self.model {
            ConvertibleModel::TsiveriotisFernandes => Some(self.tf_credit_step_dfs()?),
            ConvertibleModel::DefaultableEquity => None,
        };

        // Dividend protection (continuous-yield model): the protected rate is
        // the full dividend yield (the DividendAdjustment variants carry no
        // threshold, so nothing is carved out) and the conversion ratio
        // accretes as `ratio(t) = ratio_0 * exp(q_prot * t)` from the
        // valuation date.
        let protected_dividend_rate = if self.valuator.dividend_protected {
            dividend_yield.max(0.0)
        } else {
            0.0
        };
        let ratio_accretion_at = |step: usize| (protected_dividend_rate * step as f64 * dt).exp();

        // Constant CRR up/down factors retain recombination.
        let params = EvolutionParams::equity_crr(volatility, risk_free_rate, dividend_yield, dt)?;

        // Per-step probabilities driven by the per-step forward rates:
        // `r + lambda - q` for defaultable equity (conditional on survival),
        // `r - q` for Tsiveriotis-Fernandes (credit enters via discounting only).
        let mut step_params = Vec::with_capacity(self.steps);
        for (&df, &survival) in self
            .valuator
            .rf_step_dfs
            .iter()
            .zip(&self.valuator.survival_factors)
        {
            if df <= 0.0 {
                return Err(Error::Validation(format!(
                    "convertible tree requires positive per-step risk-free discount \
                     factors, got {df}"
                )));
            }
            let step_rate = match self.model {
                ConvertibleModel::DefaultableEquity => -(df * survival).ln() / dt,
                ConvertibleModel::TsiveriotisFernandes => -df.ln() / dt,
            };
            step_params.push(params.with_drift(step_rate - dividend_yield, dt)?);
        }

        // State tracking: (Total Value, Cash Component)
        let mut values: Vec<(f64, f64)> = Vec::with_capacity(self.steps + 1);

        let get_spot = |step: usize, node: usize| -> f64 {
            let ups = node as i32;
            let downs = step as i32 - node as i32;
            spot * params.up_factor.powi(ups) * params.down_factor.powi(downs)
        };

        // 1. Terminal Step
        let num_nodes = self.steps + 1;

        let mandatory = self.valuator.conversion_is_mandatory();
        let terminal_accretion = ratio_accretion_at(self.steps);
        let terminal_coupon = self
            .valuator
            .coupon_map
            .get(&self.steps)
            .copied()
            .unwrap_or(0.0);
        let terminal_put = self.valuator.put_price_at_step(self.steps);

        // Log-spot spacing between adjacent terminal nodes.
        let terminal_log_spacing = if num_nodes > 1 {
            (get_spot(self.steps, 1) / get_spot(self.steps, 0)).ln()
        } else {
            0.0
        };

        for i in 0..num_nodes {
            let node_spot = get_spot(self.steps, i);
            let conversion_val =
                self.valuator
                    .conversion_value(node_spot, terminal_accretion, self.steps);

            let principal = self
                .valuator
                .principal_map
                .get(&self.steps)
                .copied()
                .unwrap_or(0.0);
            let cash_level = principal.max(terminal_put.unwrap_or(principal));
            values.push(terminal_payoff(
                conversion_val,
                cash_level,
                terminal_coupon,
                self.valuator.conversion_allowed(self.steps, node_spot),
                mandatory,
                terminal_log_spacing,
            ));
        }

        // 2. Backward Induction. Double-buffer the value layers so each per-step
        // layer reuses one allocation (cleared) instead of allocating a fresh Vec.
        let mut next_values: Vec<(f64, f64)> = Vec::with_capacity(values.len());
        for step in (0..self.steps).rev() {
            let current_num_nodes = step + 1;

            // Per-step continuation from full term structure:
            // - DefaultableEquity: both components discounted at `rf * survival` + recovery.
            // - TsiveriotisFernandes: canonical two-component TF98 backward induction
            //   discounting equity component `exp_total - exp_cash` at `rf` and
            //   cash component `exp_cash` at `rf * credit_df` (no default recovery leg).
            let rf_df = self.valuator.rf_step_dfs[step];
            let survival = self.valuator.survival_factors[step];
            let recovery = self.valuator.recovery_by_step[step];
            let tf_credit_df = tf_credit_dfs.as_ref().map(|dfs| dfs[step]);
            let sp = &step_params[step];
            let step_accretion = ratio_accretion_at(step);
            // Exercise decisions are ex-coupon; the coupon is added after
            // conversion/call/put resolution so no branch can overwrite it.
            let coupon = self.valuator.coupon_map.get(&step).copied().unwrap_or(0.0)
                + self
                    .valuator
                    .principal_map
                    .get(&step)
                    .copied()
                    .unwrap_or(0.0);
            let call_price_here = self.valuator.call_price_at_step(step);
            let put_price_here = self.valuator.put_price_at_step(step);

            next_values.clear();

            for i in 0..current_num_nodes {
                let (v_up, c_up) = values[i + 1];
                let (v_down, c_down) = values[i];
                let exp_total = sp.prob_up * v_up + sp.prob_down * v_down;
                let exp_cash = sp.prob_up * c_up + sp.prob_down * c_down;

                let (continuation_total, continuation_cash) = match tf_credit_df {
                    Some(credit_df) if (credit_df - 1.0).abs() > 0.0 => {
                        let exp_equity = exp_total - exp_cash;
                        let cont_cash = exp_cash * rf_df * credit_df;
                        (exp_equity * rf_df + cont_cash, cont_cash)
                    }
                    Some(_) => (exp_total * rf_df, exp_cash * rf_df),
                    None => {
                        let discount = rf_df * survival;
                        (
                            exp_total * discount + recovery,
                            exp_cash * discount + recovery,
                        )
                    }
                };

                // Node decision logic
                let node_spot = get_spot(step, i);

                // 1. Conversion (uses variable delivery for MandatoryVariable,
                //    dividend-protection accretion at this step's time)
                let conversion_val =
                    self.valuator
                        .conversion_value(node_spot, step_accretion, step);
                let can_convert = self.valuator.conversion_allowed(step, node_spot);

                let mut final_total = continuation_total;
                let mut final_cash = continuation_cash;

                if can_convert && mandatory {
                    // A forced delivery extinguishes the bond. Ordinary
                    // call/put alternatives must not overwrite that event.
                    next_values.push(terminal_payoff(
                        conversion_val,
                        self.valuator.face_value,
                        coupon,
                        true,
                        true,
                        0.0,
                    ));
                    continue;
                } else if can_convert && conversion_val > final_total {
                    final_total = conversion_val;
                    final_cash = 0.0;
                }

                // 2. Call (Issuer minimizes value).
                //
                // Issuer exercise rule:
                //   value = min(continuation, max(call_price, conversion_value))
                // The issuer calls only when doing so *reduces* the bond value,
                // i.e. when `continuation > value_if_called`, where
                // `value_if_called` is the holder's optimal response to a call:
                //   - convert (→ conversion_value, all-equity) iff conversion
                //     is permitted at this step AND conversion_value exceeds
                //     the call price;
                //   - otherwise accept the cash call price (all-cash).
                //
                // Critically the conversion branch must be gated on
                // `can_convert`: a previous form used `conversion_val` in the
                // cash/equity split unconditionally, which forced a conversion
                // (cash = 0) even at steps where conversion is not allowed,
                // corrupting both the value and the cash component that feeds
                // the next credit-risky discounting step.
                // Uses adjusted soft-call trigger with observation window correction.
                let call_allowed = self.valuator.soft_call_triggered(node_spot);

                if call_allowed && let Some(call_price) = call_price_here {
                    // Holder converts in response to a call only if
                    // conversion is genuinely permitted here and is worth
                    // more than the cash call price.
                    let holder_converts = can_convert && conversion_val >= call_price;
                    let value_if_called = if holder_converts {
                        conversion_val
                    } else {
                        call_price
                    };

                    // Issuer calls iff it strictly lowers the bond value.
                    if final_total > value_if_called {
                        if holder_converts {
                            final_total = conversion_val;
                            final_cash = 0.0;
                        } else {
                            // Cash redemption at the call price: the whole
                            // payoff is a cash (credit-risky) component.
                            final_total = call_price;
                            final_cash = call_price;
                        }
                    }
                }

                // 3. Put (Holder maximizes value)
                if let Some(put_price) = put_price_here
                    && final_total < put_price
                {
                    final_total = put_price;
                    final_cash = final_total;
                }

                final_total += coupon;
                final_cash += coupon;

                next_values.push((final_total, final_cash));
            }
            std::mem::swap(&mut values, &mut next_values);
        }

        Ok(values[0])
    }

    /// Present value of the straight cash component (no conversion or options).
    pub(super) fn bond_floor(&self) -> Result<f64> {
        let tf_credit_dfs = match self.model {
            ConvertibleModel::TsiveriotisFernandes => Some(self.tf_credit_step_dfs()?),
            ConvertibleModel::DefaultableEquity => None,
        };
        let payment = |step| {
            self.valuator.coupon_map.get(&step).copied().unwrap_or(0.0)
                + self
                    .valuator
                    .principal_map
                    .get(&step)
                    .copied()
                    .unwrap_or(0.0)
        };
        let mut value = payment(self.steps);
        for step in (0..self.steps).rev() {
            value = match &tf_credit_dfs {
                Some(credit_dfs) => {
                    payment(step) + value * self.valuator.rf_step_dfs[step] * credit_dfs[step]
                }
                None => {
                    payment(step)
                        + value
                            * self.valuator.rf_step_dfs[step]
                            * self.valuator.survival_factors[step]
                        + self.valuator.recovery_by_step[step]
                }
            };
        }
        Ok(value)
    }
}
