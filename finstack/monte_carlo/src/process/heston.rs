//! Heston stochastic volatility model with QE discretization scheme.
//!
//! The Heston model extends Black-Scholes by allowing volatility to follow its own
//! stochastic process, capturing the empirically observed volatility smile and term structure.
//! This implementation uses the **Andersen QE (Quadratic Exponential) scheme** for
//! accurate and efficient simulation.
//!
//! # Stochastic Differential Equations
//!
//! Under the risk-neutral measure ℚ:
//!
//! ```text
//! dS_t = (r - q) S_t dt + √v_t S_t dW₁(t)
//! dv_t = κ(θ - v_t) dt + σᵥ √v_t dW₂(t)
//!
//! dW₁ · dW₂ = ρ dt
//! ```
//!
//! where:
//! - **S_t**: Spot price at time t
//! - **v_t**: Instantaneous variance (volatility squared)
//! - **κ**: Mean reversion speed for variance (> 0)
//! - **θ**: Long-term variance level
//! - **σᵥ**: Volatility of variance ("vol of vol")
//! - **ρ**: Correlation between asset and variance innovations
//! - **v₀**: Initial variance level
//!
//! # Feller Condition
//!
//! For positive variance to be guaranteed:
//!
//! ```text
//! 2κθ ≥ σᵥ²
//! ```
//!
//! When violated, variance can reach zero with positive probability.
//! The QE scheme handles this gracefully by truncating negative variances.
//!
//! # QE Discretization Scheme (Andersen 2008)
//!
//! The **Quadratic Exponential (QE)** scheme provides superior accuracy and
//! moment matching compared to simpler Euler schemes:
//!
//! 1. Variance process discretized with moment matching
//! 2. Switch between quadratic and exponential approximations based on ψ critical value
//! 3. Asset process uses a QE-style martingale-corrected log update given the
//!    simulated variance path
//!
//! **Advantages over Euler**:
//! - Maintains positive variance naturally
//! - Better moment matching
//! - Reduced discretization bias
//! - Handles high σᵥ robustly
//!
//! # References
//!
//! ## Primary Sources
//!
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic
//!   Volatility with Applications to Bond and Currency Options."
//!   *Review of Financial Studies*, 6(2), 327-343.
//!   (Original Heston model and semi-analytical pricing via FFT)
//!
//! - Andersen, L. (2008). "Simple and Efficient Simulation of the Heston
//!   Stochastic Volatility Model." *Journal of Computational Finance*, 11(3), 1-42.
//!   (QE discretization scheme - recommended method)
//!
//! ## Alternative Discretization Schemes
//!
//! - Lord, R., Koekkoek, R., & Van Dijk, D. (2010). "A Comparison of Biased
//!   Simulation Schemes for Stochastic Volatility Models." *Quantitative Finance*,
//!   10(2), 177-194.
//!   (Comprehensive comparison: Euler, Milstein, QE, IJK, Broadie-Kaya)
//!
//! - Broadie, M., & Kaya, Ö. (2006). "Exact Simulation of Stochastic Volatility
//!   and Other Affine Jump Diffusion Processes." *Operations Research*, 54(2), 217-231.
//!   (Exact scheme, computationally expensive)
//!
//! ## Calibration and Applications
//!
//! - Bakshi, G., Cao, C., & Chen, Z. (1997). "Empirical Performance of Alternative
//!   Option Pricing Models." *Journal of Finance*, 52(5), 2003-2049.
//!
//! - Gatheral, J. (2006). *The Volatility Surface: A Practitioner's Guide*. Wiley.
//!   (Practical calibration techniques)
//!
//! # Implementation Details
//!
//! - Uses **Andersen QE scheme** by default (best accuracy/speed tradeoff)
//! - Variance truncation at zero to prevent negative values
//! - Correlated Brownian motions via Cholesky: W₂ = ρW₁ + √(1-ρ²)W₂'
//! - Martingale-corrected log spot update conditional on the simulated variance path
//!
//! # Examples
//!
//! ```ignore
//! use finstack_monte_carlo::process::heston::{
//!     HestonProcess, HestonParams
//! };
//!
//! // Typical calibrated parameters for equity index
//! let params = HestonParams::new(
//!     0.05,   // r = 5% risk-free rate
//!     0.02,   // q = 2% dividend yield
//!     2.0,    // κ = mean reversion speed
//!     0.04,   // θ = long-term variance (20% long-term vol)
//!     0.3,    // σᵥ = vol of vol
//!     -0.7,   // ρ = correlation (typically negative for equity)
//!     0.04,   // v₀ = initial variance (20% current vol)
//! )
//! .unwrap();
//!
//! let heston = HestonProcess::new(params.clone());
//!
//! // Check Feller condition
//! let feller = 2.0 * params.kappa * params.theta;
//! let sigma_v_sq = params.sigma_v * params.sigma_v;
//! println!("Feller satisfied: {}", feller >= sigma_v_sq);
//! ```

use super::super::paths::ProcessParams;
use super::super::traits::StochasticProcess;
use super::metadata::ProcessMetadata;

/// Heston model parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HestonParams {
    /// Risk-free rate
    pub r: f64,
    /// Dividend/foreign rate
    pub q: f64,
    /// Mean reversion speed
    pub kappa: f64,
    /// Long-term variance
    pub theta: f64,
    /// Volatility of variance
    pub sigma_v: f64,
    /// Correlation between asset and variance
    pub rho: f64,
    /// Initial variance
    pub v0: f64,
}

impl HestonParams {
    /// Create new Heston parameters.
    ///
    /// # Arguments
    ///
    /// * `r` - Risk-free rate
    /// * `q` - Dividend yield
    /// * `kappa` - Mean reversion speed (> 0)
    /// * `theta` - Long-term variance (> 0)
    /// * `sigma_v` - Vol-of-vol (> 0)
    /// * `rho` - Correlation in [-1, 1]
    /// * `v0` - Initial variance (> 0)
    pub fn new(
        r: f64,
        q: f64,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        rho: f64,
        v0: f64,
    ) -> finstack_core::Result<Self> {
        if !r.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter r must be finite, got {r}"
            )));
        }
        if !q.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter q must be finite, got {q}"
            )));
        }
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter kappa must be positive, got {kappa}"
            )));
        }
        if theta <= 0.0 || !theta.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter theta must be positive, got {theta}"
            )));
        }
        if sigma_v <= 0.0 || !sigma_v.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter sigma_v must be positive, got {sigma_v}"
            )));
        }
        if !(-1.0..=1.0).contains(&rho) || !rho.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter rho must be in [-1, 1], got {rho}"
            )));
        }
        if v0 <= 0.0 || !v0.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter v0 must be positive, got {v0}"
            )));
        }

        Ok(Self {
            r,
            q,
            kappa,
            theta,
            sigma_v,
            rho,
            v0,
        })
    }

    /// Check Feller condition: 2κθ > σ_v²
    ///
    /// When satisfied, the variance process stays strictly positive.
    pub fn satisfies_feller(&self) -> bool {
        2.0 * self.kappa * self.theta > self.sigma_v * self.sigma_v
    }
}

/// Heston stochastic volatility process.
///
/// State: [S, v] (spot and variance)
/// Factors: 2 correlated Brownian motions
#[derive(Debug, Clone)]
pub struct HestonProcess {
    params: HestonParams,
}

impl HestonProcess {
    /// Create a new Heston process.
    ///
    /// # Feller Condition Warning
    ///
    /// If the Feller condition (2κθ ≥ σᵥ²) is violated, a warning is logged.
    /// When violated, the variance process can reach zero with positive probability,
    /// though the QE scheme handles this gracefully via truncation.
    pub fn new(params: HestonParams) -> Self {
        // Warn when Feller condition is violated (variance may hit zero)
        if !params.satisfies_feller() {
            let feller_ratio =
                2.0 * params.kappa * params.theta / (params.sigma_v * params.sigma_v);
            tracing::warn!(
                kappa = params.kappa,
                theta = params.theta,
                sigma_v = params.sigma_v,
                feller_ratio = feller_ratio,
                "Heston Feller condition violated (2κθ < σᵥ²): variance may reach zero. \
                 Feller ratio = {:.4} (should be ≥ 1.0). QE scheme will truncate at zero.",
                feller_ratio
            );
        }
        Self { params }
    }

    /// Create with explicit parameters.
    pub fn with_params(
        r: f64,
        q: f64,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        rho: f64,
        v0: f64,
    ) -> finstack_core::Result<Self> {
        Ok(Self::new(HestonParams::new(
            r, q, kappa, theta, sigma_v, rho, v0,
        )?))
    }

    /// Get parameters.
    pub fn params(&self) -> &HestonParams {
        &self.params
    }
}

impl StochasticProcess for HestonProcess {
    fn dim(&self) -> usize {
        2 // S and v
    }

    fn num_factors(&self) -> usize {
        2 // Two Brownian motions
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        let s = x[0];
        let v = x[1].max(0.0);

        // dS/dt = (r - q) S
        out[0] = (self.params.r - self.params.q) * s;

        // dv/dt = κ(θ - v)
        out[1] = self.params.kappa * (self.params.theta - v);
    }

    fn diffusion(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        let s = x[0];
        let v = x[1].max(0.0); // Ensure non-negative for sqrt

        // Diffusion for S: √v S
        out[0] = v.sqrt() * s;

        // Diffusion for v: σ_v √v
        out[1] = self.params.sigma_v * v.sqrt();
    }

    fn factor_correlation(&self) -> Option<Vec<f64>> {
        let rho = self.params.rho;
        Some(vec![1.0, rho, rho, 1.0])
    }
}

impl ProcessMetadata for HestonProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("Heston");
        params.add_param("r", self.params.r);
        params.add_param("q", self.params.q);
        params.add_param("kappa", self.params.kappa);
        params.add_param("theta", self.params.theta);
        params.add_param("sigma_v", self.params.sigma_v);
        params.add_param("rho", self.params.rho);
        params.add_param("v0", self.params.v0);

        // Create 2x2 correlation matrix for [S, v]
        let correlation = vec![1.0, self.params.rho, self.params.rho, 1.0];

        params
            .with_correlation(correlation)
            .with_factors(vec!["spot".to_string(), "variance".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heston_params() {
        let params = HestonParams::new(
            0.05, // r
            0.02, // q
            2.0,  // kappa
            0.04, // theta
            0.3,  // sigma_v
            -0.5, // rho
            0.04, // v0
        )
        .expect("valid");

        assert_eq!(params.kappa, 2.0);
        assert!(params.satisfies_feller());
    }

    #[test]
    fn test_feller_condition() {
        let params_feller =
            HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.2, -0.5, 0.04).expect("valid");
        assert!(params_feller.satisfies_feller());

        let params_no_feller =
            HestonParams::new(0.05, 0.02, 0.5, 0.04, 0.5, -0.5, 0.04).expect("valid");
        assert!(!params_no_feller.satisfies_feller());
    }

    #[test]
    fn test_heston_drift_diffusion() {
        let heston =
            HestonProcess::with_params(0.05, 0.02, 2.0, 0.04, 0.3, -0.5, 0.04).expect("valid");

        let x = vec![100.0_f64, 0.04_f64];
        let mut drift = vec![0.0_f64; 2];
        let mut diffusion = vec![0.0_f64; 2];

        heston.drift(0.0, &x, &mut drift);
        heston.diffusion(0.0, &x, &mut diffusion);

        // S drift: (r-q)S = 0.03 * 100 = 3.0
        assert!((drift[0] - 3.0).abs() < 1e-10);

        // v drift: κ(θ-v) = 2.0 * (0.04 - 0.04) = 0
        assert!((drift[1] - 0.0).abs() < 1e-10);

        // S diffusion: √v S = √0.04 * 100 = 0.2 * 100 = 20
        assert!((diffusion[0] - 20.0).abs() < 1e-10);

        // v diffusion: σ_v √v = 0.3 * 0.2 = 0.06
        assert!((diffusion[1] - 0.06).abs() < 1e-10);
    }

    #[test]
    fn test_invalid_params_negative_kappa() {
        assert!(HestonParams::new(0.05, 0.02, -1.0, 0.04, 0.3, -0.5, 0.04).is_err());
    }

    #[test]
    fn test_invalid_params_rho_out_of_range() {
        assert!(HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, 1.5, 0.04).is_err());
    }
}
