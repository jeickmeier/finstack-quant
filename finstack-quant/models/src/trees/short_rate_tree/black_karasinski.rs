use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::BrentSolver;
use finstack_quant_core::{Error, Result};

use crate::trees::hull_white_tree::HullWhiteTree;

use super::{ShortRateTree, TreeCalibrationResult, TreeDiscounting};

/// Calibrated Black-Karasinski trinomial lattice data.
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

/// Compute trinomial transition probabilities for node j.
///
/// Hull & White (1994) branching for the mean-reverting residual
/// `dx = −κx dt + σ dW`:
/// - p_up = 1/6 + (j²M² - jM)/2
/// - p_mid = 2/3 - j²M²
/// - p_down = 1/6 + (j²M² + jM)/2
///
/// where M = κ·dt
///
/// At boundaries (|j| >= j_max), we use drift-adjusted branching that:
/// 1. Prevents the tree from growing beyond j_max
/// 2. Accounts for mean reversion to maintain martingale property
///
/// # Arguments
///
/// * `kappa` - Mean reversion speed κ of `x`, per year.
/// * `dt` - Step width in years; must be positive.
/// * `dx` - Node spacing in `x`; must be positive.
/// * `j` - Signed node index (`x = j·dx`).
/// * `j_max` - Width cap; nodes with `|j| >= j_max` use shifted branching.
pub(super) fn compute_probabilities(
    kappa: f64,
    dt: f64,
    dx: f64,
    j: i32,
    j_max: usize,
) -> Result<(f64, f64, f64)> {
    if !kappa.is_finite() || !dt.is_finite() || !dx.is_finite() || dt <= 0.0 || dx <= 0.0 {
        return Err(Error::Validation(
            "Black-Karasinski probabilities require finite, positive inputs".to_string(),
        ));
    }

    let m = kappa * dt;
    let jf = j as f64;

    // Standard interior node probabilities (Hull-White trinomial).
    // The expected offset is -j*kappa*dt, pulling x back toward zero.
    let mut p_up = 1.0 / 6.0 + (jf * jf * m * m - jf * m) / 2.0;
    let mut p_mid = 2.0 / 3.0 - jf * jf * m * m;
    let mut p_down = 1.0 / 6.0 + (jf * jf * m * m + jf * m) / 2.0;

    // At boundaries (|j| >= j_max), use Hull & White (1994) shifted
    // branching to stay inside the capped lattice while matching the first
    // two moments.
    //
    // The tuple still stores probabilities in the branch order used by
    // transition_offsets(): upper boundary (0, -1, -2), lower boundary
    // (+2, +1, 0), interior (+1, 0, -1).
    let j_abs = j.unsigned_abs() as usize;
    if j_abs >= j_max && j_max > 0 {
        let mean = -jf * m;
        let second_moment = 1.0 / 3.0 + mean * mean;
        if j > 0 {
            // Upper boundary Type B: offsets 0, -1, -2.
            p_down = (second_moment + mean) / 2.0;
            p_mid = -second_moment - 2.0 * mean;
            p_up = 1.0 - p_mid - p_down;
        } else if j < 0 {
            // Lower boundary Type C: offsets +2, +1, 0.
            p_up = (second_moment - mean) / 2.0;
            p_mid = 2.0 * mean - second_moment;
            p_down = 1.0 - p_up - p_mid;
        }
    }

    HullWhiteTree::normalize_probabilities(p_up, p_mid, p_down, j)
}

/// Child offsets and probabilities of node `j`, in the branch order
/// [`compute_probabilities`] stores them.
///
/// # Arguments
///
/// * `j` - Signed node index.
/// * `j_max` - Width cap; pass `usize::MAX` while the lattice is still
///   growing so every node branches normally.
/// * `probs` - `(p_up, p_mid, p_down)` from [`compute_probabilities`].
pub(super) fn transition_offsets(j: i32, j_max: usize, probs: (f64, f64, f64)) -> [(i32, f64); 3] {
    let (p_up, p_mid, p_down) = probs;
    let j_abs = j.unsigned_abs() as usize;
    if j_abs >= j_max && j_max > 0 {
        if j > 0 {
            // Upper boundary: branches to j, j-1, j-2.
            [(0, p_up), (-1, p_mid), (-2, p_down)]
        } else if j < 0 {
            // Lower boundary: branches to j+2, j+1, j.
            [(2, p_up), (1, p_mid), (0, p_down)]
        } else {
            [(1, p_up), (0, p_mid), (-1, p_down)]
        }
    } else {
        [(1, p_up), (0, p_mid), (-1, p_down)]
    }
}

/// Storage index of child `j + offset` on a level of half-width
/// `next_j_max`, or `None` when the child falls outside that level.
///
/// # Arguments
///
/// * `j` - Signed parent node index.
/// * `offset` - Signed child offset from [`transition_offsets`].
/// * `next_j_max` - Half-width of the child level (`2·next_j_max + 1` nodes).
pub(super) fn transition_index(j: i32, offset: i32, next_j_max: usize) -> Option<usize> {
    let next_j = j + offset;
    let lower = -(next_j_max as i32);
    let upper = next_j_max as i32;
    if (lower..=upper).contains(&next_j) {
        Some((next_j + next_j_max as i32) as usize)
    } else {
        None
    }
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
    /// # Arguments
    ///
    /// * `rates` - Output rows, one per step, filled with the node short
    ///   rates in ascending `j` order.
    /// * `discount_curve` - Curve the lattice must reprice at every step.
    /// * `dt` - Uniform step width in years.
    /// * `kappa` - Mean reversion speed κ of `ln r`, per year; positive.
    ///
    /// # References
    ///
    /// - Black, F. & Karasinski, P. (1991). "Bond and Option Pricing when
    ///   Short Rates are Lognormal." *Financial Analysts Journal*, 47(4), 52-59.
    /// - Hull, J. & White, A. (1994). "Numerical Procedures for Implementing
    ///   Term Structure Models I: Single-Factor Models." *Journal of
    ///   Derivatives*, 2(1), 7-16.
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
                step_probs.push(compute_probabilities(kappa, dt, dx, j_signed, j_max)?);
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
                    model_df += qj * comp.tree_df((a + x_j).exp(), dt);
                }
                model_df - target_df
            };
            // Initial guess: log of the period forward rate.
            let prev_df = discount_curve.df(self.time_steps[step]);
            let fwd = if prev_df > 0.0 && target_df > 0.0 {
                comp.tree_rate_from_df(target_df / prev_df, dt)
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
                let contribution = qj * comp.tree_df(r_j, dt);
                for (offset, probability) in
                    transition_offsets(j_signed, boundary_j_max, step_probs[j])
                {
                    if let Some(idx) = transition_index(j_signed, offset, next_j_max) {
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
        let fit_tolerance_bp = self.config.curve_fit_tolerance_bp;
        let converged = max_error_bp.is_finite() && max_error_bp <= fit_tolerance_bp;
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
                 exceeds the {fit_tolerance_bp:.4} bp tolerance"
            )));
        }

        self.bk_trinomial = Some(BkTrinomialLattice { j_max, probs });

        Ok(())
    }
}
