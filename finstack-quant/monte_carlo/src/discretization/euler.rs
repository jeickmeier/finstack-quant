//! Euler-Maruyama discretization scheme.
//!
//! Implements the first-order explicit Euler-Maruyama method for general SDEs:
//!
//! ```text
//! X_{t+Δt} = X_t + μ(t, X_t)Δt + σ(t, X_t)√Δt Z
//! ```
//!
//! where Z ~ N(0, I) and μ, σ are drift and diffusion coefficients.
//!
//! This is a generic scheme that works for any `StochasticProcess`,
//! though specialized schemes (like exact GBM or QE for Heston) may be more accurate.

use super::super::traits::{Discretization, StochasticProcess};

/// Generic Euler-Maruyama discretization.
///
/// First-order explicit scheme for SDEs. Works for any process but may
/// have lower accuracy than specialized schemes for particular SDEs.
///
/// # Convergence
///
/// - Weak order: O(Δt)
/// - Strong order: O(√Δt)
///
/// # When to use
///
/// Use when:
/// - No exact or specialized scheme is available
/// - Process has general (non-diagonal) diffusion
/// - Quick prototyping is needed
///
/// Avoid when:
/// - Exact schemes exist (GBM, OU)
/// - Specialized schemes are more accurate (QE for Heston)
#[derive(Debug, Clone, Default)]
pub struct EulerMaruyama;

impl EulerMaruyama {
    /// Create a new Euler-Maruyama discretization.
    pub fn new() -> Self {
        Self
    }
}

impl<P: StochasticProcess> Discretization<P> for EulerMaruyama {
    fn step(&self, process: &P, t: f64, dt: f64, x: &mut [f64], z: &[f64], work: &mut [f64]) {
        let dim = process.dim();

        // Compute drift: work[0..dim] = μ(t, x)
        process.drift(t, x, &mut work[0..dim]);

        // Compute diffusion: work[dim..2*dim] = σ(t, x)
        process.diffusion(t, x, &mut work[dim..2 * dim]);

        // Euler step: x_{t+dt} = x_t + μ(t,x)dt + σ(t,x)√dt * Z
        let sqrt_dt = dt.sqrt();
        for i in 0..dim {
            x[i] += work[i] * dt + work[dim + i] * sqrt_dt * z[i];
        }
    }

    fn work_size(&self, process: &P) -> usize {
        2 * process.dim() // drift + diffusion vectors
    }
}

/// Log-Euler discretization for processes with log-normal dynamics.
///
/// Applies Euler-Maruyama to the log-transformed state:
///
/// ```text
/// ln(X_{t+Δt}) = ln(X_t) + (μ/X - ½(σ/X)²)Δt + (σ/X)√Δt Z
/// ```
///
/// This ensures positivity for processes with proportional diffusion. For plain
/// GBM, prefer [`crate::discretization::exact::ExactGbm`], which uses the
/// analytical transition and has no time-discretization error.
#[derive(Debug, Clone, Default)]
pub struct LogEuler;

impl LogEuler {
    /// Create a new log-Euler discretization.
    pub fn new() -> Self {
        Self
    }
}

impl<P: StochasticProcess> Discretization<P> for LogEuler {
    fn step(&self, process: &P, t: f64, dt: f64, x: &mut [f64], z: &[f64], work: &mut [f64]) {
        let dim = process.dim();

        // Compute drift and diffusion in original space
        process.drift(t, x, &mut work[0..dim]);
        process.diffusion(t, x, &mut work[dim..2 * dim]);

        // Transform to log-space and apply Euler
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
    use super::*;

    #[test]
    fn test_euler_maruyama_step() {
        let params = GbmParams::new(0.05, 0.02, 0.2).unwrap();
        let process = GbmProcess::new(params);
        let disc = EulerMaruyama::new();

        let t = 0.0;
        let dt = 0.01; // 1% of a year
        let mut x = vec![100.0];
        let z = vec![0.5]; // Half a std dev
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // Should be positive
        assert!(x[0] > 0.0);
        // Should have moved up (positive drift + positive shock)
        assert!(x[0] > 100.0);
    }

    #[test]
    fn test_log_euler_positivity() {
        let params = GbmParams::new(0.05, 0.02, 0.4).unwrap(); // High vol
        let process = GbmProcess::new(params);
        let disc = LogEuler::new();

        let t = 0.0;
        let dt = 0.1; // Larger step
        let mut x = vec![100.0];

        // Test multiple large negative shocks
        for _ in 0..100 {
            let z = vec![-3.0]; // Very negative shock
            let mut work = vec![0.0; disc.work_size(&process)];

            disc.step(&process, t, dt, &mut x, &z, &mut work);

            // Log-Euler should maintain positivity
            assert!(x[0] > 0.0, "State should remain positive");
        }
    }

    #[test]
    fn test_euler_convergence() {
        // Test that Euler converges to expected mean as dt -> 0
        let params = GbmParams::new(0.05, 0.02, 0.2).unwrap();
        let process = GbmProcess::new(params);
        let disc = EulerMaruyama::new();

        let t = 0.0;
        let dt = 0.001; // Very small step
        let x0 = 100.0;
        let expected_drift = (0.05 - 0.02) * x0 * dt; // μ S dt

        // Zero shock should give pure drift
        let mut x = vec![x0];
        let z = vec![0.0];
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // Should be close to x0 + expected_drift
        assert!((x[0] - (x0 + expected_drift)).abs() < 0.01);
    }
}
