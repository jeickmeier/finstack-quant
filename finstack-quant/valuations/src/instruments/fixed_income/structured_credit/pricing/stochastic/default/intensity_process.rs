//! Intensity process default model (Cox process).
//!
//! Models default intensity as a mean-reverting stochastic process
//! driven by systematic factors.
//!
//! # Mathematical Model
//!
//! The default intensity λ follows:
//! ```text
//! λ(t) = λ₀ × exp(−β × X(t) − ½β²σ²)
//! ```
//!
//! The `−½β²σ²` term is the lognormal compensator: with `X ~ N(0, 1)` the
//! shock `exp(−βσX − ½β²σ²)` has unit mean, so the simulated mean hazard
//! equals λ₀ and `expected_mdr` matches the simulated average.
//!
//! where X(t) is an Ornstein-Uhlenbeck process:
//! ```text
//! dX = κ(θ - X)dt + σ dW
//! ```
//!
//! # Factor persistence
//!
//! `X(t)` is the engine's SYSTEMATIC FACTOR path, not state held by this type.
//! `intensity()` is deliberately a pure function of the factor realization;
//! the OU dynamics are produced upstream by
//! `StochasticPricer::evolved_factors`, which generates a stationary AR(1)
//! path with autocorrelation `φ^h = e^{−κh/12}`. The scenario-tree config
//! sources `κ` from this spec's `mean_reversion`; κ = 0 holds one systematic
//! draw across the horizon. Applying the exponential intensity to that OU
//! factor realizes the model above.
//!
//! # Sign Convention
//!
//! The systematic factor follows the canonical copula convention: a LOW
//! latent factor realization (`Z < 0`) is the stress state. With a positive
//! factor sensitivity `β`, intensity therefore *rises* as `Z` falls
//! (`exp(−β·σ·Z)`), matching the Gaussian-copula barrier `Φ⁻¹(PD) − √ρ·Z`
//! used by the copula default models and ensuring defaults and
//! market-correlated recoveries co-move negatively across every engine.
//!
//! The conditional default probability over [t, t+dt]:
//! ```text
//! P(default in dt | λ) = 1 - exp(-λ × dt)
//! ```
//!
//! # References
//!
//! - Duffie, D., & Singleton, K. J. (1999). "Modeling Term Structures of Defaultable Bonds."
//! - Lando, D. (1998). "On Cox Processes and Credit Risky Securities."

#![allow(dead_code)]

use super::super::calibrations::{clo_standard, rmbs_standard};
use super::traits::{MacroCreditFactors, StochasticDefault};

/// Intensity process (Cox model) default model.
///
/// Default intensity follows an exponential of an OU process,
/// providing mean-reverting but always positive intensity.
#[derive(Debug, Clone)]
pub(crate) struct IntensityProcessDefault {
    /// Base hazard rate (annual)
    base_hazard: f64,
    /// Factor sensitivity (beta)
    factor_sensitivity: f64,
    /// Mean reversion speed (kappa)
    mean_reversion: f64,
    /// Volatility of intensity process
    volatility: f64,
    /// Asset correlation for distribution calculation
    correlation: f64,
}

impl IntensityProcessDefault {
    /// Create an intensity process default model.
    ///
    /// # Arguments
    /// * `base_hazard` - Base annual hazard rate (λ₀)
    /// * `factor_sensitivity` - Sensitivity to systematic factor (β)
    /// * `mean_reversion` - Mean reversion speed (κ)
    /// * `volatility` - Intensity volatility (σ)
    pub(crate) fn new(
        base_hazard: f64,
        factor_sensitivity: f64,
        mean_reversion: f64,
        volatility: f64,
    ) -> Self {
        Self {
            base_hazard: base_hazard.clamp(0.0, 1.0),
            factor_sensitivity: factor_sensitivity.clamp(-2.0, 2.0),
            mean_reversion: mean_reversion.clamp(0.0, 10.0),
            volatility: volatility.clamp(0.0, 2.0),
            correlation: 0.20, // Default correlation
        }
    }

    /// Create with specified correlation.
    pub(crate) fn with_correlation(mut self, correlation: f64) -> Self {
        self.correlation = correlation.clamp(0.0, 0.99);
        self
    }

    /// Standard RMBS calibration.
    ///
    /// Uses the registry-backed `rmbs_standard` calibration profile:
    /// - Base hazard: 2% annual
    /// - Factor sensitivity: 0.5
    /// - Mean reversion: 0.5 (2-year half-life)
    /// - Volatility: 0.30
    pub(crate) fn rmbs_standard() -> Self {
        let calibration = rmbs_standard();
        Self::new(
            calibration.base_cdr,
            calibration.default_factor_sensitivity,
            calibration.default_mean_reversion,
            calibration.default_volatility,
        )
        .with_correlation(calibration.default_correlation)
    }

    /// Standard CLO calibration.
    ///
    /// Uses the registry-backed `clo_standard` calibration profile:
    /// Higher base hazard and factor sensitivity for corporate loans.
    pub(crate) fn clo_standard() -> Self {
        let calibration = clo_standard();
        Self::new(
            calibration.base_cdr,
            calibration.default_factor_sensitivity,
            calibration.default_mean_reversion,
            calibration.default_volatility,
        )
        .with_correlation(calibration.default_correlation)
    }

    /// Get the base hazard rate.
    pub(crate) fn base_hazard(&self) -> f64 {
        self.base_hazard
    }

    /// Get the factor sensitivity.
    pub(crate) fn factor_sensitivity(&self) -> f64 {
        self.factor_sensitivity
    }

    /// Get the mean reversion speed.
    pub(crate) fn mean_reversion(&self) -> f64 {
        self.mean_reversion
    }

    /// Get the volatility.
    pub(crate) fn volatility(&self) -> f64 {
        self.volatility
    }

    /// Calculate intensity at given factor value.
    ///
    /// λ(Z) = λ₀ × exp(−β × Z × σ − ½β²σ²): the canonical low-factor-stress
    /// convention — intensity rises as the systematic factor falls. The
    /// `−½β²σ²` lognormal compensator gives the shock unit mean under
    /// `Z ~ N(0, 1)`, so E[λ(Z)] = λ₀ and the simulated mean hazard matches
    /// the base curve (and `expected_mdr`).
    fn intensity(&self, factor: f64) -> f64 {
        self.base_hazard * self.shock_multiplier(factor)
    }

    /// Compensated lognormal shock `exp(−βσZ − ½β²σ²)` with unit mean.
    fn shock_multiplier(&self, factor: f64) -> f64 {
        let beta_sigma = self.factor_sensitivity * self.volatility;
        (-beta_sigma * factor - 0.5 * beta_sigma * beta_sigma).exp()
    }
}

impl StochasticDefault for IntensityProcessDefault {
    fn conditional_mdr(
        &self,
        seasoning: u32,
        factors: &[f64],
        _macro_factors: &MacroCreditFactors,
    ) -> f64 {
        let z = factors.first().copied().unwrap_or(0.0);

        // Conditional intensity
        let intensity = self.intensity(z);

        // Apply seasoning ramp: linear over first 24 months, then flat at 1.0.
        // Newly originated loans have lower default rates that ramp up as they season.
        let ramp_months = 24_u32;
        let seasoning_factor = if seasoning < ramp_months {
            seasoning as f64 / ramp_months as f64
        } else {
            1.0
        };
        let adjusted_intensity = intensity * seasoning_factor;

        // Monthly survival probability
        let monthly_intensity = adjusted_intensity / 12.0;
        let survival_prob = (-monthly_intensity).exp();

        // MDR = 1 - survival
        (1.0 - survival_prob).clamp(0.0, 1.0)
    }

    fn correlation(&self) -> f64 {
        self.correlation
    }

    fn model_name(&self) -> &'static str {
        "Intensity Process Default Model"
    }

    fn expected_mdr(&self, seasoning: u32) -> f64 {
        let ramp_months = 24_u32;
        let seasoning_factor = if seasoning < ramp_months {
            seasoning as f64 / ramp_months as f64
        } else {
            1.0
        };
        // Same continuous-compounding conversion as `conditional_mdr`
        // (MDR = 1 − exp(−λ/12)); the shock is compensated to unit mean, so
        // the expected hazard is the seasoning-adjusted base hazard.
        let expected_hazard = self.base_hazard * seasoning_factor;
        (1.0 - (-expected_hazard / 12.0).exp()).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intensity_process_creation() {
        let model = IntensityProcessDefault::new(0.02, 0.5, 0.5, 0.30);

        assert!((model.base_hazard() - 0.02).abs() < 1e-10);
        assert!((model.factor_sensitivity() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_conditional_mdr_at_zero_factor() {
        let model = IntensityProcessDefault::new(0.02, 0.5, 0.5, 0.30);
        let factors = MacroCreditFactors::default();

        let mdr = model.conditional_mdr(12, &[0.0], &factors);

        // At Z=0 the compensated shock is exp(−½β²σ²), so the intensity is
        // base_hazard × exp(−½β²σ²) (the shock has unit MEAN, not unit mode).
        // With seasoning ramp (24 months), factor at month 12 = 12/24 = 0.5.
        let beta_sigma = 0.5 * 0.30;
        let intensity = 0.02 * (-0.5_f64 * beta_sigma * beta_sigma).exp();
        let seasoning_factor = 12.0 / 24.0;
        let expected = 1.0 - (-intensity * seasoning_factor / 12.0_f64).exp();
        assert!(
            (mdr - expected).abs() < 1e-6,
            "MDR {} should equal expected {}",
            mdr,
            expected
        );
    }

    /// The compensated shock `exp(−βσZ − ½β²σ²)` must have unit mean under
    /// `Z ~ N(0,1)`, so the simulated mean hazard equals the base hazard and
    /// `expected_mdr` matches the simulated average (Gauss-Hermite check).
    #[test]
    fn test_compensated_shock_has_unit_mean() {
        let model = IntensityProcessDefault::new(0.05, 0.8, 0.5, 0.40);

        // E[g(Z)] ≈ (1/√π) Σ wᵢ g(√2 xᵢ) over Gauss-Hermite nodes.
        let nodes = [
            (-2.350_604_973_674_492_3, 0.002_530_410_604_089_597_4),
            (-1.335_849_074_013_697, 0.157_067_320_322_856_64),
            (-0.436_077_411_927_616_5, 0.724_629_595_224_392_4),
            (0.436_077_411_927_616_5, 0.724_629_595_224_392_4),
            (1.335_849_074_013_697, 0.157_067_320_322_856_64),
            (2.350_604_973_674_492_3, 0.002_530_410_604_089_597_4),
        ];
        let mean_shock: f64 = nodes
            .iter()
            .map(|&(x, w)| w * model.shock_multiplier(std::f64::consts::SQRT_2 * x))
            .sum::<f64>()
            / std::f64::consts::PI.sqrt();
        // 6-node Gauss-Hermite truncates the lognormal tail slightly; the
        // residual quadrature error at βσ = 0.32 is ~3.5e-3.
        assert!(
            (mean_shock - 1.0).abs() < 5e-3,
            "compensated shock mean {mean_shock} should be 1"
        );
    }

    #[test]
    fn test_negative_factor_increases_mdr() {
        let model = IntensityProcessDefault::new(0.02, 0.5, 0.5, 0.30);
        let factors = MacroCreditFactors::default();

        let mdr_neg = model.conditional_mdr(12, &[-2.0], &factors);
        let mdr_zero = model.conditional_mdr(12, &[0.0], &factors);
        let mdr_pos = model.conditional_mdr(12, &[2.0], &factors);

        // Canonical convention: low latent factor = stress.
        // Negative factor increases intensity -> higher MDR
        // Positive factor decreases intensity -> lower MDR
        assert!(mdr_neg > mdr_zero, "Negative factor should increase MDR");
        assert!(mdr_pos < mdr_zero, "Positive factor should decrease MDR");
    }

    #[test]
    fn test_intensity_calculation() {
        let model = IntensityProcessDefault::new(0.02, 1.0, 0.5, 1.0);

        // At Z=0: intensity = base × exp(−½β²σ²) (compensated shock)
        let int_zero = model.intensity(0.0);
        let expected_zero = 0.02 * (-0.5_f64).exp();
        assert!((int_zero - expected_zero).abs() < 1e-10);

        // At Z=-1 (stress): intensity ratio to Z=0 is exp(βσ) ≈ 2.718
        // (the compensator cancels in the ratio).
        let int_stress = model.intensity(-1.0);
        assert!(int_stress > int_zero);
        assert!((int_stress / int_zero - 1.0_f64.exp()).abs() < 1e-6);
    }

    #[test]
    fn test_standard_calibrations() {
        let rmbs = IntensityProcessDefault::rmbs_standard();
        assert!((rmbs.base_hazard() - 0.02).abs() < 1e-10);

        let clo = IntensityProcessDefault::clo_standard();
        assert!(clo.base_hazard() > rmbs.base_hazard());
        assert!(clo.correlation() > rmbs.correlation());
    }
}
