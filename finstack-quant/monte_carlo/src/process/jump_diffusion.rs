//! Jump-diffusion processes (Merton, Kou).
//!
//! Implements GBM with Poisson jumps for modeling:
//! - Equity returns with crashes/rallies
//! - FX rates with regime changes
//! - Commodities with supply shocks
//!
//! # Merton Jump-Diffusion SDE
//!
//! ```text
//! dS_t/S_t = (μ - λk)dt + σ dW_t + (J-1)dN_t
//! ```
//!
//! where:
//! - μ = drift rate (r - q in risk-neutral measure)
//! - σ = continuous diffusion volatility
//! - λ = jump intensity (jumps per year)
//! - k = E[J - 1] = jump compensation term
//! - J = jump size (typically log-normal)
//! - N_t = Poisson process with intensity λ
//!
//! # Jump Size Distribution
//!
//! For log-normal jumps: ln(J) ~ N(μ_J, σ_J²)
//! Then k = E[J - 1] = e^{μ_J + σ_J²/2} - 1

use super::super::traits::StochasticProcess;
use super::gbm::GbmParams;

/// Number of standard-normal factors used by [`crate::discretization::JumpEuler`].
///
/// The factors are the diffusion shock, the Poisson-arrival shock, and one
/// aggregate jump-size shock. Conditional on a sampled jump count `N`, the sum
/// of `N` independent normal log-jump sizes is sampled as
/// `N * mu_j + sqrt(N) * sigma_j * z`.
pub const MERTON_JUMP_FACTORS: usize = 3;

/// Merton jump-diffusion parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MertonJumpParams {
    /// Base GBM parameters (drift, dividend, volatility)
    pub gbm: GbmParams,
    /// Jump intensity (average jumps per year)
    pub lambda: f64,
    /// Log-jump mean (μ_J)
    pub mu_j: f64,
    /// Log-jump standard deviation (σ_J)
    pub sigma_j: f64,
}

impl MertonJumpParams {
    /// Construct risk-neutral Merton jump-diffusion parameters.
    ///
    /// # Arguments
    ///
    /// * `r` - Annual continuously compounded risk-free rate.
    /// * `q` - Annual continuously compounded dividend or foreign rate.
    /// * `sigma` - Annualized continuous-diffusion volatility.
    /// * `lambda` - Poisson intensity in expected jumps per year.
    /// * `mu_j` - Dimensionless mean of the normal log-jump size `ln(J)`.
    /// * `sigma_j` - Dimensionless standard deviation of `ln(J)`.
    ///
    /// The process applies the risk-neutral compensation
    /// `lambda * (exp(mu_j + sigma_j² / 2) - 1)` to the GBM drift.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] for non-finite
    /// `mu_j`, negative or non-finite `lambda`/`sigma_j`, or invalid base GBM
    /// inputs as checked by [`GbmParams::new`].
    pub fn new(
        r: f64,
        q: f64,
        sigma: f64,
        lambda: f64,
        mu_j: f64,
        sigma_j: f64,
    ) -> finstack_quant_core::Result<Self> {
        if !lambda.is_finite() || lambda < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Jump intensity must be finite and non-negative, got {lambda}"
            )));
        }
        if !sigma_j.is_finite() || sigma_j < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Jump volatility must be finite and non-negative, got {sigma_j}"
            )));
        }
        if !mu_j.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Jump mean must be finite, got {mu_j}"
            )));
        }

        Ok(Self {
            gbm: GbmParams::new(r, q, sigma)?,
            lambda,
            mu_j,
            sigma_j,
        })
    }

    /// Compute jump compensation term k = E[J - 1].
    ///
    /// For log-normal jumps: k = e^{μ_J + σ_J²/2} - 1
    pub fn jump_compensation(&self) -> f64 {
        (self.mu_j + 0.5 * self.sigma_j * self.sigma_j).exp() - 1.0
    }

    /// Compensated drift for risk-neutral measure.
    ///
    /// μ_compensated = r - q - λk
    pub fn compensated_drift(&self) -> f64 {
        self.gbm.r - self.gbm.q - self.lambda * self.jump_compensation()
    }
}

/// Merton jump-diffusion process.
///
/// Combines geometric Brownian motion with Poisson jumps.
///
/// State dimension: 1 (spot S)
/// Factor dimension: 3 (diffusion shock + Poisson shock + aggregate jump shock)
///
/// # Usage
///
/// Requires specialized `JumpEuler` discretization that handles
/// Poisson arrival and log-normal jump sizes.
#[derive(Debug, Clone)]
pub struct MertonJumpProcess {
    params: MertonJumpParams,
}

impl MertonJumpProcess {
    /// Create a new Merton jump-diffusion process.
    pub fn new(params: MertonJumpParams) -> Self {
        Self { params }
    }

    /// Create with explicit parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if any parameter is invalid (see [`MertonJumpParams::new`]).
    pub fn with_params(
        r: f64,
        q: f64,
        sigma: f64,
        lambda: f64,
        mu_j: f64,
        sigma_j: f64,
    ) -> finstack_quant_core::Result<Self> {
        Ok(Self::new(MertonJumpParams::new(
            r, q, sigma, lambda, mu_j, sigma_j,
        )?))
    }

    /// Get parameters.
    pub fn params(&self) -> &MertonJumpParams {
        &self.params
    }
}

impl StochasticProcess for MertonJumpProcess {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        MERTON_JUMP_FACTORS
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // Compensated drift: μ(S) = (r - q - λk) S
        out[0] = self.params.compensated_drift() * x[0];
    }

    fn diffusion(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // Continuous diffusion: σ(S) = σ S
        out[0] = self.params.gbm.sigma * x[0];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merton_params() {
        let params = MertonJumpParams::new(
            0.05,  // r
            0.02,  // q
            0.2,   // sigma
            2.0,   // lambda (2 jumps/year on average)
            -0.05, // mu_j (slightly negative jumps)
            0.1,   // sigma_j
        )
        .unwrap();

        assert_eq!(params.lambda, 2.0);
        assert_eq!(params.mu_j, -0.05);
        assert_eq!(params.sigma_j, 0.1);
    }

    #[test]
    fn test_jump_compensation() {
        let params = MertonJumpParams::new(
            0.05, 0.02, 0.2, 1.0,  // lambda
            -0.1, // mu_j (negative jumps)
            0.15, // sigma_j
        )
        .unwrap();

        let k = params.jump_compensation();

        // k = e^{-0.1 + 0.5*0.15²} - 1 ≈ e^{-0.08875} - 1 ≈ -0.085
        assert!(k < 0.0); // Negative jumps → negative compensation
        let expected: f64 = (-0.1_f64 + 0.5 * 0.15 * 0.15).exp() - 1.0;
        assert!((k - expected).abs() < 1e-10);
    }

    #[test]
    fn test_compensated_drift() {
        let params = MertonJumpParams::new(
            0.05, 0.02, 0.2, 2.0, // lambda
            0.0, // mu_j (neutral jumps)
            0.1, // sigma_j
        )
        .unwrap();

        let drift = params.compensated_drift();

        // Drift = r - q - λk = 0.05 - 0.02 - 2*(e^{0.005} - 1)
        // With mu_j=0, sigma_j=0.1: k = e^{0.005} - 1 ≈ 0.00501
        let k: f64 = (0.5_f64 * 0.1 * 0.1).exp() - 1.0;
        let expected = 0.05 - 0.02 - 2.0 * k;

        assert!((drift - expected).abs() < 1e-6);
    }

    #[test]
    fn test_merton_drift() {
        let params = MertonJumpParams::new(0.05, 0.02, 0.2, 1.0, -0.05, 0.1).unwrap();
        let process = MertonJumpProcess::new(params);

        let x = vec![100.0];
        let mut drift = vec![0.0];

        process.drift(0.0, &x, &mut drift);

        // Drift should be compensated_drift * S
        let expected_drift_rate = process.params().compensated_drift();
        assert_eq!(drift[0], expected_drift_rate * 100.0);
    }

    #[test]
    fn test_merton_diffusion() {
        let params = MertonJumpParams::new(0.05, 0.02, 0.25, 1.0, 0.0, 0.1).unwrap();
        let process = MertonJumpProcess::new(params);

        let x = vec![100.0];
        let mut diffusion = vec![0.0];

        process.diffusion(0.0, &x, &mut diffusion);

        // Diffusion should be sigma * S
        assert_eq!(diffusion[0], 0.25 * 100.0);
    }

    #[test]
    fn test_zero_jumps_reduces_to_gbm() {
        // With lambda = 0, should behave like GBM
        let params = MertonJumpParams::new(0.05, 0.02, 0.2, 0.0, 0.0, 0.0).unwrap();
        let process = MertonJumpProcess::new(params);

        // Drift should be (r - q) * S
        let x = vec![100.0];
        let mut drift = vec![0.0];
        process.drift(0.0, &x, &mut drift);

        assert_eq!(drift[0], (0.05 - 0.02) * 100.0);
    }
}
