use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::{BrentSolver, Solver};
use finstack_quant_core::{Error, Result};

use crate::trees::hull_white_tree::HullWhiteTree;

use super::{ShortRateTree, TreeCalibrationResult};

/// Calibrated Black-Karasinski trinomial lattice data (κ ≠ 0).
///
/// The lattice lives in x = ln r with Hull-White trinomial geometry: node
/// spacing `dx = σ√(3Δt)`, width capped at `j_max` with branch switching at
/// the edges, and per-node mean-reverting transition probabilities. The
/// short rate at node (i, j) is `r = exp(a_i + (j − j_max_i)·dx)` where the
/// per-step additive shift `a_i` is calibrated to the discount curve via
/// Arrow-Debreu forward induction .
#[derive(Debug, Clone)]
pub(super) struct BkTrinomialLattice {
    /// Width cap on |j| (Hull-White branch-switching boundary)
    pub(super) j_max: usize,
    /// Per-step per-node transition probabilities `(p_up, p_mid, p_down)`
    pub(super) probs: Vec<Vec<(f64, f64, f64)>>,
}

impl ShortRateTree {
    /// Calibrate a mean-reverting Black-Karasinski model on a trinomial
    /// lattice in x = ln r .
    ///
    /// # Model
    ///
    /// ```text
    /// d(ln r) = [θ(t) − κ·ln r] dt + σ dW
    /// ```
    ///
    /// Writing `x = ln r − a(t)`, the residual `dx = −κx dt + σ dW` is the
    /// same mean-reverting OU process the Hull-White trinomial discretizes,
    /// so the lattice reuses that geometry: spacing `dx = σ√(3Δt)`, width cap
    /// `j_max` with Hull & White (1994) branch switching at the edges, and
    /// per-node probabilities matching the conditional mean `−jκΔt·dx` and
    /// variance `σ²Δt`. The per-step shift `a_i` is calibrated by forward
    /// induction on Arrow-Debreu prices with a Brent solve (the rate enters
    /// the discount factor as `exp(a_i + x_j)`, so no closed form exists).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a target discount factor is
    /// non-positive, a drift solve fails, or the calibrated lattice fails to
    /// reprice the curve within tolerance.
    pub(super) fn calibrate_bk_trinomial(
        &mut self,
        rates: &mut [Vec<f64>],
        discount_curve: &dyn Discounting,
        dt: f64,
        kappa: f64,
    ) -> Result<()> {
        let sigma = self.config.volatility;
        let comp = self.config.compounding;
        let steps = self.config.steps;

        // Trinomial spacing in x = ln r: matches per-step variance σ²Δt.
        let dx = sigma * (3.0 * dt).sqrt();
        // Hull-White width cap keeping branch probabilities positive.
        let j_max = ((0.184 / (kappa * dt)).ceil() as usize).max(1);

        let mut alpha = vec![0.0; steps + 1];
        let mut probs: Vec<Vec<(f64, f64, f64)>> = Vec::with_capacity(steps);
        let mut state_prices: Vec<f64> = vec![1.0];

        let mut max_error_bp = 0.0_f64;
        let mut max_error_step = 0_usize;

        for step in 0..steps {
            let curr_j_max = step.min(j_max);
            let next_j_max = (step + 1).min(j_max);
            let num_nodes = 2 * curr_j_max + 1;

            let mut step_probs = Vec::with_capacity(num_nodes);
            for j in 0..num_nodes {
                let j_signed = j as i32 - curr_j_max as i32;
                step_probs.push(HullWhiteTree::compute_probabilities(
                    kappa, dt, dx, j_signed, j_max,
                )?);
            }

            let t_next = self.time_steps[step + 1];
            let target_df = discount_curve.df(t_next);
            if target_df <= 0.0 {
                return Err(Error::Validation(format!(
                    "Black-Karasinski calibration: non-positive discount factor \
                     {target_df} at time {t_next}"
                )));
            }

            // Solve the additive x-shift a so the lattice reprices P(0, t_next):
            //   Σ_j Q_j · df(exp(a + x_j), Δt) = target_df
            let q = &state_prices;
            let objective = |a: f64| -> f64 {
                let mut model_df = 0.0;
                for (j, &qj) in q.iter().enumerate() {
                    let x_j = (j as i32 - curr_j_max as i32) as f64 * dx;
                    model_df += qj * comp.df((a + x_j).exp(), dt);
                }
                model_df - target_df
            };
            // Initial guess: log of the period forward rate.
            let prev_df = discount_curve.df(self.time_steps[step]);
            let fwd = if prev_df > 0.0 && target_df > 0.0 {
                comp.rate_from_df(target_df / prev_df, dt)
            } else {
                0.03
            };
            let guess = fwd.max(1e-8).ln();
            let a = BrentSolver::new().solve(objective, guess).map_err(|e| {
                Error::Validation(format!(
                    "Black-Karasinski calibration: drift solve failed at step {step}: {e}"
                ))
            })?;
            alpha[step] = a;

            rates[step] = (0..num_nodes)
                .map(|j| {
                    let x_j = (j as i32 - curr_j_max as i32) as f64 * dx;
                    (a + x_j).exp()
                })
                .collect();

            let mut next_q = vec![0.0; 2 * next_j_max + 1];
            // Branch switching only applies once the lattice has reached its
            // cap (curr and next widths equal); while still growing, all
            // nodes branch normally.
            let boundary_j_max = if curr_j_max == next_j_max {
                curr_j_max
            } else {
                usize::MAX
            };
            for (j, &qj) in q.iter().enumerate() {
                let j_signed = j as i32 - curr_j_max as i32;
                let r_j = (a + j_signed as f64 * dx).exp();
                let contribution = qj * comp.df(r_j, dt);
                for (offset, probability) in
                    HullWhiteTree::transition_offsets(j_signed, boundary_j_max, step_probs[j])
                {
                    if let Some(idx) = HullWhiteTree::transition_index(j_signed, offset, next_j_max)
                    {
                        if idx < next_q.len() {
                            next_q[idx] += contribution * probability;
                        }
                    }
                }
            }

            let model_df: f64 = next_q.iter().sum();
            let error_bp = ((model_df - target_df) / target_df).abs() * 10_000.0;
            if error_bp > max_error_bp {
                max_error_bp = error_bp;
                max_error_step = step;
            }

            probs.push(step_probs);
            state_prices = next_q;
        }

        // Terminal row: no interval beyond maturity to calibrate; extend the
        // last drift for accessor consistency (never used for discounting).
        if steps > 0 {
            alpha[steps] = alpha[steps - 1];
        }
        let term_j_max = steps.min(j_max);
        rates[steps] = (0..=(2 * term_j_max))
            .map(|j| {
                let x_j = (j as i32 - term_j_max as i32) as f64 * dx;
                (alpha[steps] + x_j).exp()
            })
            .collect();

        // Same hard repricing gate philosophy as BDT: a well-posed lattice
        // calibrates to float noise; anything materially off must not escape.
        const MAX_CALIBRATION_ERROR_BPS: f64 = 25.0;
        let converged = max_error_bp.is_finite() && max_error_bp <= MAX_CALIBRATION_ERROR_BPS;
        self.calibration_quality = Some(TreeCalibrationResult {
            max_error_bp,
            max_error_step,
            fallback_count: 0,
            converged,
        });
        if !converged {
            return Err(Error::Validation(format!(
                "Black-Karasinski calibration failed to reprice the discount \
                 curve: max error {max_error_bp:.2} bp at step {max_error_step} \
                 exceeds the {MAX_CALIBRATION_ERROR_BPS:.1} bp tolerance"
            )));
        }

        self.bk_trinomial = Some(BkTrinomialLattice { j_max, probs });

        Ok(())
    }
}
