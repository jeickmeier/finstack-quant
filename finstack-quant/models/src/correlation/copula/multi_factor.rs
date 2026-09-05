//! Two-factor Gaussian copula with one global and one shared sector factor.
//!
//! The trait-level [`Copula::conditional_default_prob`] consumes `[Z_G, Z_S]`
//! and prices every name under the same pair of systematic shocks:
//!
//! ```text
//! Aᵢ = β_G · Z_G + β_S · Z_S + γ · εᵢ
//! γ = √(1 − β_G² − β_S²)
//! ```
//!
//! This realizes the intra-sector correlation `β_G² + β_S²` for every pair.
//! Per-name sector routing is intentionally outside this model; callers that
//! need a full sector-assignment model must provide that structure before
//! selecting a copula.
//!
//! # Use Cases
//!
//! - Single-sector basket and tranche pricing
//! - Global-versus-sector factor sensitivity analysis
//!
//! # References
//!
//! - Multi-factor basket and bespoke CDO modeling: `docs/REFERENCES.md#andersen-sidenius-basu-2003`
//!
//! - Analytical correlation-product valuation: `docs/REFERENCES.md#hull-white-2004-cdo`
//!

use super::{select_quadrature, Copula};
use finstack_quant_core::math::{norm_cdf, GaussHermiteQuadrature};

/// CDF argument clipping to prevent overflow.
const CDF_CLIP: f64 = 10.0;
/// Default quadrature order for multi-dimensional integration.
const MULTI_FACTOR_QUADRATURE_ORDER: u8 = 10;

/// Global-plus-sector two-factor Gaussian copula.
///
/// The total correlation supplied to each pricing call is split between the
/// global and sector factors. The default sector share is 40%; callers may
/// set another finite share with [`Self::with_sector_fraction`].
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-sidenius-basu-2003`
/// - `docs/REFERENCES.md#hull-white-2004-cdo`
pub struct MultiFactorCopula {
    /// Fraction of total correlation attributed to the shared sector factor.
    sector_fraction: f64,
    /// Cached quadrature for integration.
    quadrature: GaussHermiteQuadrature,
}

impl Clone for MultiFactorCopula {
    fn clone(&self) -> Self {
        let order =
            u8::try_from(self.quadrature.points.len()).unwrap_or(MULTI_FACTOR_QUADRATURE_ORDER);
        Self {
            sector_fraction: self.sector_fraction,
            quadrature: select_quadrature(order),
        }
    }
}

impl std::fmt::Debug for MultiFactorCopula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiFactorCopula")
            .field("sector_fraction", &self.sector_fraction)
            .finish()
    }
}

const NUM_FACTORS: usize = 2;
impl Default for MultiFactorCopula {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiFactorCopula {
    /// Create a global-plus-sector Gaussian copula.
    #[must_use]
    pub fn new() -> Self {
        Self::with_quadrature_order(MULTI_FACTOR_QUADRATURE_ORDER)
    }

    /// Create a global-plus-sector copula with explicit quadrature order.
    ///
    /// # Arguments
    ///
    /// * `quadrature_order` - Gauss-Hermite points per systematic factor.
    #[must_use]
    pub fn with_quadrature_order(quadrature_order: u8) -> Self {
        Self {
            sector_fraction: 0.4,
            quadrature: select_quadrature(quadrature_order),
        }
    }

    /// Set the share of total correlation assigned to the sector factor.
    ///
    /// # Arguments
    ///
    /// * `sector_fraction` - Fraction in `[0, 1]`; finite values are clamped.
    ///
    /// # Errors
    ///
    /// Returns an error when `sector_fraction` is non-finite.
    pub fn with_sector_fraction(
        mut self,
        sector_fraction: f64,
    ) -> finstack_quant_core::Result<Self> {
        if !sector_fraction.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "multi-factor copula sector_fraction must be finite".into(),
            ));
        }
        self.sector_fraction = sector_fraction.clamp(0.0, 1.0);
        Ok(self)
    }

    /// Compute idiosyncratic loading given factor loadings.
    ///
    /// γ = √(1 - β_G² - β_S²) to ensure Var(Aᵢ) = 1
    fn idiosyncratic_loading(&self, global_loading: f64, sector_loading: f64) -> f64 {
        let sum_sq = global_loading * global_loading + sector_loading * sector_loading;
        (1.0 - sum_sq).max(0.0).sqrt()
    }

    /// Recursive two-dimensional Gauss-Hermite integration.
    ///
    /// The quadrature uses the physicists' `e^{-z²}` convention. Each
    /// dimension applies `x = √2·z` and the `1/√π` normalization.
    fn integrate_recursive(
        &self,
        f: &dyn Fn(&[f64]) -> f64,
        scratch: &mut [f64],
        depth: usize,
    ) -> f64 {
        const SQRT_2: f64 = std::f64::consts::SQRT_2;
        if depth == scratch.len() {
            return f(scratch);
        }
        // Collect nodes so we can mutate `scratch` inside the loop
        // without the quadrature driver's closure keeping a borrow.
        let nodes: Vec<(f64, f64)> = self
            .quadrature
            .points
            .iter()
            .zip(self.quadrature.weights.iter())
            .map(|(&z, &w)| (SQRT_2 * z, w))
            .collect();
        let mut acc = 0.0_f64;
        for (x, w) in nodes {
            scratch[depth] = x;
            acc += w * self.integrate_recursive(f, scratch, depth + 1);
        }
        acc / std::f64::consts::PI.sqrt()
    }

    fn decompose_correlation(&self, total_correlation: f64) -> (f64, f64) {
        let rho = total_correlation.clamp(0.0, 0.99);
        let global_sq = rho * (1.0 - self.sector_fraction);
        let sector_sq = rho * self.sector_fraction;
        (global_sq.sqrt(), sector_sq.sqrt())
    }
}

impl Copula for MultiFactorCopula {
    fn conditional_default_prob(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
    ) -> f64 {
        debug_assert_eq!(
            factor_realization.len(),
            NUM_FACTORS,
            "MultiFactorCopula expects exactly {NUM_FACTORS} factors, got {}",
            factor_realization.len()
        );
        if factor_realization.len() != NUM_FACTORS {
            tracing::error!(
                expected = NUM_FACTORS,
                actual = factor_realization.len(),
                "MultiFactorCopula: factor length mismatch; returning unconditional PD"
            );
            return norm_cdf(default_threshold);
        }

        let (global_loading, sector_loading) = self.decompose_correlation(correlation);
        let z_global = factor_realization.first().copied().unwrap_or(0.0);
        let z_sector = factor_realization.get(1).copied().unwrap_or(0.0);
        let gamma = self.idiosyncratic_loading(global_loading, sector_loading);

        if gamma < 1e-10 {
            let systematic = global_loading * z_global + sector_loading * z_sector;
            return norm_cdf(default_threshold - systematic);
        }

        let systematic = global_loading * z_global + sector_loading * z_sector;
        let conditional_threshold = (default_threshold - systematic) / gamma;
        norm_cdf(conditional_threshold.clamp(-CDF_CLIP, CDF_CLIP))
    }

    fn conditional_default_prob_given_systematic_and_mixing(
        &self,
        default_threshold: f64,
        systematic: f64,
        mixing: f64,
        correlation: f64,
    ) -> f64 {
        // Single-Z conditional with the sector factor(s) integrated out. The
        // latent variable is Aᵢ = β_G·Z_G + β_S·Z_S + γ·εᵢ; conditioning on
        // Z_G only, the remainder β_S·Z_S + γ·εᵢ is N(0, 1 − β_G²), so
        //   P(default | Z_G) = Φ((c − β_G·Z_G)/√(1−β_G²)).
        // There is no mixing variable. Note this is the GLOBAL-factor
        // conditional only — per-name engines that cannot supply sector
        // factor realizations must not simulate this copula name-by-name
        // (see `PerNameCopulaDefault::new`, which rejects multi-factor specs).
        let _ = mixing;
        let (global_loading, _) = self.decompose_correlation(correlation);
        let residual_sd = (1.0 - global_loading * global_loading).max(0.0).sqrt();
        if residual_sd < 1e-10 {
            return norm_cdf(default_threshold - global_loading * systematic);
        }
        let conditional_threshold = (default_threshold - global_loading * systematic) / residual_sd;
        norm_cdf(conditional_threshold.clamp(-CDF_CLIP, CDF_CLIP))
    }

    fn integrate_fn(&self, f: &dyn Fn(&[f64]) -> f64) -> f64 {
        // Nested Gauss-Hermite integration over one global dimension and,
        // when configured, one shared sector dimension.
        let mut scratch = [0.0_f64; NUM_FACTORS];
        self.integrate_recursive(f, &mut scratch, 0)
    }

    fn num_factors(&self) -> usize {
        NUM_FACTORS
    }

    fn model_name(&self) -> &'static str {
        "Multi-Factor Gaussian Copula"
    }

    fn tail_dependence(&self, _correlation: f64) -> f64 {
        // Multi-factor Gaussian still has zero tail dependence
        // (sum of Gaussians is Gaussian)
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::math::standard_normal_inv_cdf;

    #[test]
    fn test_multi_factor_creation() {
        let copula = MultiFactorCopula::new();
        assert_eq!(copula.num_factors(), 2);
        assert_eq!(copula.model_name(), "Multi-Factor Gaussian Copula");
    }

    #[test]
    fn test_correlation_decomposition() {
        let copula = MultiFactorCopula::new()
            .with_sector_fraction(0.5)
            .expect("finite sector fraction");
        let (global, sector) = copula.decompose_correlation(0.36);

        // Total correlation should reconstruct
        let reconstructed = global * global + sector * sector;
        assert!(
            (reconstructed - 0.36).abs() < 1e-6,
            "Reconstructed {} should equal original 0.36",
            reconstructed
        );
    }

    #[test]
    fn test_zero_tail_dependence() {
        let copula = MultiFactorCopula::new();
        assert_eq!(copula.tail_dependence(0.5), 0.0);
    }

    #[test]
    fn test_integration_recovers_unconditional() {
        let copula = MultiFactorCopula::new();
        let pd = 0.05;
        let threshold = standard_normal_inv_cdf(pd);
        let correlation = 0.30;

        let integrated_prob = copula.integrate_fn(&|factors| {
            copula.conditional_default_prob(threshold, factors, correlation)
        });

        assert!(
            (integrated_prob - pd).abs() < 0.01,
            "Integrated probability {} should be close to unconditional {}",
            integrated_prob,
            pd
        );
    }

    #[test]
    fn test_factor_length_mismatch_contract() {
        let copula = MultiFactorCopula::new();
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

        assert_contract(&[-1.0]);
        assert_contract(&[0.5, 1.0, -0.3]);
        assert_contract(&[]);
    }
}
