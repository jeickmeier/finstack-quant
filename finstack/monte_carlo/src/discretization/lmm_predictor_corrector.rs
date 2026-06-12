//! Predictor-corrector discretization for the LMM/BGM forward rate model.
//!
//! Implements the Hunter–Jäckel–Joshi predictor-corrector scheme on the
//! **log of the displaced forwards**: the proportional drift is averaged
//! between an evaluation at the current state/time and an evaluation at the
//! predicted state at the **end** of the step, and the update is a log-Euler
//! exponential step. Only "alive" forwards (those with fixing date T_i > t)
//! are evolved; dead forwards are frozen at their last value.
//!
//! # Algorithm (one time step t → t + dt, per alive forward i)
//!
//! With `X_i = F_i + d_i` (displaced forward), proportional drift
//! `m_i = drift_i / X_i`, and loadings `λ_i(t)`:
//!
//! 1. **Predictor** (log-Euler at the start of the step):
//!    `X_i^pred = X_i · exp[(m_i(t, F) − ½|λ_i(t)|²)dt + λ_i(t)·Z √dt]`
//! 2. **Corrector**: recompute the drift **at t + dt** on the predicted
//!    state (so piecewise-constant loadings that change inside the step are
//!    seen), average `m̄ = ½(m_i(t, F) + m_i(t+dt, F^pred))`.
//! 3. **Final step**: `X_i(t+dt) = X_i · exp[(m̄ − ½|λ_i(t)|²)dt + λ_i(t)·Z √dt]`.
//!
//! The exponential update keeps `F_i > −d_i` by construction (no clamping
//! floor concentrating mass at the boundary) and is **exact** for frozen
//! coefficients — in particular the terminal-measure terminal forward, whose
//! drift is identically zero, is simulated from its exact lognormal law.
//!
//! # References
//!
//! - Hunter, C., Jäckel, P. & Joshi, M. (2001). "Getting the Drift."
//!   *Risk*, 14(7), 81–84.
//! - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*,
//!   Ch. 7, Springer.

use super::super::process::lmm::LmmProcess;
use super::super::traits::{Discretization, StochasticProcess};

/// Predictor-corrector discretization for the [`LmmProcess`].
///
/// Each time step evaluates the terminal-measure drift twice (at the current
/// and predicted states) and uses their average together with the diffusion
/// shocks applied only once.
#[derive(Debug, Clone)]
pub struct LmmPredictorCorrector;

impl LmmPredictorCorrector {
    /// Create a new predictor-corrector discretization.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LmmPredictorCorrector {
    fn default() -> Self {
        Self::new()
    }
}

impl Discretization<LmmProcess> for LmmPredictorCorrector {
    fn step(
        &self,
        process: &LmmProcess,
        t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        work: &mut [f64],
    ) {
        let params = process.params();
        let n = params.num_forwards;
        let nf = params.num_factors;
        let sqrt_dt = dt.sqrt();
        let first = process.first_alive(t);

        // Work buffer layout:
        //   [0..n]         = drift at current state
        //   [n..2n]        = predicted forwards
        //   [2n..3n]       = drift at predicted state

        // --- Compute drift at current state ---
        let (drift_curr, rest) = work.split_at_mut(n);
        process.drift(t, x, drift_curr);

        // --- Predictor: log-Euler on the displaced forwards ---
        let (predicted, rest2) = rest.split_at_mut(n);

        // Copy dead forwards unchanged (and seed alive slots; overwritten below)
        predicted.copy_from_slice(x);

        for i in first..n {
            let x_disp = x[i] + params.displacements[i];
            if x_disp <= 0.0 {
                // Absorbed exactly at the displacement boundary (only
                // reachable via an absorbed input state).
                continue;
            }
            let lam = params.factor_loadings(i, t);
            let mut diff_sum = 0.0;
            let mut lam_sq = 0.0;
            for k in 0..nf {
                diff_sum += lam[k] * z[k];
                lam_sq += lam[k] * lam[k];
            }
            let m_curr = drift_curr[i] / x_disp;
            predicted[i] = x_disp * ((m_curr - 0.5 * lam_sq) * dt + diff_sum * sqrt_dt).exp()
                - params.displacements[i];
        }

        // --- Corrector drift at the END of the step on the predicted state
        // (Hunter-Jäckel-Joshi): loadings with a breakpoint inside (t, t+dt]
        // are evaluated on their new segment instead of staying stale.
        let drift_pred = &mut rest2[..n];
        process.drift(t + dt, predicted, drift_pred);

        // --- Final log-Euler step with the averaged proportional drift ---
        for i in first..n {
            let x_disp = x[i] + params.displacements[i];
            if x_disp <= 0.0 {
                continue;
            }
            let lam = params.factor_loadings(i, t);
            let mut diff_sum = 0.0;
            let mut lam_sq = 0.0;
            for k in 0..nf {
                diff_sum += lam[k] * z[k];
                lam_sq += lam[k] * lam[k];
            }
            let m_curr = drift_curr[i] / x_disp;
            let pred_disp = predicted[i] + params.displacements[i];
            let m_pred = if pred_disp > 0.0 {
                drift_pred[i] / pred_disp
            } else {
                0.0
            };
            let avg_m = 0.5 * (m_curr + m_pred);

            x[i] = x_disp * ((avg_m - 0.5 * lam_sq) * dt + diff_sum * sqrt_dt).exp()
                - params.displacements[i];
        }
    }

    fn work_size(&self, process: &LmmProcess) -> usize {
        let n = process.params().num_forwards;
        // drift_curr(n) + predicted(n) + drift_pred(n) = 3n
        3 * n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::lmm::LmmParams;
    use crate::traits::Discretization;

    fn simple_params() -> LmmParams {
        LmmParams::try_new(
            3,
            2,
            vec![0.0, 1.0, 2.0, 3.0],
            vec![1.0, 1.0, 1.0],
            vec![0.005, 0.005, 0.005],
            vec![],
            vec![vec![
                [0.15, 0.05, 0.0],
                [0.12, 0.08, 0.0],
                [0.10, 0.10, 0.0],
            ]],
            vec![0.03, 0.03, 0.03],
        )
        .expect("valid")
    }

    #[test]
    fn test_single_step_preserves_positivity() {
        let params = simple_params();
        let process = LmmProcess::new(params);
        let disc = LmmPredictorCorrector::new();
        let ws = disc.work_size(&process);
        let mut work = vec![0.0; ws];
        let mut x = vec![0.03, 0.03, 0.03];
        let z = vec![0.5, -0.3]; // 2 normal shocks

        disc.step(&process, 0.0, 0.01, &mut x, &z, &mut work);

        // All forwards should remain > -displacement
        for (i, &f) in x.iter().enumerate() {
            assert!(f >= -0.005, "forward {i} below displacement floor: {f}");
        }
    }

    #[test]
    fn test_zero_shocks_terminal_forward_follows_lognormal_median() {
        let params = simple_params();
        let process = LmmProcess::new(params);
        let disc = LmmPredictorCorrector::new();
        let ws = disc.work_size(&process);
        let mut work = vec![0.0; ws];
        let mut x = vec![0.03, 0.03, 0.03];
        let z = vec![0.0, 0.0]; // zero shocks

        disc.step(&process, 0.0, 0.01, &mut x, &z, &mut work);

        // The terminal forward is driftless under the terminal measure; the
        // log-Euler step at zero shock lands on the lognormal MEDIAN:
        // X_next = X·exp(−½|λ|²dt) with λ = [0.10, 0.10] ⇒ |λ|² = 0.02.
        let expected = 0.035 * (-0.5 * 0.02 * 0.01_f64).exp() - 0.005;
        assert!(
            (x[2] - expected).abs() < 1e-14,
            "terminal forward should follow the exact lognormal zero-shock \
             path: got {}, expected {expected}",
            x[2]
        );
    }

    /// The terminal-measure terminal forward is driftless, so the displaced
    /// log-Euler step simulates its EXACT lognormal law and a caplet on it
    /// must reproduce the displaced-Black price within pure MC error — no
    /// discretization-bias allowance. The previous arithmetic-Euler step
    /// carried an O(dt) bias here.
    #[test]
    fn test_terminal_caplet_matches_displaced_black() {
        use crate::rng::philox::PhiloxRng;
        use crate::traits::RandomStream;
        use finstack_core::math::special_functions::norm_cdf;

        let params = simple_params();
        let process = LmmProcess::new(params);
        let disc = LmmPredictorCorrector::new();
        let ws = disc.work_size(&process);

        let (t_expiry, num_steps) = (1.0, 8usize);
        let dt = t_expiry / num_steps as f64;
        let strike = 0.03;
        let disp = 0.005;
        let num_paths = 60_000usize;

        let root = PhiloxRng::new(13);
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        for path in 0..num_paths {
            let mut rng = root.substream(path as u64);
            let mut x = vec![0.03, 0.03, 0.03];
            let mut work = vec![0.0; ws];
            let mut z = vec![0.0; 2];
            for step in 0..num_steps {
                rng.fill_std_normals(&mut z);
                disc.step(&process, step as f64 * dt, dt, &mut x, &z, &mut work);
            }
            let payoff = (x[2] - strike).max(0.0);
            sum += payoff;
            sum_sq += payoff * payoff;
        }
        let n = num_paths as f64;
        let mean = sum / n;
        let stderr = ((sum_sq / n - mean * mean) / n).sqrt();

        // Displaced Black on X = F + d with σ² = |λ|² = 0.02.
        let (x0, k_disp) = (0.03 + disp, strike + disp);
        let sigma = 0.02_f64.sqrt();
        let d1 = ((x0 / k_disp).ln() + 0.5 * sigma * sigma * t_expiry) / (sigma * t_expiry.sqrt());
        let d2 = d1 - sigma * t_expiry.sqrt();
        let black = x0 * norm_cdf(d1) - k_disp * norm_cdf(d2);

        assert!(
            (mean - black).abs() < 4.0 * stderr,
            "terminal caplet {mean:.6} should match displaced Black {black:.6} \
             within 4×stderr = {:.6}",
            4.0 * stderr
        );
    }

    #[test]
    fn test_dead_forwards_frozen() {
        let params = simple_params();
        let process = LmmProcess::new(params);
        let disc = LmmPredictorCorrector::new();
        let ws = disc.work_size(&process);
        let mut work = vec![0.0; ws];
        let mut x = vec![0.03, 0.03, 0.03];
        let z = vec![1.0, -1.0];

        // At t=1.5, forward 0 is dead (T_0=0.0) and forward 1 is dead (T_1=1.0)
        let x0_before = x[0];
        let x1_before = x[1];
        disc.step(&process, 1.5, 0.01, &mut x, &z, &mut work);

        assert!(
            (x[0] - x0_before).abs() < 1e-15,
            "dead forward 0 should be frozen"
        );
        assert!(
            (x[1] - x1_before).abs() < 1e-15,
            "dead forward 1 should be frozen"
        );
    }

    #[test]
    fn test_work_size() {
        let params = simple_params();
        let process = LmmProcess::new(params);
        let disc = LmmPredictorCorrector::new();
        assert_eq!(disc.work_size(&process), 9); // 3 * 3
    }
}
