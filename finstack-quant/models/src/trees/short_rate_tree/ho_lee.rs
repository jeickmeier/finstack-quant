use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::{BrentSolver, Solver};
use finstack_quant_core::{Error, Result};

use super::{ShortRateTree, TreeCalibrationResult, TreeCompounding};

impl ShortRateTree {
    /// Calibrate Ho-Lee model parameters.
    ///
    /// Ho-Lee does **not** support mean reversion because the rate-dependent
    /// drift `κ·r` breaks lattice recombination. Use
    /// [`HullWhiteTree`](crate::trees::HullWhiteTree) for mean-reverting
    /// normal short-rate models.
    ///
    /// Negative short rates are a correct and expected feature of Ho-Lee and
    /// are not treated as errors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `mean_reversion` is non-zero, or if an
    /// extreme volatility drives the lattice to a pathologically extreme node
    /// discount factor (a numerically degenerate tree unfit for pricing).
    pub(super) fn calibrate_ho_lee(
        &mut self,
        rates: &mut [Vec<f64>],
        discount_curve: &dyn Discounting,
        dt: f64,
    ) -> Result<()> {
        if self.config.mean_reversion.abs() > 1e-12 {
            return Err(Error::Validation(
                "Ho-Lee model does not support mean reversion (breaks lattice recombination); \
                 use HullWhiteTree for mean-reverting normal short-rate models"
                    .into(),
            ));
        }

        let sigma = self.config.volatility;
        // Calibration must use the same per-node discount convention as
        // pricing: a tree calibrated with continuous
        // `exp(-r*dt)` but priced with e.g. simple `1/(1+r*dt)` silently
        // fails to reprice the curve.
        let comp = self.config.compounding;

        // Initialize first step with current short rate: r0 satisfies
        // comp.df(r0, T1) = P(0, T1) under the configured convention.
        // `calibrate` guarantees steps >= 1 and a positive horizon, so T1 > 0.
        let t1 = self.time_steps[1];
        let r0 = comp.rate_from_df(discount_curve.df(t1), t1);

        rates[0] = vec![r0];

        let mut state_prices = vec![1.0]; // Q[0] = 1.0

        for step in 0..self.config.steps {
            // rates[step] discounts the interval [t_step, t_{step+1}].
            // The next row rates[step + 1] discounts [t_{step+1}, t_{step+2}],
            // so it is calibrated to P(0, t_{step+2}) when that maturity
            // exists. The terminal row rates[N] is populated for lattice
            // geometry and accessor consistency; backward induction never uses
            // it for discounting because pricing stops at maturity.

            let next_next_time = if step + 2 < self.time_steps.len() {
                self.time_steps[step + 2]
            } else {
                // Terminal row: populate but do not calibrate an unused
                // post-maturity discounting interval.
                0.0
            };

            let next_nodes = step + 2;
            let mut next_rates_base = vec![0.0; next_nodes];
            let mut next_state_prices = vec![0.0; next_nodes];

            for (i, &current_rate) in rates[step].iter().enumerate() {
                let q = state_prices[i];
                let df = comp.df(current_rate, dt);

                let r_up_base = current_rate + sigma * dt.sqrt();
                if i + 1 < next_nodes {
                    next_rates_base[i + 1] = r_up_base;
                    next_state_prices[i + 1] += q * df * 0.5;
                }

                let r_down_base = current_rate - sigma * dt.sqrt();
                if i < next_nodes {
                    next_rates_base[i] = r_down_base;
                    next_state_prices[i] += q * df * 0.5;
                }
            }

            // Ho-Lee calibration: r_next[j] = r_base[j] + θ. The model ZCB
            // price Σ Q_next[j] · df(r_base[j] + θ, dt) must equal P_target.
            //
            // Under continuous compounding the θ-dependence factors out:
            // df(r+θ) = exp(-θ·dt)·df(r) ⇒ θ = -ln(P_target/P_model_base)/dt,
            // which is exact. Other conventions do not factor θ out of df(r),
            // so θ is root-found with that closed form as the initial
            // guess.
            let theta = if next_next_time > 0.0 {
                let p_target = discount_curve.df(next_next_time);
                let mut p_model_base = 0.0;
                let mut p_model_base_cont = 0.0;
                for (j, &q_next) in next_state_prices.iter().enumerate() {
                    let r_base = next_rates_base[j];
                    p_model_base += q_next * comp.df(r_base, dt);
                    p_model_base_cont += q_next * (-r_base * dt).exp();
                }

                if p_model_base > 0.0 && p_target > 0.0 {
                    let theta_cont = if p_model_base_cont > 0.0 {
                        -(p_target / p_model_base_cont).ln() / dt
                    } else {
                        0.0
                    };
                    if comp == TreeCompounding::Continuous {
                        theta_cont
                    } else {
                        let objective = |theta: f64| -> f64 {
                            let mut p_model = 0.0;
                            for (j, &q_next) in next_state_prices.iter().enumerate() {
                                p_model += q_next * comp.df(next_rates_base[j] + theta, dt);
                            }
                            p_model - p_target
                        };
                        match BrentSolver::new().solve(objective, theta_cont) {
                            Ok(t) => t,
                            Err(e) => {
                                return Err(Error::Validation(format!(
                                    "Ho-Lee calibration: failed to solve drift theta at \
                                     step {step} under {comp:?} compounding: {e}"
                                )));
                            }
                        }
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let mut next_rates = vec![0.0; next_nodes];
            for j in 0..next_nodes {
                next_rates[j] = next_rates_base[j] + theta;
            }

            rates[step + 1] = next_rates;
            state_prices = next_state_prices;
        }

        // Measure actual calibration error (floating-point accumulation)
        let mut max_error_bp = 0.0_f64;
        let mut max_error_step = 0_usize;
        {
            let max_nodes = self.config.steps + 2;
            let mut q = vec![0.0_f64; max_nodes];
            let mut next_q = vec![0.0_f64; max_nodes];
            q[0] = 1.0; // Arrow-Debreu prices
            for (step, rates_step) in rates.iter().enumerate().take(self.config.steps) {
                let next_nodes = step + 2;
                next_q[..next_nodes].fill(0.0);
                for (i, &rate_i) in rates_step.iter().enumerate() {
                    let df_i = comp.df(rate_i, dt);
                    if i + 1 < next_nodes {
                        next_q[i + 1] += q[i] * df_i * 0.5;
                    }
                    if i < next_nodes {
                        next_q[i] += q[i] * df_i * 0.5;
                    }
                }
                let model_df: f64 = next_q[..next_nodes].iter().sum();
                let t_next = self.time_steps[step + 1];
                let target_df = discount_curve.df(t_next);
                if target_df > 0.0 {
                    let err = ((model_df - target_df) / target_df).abs() * 10_000.0;
                    if err > max_error_bp {
                        max_error_bp = err;
                        max_error_step = step;
                    }
                }
                std::mem::swap(&mut q, &mut next_q);
            }
        }

        // Diagnostic guard for pathologically extreme node discount factors.
        //
        // Ho-Lee legitimately admits negative short rates, so a node discount
        // factor `exp(-r*dt)` modestly above 1 is expected and is NOT flagged.
        // But an *extreme* normal volatility drives the lattice to wildly
        // dispersed node rates: the deeply-negative tail produces a per-step
        // DF that explodes far above 1, and the deeply-positive tail produces
        // one that collapses toward 0. Either is a numerical-breakdown signal
        // — the lattice is unfit for pricing. The two-sided window below spans
        // ~140 orders of magnitude, so a normal-volatility tree (whose rates
        // stay within a few percent) never trips it. We do not change the
        // model (negative rates remain valid); we only refuse to return a
        // numerically degenerate lattice silently.
        const MAX_NODE_DISCOUNT_FACTOR: f64 = 1.0e6;
        const MIN_NODE_DISCOUNT_FACTOR: f64 = 1.0e-30;
        for (step, rates_step) in rates.iter().enumerate() {
            for (node, &rate) in rates_step.iter().enumerate() {
                let node_df = comp.df(rate, dt);
                // `contains` is `false` for a `NaN` node_df, so the negation
                // correctly flags non-finite values as pathological too.
                let df_in_range =
                    (MIN_NODE_DISCOUNT_FACTOR..=MAX_NODE_DISCOUNT_FACTOR).contains(&node_df);
                if !df_in_range {
                    self.calibration_quality = Some(TreeCalibrationResult {
                        max_error_bp,
                        max_error_step,
                        fallback_count: 0,
                        converged: false,
                    });
                    return Err(Error::Validation(format!(
                        "Ho-Lee calibration produced a pathologically extreme \
                         node discount factor {node_df:.3e} at step {step}, \
                         node {node} (short rate {rate:.4}): the lattice is \
                         numerically degenerate and unfit for pricing. Reduce \
                         the volatility, the step count, or the maturity."
                    )));
                }
            }
        }

        self.calibration_quality = Some(TreeCalibrationResult {
            max_error_bp,
            max_error_step,
            fallback_count: 0,
            converged: true,
        });

        Ok(())
    }
}
