//! Tsiveriotis-Zhang backward-induction engine.

use finstack_quant_core::HashMap;
use finstack_quant_core::InputError;
use finstack_quant_core::{Error, Result};

use finstack_quant_models::EvolutionParams;

use super::engine::ConvertibleTreeType;
use super::valuator::ConvertibleBondValuator;

/// Implementation of Tsiveriotis-Zhang tree pricing logic.
///
/// Uses per-step discount factors from the full term structure instead of
/// flat-rate discounting. The equity component is discounted at the risk-free
/// forward rate and the cash component at the recovery-adjusted risky forward
/// rate, both extracted step-by-step from the respective discount curves.
/// The tree's risk-neutral branch probabilities are recomputed per step from
/// the same risk-free forwards, so drift and discounting stay consistent on
/// non-flat curves.
///
/// ## Credit model
///
/// **Credit curve convention**: the supplied `credit_curve_id` curve must
/// represent ZERO-RECOVERY (pure hazard) risky discounting, i.e.
/// `risky_df = rf_df * survival_probability`. The recovery blend below is what
/// converts it to recovery-adjusted discounting. Supplying a market
/// recovery-adjusted spread curve here would double-count `(1 - R)`.
///
/// The risky step discount factors are adjusted for recovery:
///
/// ```text
/// risky_fwd_adj = risky_fwd * (1 - R) + rf_fwd * R
/// ```
///
/// where `R` is the recovery rate (0.0 to 1.0). At R=0 this reduces to the
/// zero-recovery TZ model. Setting R=0.40 (ISDA standard for senior unsecured)
/// reflects that bondholders recover 40% of face value on default, reducing
/// the effective credit spread impact on the cash component.
pub(super) struct TsiveriotisZhangEngine<'a> {
    pub(super) valuator: &'a ConvertibleBondValuator,
    pub(super) steps: usize,
    pub(super) time_to_maturity: f64,
}

impl<'a> TsiveriotisZhangEngine<'a> {
    pub(super) fn price(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        tree_type: ConvertibleTreeType,
    ) -> Result<(f64, f64)> {
        let spot = *initial_vars
            .get("spot")
            .ok_or(Error::Input(InputError::NotFound {
                id: "spot price".to_string(),
            }))?;
        let volatility =
            *initial_vars
                .get("volatility")
                .ok_or(Error::Input(InputError::NotFound {
                    id: "volatility".to_string(),
                }))?;
        let risk_free_rate =
            *initial_vars
                .get("interest_rate")
                .ok_or(Error::Input(InputError::NotFound {
                    id: "interest_rate".to_string(),
                }))?;
        let dividend_yield =
            *initial_vars
                .get("dividend_yield")
                .ok_or(Error::Input(InputError::NotFound {
                    id: "dividend_yield".to_string(),
                }))?;

        let dt = self.time_to_maturity / self.steps as f64;

        // Dividend protection (continuous-yield model): the protected rate is
        // the full dividend yield (the DividendAdjustment variants carry no
        // threshold, so nothing is carved out) and the conversion ratio
        // accretes as `ratio(t) = ratio_0 * exp(q_prot * t)` from the
        // valuation date. The stock keeps its unprotected drift `r - q`;
        // protection enters only through the conversion payoff, so with full
        // protection the discounted conversion claim
        // `E[e^{-∫r} * ratio_0 * e^{qT} * S_T] = ratio_0 * S_0` is restored to
        // a martingale independent of `q`.
        let protected_dividend_rate = if self.valuator.dividend_protected {
            dividend_yield.max(0.0)
        } else {
            0.0
        };
        let ratio_accretion_at = |step: usize| (protected_dividend_rate * step as f64 * dt).exp();

        // Evolution parameters for the recombining tree.
        //
        // Drift-discount consistency: the up/down (and middle) factors depend
        // only on volatility and dt, so they are constant across steps and the
        // lattice recombines. The branch probabilities, however, are recomputed
        // per step from the SAME per-step risk-free forward rate used for
        // discounting in backward induction (`rf_step_dfs`):
        //
        //   r_i = -ln(df_rf[i]) / dt,   p_i = (e^{(r_i - q)dt} - d) / (u - d)
        //
        // so the stock's tree forward matches the discount curve step by step
        // (martingale property) even on non-flat curves. The base parameters
        // below (built from the t=0 short rate) only supply the constant
        // factors; their probabilities are replaced by the per-step set.
        let params = match tree_type {
            ConvertibleTreeType::Binomial(_) => {
                EvolutionParams::equity_crr(volatility, risk_free_rate, dividend_yield, dt)?
            }
            ConvertibleTreeType::Trinomial(_) => {
                EvolutionParams::equity_trinomial(volatility, risk_free_rate, dividend_yield, dt)?
            }
        };

        // Per-step probabilities driven by the per-step forward rates implied
        // by the risk-free step discount factors (u/d/middle unchanged).
        let mut step_params = Vec::with_capacity(self.steps);
        for &df in &self.valuator.rf_step_dfs {
            if df <= 0.0 {
                return Err(Error::Validation(format!(
                    "convertible tree requires positive per-step risk-free discount \
                     factors, got {df}"
                )));
            }
            let step_rate = -df.ln() / dt;
            step_params.push(params.with_drift(step_rate - dividend_yield, dt)?);
        }

        // State tracking: (Total Value, Cash Component)
        let mut values: Vec<(f64, f64)> = Vec::with_capacity(2 * self.steps + 1);

        // Trinomial node spot: for a recombining trinomial the recombination
        // identity is `up·down = middle²`, so the spot at `net` net up-moves
        // after `step` total moves depends only on `net` and equals
        //   S₀ · up^net · middle^(step − net).
        // The previous form `up^max(net,0) · down^max(-net,0)` silently
        // assumed `up·down = 1` (it dropped the middle factor entirely) and is
        // malformed for any trinomial whose middle factor is not 1. Using the
        // explicit `middle_factor` is correct for any recombining trinomial.
        let trinomial_middle = params.middle_factor.unwrap_or(1.0);
        let get_spot = |step: usize, node: usize| -> f64 {
            match tree_type {
                ConvertibleTreeType::Binomial(_) => {
                    let ups = node as i32;
                    let downs = step as i32 - node as i32;
                    spot * params.up_factor.powi(ups) * params.down_factor.powi(downs)
                }
                ConvertibleTreeType::Trinomial(_) => {
                    let net_moves = node as i32 - step as i32;
                    // `powi` accepts negative exponents (u^(-k) = 1/u^k), so
                    // this is correct for both up and down net moves.
                    spot * params.up_factor.powi(net_moves)
                        * trinomial_middle.powi(step as i32 - net_moves)
                }
            }
        };

        // 1. Terminal Step
        let num_nodes = match tree_type {
            ConvertibleTreeType::Binomial(n) => n + 1,
            ConvertibleTreeType::Trinomial(n) => 2 * n + 1,
        };

        let mandatory = self.valuator.conversion_is_mandatory();
        let terminal_accretion = ratio_accretion_at(self.steps);
        let terminal_coupon = self
            .valuator
            .coupon_map
            .get(&self.steps)
            .copied()
            .unwrap_or(0.0);
        let terminal_put = self.valuator.put_price_at_step(self.steps);

        for i in 0..num_nodes {
            let node_spot = get_spot(self.steps, i);
            let conversion_val = self
                .valuator
                .conversion_value(node_spot, terminal_accretion);

            let coupon = terminal_coupon;
            let redemption_val = self.valuator.face_value;

            let can_convert = self.valuator.conversion_allowed(self.steps, node_spot);

            let (mut ex_coupon_total, mut ex_coupon_cash) = if can_convert && mandatory {
                // Mandatory conversion: holder must convert regardless of optimality.
                // For PERCS/DECS below the lower strike, this correctly reflects
                // the holder bearing equity downside risk.
                (conversion_val, 0.0)
            } else if can_convert && conversion_val > redemption_val {
                (conversion_val, 0.0)
            } else {
                (redemption_val, redemption_val)
            };

            // Put at maturity: an accreting put whose window extends to the
            // final date lets the holder redeem at the put price instead of
            // face. The holder maximizes; the put payoff is all-cash. Forced
            // (mandatory) conversion overrides the put right. An issuer call
            // at maturity is deliberately ignored: the contractual redemption
            // at face dominates, so a call cannot reduce the maturity payoff.
            if !(can_convert && mandatory) {
                if let Some(put_price) = terminal_put {
                    if ex_coupon_total < put_price {
                        ex_coupon_total = put_price;
                        ex_coupon_cash = put_price;
                    }
                }
            }

            // Coupon entitlement is independent of the exercise choice under
            // the public contract (there is no coupon-forfeiture flag). Make
            // the exercise decision ex-coupon, then add the date's coupon as
            // a cash component.
            values.push((ex_coupon_total + coupon, ex_coupon_cash + coupon));
        }

        // 2. Backward Induction. Double-buffer the value layers so each per-step
        // layer reuses one allocation (cleared) instead of allocating a fresh Vec.
        let mut next_values: Vec<(f64, f64)> = Vec::with_capacity(values.len());
        for step in (0..self.steps).rev() {
            let current_num_nodes = match tree_type {
                ConvertibleTreeType::Binomial(_) => step + 1,
                ConvertibleTreeType::Trinomial(_) => 2 * step + 1,
            };

            // Per-step discount factors from full term structure, and the
            // per-step branch probabilities derived from the same forwards.
            let df_rf = self.valuator.rf_step_dfs[step];
            let df_risky = self.valuator.risky_step_dfs[step];
            let sp = &step_params[step];
            let step_accretion = ratio_accretion_at(step);
            // Exercise decisions are ex-coupon; the coupon is added after
            // conversion/call/put resolution so no branch can overwrite it.
            let coupon = self.valuator.coupon_map.get(&step).copied().unwrap_or(0.0);
            let call_price_here = self.valuator.call_price_at_step(step);
            let put_price_here = self.valuator.put_price_at_step(step);

            next_values.clear();

            for i in 0..current_num_nodes {
                let (exp_total, exp_cash) = match tree_type {
                    ConvertibleTreeType::Binomial(_) => {
                        let (v_up, c_up) = values[i + 1];
                        let (v_down, c_down) = values[i];

                        (
                            sp.prob_up * v_up + sp.prob_down * v_down,
                            sp.prob_up * c_up + sp.prob_down * c_down,
                        )
                    }
                    ConvertibleTreeType::Trinomial(_) => {
                        let (v_up, c_up) = values[i + 2];
                        let (v_mid, c_mid) = values[i + 1];
                        let (v_down, c_down) = values[i];

                        let pm = sp.prob_middle.unwrap_or(0.0);
                        (
                            sp.prob_up * v_up + pm * v_mid + sp.prob_down * v_down,
                            sp.prob_up * c_up + pm * c_mid + sp.prob_down * c_down,
                        )
                    }
                };

                // TZ discounting: equity at risk-free, cash at risky
                let equity_part = (exp_total - exp_cash) * df_rf;
                let cash_part = exp_cash * df_risky;
                let continuation_total = equity_part + cash_part;
                let continuation_cash = cash_part;

                // Node decision logic
                let node_spot = get_spot(step, i);

                // 1. Conversion (uses variable delivery for MandatoryVariable,
                //    dividend-protection accretion at this step's time)
                let conversion_val = self.valuator.conversion_value(node_spot, step_accretion);
                let can_convert = self.valuator.conversion_allowed(step, node_spot);

                let mut final_total = continuation_total;
                let mut final_cash = continuation_cash;

                if can_convert && mandatory {
                    // Mandatory conversion: forced regardless of optimality.
                    final_total = conversion_val;
                    final_cash = 0.0;
                } else if can_convert && conversion_val > final_total {
                    final_total = conversion_val;
                    final_cash = 0.0;
                }

                // 2. Call (Issuer minimizes value).
                //
                // Tsiveriotis-Zhang call rule:
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

                if call_allowed {
                    if let Some(call_price) = call_price_here {
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
                }

                // 3. Put (Holder maximizes value)
                if let Some(put_price) = put_price_here {
                    if final_total < put_price {
                        final_total = put_price;
                        final_cash = final_total;
                    }
                }

                final_total += coupon;
                final_cash += coupon;

                next_values.push((final_total, final_cash));
            }
            std::mem::swap(&mut values, &mut next_values);
        }

        Ok(values[0])
    }
}
