//! Market-correlated stochastic recovery model (Andersen-Sidenius).
//!
//! Models recovery through a latent-factor shock plus a smooth bounding
//! transform. The public parameters are quoted in decimals, so `0.40` means a
//! 40% recovery rate and `0.25` means 25% recovery volatility.
//!
//! # Mathematical Model
//!
//! ```text
//! shock(Z) = ρ_R · σ_R · Z
//! R(Z) = min_R + (max_R - min_R) * logistic(center(μ_R) + shock(Z) / local_slope)
//! ```
//!
//! where:
//! - `μ_R` is the mean recovery at `Z = 0`
//! - `σ_R` is the recovery-volatility scale
//! - `ρ_R` is the factor sensitivity
//! - `Z` is the supplied latent market factor
//!
//! The implementation does **not** hard-clamp an affine recovery rule. Instead,
//! it uses a logistic transform so recovery stays inside the configured bounds
//! smoothly while preserving the target mean exactly at `Z = 0`.
//!
//! # Sign Convention
//!
//! The crate-wide canonical convention is "low latent factor = stress": the
//! copula default models concentrate defaults at `Z < 0`. The preset
//! calibrations therefore use `ρ_R > 0`, so negative (stress) factor
//! realizations DECREASE recovery and defaults/recoveries co-move
//! negatively. Callers that want the opposite mapping should either negate
//! the factor they pass in or choose a negative `ρ_R`.
//!
//! # Calibration
//!
//! Typical market calibration from CDX equity tranche:
//! - Mean recovery: 40%
//! - Recovery volatility: 20-30%
//! - Factor correlation: +30% to +50% (canonical low-factor-stress convention)
//!
//! # References
//!
//! - Stochastic recovery and random loading context:
//!   `docs/REFERENCES.md#andersen-sidenius-2005-rfl`
//! - Tranche calibration background:
//!   `docs/REFERENCES.md#krekel-stumpp-2006-correlation-products`

use super::RecoveryModel;
use finstack_quant_core::math::GaussHermiteQuadrature;

/// Quadrature order for precomputing the Jensen-corrected unconditional mean
/// `E_Z[R(Z)]`. Order 20 is more than sufficient for a smooth logistic-bounded
/// recovery integrand and matches the other copula integrands in this crate.
const EXPECTED_RECOVERY_QUAD_ORDER: usize = 20;

/// Market-correlated stochastic recovery model.
///
/// Recovery varies with the systematic market factor, capturing
/// the empirical negative correlation between defaults and recovery.
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-sidenius-2005-rfl`
/// - `docs/REFERENCES.md#altman-et-al-2005-recovery`
#[derive(Debug, Clone)]
pub struct CorrelatedRecovery {
    /// Target recovery at `Z = 0` (median / location parameter of the
    /// logistic-bounded recovery transform). Due to Jensen's inequality, the
    /// true unconditional mean `E_Z[R(Z)]` differs from this value whenever
    /// `ρ_R · σ_R ≠ 0`.
    mean_recovery: f64,
    /// Recovery volatility (standard deviation)
    recovery_volatility: f64,
    /// Correlation with the systematic factor (typically positive under the
    /// canonical low-factor-stress convention: recovery falls in stress)
    factor_correlation: f64,
    /// Minimum recovery (floor)
    min_recovery: f64,
    /// Maximum recovery (ceiling)
    max_recovery: f64,
    /// Cached `E_Z[R(Z)]` computed once at construction by Gauss-Hermite
    /// quadrature against `N(0, 1)`. Used by [`RecoveryModel::expected_recovery`]
    /// so `lgd()` reflects the Jensen-corrected unconditional mean, not the
    /// biased R(0) location parameter.
    unconditional_expected_recovery: f64,
}

impl CorrelatedRecovery {
    /// Create a correlated recovery model.
    ///
    /// # Arguments
    /// * `mean` - Mean recovery rate, clamped to [0.05, 0.95]. Typical: 0.40
    /// * `vol` - Recovery volatility, clamped to [0.0, 0.50]. Typical: 0.20-0.30
    /// * `corr` - Correlation with market factor, clamped to [-1.0, 1.0]. Typical: +0.30 to +0.50
    ///
    /// # Returns
    ///
    /// A bounded stochastic recovery model with default bounds `[0.0, 1.0]`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::correlation::{CorrelatedRecovery, RecoveryModel};
    ///
    /// let model = CorrelatedRecovery::new(0.40, 0.25, 0.40);
    /// let mean_at_zero = model.conditional_recovery(0.0);
    ///
    /// assert!((mean_at_zero - 0.40).abs() < 1e-12);
    /// ```
    #[must_use]
    pub fn new(mean: f64, vol: f64, corr: f64) -> Self {
        let mut model = Self {
            mean_recovery: mean.clamp(0.05, 0.95),
            recovery_volatility: vol.clamp(0.0, 0.50),
            factor_correlation: corr.clamp(-1.0, 1.0),
            min_recovery: 0.0,
            max_recovery: 1.0,
            unconditional_expected_recovery: 0.0,
        };
        model.unconditional_expected_recovery = model.compute_unconditional_expected_recovery();
        model
    }

    /// Create with custom bounds.
    ///
    /// # Arguments
    /// * `mean` - Mean recovery rate
    /// * `vol` - Recovery volatility
    /// * `corr` - Correlation with market factor
    /// * `min` - Minimum recovery (floor), clamped to [0.0, 0.5]
    /// * `max` - Maximum recovery (ceiling), clamped to [0.5, 1.0]
    ///
    /// # Returns
    ///
    /// A bounded stochastic recovery model with caller-specified recovery bounds.
    #[must_use]
    pub fn with_bounds(mean: f64, vol: f64, corr: f64, min: f64, max: f64) -> Self {
        let mut model = Self::new(mean, vol, corr);
        model.min_recovery = min.clamp(0.0, 0.5);
        model.max_recovery = max.clamp(0.5, 1.0);
        // Bounds affect the logistic transform, so the cached expectation must
        // be recomputed after overriding them.
        model.unconditional_expected_recovery = model.compute_unconditional_expected_recovery();
        model
    }

    /// Market-standard calibration from CDX equity tranche.
    ///
    /// Parameters:
    /// - Mean: 40%
    /// - Vol: 25%
    /// - Correlation: +40% (recovery falls in stress under the canonical
    ///   low-factor-stress convention)
    ///
    /// # Returns
    ///
    /// The default stochastic-recovery calibration used by this crate.
    #[must_use]
    pub fn market_standard() -> Self {
        Self::new(0.40, 0.25, 0.40)
    }

    /// Conservative calibration with higher vol and correlation.
    ///
    /// Parameters:
    /// - Mean: 40%
    /// - Vol: 30%
    /// - Correlation: +50%
    ///
    /// # Returns
    ///
    /// A higher-volatility, more factor-sensitive stochastic-recovery calibration.
    #[must_use]
    pub fn conservative() -> Self {
        Self::new(0.40, 0.30, 0.50)
    }

    /// Get the target recovery at `Z = 0` (location parameter).
    ///
    /// This is the `μ_R` input parameter after clamping — the median of the
    /// logistic-bounded recovery distribution, not the Jensen-corrected
    /// unconditional mean. Use [`RecoveryModel::expected_recovery`] when you
    /// need `E_Z[R(Z)]`.
    ///
    /// # Returns
    ///
    /// The target recovery at zero market shock, in decimal form.
    #[must_use]
    pub fn mean(&self) -> f64 {
        self.mean_recovery
    }

    /// Get the recovery volatility.
    ///
    /// # Returns
    ///
    /// The recovery-volatility scale in decimal form.
    #[must_use]
    pub fn volatility(&self) -> f64 {
        self.recovery_volatility
    }

    /// Get the factor correlation.
    ///
    /// # Returns
    ///
    /// The signed factor-sensitivity parameter.
    #[must_use]
    pub fn correlation(&self) -> f64 {
        self.factor_correlation
    }

    /// Compute `E_Z[R(Z)]` via Gauss-Hermite quadrature against `N(0, 1)`.
    ///
    /// The logistic transform is smooth and bounded in `[0, 1]`, so a
    /// moderate-order Gauss-Hermite rule achieves machine precision. When the
    /// model is effectively deterministic (ρ_R·σ_R = 0) we short-circuit to
    /// avoid a spurious quadrature call.
    fn compute_unconditional_expected_recovery(&self) -> f64 {
        if self.factor_correlation == 0.0 || self.recovery_volatility == 0.0 {
            return self.logistic_bounded_recovery(0.0);
        }

        // Fallback to R(0) if the requested quadrature order is unsupported —
        // this keeps the constructor infallible while logging the anomaly.
        let quad = match GaussHermiteQuadrature::new(EXPECTED_RECOVERY_QUAD_ORDER) {
            Ok(q) => q,
            Err(err) => {
                tracing::warn!(
                    order = EXPECTED_RECOVERY_QUAD_ORDER,
                    %err,
                    "CorrelatedRecovery: falling back to R(0) for expected_recovery; \
                     GaussHermiteQuadrature rejected the requested order"
                );
                return self.logistic_bounded_recovery(0.0);
            }
        };
        quad.integrate(|z| self.conditional_recovery(z))
    }

    fn logistic_bounded_recovery(&self, shock: f64) -> f64 {
        let width = (self.max_recovery - self.min_recovery).max(f64::EPSILON);
        let mean = self
            .mean_recovery
            .clamp(self.min_recovery + 1e-9, self.max_recovery - 1e-9);
        let p = ((mean - self.min_recovery) / width).clamp(1e-9, 1.0 - 1e-9);
        let center = (p / (1.0 - p)).ln();
        let local_slope = (width * p * (1.0 - p)).max(1e-9);
        let squashed = 1.0 / (1.0 + (-(center + shock / local_slope)).exp());
        self.min_recovery + width * squashed
    }
}

impl RecoveryModel for CorrelatedRecovery {
    fn expected_recovery(&self) -> f64 {
        // Jensen-corrected unconditional mean E_Z[R(Z)]. Because the logistic
        // transform is non-linear and saturates near the bounds, this differs
        // from `self.mean_recovery` (R(0)) whenever ρ_R·σ_R ≠ 0.
        self.unconditional_expected_recovery
    }

    fn conditional_recovery(&self, market_factor: f64) -> f64 {
        let shock = self.factor_correlation * self.recovery_volatility * market_factor;
        self.logistic_bounded_recovery(shock)
    }

    fn recovery_volatility(&self) -> f64 {
        self.recovery_volatility
    }

    fn model_name(&self) -> &'static str {
        "Market-Correlated Stochastic Recovery"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correlated_recovery_creation() {
        let model = CorrelatedRecovery::new(0.40, 0.25, 0.40);
        assert!((model.mean() - 0.40).abs() < 1e-10);
        assert!((model.volatility() - 0.25).abs() < 1e-10);
        assert!((model.correlation() - 0.40).abs() < 1e-10);
    }

    #[test]
    fn test_conditional_recovery_in_stress() {
        let model = CorrelatedRecovery::market_standard();

        // Canonical convention: low latent factor = stress, so Z = -2 is a
        // stress scenario and recovery must FALL below R(0).
        let stress_recovery = model.conditional_recovery(-2.0);

        // Compare against R(0) (mean location), which is the reference
        // unaffected by the Jensen correction.
        assert!(
            stress_recovery < model.mean(),
            "Stress (low factor) must depress recovery below R(0) so defaults and recoveries co-move negatively"
        );
    }

    #[test]
    fn test_conditional_recovery_varies() {
        let model = CorrelatedRecovery::market_standard();

        let r_neg = model.conditional_recovery(-2.0);
        let r_zero = model.conditional_recovery(0.0);
        let r_pos = model.conditional_recovery(2.0);

        // With positive correlation (canonical convention):
        // - Negative Z (stress) gives lower recovery
        // - Positive Z (benign) gives higher recovery
        assert!(r_neg < r_zero, "Neg Z (stress) should give lower recovery");
        assert!(r_pos > r_zero, "Pos Z (benign) should give higher recovery");
    }

    #[test]
    fn test_conditional_recovery_at_zero_equals_location() {
        let model = CorrelatedRecovery::market_standard();

        // At Z=0, R(Z) equals the location parameter μ_R (median recovery).
        // This is *not* generally the same as the unconditional mean E[R(Z)]
        // because the logistic transform is non-linear (Jensen's inequality).
        let r_at_zero = model.conditional_recovery(0.0);
        assert!(
            (r_at_zero - model.mean()).abs() < 1e-10,
            "Recovery at Z=0 should equal the location parameter μ_R"
        );
    }

    #[test]
    fn test_expected_recovery_is_jensen_corrected_mean() {
        // Regression: expected_recovery() must return E_Z[R(Z)] computed by
        // integrating the logistic-bounded recovery against N(0,1). It should
        // differ from R(0) when ρ_R·σ_R ≠ 0, and match R(0) exactly when the
        // recovery is deterministic.
        let model = CorrelatedRecovery::market_standard();
        let r_at_zero = model.mean();
        let e_r = model.expected_recovery();

        // The two differ by the Jensen correction; it is small but non-zero.
        assert!(
            (e_r - r_at_zero).abs() > 1e-6,
            "expected_recovery {e_r} should differ from R(0) {r_at_zero}"
        );
        // Both values remain inside the recovery bounds.
        assert!((0.0..=1.0).contains(&e_r));

        // When correlation is zero, R is deterministic and E[R] = R(0).
        let det_model = CorrelatedRecovery::new(0.40, 0.25, 0.0);
        assert!(
            (det_model.expected_recovery() - det_model.mean()).abs() < 1e-12,
            "Deterministic recovery: E[R] must equal R(0)"
        );

        // Cross-check against an independent Gauss-Hermite computation.
        let independent_quad = GaussHermiteQuadrature::new(20)
            .expect("order 20 is a supported Gauss-Hermite quadrature");
        let e_r_check = independent_quad.integrate(|z| model.conditional_recovery(z));
        assert!(
            (e_r - e_r_check).abs() < 1e-12,
            "cached E[R]={e_r} should match independently computed value {e_r_check}"
        );
    }

    #[test]
    fn test_recovery_bounded() {
        let model = CorrelatedRecovery::new(0.40, 0.30, 0.50);

        // Even with extreme factors, recovery should be bounded
        let extreme_neg = model.conditional_recovery(-10.0);
        let extreme_pos = model.conditional_recovery(10.0);

        assert!(
            (0.0..=1.0).contains(&extreme_neg),
            "Recovery {} should be in [0, 1]",
            extreme_neg
        );
        assert!(
            (0.0..=1.0).contains(&extreme_pos),
            "Recovery {} should be in [0, 1]",
            extreme_pos
        );
        assert!(
            extreme_neg < 1.0,
            "smooth bounding should avoid hard ceiling clamp"
        );
        assert!(
            extreme_pos > 0.0,
            "smooth bounding should avoid hard floor clamp"
        );
    }

    #[test]
    fn test_is_stochastic() {
        let model = CorrelatedRecovery::market_standard();
        assert!(model.is_stochastic());
        assert!(model.recovery_volatility() > 0.0);
    }

    #[test]
    fn test_zero_volatility_is_constant() {
        let model = CorrelatedRecovery::new(0.40, 0.0, 0.40);

        // With zero volatility, should behave like constant
        let r_neg = model.conditional_recovery(-2.0);
        let r_pos = model.conditional_recovery(2.0);

        assert!(
            (r_neg - r_pos).abs() < 1e-10,
            "Zero vol should give constant recovery"
        );
    }

    #[test]
    fn test_zero_correlation_is_constant() {
        let model = CorrelatedRecovery::new(0.40, 0.25, 0.0);

        // With zero correlation, should behave like constant
        let r_neg = model.conditional_recovery(-2.0);
        let r_pos = model.conditional_recovery(2.0);

        assert!(
            (r_neg - r_pos).abs() < 1e-10,
            "Zero correlation should give constant recovery"
        );
    }

    #[test]
    fn test_lgd_calculation() {
        let model = CorrelatedRecovery::market_standard();

        // LGD = 1 - E[R(Z)]. The Jensen correction is small (a few bp), so
        // LGD should be close to but not exactly equal to 1 - R(0) = 0.60.
        let expected_lgd = 1.0 - model.expected_recovery();
        assert!((model.lgd() - expected_lgd).abs() < 1e-12);
        assert!(
            (model.lgd() - 0.60).abs() < 1e-2,
            "LGD {} should be within a few bp of the naive 1 - R(0) = 0.60",
            model.lgd()
        );

        // Conditional LGD at Z=0 must equal 1 - R(0) exactly.
        assert!((model.conditional_lgd(0.0) - (1.0 - model.mean())).abs() < 1e-10);
    }

    #[test]
    fn test_market_standard_and_conservative() {
        let standard = CorrelatedRecovery::market_standard();
        let conservative = CorrelatedRecovery::conservative();

        // Conservative should have higher vol
        assert!(conservative.volatility() > standard.volatility());

        // Conservative should have stronger (positive) factor correlation
        assert!(conservative.correlation() > standard.correlation());
    }
}
