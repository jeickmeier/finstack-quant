//! Discretization schemes for Schwartz-Smith two-factor commodity model.
//!
//! Uses exact solutions for both components where possible, with correlation
//! handled via Cholesky decomposition.
//!
//! # Exact Solutions
//!
//! - **X (OU with constant drift shift)**:
//!   X_{t+Δt} = X_t e^{-κ_X Δt} − (λ_X/κ_X)(1 − e^{-κ_X Δt})
//!   + σ_X √[(1-e^{-2κ_X Δt})/(2κ_X)] Z_X
//! - **Y (ABM)**: Y_{t+Δt} = Y_t + μ_Y Δt + σ_Y √Δt Z_Y
//!
//! With correlation ρ, the shocks Z_X and Z_Y are correlated.

use super::super::process::schwartz_smith::SchwartzSmithProcess;
use super::super::traits::Discretization;
use finstack_quant_core::math::linalg::{cholesky_correlation, CholeskyError};

/// Exact discretization for Schwartz-Smith process.
///
/// Uses analytical solutions for both X (OU) and Y (arithmetic Brownian motion)
/// with correlation handled via pivoted Cholesky decomposition.
#[derive(Debug, Clone)]
pub struct ExactSchwartzSmith {
    /// Precomputed Cholesky factor for 2×2 correlation matrix [[1, ρ], [ρ, 1]].
    /// Stored in original variable order via `CorrelationFactor`.
    cholesky_factor: finstack_quant_core::math::linalg::CorrelationFactor,
    /// Per-run cache of the `dt`-dependent X-leg constants, populated by
    /// [`Discretization::prepare`]. `None` until prepared (e.g. stepped
    /// directly without the engine), in which case constants are computed inline.
    prepared: Option<SsStepConstants>,
}

/// Path-independent Schwartz-Smith step constants for a fixed step size.
///
/// All quantities depend only on `(κ_X, σ_X, Δt)`, so they are identical on
/// every step of a uniform grid and across every path.
#[derive(Debug, Clone, Copy)]
struct SsStepConstants {
    dt: f64,
    exp_kappa_dt: f64,
    one_minus_exp_over_kappa: f64,
    x_std: f64,
    sqrt_dt: f64,
}

impl SsStepConstants {
    /// Compute the constants for one step size. Mirrors the arithmetic in
    /// [`ExactSchwartzSmith::step`] exactly so cached and inline paths are
    /// bit-identical.
    #[inline]
    fn compute(kappa_x: f64, sigma_x: f64, dt: f64) -> Self {
        let exp_kappa_dt = (-kappa_x * dt).exp();
        let one_minus_exp_over_kappa = -(-kappa_x * dt).exp_m1() / kappa_x;
        let x_std = if (kappa_x * dt).abs() < 1e-8 {
            sigma_x * dt.sqrt() * (1.0 - kappa_x * dt / 2.0)
        } else {
            sigma_x * ((1.0 - (-2.0 * kappa_x * dt).exp()) / (2.0 * kappa_x)).sqrt()
        };
        Self {
            dt,
            exp_kappa_dt,
            one_minus_exp_over_kappa,
            x_std,
            sqrt_dt: dt.sqrt(),
        }
    }
}

impl ExactSchwartzSmith {
    /// Create a new exact Schwartz-Smith discretization.
    ///
    /// # Arguments
    ///
    /// * `rho` - Correlation between X and Y Brownian motions
    ///
    /// The discretization stores a factorization of the two Brownian shocks and
    /// applies the exact Gaussian transition for the mean-reverting short-term
    /// factor and long-term equilibrium factor. It does not own the economic
    /// process parameters; supply those to [`Discretization::step`].
    ///
    /// # Errors
    ///
    /// Returns an error if `rho` cannot form a positive-semidefinite 2×2
    /// correlation matrix (including non-finite or out-of-range values).
    pub fn new(rho: f64) -> finstack_quant_core::Result<Self> {
        // Build 2x2 correlation matrix: [[1.0, rho], [rho, 1.0]]
        let corr_matrix = vec![1.0, rho, rho, 1.0];
        let chol = cholesky_correlation(&corr_matrix, 2).map_err(|e| match e {
            CholeskyError::NotPositiveDefinite { .. } => {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::Invalid)
            }
            CholeskyError::DimensionMismatch { .. } => finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::DimensionMismatch,
            ),
            _ => finstack_quant_core::Error::Input(finstack_quant_core::InputError::Invalid),
        })?;

        Ok(Self {
            cholesky_factor: chol,
            prepared: None,
        })
    }

    /// Create from Schwartz-Smith process (convenience method).
    ///
    /// Uses the process's `rho` and retains no reference to the process, so the
    /// same discretization can be reused only with processes using compatible
    /// two-factor shock conventions.
    ///
    /// # Errors
    ///
    /// Returns the same correlation-factorization error as [`Self::new`].
    pub fn from_process(process: &SchwartzSmithProcess) -> finstack_quant_core::Result<Self> {
        Self::new(process.params().rho)
    }
}

impl Discretization<SchwartzSmithProcess> for ExactSchwartzSmith {
    fn step(
        &self,
        process: &SchwartzSmithProcess,
        _t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        _work: &mut [f64],
    ) {
        let params = process.params();
        let kappa_x = params.kappa_x;
        let sigma_x = params.sigma_x;
        let lambda_x = params.lambda_x;
        let mu_y = params.mu_y;
        let sigma_y = params.sigma_y;

        // Apply correlation to independent shocks via CorrelationFactor::apply.
        // This avoids manual slot indexing and is robust to future pivoting changes.
        let mut z_corr = [0.0; 2];
        let _ = self.cholesky_factor.apply(z, &mut z_corr);

        // Exact solution for X (OU process with constant drift shift −λ_X)
        // X_{t+Δt} = X_t e^{-κ_X Δt} − (λ_X/κ_X)(1 − e^{-κ_X Δt})
        //          + σ_X √[(1-e^{-2κ_X Δt})/(2κ_X)] Z_X
        // The `dt`-dependent constants (e^{-κΔt}, (1−e^{-κΔt})/κ, the X std-dev,
        // and √Δt) are reused from the prepared cache when this step's `dt`
        // matches (exact bit match → identical value); otherwise computed inline
        // so unprepared/non-uniform grids stay bit-identical.
        let consts = match self.prepared {
            Some(c) if c.dt.to_bits() == dt.to_bits() => c,
            _ => SsStepConstants::compute(kappa_x, sigma_x, dt),
        };
        let x_mean = x[0] * consts.exp_kappa_dt - lambda_x * consts.one_minus_exp_over_kappa;
        x[0] = x_mean + consts.x_std * z_corr[0];

        // Exact solution for Y (arithmetic Brownian motion)
        // Y_{t+Δt} = Y_t + μ_Y Δt + σ_Y √Δt Z_Y
        x[1] = x[1] + mu_y * dt + sigma_y * consts.sqrt_dt * z_corr[1];
    }

    fn prepare(&mut self, process: &SchwartzSmithProcess, time_grid: &crate::time_grid::TimeGrid) {
        if time_grid.num_steps() == 0 {
            return;
        }
        let params = process.params();
        self.prepared = Some(SsStepConstants::compute(
            params.kappa_x,
            params.sigma_x,
            time_grid.dt(0),
        ));
    }

    fn work_size(&self, _process: &SchwartzSmithProcess) -> usize {
        0 // No workspace needed (correlation applied inline)
    }

    fn applies_correlation_internally(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::process::schwartz_smith::{SchwartzSmithParams, SchwartzSmithProcess};
    use super::*;

    #[test]
    fn test_exact_schwartz_smith_creation() {
        let _params = SchwartzSmithParams::new(2.0, 0.30, 0.02, 0.15, -0.5);
        let disc = ExactSchwartzSmith::new(-0.5).expect("should succeed");

        assert_eq!(disc.cholesky_factor.factor_matrix().len(), 4);
    }

    #[test]
    fn test_exact_schwartz_smith_step() {
        let params = SchwartzSmithParams::new(2.0, 0.30, 0.02, 0.15, -0.5).unwrap();
        let process = SchwartzSmithProcess::new(params, 0.0, 4.5);
        let disc = ExactSchwartzSmith::from_process(&process).expect("should succeed");

        let mut x = [0.0, 4.5];
        let z = [0.0, 0.0]; // No shock
        let mut work = vec![];

        disc.step(&process, 0.0, 1.0, &mut x, &z, &mut work);

        // With z=0, X should decay: X(1) = 0 * exp(-2) = 0
        assert!((x[0] - 0.0).abs() < 1e-10);
        // Y should drift: Y(1) = 4.5 + 0.02 * 1 = 4.52
        assert!((x[1] - 4.52).abs() < 1e-10);
    }

    #[test]
    fn test_exact_schwartz_smith_spot_computation() {
        let params = SchwartzSmithParams::new(2.0, 0.30, 0.02, 0.15, -0.5).unwrap();
        let process = SchwartzSmithProcess::new(params, 0.0, 4.5);
        let disc = ExactSchwartzSmith::from_process(&process).expect("should succeed");

        let mut x = [0.0, 4.5];
        let z = [0.0, 0.0];
        let mut work = vec![];

        disc.step(&process, 0.0, 1.0, &mut x, &z, &mut work);

        let spot = process.spot_from_state(&x);
        // S = exp(X + Y) = exp(0 + 4.52) ≈ 91.8
        assert!(spot > 90.0 && spot < 92.0);
    }
}
