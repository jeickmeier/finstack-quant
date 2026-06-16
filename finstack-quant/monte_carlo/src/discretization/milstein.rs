//! Milstein discretization scheme.
//!
//! Implements the Milstein method for SDEs with diagonal diffusion.
//! Provides higher-order strong convergence than Euler-Maruyama.
//!
//! For scalar SDE:
//! ```text
//! dX_t = μ(t, X_t)dt + σ(t, X_t)dW_t
//! ```
//!
//! The Milstein scheme is:
//! ```text
//! X_{t+Δt} = X_t + μΔt + σ√Δt Z + ½σσ'(Z² - 1)Δt
//! ```
//!
//! where σ' = ∂σ/∂x and Z ~ N(0, 1).
//!
//! # Convergence
//!
//! - Weak order: O(Δt) (same as Euler)
//! - Strong order: O(Δt) (better than Euler's O(√Δt))
//!
//! # Limitations
//!
//! Only works for diagonal diffusion. For non-diagonal diffusion,
//! use Euler-Maruyama instead.

use super::super::traits::{Discretization, ProportionalDiffusion, StochasticProcess};

/// Milstein discretization for diagonal diffusion.
///
/// Adds a correction term to Euler-Maruyama to achieve higher strong
/// convergence order. Requires computing ∂σ/∂x.
///
/// For GBM (σ(X) = σX), the correction is ½σ²X(Z² - 1)Δt.
///
/// # Important: Proportional Volatility Assumption
///
/// This implementation approximates σ'(X) ≈ σ(X)/X, which is **only exact
/// for GBM** (σ(X) = σ_const × X, so σ' = σ_const = σ/X). For other processes:
/// - **CIR** (σ(X) = σ√X): true σ' = σ/(2√X), but this computes σ√X/X = σ/√X — **incorrect**
/// - **OU** (σ(X) = σ_const): true σ' = 0, but this computes σ/X — **incorrect**
///
/// Using Milstein with non-GBM processes will silently degrade strong convergence
/// back to Euler-Maruyama's O(√Δt). Use exact schemes (ExactGbm, ExactHw1f) or
/// process-specific discretizations (QE-CIR, QE-Heston) instead.
///
/// # When to use
///
/// Use **only** when:
/// - The process has proportional (GBM-like) volatility: σ(X) = σ_const × X
/// - Strong convergence is important (pathwise accuracy)
/// - An exact scheme is not available
///
/// Avoid when:
/// - Diffusion is non-diagonal (use Euler instead)
/// - Diffusion is not proportional to state (CIR, OU — use exact/QE schemes)
/// - Weak convergence is sufficient (Euler is simpler)
/// - Exact schemes are available (GBM, OU)
#[derive(Debug, Clone, Default)]
pub struct Milstein;

impl Milstein {
    /// Create a new Milstein discretization.
    pub fn new() -> Self {
        Self
    }
}

impl<P: StochasticProcess + ProportionalDiffusion> Discretization<P> for Milstein {
    fn step(&self, process: &P, t: f64, dt: f64, x: &mut [f64], z: &[f64], work: &mut [f64]) {
        let dim = process.dim();

        // Compute drift: work[0..dim] = μ(t, x)
        process.drift(t, x, &mut work[0..dim]);

        // Compute diffusion: work[dim..2*dim] = σ(t, x)
        process.diffusion(t, x, &mut work[dim..2 * dim]);

        // For diagonal diffusion, we need ∂σ/∂x.
        // WARNING: This approximation σ' ≈ σ/X is only exact for GBM.
        // See struct-level docs for limitations with other processes.

        let sqrt_dt = dt.sqrt();

        for i in 0..dim {
            let mu = work[i];
            let sigma = work[dim + i];

            // Numerical approximation of σ'
            // For GBM: σ(X) = σ_const * X, so σ'(X) = σ_const
            // In general, we approximate: σ' ≈ σ/X (assumes proportional vol)
            let sigma_prime = if x[i].abs() > 1e-10 {
                sigma / x[i] // Approximation for proportional volatility
            } else {
                0.0
            };

            // Milstein correction term: 0.5 * σ * σ' * (Z² - 1) * dt
            let correction = 0.5 * sigma * sigma_prime * (z[i] * z[i] - 1.0) * dt;

            // Milstein step
            x[i] += mu * dt + sigma * sqrt_dt * z[i] + correction;
        }
    }

    fn work_size(&self, process: &P) -> usize {
        2 * process.dim() // drift + diffusion vectors
    }
}

/// Log-space discretization for GBM (proportional diffusion).
///
/// Evolves log(X) directly, guaranteeing positivity:
///
/// ```text
/// ln(X_{t+Δt}) = ln(X_t) + (μ/X - ½(σ/X)²)Δt + (σ/X)√Δt Z
/// ```
///
/// For GBM, σ(X)/X = σ_const, so the Milstein correction dσ/d(log X)
/// is exactly zero and the scheme reduces to the exact log-Euler step.
#[derive(Debug, Clone, Default)]
pub struct LogMilstein;

impl LogMilstein {
    /// Create a new log-Milstein discretization.
    pub fn new() -> Self {
        Self
    }
}

impl<P: StochasticProcess + ProportionalDiffusion> Discretization<P> for LogMilstein {
    fn step(&self, process: &P, t: f64, dt: f64, x: &mut [f64], z: &[f64], work: &mut [f64]) {
        let dim = process.dim();

        // Compute drift and diffusion in original space
        process.drift(t, x, &mut work[0..dim]);
        process.diffusion(t, x, &mut work[dim..2 * dim]);

        let sqrt_dt = dt.sqrt();

        for i in 0..dim {
            let x_safe = x[i].max(f64::MIN_POSITIVE);
            let mu_x = work[i] / x_safe;
            let sigma_x = work[dim + i] / x_safe;

            let drift_term = (mu_x - 0.5 * sigma_x * sigma_x) * dt;
            let diffusion_term = sigma_x * sqrt_dt * z[i];

            x[i] *= (drift_term + diffusion_term).exp();
        }
    }

    fn work_size(&self, process: &P) -> usize {
        2 * process.dim()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::process::gbm::{GbmParams, GbmProcess};
    use super::super::euler::EulerMaruyama;
    use super::*;

    #[test]
    fn test_milstein_step() {
        let params = GbmParams::new(0.05, 0.02, 0.2).unwrap();
        let process = GbmProcess::new(params);
        let disc = Milstein::new();

        let t = 0.0;
        let dt = 0.01;
        let mut x = vec![100.0];
        let z = vec![1.0]; // One std dev
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // Should be positive
        assert!(x[0] > 0.0);
        // Should have moved up (positive drift + positive shock)
        assert!(x[0] > 100.0);
    }

    #[test]
    fn test_log_milstein_positivity() {
        let params = GbmParams::new(0.05, 0.02, 0.5).unwrap(); // High vol
        let process = GbmProcess::new(params);
        let disc = LogMilstein::new();

        let t = 0.0;
        let dt = 0.1; // Larger step
        let mut x = vec![100.0];

        // Test multiple large shocks
        for shock in [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0] {
            let z = vec![shock];
            let mut work = vec![0.0; disc.work_size(&process)];

            disc.step(&process, t, dt, &mut x, &z, &mut work);

            // Log-Milstein should maintain positivity
            assert!(
                x[0] > 0.0,
                "State should remain positive with shock {}",
                shock
            );
        }
    }

    #[test]
    fn test_milstein_vs_euler() {
        // Milstein should have better strong convergence than Euler
        // Test with same random shocks - Milstein should track exact solution better
        let params = GbmParams::new(0.05, 0.02, 0.3).unwrap();
        let process = GbmProcess::new(params);

        let t: f64 = 0.0;
        let dt: f64 = 0.05;
        let x0: f64 = 100.0;
        let z_val: f64 = 0.5;

        // Exact GBM
        let exact = x0 * ((0.05 - 0.02 - 0.5 * 0.3 * 0.3) * dt + 0.3 * dt.sqrt() * z_val).exp();

        // Milstein
        let mut x_milstein = vec![x0];
        let mut work = vec![0.0; 2];
        Milstein::new().step(&process, t, dt, &mut x_milstein, &[z_val], &mut work);

        // Euler
        let mut x_euler = vec![x0];
        EulerMaruyama::new().step(&process, t, dt, &mut x_euler, &[z_val], &mut work);

        let milstein_error = (x_milstein[0] - exact).abs();
        let euler_error = (x_euler[0] - exact).abs();

        println!(
            "Exact: {:.6}, Milstein: {:.6}, Euler: {:.6}",
            exact, x_milstein[0], x_euler[0]
        );
        println!(
            "Milstein error: {:.6}, Euler error: {:.6}",
            milstein_error, euler_error
        );

        // Milstein should generally be closer (though not guaranteed for single path)
        // At least verify both are reasonable approximations
        assert!(milstein_error / x0 < 0.1); // Within 10%
        assert!(euler_error / x0 < 0.1);
    }

    // Compile-time safety: Milstein only accepts ProportionalDiffusion processes.
    // The following would fail to compile (BrownianProcess does not implement
    // ProportionalDiffusion):
    //
    //   let process = BrownianProcess::new(BrownianParams::new(0.05, 0.2));
    //   Milstein::new().step(&process, 0.0, 0.01, &mut [100.0], &[1.0], &mut [0.0; 2]);
}
