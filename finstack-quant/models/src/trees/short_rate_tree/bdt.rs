use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::{BrentSolver, Solver};
use finstack_quant_core::{Error, Result};

use super::{ShortRateTree, TreeCalibrationResult};

impl ShortRateTree {
    /// Calibrate the standard (κ = 0) Black-Derman-Toy model using
    /// state-price recursion on a binomial lattice with constant lognormal
    /// volatility.
    ///
    /// Mean-reverting Black-Karasinski (κ ≠ 0) is handled by
    /// [`calibrate_bk_trinomial`](Self::calibrate_bk_trinomial), which builds
    /// a genuine trinomial lattice in x = ln r — a binomial lattice cannot
    /// represent the rate-dependent drift `−κ·ln r` while staying
    /// recombining .
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a discount factor is non-positive, if
    /// the node-rate clamp `[1e-8, 5.0]` engages materially (a tree too wide
    /// to calibrate — the lattice would silently misprice the curve), or if
    /// the calibrated tree fails to reprice the curve within tolerance.
    pub(super) fn calibrate_bdt(
        &mut self,
        rates: &mut [Vec<f64>],
        discount_curve: &dyn Discounting,
        dt: f64,
    ) -> Result<()> {
        let sigma = self.config.volatility;
        let solver = BrentSolver::new();

        // Standard BDT (κ = 0): constant lognormal volatility, per-step
        // log-spread σ√dt. κ ≠ 0 never reaches this path — calibrate()
        // routes it to the trinomial Black-Karasinski lattice.
        let step_vol = sigma * dt.sqrt();
        let u = step_vol.exp();
        let p = 0.5;

        // Bounds for alpha solver.
        // Upper bound is generous to avoid distorting the tail of the lognormal
        // distribution; individual node rates can legitimately exceed 100% in
        // wide trees (high vol, many steps, long maturity).
        let alpha_lb = 1e-8;
        let alpha_ub = 5.0;

        // Relative tolerance for deciding that the `[alpha_lb, alpha_ub]` clamp
        // has *materially* altered a node rate. A node rate that merely sits
        // near a bound is fine; one that the clamp has moved by more than this
        // fraction means the Brent objective no longer responds to `alpha` at
        // that node, so the tree can no longer reprice the curve. When that
        // happens the calibration is unsound and is failed below rather than
        // silently returning a mispriced lattice (`max_error_bp` alone only
        // *reports* the damage — it does not prevent the tree from escaping).
        let clamp_rel_tol = 1.0e-6;
        let materially_clamped = |raw: f64| -> bool {
            let clamped = raw.clamp(alpha_lb, alpha_ub);
            // Relative deviation, guarding the (here impossible) zero `raw`.
            let denom = raw.abs().max(f64::MIN_POSITIVE);
            (raw - clamped).abs() / denom > clamp_rel_tol
        };
        let mut clamp_engaged = false;
        let mut clamp_engaged_step = 0_usize;

        // Initial continuous zero rate from the discount curve. `calibrate`
        // guarantees steps >= 1 and a positive horizon, so T1 > 0.
        let t1 = self.time_steps[1];
        let r0 = -discount_curve.df(t1).ln() / t1;

        rates[0] = vec![r0.clamp(alpha_lb, alpha_ub)];
        let mut state_prices = vec![vec![1.0]]; // Q[0] = [1.0]

        let mut max_error_bp = 0.0_f64;
        let mut max_error_step = 0_usize;
        let mut fallback_count = 0_usize;

        for step in 0..self.config.steps {
            let current_time = self.time_steps[step + 1];
            let target_df = discount_curve.df(current_time);

            if target_df <= 0.0 {
                return Err(Error::Validation(format!(
                    "BDT calibration: non-positive discount factor {} at time {}",
                    target_df, current_time
                )));
            }

            let num_nodes = step + 1;
            let current_state_prices = &state_prices[step];
            let current_rates = &rates[step];

            // Solve for drift parameter alpha such that model ZCB price matches market
            let comp = self.config.compounding;
            let objective = |alpha: f64| -> f64 {
                let mut model_price = 0.0;

                for (j, &state_price) in current_state_prices.iter().enumerate().take(num_nodes) {
                    let rate = alpha * u.powf(num_nodes as f64 - 1.0 - 2.0 * j as f64);
                    let rate_clamped = rate.clamp(alpha_lb, alpha_ub);
                    model_price += state_price * comp.df(rate_clamped, dt);
                }

                model_price - target_df
            };

            let initial_alpha = if step == 0 {
                r0.clamp(alpha_lb, alpha_ub)
            } else {
                // Geometric mean of previous step rates as the initial guess.
                let mean_rate =
                    current_rates.iter().map(|&r| r.ln()).sum::<f64>() / current_rates.len() as f64;
                mean_rate.exp().clamp(alpha_lb, alpha_ub)
            };

            let (alpha, used_fallback) = match solver.solve(objective, initial_alpha) {
                Ok(a) => (a.clamp(alpha_lb, alpha_ub), false),
                Err(_) => {
                    // Solver failed - use fallback based on the market zero
                    // rate (current_time > 0 for every calibrated step).
                    let market_rate = -target_df.ln() / current_time;
                    fallback_count += 1;
                    (market_rate.clamp(alpha_lb, alpha_ub), true)
                }
            };

            let current_step_rates: Vec<f64> = (0..num_nodes)
                .map(|j| {
                    let rate = alpha * u.powf(num_nodes as f64 - 1.0 - 2.0 * j as f64);
                    if materially_clamped(rate) && !clamp_engaged {
                        clamp_engaged = true;
                        clamp_engaged_step = step;
                    }
                    rate.clamp(alpha_lb, alpha_ub)
                })
                .collect();
            rates[step] = current_step_rates.clone();

            let model_df = {
                let mut model_price = 0.0;
                for (j, &state_price) in current_state_prices.iter().enumerate().take(num_nodes) {
                    model_price += state_price * comp.df(current_step_rates[j], dt);
                }
                model_price
            };
            let error_bp = ((model_df - target_df) / target_df).abs() * 10000.0;

            if error_bp > max_error_bp {
                max_error_bp = error_bp;
                max_error_step = step;
            }

            if error_bp > 1.0 || used_fallback {
                tracing::warn!(
                    "BDT calibration step {}: error={:.2}bp, target_df={:.6}, model_df={:.6}{}",
                    step,
                    error_bp,
                    target_df,
                    model_df,
                    if used_fallback {
                        " (FALLBACK USED)"
                    } else {
                        ""
                    }
                );
            }

            // Terminal row note (same convention as Ho-Lee and BK): the final
            // iteration populates rates[N] for lattice geometry and accessor
            // consistency, but that row's alpha is the one solved for the last
            // pre-maturity interval — there is no interval beyond maturity to
            // drift-calibrate, and backward induction never uses rates[N] for
            // discounting because pricing stops at maturity.
            let next_nodes = num_nodes + 1;
            let mut next_rates = vec![0.0; next_nodes];
            let mut next_state_prices = vec![0.0; next_nodes];

            for (j, &state_price) in current_state_prices.iter().enumerate().take(num_nodes) {
                let discount_factor = comp.df(current_step_rates[j], dt);
                let state_price_contribution = state_price * discount_factor;

                if j + 1 < next_nodes {
                    let up_rate = alpha * u.powf(next_nodes as f64 - 1.0 - 2.0 * (j + 1) as f64);
                    if materially_clamped(up_rate) && !clamp_engaged {
                        clamp_engaged = true;
                        clamp_engaged_step = step + 1;
                    }
                    next_rates[j + 1] = up_rate.clamp(alpha_lb, alpha_ub);
                    next_state_prices[j + 1] += state_price_contribution * p;
                }

                if j < next_nodes {
                    let down_rate = alpha * u.powf(next_nodes as f64 - 1.0 - 2.0 * j as f64);
                    if materially_clamped(down_rate) && !clamp_engaged {
                        clamp_engaged = true;
                        clamp_engaged_step = step + 1;
                    }
                    next_rates[j] = down_rate.clamp(alpha_lb, alpha_ub);
                    next_state_prices[j] += state_price_contribution * (1.0 - p);
                }
            }

            rates[step + 1] = next_rates;
            state_prices.push(next_state_prices);
        }

        if max_error_bp > 1.0 || fallback_count > 0 {
            tracing::warn!(
                "BDT calibration completed: max error={:.2}bp at step {}, fallbacks={} (target: <1bp, 0 fallbacks)",
                max_error_bp,
                max_error_step,
                fallback_count
            );
        } else {
            tracing::debug!(
                "BDT calibration completed: max error={:.4}bp at step {}",
                max_error_bp,
                max_error_step
            );
        }

        let fit_tolerance_bp = self.config.curve_fit_tolerance_bp;
        if !fit_tolerance_bp.is_finite() || fit_tolerance_bp <= 0.0 {
            return Err(Error::Validation(format!(
                "BDT calibration curve-fit tolerance must be finite and positive, got {fit_tolerance_bp}"
            )));
        }

        // Enforce that the calibrated tree actually reprices the curve.
        //
        // The node-rate clamp `[1e-8, 5.0]` is applied inside the Brent
        // objective. When it engages on a node with material Arrow-Debreu
        // weight the objective stops responding to `alpha`, the solver settles
        // on the wrong drift, and the lattice silently stops repricing the
        // curve — exactly the failure this gate catches. (Clamp engagement on
        // a deep, vanishing-weight tail node is harmless: it leaves
        // `max_error_bp` at ~0 and is intentionally *not* failed here.)
        //
        // `max_error_bp` is re-derived above by an independent forward pass
        // over the final `rates`, so it faithfully reflects any clamp-induced
        // mispricing. When the tolerance is breached, the diagnostic message
        // reports whether the clamp engaged (the usual root cause for a wide
        // tree) so the caller knows which knob to turn.
        if !max_error_bp.is_finite() || max_error_bp > fit_tolerance_bp || fallback_count > 0 {
            self.calibration_quality = Some(TreeCalibrationResult {
                max_error_bp,
                max_error_step,
                fallback_count,
                converged: false,
            });
            let clamp_note = if clamp_engaged {
                format!(
                    " The node-rate clamp [{alpha_lb:.0e}, {alpha_ub}] engaged \
                     materially (first at step {clamp_engaged_step}) — the tree \
                     is too wide; lower the volatility, the step count, or the \
                     maturity."
                )
            } else {
                String::new()
            };
            return Err(Error::Validation(format!(
                "BDT calibration did not converge: max curve error {max_error_bp:.4} bp \
                 at step {max_error_step}, tolerance {fit_tolerance_bp:.4} bp, \
                 solver fallbacks {fallback_count}.{clamp_note}"
            )));
        }

        self.calibration_quality = Some(TreeCalibrationResult {
            max_error_bp,
            max_error_step,
            fallback_count,
            converged: true,
        });

        Ok(())
    }
}
