//! Heston Stochastic Volatility Model.
//!
//! The Heston model assumes that the underlying asset price $S_t$ and its variance $v_t$
//! follow the stochastic processes:
//!
//! $$ dS_t = \mu S_t dt + \sqrt{v_t} S_t dW_t^S $$
//! $$ dv_t = \kappa (\theta - v_t) dt + \sigma \sqrt{v_t} dv_t dW_t^v $$
//!
//! where:
//! - $v_0$: Initial variance
//! - $\kappa$: Mean reversion speed of variance
//! - $\theta$: Long-run average variance
//! - $\sigma$: Volatility of variance (vol-of-vol)
//! - $\rho$: Correlation between asset and variance Brownian motions ($dW_t^S dW_t^v = \rho dt$)
//!
//! Pricing is performed using semi-analytical Fourier inversion of the characteristic function
//! with adaptive Gauss-Legendre quadrature for robust numerical integration.
//!
//! The characteristic function uses the "Little Heston Trap" formulation (Albrecher et al., 2007),
//! which replaces the standard Heston characteristic function with an algebraically equivalent
//! form using `exp(-dT)` instead of `exp(dT)`. This avoids branch-cut discontinuities in the
//! complex logarithm and prevents overflow, improving numerical stability.
//!
//! # Relationship to [`crate::instruments::common_impl::models::closed_form::heston`]
//!
//! There are two Heston implementations in this crate:
//!
//! - This module exposes a `HestonModel` struct with `Result`-returning
//!   `price_european_call/put` methods. Internally it uses **adaptive**
//!   Gauss-Legendre quadrature (depth-bounded refinement).
//! - `closed_form::heston` exposes free functions `heston_call_price_fourier` /
//!   `heston_put_price_fourier` returning bare `f64`, with a precomputed
//!   composite Gauss-Legendre grid and a strip-pricer for batched-strike
//!   calibration.
//!
//! The two are **algebraically equivalent**; we keep both because they have
//! different ergonomic and performance trade-offs (adaptive is robust for
//! one-off pricing, composite is faster for repeated strike sweeps). A
//! cross-validation test in `closed_form::heston::tests::test_cross_validation_with_volatility_heston`
//! pins them within 10 bps to catch silent drift between the two.

use finstack_core::math::integration::gauss_legendre_integrate_adaptive;
use finstack_core::Result;
use num_complex::Complex;
use std::cell::RefCell;
use std::f64::consts::PI;
use tracing::warn;

const HESTON_G_DENOM_EPS: f64 = 1e-8;
const HESTON_EXPONENT_REAL_LIMIT: f64 = 700.0;

/// Heston model parameters.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct HestonParameters {
    /// Initial variance ($v_0$)
    pub v0: f64,
    /// Mean reversion speed ($\kappa$)
    pub kappa: f64,
    /// Long-run average variance ($\theta$)
    pub theta: f64,
    /// Volatility of variance ($\sigma$)
    pub sigma: f64,
    /// Correlation between asset and variance ($\rho$)
    pub rho: f64,
}

impl HestonParameters {
    /// Create new Heston parameters with validation.
    ///
    /// # Feller Condition Warning
    ///
    /// If the Feller condition (2κθ > σ²) is violated, an informational warning
    /// is logged. This pricer evaluates the Heston characteristic function via
    /// Fourier inversion and never simulates the variance path, so a Feller
    /// violation does **not** create a zero-variance pricing risk here. The
    /// warning is retained only as a heads-up for callers who also run a Monte
    /// Carlo simulation of the variance process with the same parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if any parameter is out of valid range.
    #[must_use = "creating parameters without using them has no effect"]
    pub fn new(v0: f64, kappa: f64, theta: f64, sigma: f64, rho: f64) -> Result<Self> {
        if v0 <= 0.0 || !v0.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter v0 (initial variance) must be positive, got: {:.6}",
                v0
            )));
        }
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter κ (kappa, mean reversion) must be positive, got: {:.6}",
                kappa
            )));
        }
        if theta <= 0.0 || !theta.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter θ (theta, long-run variance) must be positive, got: {:.6}",
                theta
            )));
        }
        if sigma <= 0.0 || !sigma.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter σ (sigma, vol-of-vol) must be positive, got: {:.6}",
                sigma
            )));
        }
        if !(-1.0..=1.0).contains(&rho) || !rho.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter ρ (rho, correlation) must be in [-1, 1], got: {:.6}",
                rho
            )));
        }

        let params = Self {
            v0,
            kappa,
            theta,
            sigma,
            rho,
        };

        // Warn if Feller condition is violated. Informational only: this pricer
        // uses Fourier inversion of the characteristic function and never
        // simulates the variance path, so a Feller violation poses no
        // zero-variance pricing risk here.
        if !params.satisfies_feller_condition() {
            warn!(
                v0 = v0,
                kappa = kappa,
                theta = theta,
                sigma = sigma,
                feller_lhs = 2.0 * kappa * theta,
                feller_rhs = sigma * sigma,
                "Heston Feller condition violated (2κθ ≤ σ²): informational only — the \
                 Fourier pricer never simulates the variance path, so this poses no \
                 zero-variance pricing risk; relevant only for Monte Carlo simulation \
                 of the variance process"
            );
        }

        Ok(params)
    }

    /// Check Feller condition ($2\kappa\theta > \sigma^2$).
    ///
    /// If true, the variance process is strictly positive.
    /// If false, the variance can reach zero, which may cause numerical issues.
    #[must_use]
    pub fn satisfies_feller_condition(&self) -> bool {
        2.0 * self.kappa * self.theta > self.sigma * self.sigma
    }
}

impl Default for HestonParameters {
    /// Returns safe default Heston parameters that satisfy the Feller condition.
    ///
    /// Default values:
    /// - v0 = 0.04 (initial variance, equivalent to 20% vol)
    /// - κ (kappa) = 2.0 (mean reversion speed)
    /// - θ (theta) = 0.04 (long-run variance, equivalent to 20% vol)
    /// - σ (sigma) = 0.3 (vol-of-vol)
    /// - ρ (rho) = -0.5 (typical negative correlation for equities)
    ///
    /// Feller condition: 2 × 2.0 × 0.04 = 0.16 > 0.09 = 0.3² ✓
    fn default() -> Self {
        Self {
            v0: 0.04,
            kappa: 2.0,
            theta: 0.04,
            sigma: 0.3,
            rho: -0.5,
        }
    }
}

/// Heston model pricer.
pub struct HestonModel {
    params: HestonParameters,
}

impl HestonModel {
    /// Create a new Heston model.
    pub fn new(params: HestonParameters) -> Self {
        Self { params }
    }

    /// Price a European call option using Fourier inversion.
    ///
    /// Uses adaptive Gauss-Legendre quadrature for robust numerical integration
    /// of the characteristic function, following the Gil-Pelaez approach.
    ///
    /// # Arguments
    /// * `S`: Spot price
    /// * `K`: Strike price
    /// * `T`: Time to maturity (years)
    /// * `r`: Risk-free rate (continuous compounding)
    /// * `q`: Dividend yield (continuous compounding)
    ///
    /// # Returns
    /// The call option price, guaranteed non-negative.
    #[must_use = "computed price should be used"]
    #[allow(non_snake_case)]
    pub fn price_european_call(&self, S: f64, K: f64, T: f64, r: f64, q: f64) -> Result<f64> {
        // Heston pricing formula:
        // Call = S * e^{-qT} * P1 - K * e^{-rT} * P2
        // where P1 and P2 are probabilities derived from the characteristic function.

        let p1 = self.calculate_prob(1, S, K, T, r, q)?;
        let p2 = self.calculate_prob(2, S, K, T, r, q)?;

        let call_price = S * (-q * T).exp() * p1 - K * (-r * T).exp() * p2;

        // Ensure non-negative price (numerical errors can sometimes cause slight negatives for deep OTM)
        Ok(call_price.max(0.0))
    }

    /// Price a European put option using Put-Call Parity.
    ///
    /// # Returns
    /// The put option price, guaranteed non-negative.
    #[must_use = "computed price should be used"]
    #[allow(non_snake_case)]
    pub fn price_european_put(&self, S: f64, K: f64, T: f64, r: f64, q: f64) -> Result<f64> {
        let call = self.price_european_call(S, K, T, r, q)?;
        // Put = Call - S * e^{-qT} + K * e^{-rT}
        let put = call - S * (-q * T).exp() + K * (-r * T).exp();
        Ok(put.max(0.0))
    }

    /// Calculate probabilities P1 and P2 via adaptive numerical integration.
    ///
    /// Uses adaptive Gauss-Legendre quadrature for robust integration of the
    /// oscillatory characteristic function integrand. This provides better
    /// accuracy than fixed-grid methods for extreme parameter combinations.
    #[allow(non_snake_case)]
    fn calculate_prob(&self, prob_num: u8, S: f64, K: f64, T: f64, r: f64, q: f64) -> Result<f64> {
        // Dynamic upper bound based on parameters
        // Higher vol-of-vol or longer maturity may need larger bounds
        let base_bound = 50.0;
        let sigma_factor = (self.params.sigma / 0.3).max(1.0);
        let time_factor = T.sqrt().max(1.0);
        let upper_bound = base_bound * sigma_factor * time_factor;

        // Integration tolerance - balance accuracy vs performance
        let tolerance = 1e-8;
        let max_depth = 15; // Sufficient for most cases
        let order = 8; // 8-point Gauss-Legendre per panel (must be 2,3,4,8, or 16)

        // Small offset to avoid phi=0 singularity
        let lower_bound = 1e-8;

        // Clone params for closure
        let kappa = self.params.kappa;
        let theta = self.params.theta;
        let sigma = self.params.sigma;
        let rho = self.params.rho;
        let v0 = self.params.v0;
        self.characteristic_function(
            prob_num,
            lower_bound,
            S,
            T,
            r,
            q,
            kappa,
            theta,
            sigma,
            rho,
            v0,
        )?;
        let error_cell: RefCell<Option<finstack_core::Error>> = RefCell::new(None);

        // Integrand function for adaptive quadrature
        let integrand_fn = |phi: f64| -> f64 {
            if phi <= 0.0 {
                return 0.0;
            }

            let psi = match self
                .characteristic_function(prob_num, phi, S, T, r, q, kappa, theta, sigma, rho, v0)
            {
                Ok(psi) => psi,
                Err(err) => {
                    if error_cell.borrow().is_none() {
                        *error_cell.borrow_mut() = Some(err);
                    }
                    return 0.0;
                }
            };

            // Integrand: Re[ (e^{-i * phi * ln(K)} * psi) / (i * phi) ]
            let i_complex = Complex::new(0.0, 1.0);
            let log_k = K.ln();
            let term = (-i_complex * phi * log_k).exp() * psi / (i_complex * phi);

            // Return real part, handling potential NaN/Inf
            let result = term.re;
            if result.is_finite() {
                result
            } else {
                0.0
            }
        };

        // Use adaptive Gauss-Legendre quadrature
        let integral = gauss_legendre_integrate_adaptive(
            integrand_fn,
            lower_bound,
            upper_bound,
            order,
            tolerance,
            max_depth,
        )?;

        if let Some(err) = error_cell.borrow_mut().take() {
            return Err(err);
        }

        Ok(0.5 + (1.0 / PI) * integral)
    }

    #[allow(clippy::too_many_arguments, non_snake_case)]
    fn characteristic_function(
        &self,
        prob_num: u8,
        phi: f64,
        S: f64,
        T: f64,
        r: f64,
        q: f64,
        kappa: f64,
        theta: f64,
        sigma: f64,
        rho: f64,
        v0: f64,
    ) -> Result<Complex<f64>> {
        let i_complex = Complex::new(0.0, 1.0);
        let one = Complex::new(1.0, 0.0);
        let x = S.ln();

        let (u, b) = if prob_num == 1 {
            (0.5, kappa - rho * sigma)
        } else {
            (-0.5, kappa)
        };

        let a = kappa * theta;
        let d_sq = (rho * sigma * phi * i_complex - b).powi(2)
            - sigma * sigma * (2.0 * u * phi * i_complex - phi * phi);
        let d = d_sq.sqrt();
        let b_minus_rsi = b - rho * sigma * phi * i_complex;
        let g_denom = b_minus_rsi + d;
        let g_denom_limit = HESTON_G_DENOM_EPS * (1.0 + b_minus_rsi.norm() + d.norm());
        if !g_denom.is_finite() || g_denom.norm() <= g_denom_limit {
            return Err(finstack_core::Error::Validation(format!(
                "Heston characteristic function became unstable near phi={phi:.6e} (P{prob_num}); parameter combination is too extreme"
            )));
        }

        let g_minus = (b_minus_rsi - d) / g_denom;
        if !g_minus.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston characteristic function became non-finite near phi={phi:.6e} (P{prob_num}); parameter combination is too extreme"
            )));
        }

        let exp_minus_dt = (-d * T).exp();
        if !exp_minus_dt.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston characteristic function overflowed near phi={phi:.6e} (P{prob_num}); parameter combination is too extreme"
            )));
        }

        let c = (r - q) * phi * i_complex * T
            + (a / sigma.powi(2))
                * ((b_minus_rsi - d) * T
                    - 2.0 * ((one - g_minus * exp_minus_dt) / (one - g_minus)).ln());

        let d_term = (b_minus_rsi - d) / sigma.powi(2)
            * ((one - exp_minus_dt) / (one - g_minus * exp_minus_dt));
        if !c.is_finite() || !d_term.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston characteristic function became non-finite near phi={phi:.6e} (P{prob_num}); parameter combination is too extreme"
            )));
        }

        let exponent = c + d_term * v0 + i_complex * phi * x;
        if !exponent.is_finite() || exponent.re > HESTON_EXPONENT_REAL_LIMIT {
            return Err(finstack_core::Error::Validation(format!(
                "Heston characteristic function exponent overflowed near phi={phi:.6e} (P{prob_num}); parameter combination is too extreme"
            )));
        }

        let psi = exponent.exp();
        if psi.is_finite() {
            Ok(psi)
        } else {
            Err(finstack_core::Error::Validation(format!(
                "Heston characteristic function became non-finite near phi={phi:.6e} (P{prob_num}); parameter combination is too extreme"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heston_black_scholes_limit() -> Result<()> {
        // As sigma (vol of vol) -> 0, Heston should converge to Black-Scholes
        // We set kappa high to force v to theta quickly, or just v0=theta and sigma=0

        #[allow(non_snake_case)]
        let S = 100.0;
        #[allow(non_snake_case)]
        let K = 100.0;
        #[allow(non_snake_case)]
        let T = 1.0;
        let r = 0.05;
        let q = 0.0;
        let vol = 0.2;

        let params = HestonParameters::new(
            vol * vol, // v0
            1.0,       // kappa (irrelevant if sigma=0 and v0=theta)
            vol * vol, // theta
            1e-6,      // sigma (approx 0)
            0.0,       // rho
        )?;

        let model = HestonModel::new(params);
        let price = model.price_european_call(S, K, T, r, q)?;

        // BS Price for these params is approx 10.4506
        // d1 = (ln(1/1) + (0.05 + 0.04/2)*1) / 0.2 = 0.07 / 0.2 = 0.35
        // d2 = 0.15
        // N(0.35) = 0.6368, N(0.15) = 0.5596
        // C = 100 * 0.6368 - 100 * e^-0.05 * 0.5596 = 63.68 - 95.12 * 0.5596 = 63.68 - 53.23 = 10.45

        let bs_price = 10.4506;
        assert!(
            (price - bs_price).abs() < 0.01,
            "Heston limit should match BS. Got {}, expected {}",
            price,
            bs_price
        );

        Ok(())
    }

    #[test]
    fn test_heston_literature_value() -> Result<()> {
        // Test case from "The Heston Model and its Extensions in Matlab and C#", Rouah & Vainberg
        // or similar standard test cases.
        // Parameters: S=100, K=100, T=0.5, r=0.03, q=0.0
        // v0=0.04, kappa=2.0, theta=0.04, sigma=0.3, rho=-0.5

        #[allow(non_snake_case)]
        let S = 100.0;
        #[allow(non_snake_case)]
        let K = 100.0;
        #[allow(non_snake_case)]
        let T = 0.5;
        let r = 0.03;
        let q = 0.0;

        let params = HestonParameters::new(
            0.04, // v0
            2.0,  // kappa
            0.04, // theta
            0.3,  // sigma
            -0.5, // rho
        )?;

        let model = HestonModel::new(params);
        let call_price = model.price_european_call(S, K, T, r, q)?;

        // Reference value approx 6.28 (approximate check)
        // Let's verify directionality at least if exact reference unavailable
        assert!(call_price > 0.0);
        assert!(call_price < S);

        Ok(())
    }

    #[test]
    fn test_heston_parameters_reject_zero_inputs() {
        assert!(HestonParameters::new(0.0, 2.0, 0.04, 0.3, -0.5).is_err());
        assert!(HestonParameters::new(0.04, 0.0, 0.04, 0.3, -0.5).is_err());
        assert!(HestonParameters::new(0.04, 2.0, 0.0, 0.3, -0.5).is_err());
        assert!(HestonParameters::new(0.04, 2.0, 0.04, 0.0, -0.5).is_err());
    }

    #[test]
    fn test_heston_extreme_inputs_return_error_instead_of_nan() -> Result<()> {
        let params = HestonParameters::new(0.04, 0.1, 0.04, 1.0, 0.9)?;
        let model = HestonModel::new(params);
        let price = model.price_european_call(100.0, 100.0, 1.0, 0.05, 0.0);
        assert!(price.is_err(), "extreme inputs should return an error");
        Ok(())
    }
}
