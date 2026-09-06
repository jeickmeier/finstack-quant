//! Discretization schemes for Schwartz-Smith two-factor commodity model.
//!
//! Uses the exact joint Gaussian transition for both components.
//!
//! # Exact Solutions
//!
//! - **X (OU with constant drift shift)**:
//!   X_{t+Δt} = X_t e^{-κ_X Δt} − (λ_X/κ_X)(1 − e^{-κ_X Δt})
//!   + σ_X √[(1-e^{-2κ_X Δt})/(2κ_X)] Z_X
//! - **Y (ABM)**: Y_{t+Δt} = Y_t + μ_Y Δt + σ_Y √Δt Z_Y
//!
//! The integrated shocks have covariance
//! `ρ σ_X σ_Y (1 − exp(−κ_X Δt)) / κ_X`. Their correlation differs
//! from the instantaneous Brownian correlation ρ when Δt is positive.

use super::super::process::schwartz_smith::SchwartzSmithProcess;
use super::super::traits::Discretization;

/// Exact discretization for Schwartz-Smith process.
///
/// Uses analytical solutions for both X (OU) and Y (arithmetic Brownian motion)
/// with the analytical 2×2 Cholesky factor of the integrated covariance.
#[derive(Debug, Clone)]
pub struct ExactSchwartzSmith {
    /// Instantaneous correlation of the driving Brownian motions.
    rho: f64,
    /// Per-run cache of the `dt`-dependent X-leg constants, populated by
    /// [`Discretization::prepare`]. `None` until prepared (e.g. stepped
    /// directly without the engine), in which case constants are computed inline.
    prepared: Option<SsStepConstants>,
}

/// Path-independent Schwartz-Smith step constants for a fixed step size.
///
/// All quantities depend only on `(κ_X, σ_X, ρ, Δt)`, so they are identical on
/// every step of a uniform grid and across every path.
#[derive(Debug, Clone, Copy)]
struct SsStepConstants {
    dt: f64,
    kappa_x: f64,
    sigma_x: f64,
    exp_kappa_dt: f64,
    one_minus_exp_over_kappa: f64,
    x_std: f64,
    sqrt_dt: f64,
    transition_rho: f64,
    independent_weight: f64,
}

impl SsStepConstants {
    /// Compute the constants for one step size. Mirrors the arithmetic in
    /// [`ExactSchwartzSmith::step`] exactly so cached and inline paths are
    /// bit-identical.
    #[inline]
    fn compute(kappa_x: f64, sigma_x: f64, rho: f64, dt: f64) -> Self {
        let exp_kappa_dt = (-kappa_x * dt).exp();
        let one_minus_exp_over_kappa = -(-kappa_x * dt).exp_m1() / kappa_x;
        let x_time_std = (-(-2.0 * kappa_x * dt).exp_m1() / (2.0 * kappa_x)).sqrt();
        let sqrt_dt = dt.sqrt();
        let transition_rho =
            (rho * (one_minus_exp_over_kappa / x_time_std / sqrt_dt)).clamp(-1.0, 1.0);
        Self {
            dt,
            kappa_x,
            sigma_x,
            exp_kappa_dt,
            one_minus_exp_over_kappa,
            x_std: sigma_x * x_time_std,
            sqrt_dt,
            transition_rho,
            independent_weight: ((1.0 - transition_rho) * (1.0 + transition_rho)).sqrt(),
        }
    }
}

impl ExactSchwartzSmith {
    /// Create a new exact Schwartz-Smith discretization.
    ///
    /// # Arguments
    ///
    /// * `rho` - Instantaneous correlation between X and Y Brownian motions;
    ///   must be finite and in `[-1, 1]`.
    ///
    /// The discretization stores the instantaneous Brownian correlation and
    /// applies the exact Gaussian transition for the mean-reverting short-term
    /// factor and long-term equilibrium factor. It does not own the economic
    /// process parameters; supply those to [`Discretization::step`].
    ///
    /// # Errors
    ///
    /// Returns an input error for non-finite or out-of-range correlation.
    pub fn new(rho: f64) -> finstack_quant_core::Result<Self> {
        if !rho.is_finite() || !(-1.0..=1.0).contains(&rho) {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        Ok(Self {
            rho,
            prepared: None,
        })
    }

    /// Create from Schwartz-Smith process (convenience method).
    ///
    /// Uses the process's `rho` and retains no reference to the process, so the
    /// same discretization can be reused only with processes using compatible
    /// two-factor shock conventions.
    ///
    /// # Arguments
    ///
    /// * `process` - Process supplying the instantaneous Brownian correlation.
    ///
    /// # Errors
    ///
    /// Returns the same invalid-correlation error as [`Self::new`].
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
        if dt == 0.0 {
            return;
        }
        let params = process.params();
        let kappa_x = params.kappa_x;
        let sigma_x = params.sigma_x;
        let lambda_x = params.lambda_x;
        let mu_y = params.mu_y;
        let sigma_y = params.sigma_y;

        // Reuse the prepared `dt`-dependent constants on an exact bit match;
        // otherwise compute inline for unprepared or non-uniform grids.
        let consts = match self.prepared {
            Some(c)
                if c.dt.to_bits() == dt.to_bits()
                    && c.kappa_x.to_bits() == kappa_x.to_bits()
                    && c.sigma_x.to_bits() == sigma_x.to_bits() =>
            {
                c
            }
            _ => SsStepConstants::compute(kappa_x, sigma_x, self.rho, dt),
        };
        let x_mean = x[0] * consts.exp_kappa_dt - lambda_x * consts.one_minus_exp_over_kappa;
        x[0] = x_mean + consts.x_std * z[0];

        let y_shock = consts.transition_rho * z[0] + consts.independent_weight * z[1];
        x[1] = x[1] + mu_y * dt + sigma_y * consts.sqrt_dt * y_shock;
    }

    fn prepare(
        &mut self,
        process: &SchwartzSmithProcess,
        time_grid: &crate::monte_carlo::TimeGrid,
    ) {
        if time_grid.num_steps() == 0 {
            return;
        }
        let params = process.params();
        self.prepared = Some(SsStepConstants::compute(
            params.kappa_x,
            params.sigma_x,
            self.rho,
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
        for rho in [-1.0, -0.5, 0.0, 1.0] {
            assert!(ExactSchwartzSmith::new(rho).is_ok());
        }
        for rho in [f64::NAN, f64::INFINITY, -1.01, 1.01] {
            assert!(ExactSchwartzSmith::new(rho).is_err());
        }
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
