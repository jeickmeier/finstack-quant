//! One-factor Gaussian copula for credit portfolio correlation.
//!
//! The standard market model for credit derivative correlation modeling.
//! Assumes a single systematic factor drives all defaults.
//!
//! # Mathematical Model
//!
//! Latent variable for entity i:
//! ```text
//! Aᵢ = √ρ · Z + √(1-ρ) · εᵢ
//! ```
//!
//! where:
//! - ρ is the asset correlation
//! - Z ~ N(0,1) is the common (systematic) factor
//! - εᵢ ~ N(0,1) is the idiosyncratic factor for entity i
//!
//! Default occurs when: Aᵢ ≤ Φ⁻¹(PD)
//!
//! # Conditional Default Probability
//!
//! Given the systematic factor Z:
//! ```text
//! P(default | Z) = Φ((Φ⁻¹(PD) - √ρ · Z) / √(1-ρ))
//! ```
//!
//! # Limitations
//!
//! - Zero tail dependence: joint extreme events are underestimated
//! - Correlation "smile" required via base correlation framework
//! - Static correlation doesn't capture stress dynamics
//!
//! # Numerical Dependencies
//!
//! - Standard-normal CDF Φ and inverse-CDF Φ⁻¹ are delegated to
//!   [`finstack_quant_core::math::norm_cdf`] and `standard_normal_inv_cdf`, which
//!   wrap the `statrs` crate's `Normal` distribution. No named approximation
//!   such as Beasley-Springer-Moro or Acklam is used; tail accuracy is
//!   determined by the underlying statrs implementation.
//! - Conditional-PD integration uses Gauss-Hermite quadrature; see
//!   [`super::DEFAULT_QUADRATURE_ORDER`] (typically 20 nodes).
//!
//! # References
//!
//! - Gaussian copula reference: `docs/REFERENCES.md#li-2000-gaussian-copula`
//!

use super::{get_cached_quadrature, Copula, DEFAULT_QUADRATURE_ORDER};
use finstack_quant_core::math::{norm_cdf, GaussHermiteQuadrature};
use std::sync::Arc;

/// Lower valid asset-correlation boundary.
const MIN_CORRELATION: f64 = 0.0;
/// Upper valid asset-correlation boundary.
const MAX_CORRELATION: f64 = 1.0;
/// CDF argument clipping to prevent overflow.
const CDF_CLIP: f64 = 10.0;

/// One-factor Gaussian copula model.
///
/// The industry-standard model for credit index tranche pricing,
/// used with base correlation to capture the correlation smile.
///
/// # Numerical Stability
///
/// - Every valid correlation in `[0, 1]` is preserved exactly.
/// - The exact independence and comonotonic boundary limits are implemented.
/// - CDF arguments are clipped to `[-10, 10]` for interior correlations.
///
/// # References
///
/// - `docs/REFERENCES.md#li-2000-gaussian-copula`
pub struct GaussianCopula {
    /// Quadrature order for integration
    quadrature_order: u8,
    /// Cached quadrature for performance (Arc for cheap clone)
    quadrature: Arc<GaussHermiteQuadrature>,
}

impl Clone for GaussianCopula {
    fn clone(&self) -> Self {
        Self {
            quadrature_order: self.quadrature_order,
            quadrature: Arc::clone(&self.quadrature),
        }
    }
}

impl std::fmt::Debug for GaussianCopula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GaussianCopula")
            .field("quadrature_order", &self.quadrature_order)
            .finish()
    }
}

impl Default for GaussianCopula {
    fn default() -> Self {
        Self::new()
    }
}

impl GaussianCopula {
    /// Create a new Gaussian copula with default parameters.
    ///
    /// Uses 20-point Gauss-Hermite quadrature for integration
    /// (industry standard range: 20-50 points for tranche pricing).
    ///
    /// # Returns
    ///
    /// A one-factor Gaussian copula using the default quadrature order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_models::correlation::{Copula, GaussianCopula};
    /// use finstack_quant_core::math::standard_normal_inv_cdf;
    ///
    /// let copula = GaussianCopula::new();
    /// let threshold = standard_normal_inv_cdf(0.05);
    /// let cond_pd = copula.conditional_default_prob(threshold, &[-1.0], 0.30);
    ///
    /// assert!(cond_pd > 0.0 && cond_pd < 1.0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        let order = DEFAULT_QUADRATURE_ORDER;
        Self {
            quadrature_order: order,
            quadrature: get_cached_quadrature(order),
        }
    }

    /// Create with custom quadrature order for higher precision.
    ///
    /// # Arguments
    /// * `order` - Number of Gauss-Hermite quadrature points. Higher = more accuracy.
    ///
    /// # Returns
    ///
    /// A one-factor Gaussian copula using the requested quadrature order, or the
    /// default order if the requested value is unsupported.
    #[must_use]
    pub fn with_quadrature_order(order: u8) -> Self {
        Self {
            quadrature_order: order,
            quadrature: get_cached_quadrature(order),
        }
    }
}

impl Copula for GaussianCopula {
    fn conditional_default_prob(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
    ) -> f64 {
        // Length mismatch is a programmer error: caller passed the wrong
        // number of factors for this copula. In debug, fail loudly so bugs
        // are caught at test time. In release, fall back to the *unconditional*
        // probability Φ(c) rather than a biased Z=0 assumption — this is the
        // no-information answer, which keeps downstream aggregations unbiased
        // even if individual draws lose per-name conditional structure.
        debug_assert_eq!(
            factor_realization.len(),
            1,
            "GaussianCopula expects exactly 1 factor, got {}",
            factor_realization.len()
        );
        if factor_realization.len() != 1 {
            tracing::error!(
                expected = 1,
                actual = factor_realization.len(),
                "GaussianCopula: factor length mismatch; returning unconditional PD"
            );
            return norm_cdf(default_threshold);
        }
        let [z] = factor_realization else {
            return norm_cdf(default_threshold);
        };
        let z = *z;

        if !correlation.is_finite() || !(MIN_CORRELATION..=MAX_CORRELATION).contains(&correlation) {
            return f64::NAN;
        }
        if correlation == MIN_CORRELATION {
            return norm_cdf(default_threshold);
        }
        if correlation == MAX_CORRELATION {
            return f64::from(z <= default_threshold);
        }

        let sqrt_rho = correlation.sqrt();
        let sqrt_1mr = (1.0 - correlation).sqrt();
        let conditional_threshold = (default_threshold - sqrt_rho * z) / sqrt_1mr;
        norm_cdf(conditional_threshold.clamp(-CDF_CLIP, CDF_CLIP))
    }

    fn conditional_default_prob_given_systematic_and_mixing(
        &self,
        default_threshold: f64,
        systematic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64 {
        // The Gaussian copula has no mixing variable (`sample_mixing` returns
        // 1.0); the (Z, W) conditional reduces to the plain Z conditional.
        let _ = mixing;
        self.conditional_default_prob(default_threshold, &[systematic], correlation)
    }

    fn integrate_fn(&self, f: &dyn Fn(&[f64]) -> f64) -> f64 {
        // Gauss-Hermite quadrature over standard normal factor Z
        self.quadrature.integrate(|z| f(&[z]))
    }

    fn num_factors(&self) -> usize {
        1
    }

    fn model_name(&self) -> &'static str {
        "One-Factor Gaussian Copula"
    }

    fn tail_dependence(&self, _correlation: f64) -> f64 {
        // Gaussian copula has zero tail dependence
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::math::standard_normal_inv_cdf;

    #[test]
    fn test_gaussian_copula_creation() {
        let copula = GaussianCopula::new();
        assert_eq!(copula.num_factors(), 1);
        assert_eq!(copula.model_name(), "One-Factor Gaussian Copula");
    }

    #[test]
    fn test_conditional_default_prob_at_zero_factor() {
        let copula = GaussianCopula::new();
        let pd = 0.05; // 5% default probability
        let threshold = standard_normal_inv_cdf(pd);
        let correlation = 0.30;

        // At Z=0, the conditional probability is Φ(Φ⁻¹(PD) / √(1-ρ))
        let cond_prob = copula.conditional_default_prob(threshold, &[0.0], correlation);

        assert!(cond_prob > 0.0 && cond_prob < 1.0);
        // At Z=0 with positive correlation, conditional should differ from unconditional
        assert!(
            cond_prob < pd,
            "At Z=0 with correlation, conditional should differ from unconditional"
        );
    }

    #[test]
    fn test_conditional_default_prob_increases_with_negative_z() {
        let copula = GaussianCopula::new();
        let threshold = standard_normal_inv_cdf(0.05);
        let correlation = 0.30;

        let prob_z_neg = copula.conditional_default_prob(threshold, &[-2.0], correlation);
        let prob_z_zero = copula.conditional_default_prob(threshold, &[0.0], correlation);
        let prob_z_pos = copula.conditional_default_prob(threshold, &[2.0], correlation);

        // Negative Z (bad market) should increase default probability
        assert!(prob_z_neg > prob_z_zero);
        // Positive Z (good market) should decrease default probability
        assert!(prob_z_pos < prob_z_zero);
    }

    #[test]
    fn test_integration_recovers_unconditional() {
        let copula = GaussianCopula::new();
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
        let correlation = 0.30;

        // E[P(default|Z)] should equal P(default)
        let integrated_prob =
            copula.integrate_fn(&|z| copula.conditional_default_prob(threshold, z, correlation));

        assert!(
            (integrated_prob - pd).abs() < 0.001,
            "Integrated probability {} should equal unconditional {}",
            integrated_prob,
            pd
        );
    }

    #[test]
    fn test_zero_tail_dependence() {
        let copula = GaussianCopula::new();
        assert_eq!(copula.tail_dependence(0.5), 0.0);
    }

    #[test]
    fn test_extreme_correlation_handling() {
        let copula = GaussianCopula::new();
        let threshold = standard_normal_inv_cdf(0.05);

        // Very low correlation should give near-unconditional probability
        let prob_low = copula.conditional_default_prob(threshold, &[0.0], 0.001);
        assert!((prob_low - 0.05).abs() < 0.001);

        // Near-perfect correlation should saturate toward the indicator
        // 1{z ≥ -c} (i.e. Aᵢ ≈ Z so default iff Z ≤ c = -1.645).
        let prob_high_neg_z = copula.conditional_default_prob(threshold, &[-2.0], 0.99);
        let prob_high_pos_z = copula.conditional_default_prob(threshold, &[2.0], 0.99);
        assert!(
            prob_high_neg_z > 0.99,
            "ρ=0.99, z=-2 should be near 1 (default virtually certain), got {prob_high_neg_z}"
        );
        assert!(
            prob_high_pos_z < 1e-6,
            "ρ=0.99, z=+2 should be effectively zero, got {prob_high_pos_z}"
        );
    }

    #[test]
    fn perfect_correlation_uses_exact_indicator_limit() {
        let copula = GaussianCopula::new();
        let threshold = standard_normal_inv_cdf(0.05);
        assert_eq!(
            copula.conditional_default_prob(threshold, &[-2.0], 1.0),
            1.0
        );
        assert_eq!(copula.conditional_default_prob(threshold, &[2.0], 1.0), 0.0);
        assert_eq!(
            copula.conditional_default_prob(threshold, &[threshold], 1.0),
            1.0
        );
    }

    #[test]
    fn test_factor_length_mismatch_contract() {
        let copula = GaussianCopula::new();
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
        let correlation = 0.30;

        let assert_contract = |factors: &[f64]| {
            if cfg!(debug_assertions) {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    copula.conditional_default_prob(threshold, factors, correlation)
                }));
                assert!(
                    outcome.is_err(),
                    "debug builds should panic on factor length mismatch"
                );
            } else {
                let result = copula.conditional_default_prob(threshold, factors, correlation);
                assert!(
                    (result - pd).abs() < 1e-9,
                    "factor length mismatch should return unconditional PD ({pd}), got {result}"
                );
            }
        };

        assert_contract(&[]);
        assert_contract(&[0.5, 1.0, -0.3]);
    }

    #[test]
    fn test_latent_variable_marginal_recovers_pd() {
        // Per-name latent draw Aᵢ = √ρ·Z + √(1−ρ)·εᵢ must be marginally
        // N(0,1): the fraction of draws below Φ⁻¹(PD) must equal PD. This is
        // the correctness anchor for finite-pool simulation — without it the
        // realized default count would be biased.
        use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
        use finstack_quant_models::monte_carlo::traits::RandomStream;

        let copula = GaussianCopula::new();
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
        let rho = 0.30;

        let mut rng = PhiloxRng::new(7);
        let trials = 400_000usize;
        let mut defaults = 0usize;
        for _ in 0..trials {
            let z = rng.next_std_normal();
            let eps = rng.next_std_normal();
            let a = copula.latent_variable(z, eps, 1.0, rho);
            if a <= threshold {
                defaults += 1;
            }
        }
        let realized = defaults as f64 / trials as f64;
        // 3-sigma binomial MC error at p=0.05, n=4e5 is ≈ 0.001.
        assert!(
            (realized - pd).abs() < 0.0015,
            "latent-variable marginal {realized} should recover PD {pd}"
        );
    }

    #[test]
    fn zero_correlation_preserves_independence_exactly() {
        let copula = GaussianCopula::new();
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
        let unconditional = norm_cdf(threshold);

        for z in [-2.0, 0.0, 2.0] {
            let conditional = copula.conditional_default_prob(threshold, &[z], 0.0);
            assert_eq!(conditional, unconditional);
        }
    }

    #[test]
    fn checked_probability_rejects_invalid_correlation() {
        let copula = GaussianCopula::new();
        let threshold = standard_normal_inv_cdf(0.05);
        assert!(copula
            .conditional_default_prob_checked(threshold, &[0.0], -0.01)
            .is_err());
        assert!(copula
            .conditional_default_prob_checked(threshold, &[0.0], 1.01)
            .is_err());
    }
}
