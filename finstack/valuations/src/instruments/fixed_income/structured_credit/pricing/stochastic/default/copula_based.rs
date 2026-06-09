//! Copula-based default model.
//!
//! Implements default correlation using the Li (2000) copula framework,
//! leveraging the shared copula infrastructure.
//!
//! # Mathematical Model
//!
//! For each obligor i:
//! ```text
//! Aᵢ = √ρ · Z + √(1-ρ) · εᵢ
//! Default: Aᵢ ≤ Φ⁻¹(PD)
//! ```
//!
//! The conditional default probability given Z:
//! ```text
//! P(default | Z) = Φ((Φ⁻¹(PD) - √ρ · Z) / √(1-ρ))
//! ```
//!
//! # References
//!
//! - Li, D. X. (2000). "On Default Correlation: A Copula Function Approach."

#![allow(dead_code)]

use super::super::calibrations::{clo_standard, rmbs_standard};
use super::traits::{MacroCreditFactors, StochasticDefault};
use crate::correlation::copula::{Copula, CopulaSpec, GaussianCopula};
use crate::instruments::fixed_income::structured_credit::utils::rates::cdr_to_mdr;
use finstack_core::math::distributions::binomial_distribution;
use finstack_core::math::{standard_normal_inv_cdf, student_t_inv_cdf};

/// Seasoning curve specification for default models.
///
/// Allows applying seasoning-dependent multipliers to base CDR,
/// following industry-standard curves like SDA for mortgages.
#[derive(Debug, Clone, Default)]
pub(crate) enum SeasoningCurve {
    /// No seasoning adjustment (constant CDR).
    #[default]
    Flat,

    /// SDA (Standard Default Assumption) curve for residential mortgages.
    ///
    /// Follows the PSA/BMA standard shape (expressed as a multiplier of the
    /// base/peak CDR):
    /// - Ramps from 0% to the peak over months 1-30
    /// - Flat plateau at the peak through month 60
    /// - Declines linearly to 5% of the peak by month 120
    /// - Stays flat thereafter
    ///
    /// The multiplier scales the entire curve (e.g., 150% SDA = 1.5x).
    Sda {
        /// SDA speed multiplier (1.0 = 100% SDA)
        speed_multiplier: f64,
    },

    /// Custom vintage curve with explicit monthly multipliers.
    ///
    /// The vector contains multipliers for each month starting from month 1.
    /// Seasoning beyond the vector length uses the last value.
    Custom {
        /// Monthly multipliers starting from month 1.
        multipliers: Vec<f64>,
    },
}

impl SeasoningCurve {
    /// Create a flat (no seasoning) curve.
    pub(crate) fn flat() -> Self {
        SeasoningCurve::Flat
    }

    /// Create an SDA curve with the specified speed multiplier.
    pub(crate) fn sda(speed_multiplier: f64) -> Self {
        SeasoningCurve::Sda { speed_multiplier }
    }

    /// Get the seasoning multiplier for a given month.
    ///
    /// Returns the multiplier to apply to the base CDR at this seasoning.
    pub(crate) fn multiplier(&self, seasoning_months: u32) -> f64 {
        match self {
            SeasoningCurve::Flat => 1.0,

            SeasoningCurve::Sda { speed_multiplier } => {
                // PSA/BMA SDA curve shape: ramp to peak at month 30, flat
                // plateau through month 60, linear decline to the terminal
                // level (5% of peak: 0.03%/0.60%) by month 120, flat after.
                let peak_month = 30;
                let peak_cdr_mult = 1.0; // Peak at 100% of base at month 30
                let plateau_end_month = 60;
                let terminal_month = 120;
                let terminal_cdr_mult = 0.05; // Terminal at 5% of peak

                let base_mult = if seasoning_months == 0 {
                    0.0
                } else if seasoning_months <= peak_month {
                    // Ramp up to peak
                    (seasoning_months as f64 / peak_month as f64) * peak_cdr_mult
                } else if seasoning_months <= plateau_end_month {
                    // Flat plateau at the peak
                    peak_cdr_mult
                } else if seasoning_months <= terminal_month {
                    // Decline to terminal
                    let months_past_plateau = (seasoning_months - plateau_end_month) as f64;
                    let decline_period = (terminal_month - plateau_end_month) as f64;
                    peak_cdr_mult
                        - (months_past_plateau / decline_period)
                            * (peak_cdr_mult - terminal_cdr_mult)
                } else {
                    // Terminal rate
                    terminal_cdr_mult
                };

                base_mult * speed_multiplier
            }

            SeasoningCurve::Custom { multipliers } => {
                if seasoning_months == 0 || multipliers.is_empty() {
                    1.0
                } else {
                    // Use 1-based indexing for seasoning
                    let idx = (seasoning_months as usize - 1).min(multipliers.len() - 1);
                    multipliers[idx]
                }
            }
        }
    }
}

/// Copula-based stochastic default model.
///
/// Uses the shared copula infrastructure for default correlation modeling.
/// Supports seasoning-adjusted default rates via optional seasoning curve.
///
/// Dispatches threshold computation based on copula type:
/// - Gaussian/RFL/Multi-factor: Φ⁻¹(PD)
/// - Student-t: t_ν⁻¹(PD)
pub(crate) struct CopulaBasedDefault {
    /// Base annual CDR
    base_cdr: f64,
    /// Copula specification (kept for Clone and threshold dispatch)
    copula_spec: CopulaSpec,
    /// Asset correlation
    correlation: f64,
    /// Copula instance
    copula: Box<dyn Copula>,
    /// Optional seasoning curve for time-varying default rates
    seasoning_curve: SeasoningCurve,
}

impl std::fmt::Debug for CopulaBasedDefault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopulaBasedDefault")
            .field("base_cdr", &self.base_cdr)
            .field("copula_spec", &self.copula_spec)
            .field("correlation", &self.correlation)
            .field("copula_model", &self.copula.model_name())
            .finish()
    }
}

impl Clone for CopulaBasedDefault {
    fn clone(&self) -> Self {
        Self {
            base_cdr: self.base_cdr,
            copula_spec: self.copula_spec.clone(),
            correlation: self.correlation,
            copula: Self::build_copula(&self.copula_spec),
            seasoning_curve: self.seasoning_curve.clone(),
        }
    }
}

impl CopulaBasedDefault {
    /// Create a copula-based default model.
    ///
    /// # Arguments
    /// * `base_cdr` - Base annual CDR (unconditional)
    /// * `copula_spec` - Copula model specification
    /// * `correlation` - Asset correlation
    pub(crate) fn new(base_cdr: f64, copula_spec: CopulaSpec, correlation: f64) -> Self {
        let copula = Self::build_copula(&copula_spec);

        Self {
            base_cdr: base_cdr.clamp(0.0, 1.0),
            copula_spec,
            correlation: correlation.clamp(0.0, 0.99),
            copula,
            seasoning_curve: SeasoningCurve::Flat,
        }
    }

    fn build_copula(spec: &CopulaSpec) -> Box<dyn Copula> {
        match spec.build() {
            Ok(copula) => copula,
            Err(err) => {
                tracing::warn!(
                    %err,
                    "Invalid copula spec in stochastic default model; using Gaussian copula fallback"
                );
                Box::new(GaussianCopula::new())
            }
        }
    }

    /// Compute the default threshold appropriate for the copula type.
    ///
    /// - Gaussian/RFL/Multi-factor: Φ⁻¹(PD)
    /// - Student-t: t_ν⁻¹(PD)
    fn default_threshold(&self, pd: f64) -> f64 {
        let p = pd.clamp(1e-10, 1.0 - 1e-10);
        match &self.copula_spec {
            CopulaSpec::StudentT { degrees_of_freedom } => {
                student_t_inv_cdf(p, *degrees_of_freedom)
            }
            _ => standard_normal_inv_cdf(p),
        }
    }

    /// Create with Gaussian copula and specified correlation.
    pub(crate) fn gaussian(base_cdr: f64, correlation: f64) -> Self {
        Self::new(base_cdr, CopulaSpec::Gaussian, correlation)
    }

    /// Standard RMBS calibration.
    ///
    /// Uses the registry-backed `rmbs_standard` calibration profile:
    /// - Base CDR: 2%
    /// - Correlation: 5% (low for diversified pools)
    /// - SDA seasoning curve (100%)
    pub(crate) fn rmbs_standard() -> Self {
        let calibration = rmbs_standard();
        Self::gaussian(calibration.base_cdr, calibration.default_correlation)
            .with_seasoning_curve(SeasoningCurve::sda(1.0))
    }

    /// Standard CLO calibration.
    ///
    /// Uses the registry-backed `clo_standard` calibration profile:
    /// - Base CDR: 3%
    /// - Correlation: 20% (higher for corporate loans)
    /// - No seasoning curve (flat CDR)
    pub(crate) fn clo_standard() -> Self {
        let calibration = clo_standard();
        Self::gaussian(calibration.base_cdr, calibration.default_correlation)
    }

    /// Add a seasoning curve to the model.
    ///
    /// This allows time-varying default rates based on loan seasoning,
    /// following industry-standard curves like SDA.
    pub(crate) fn with_seasoning_curve(mut self, curve: SeasoningCurve) -> Self {
        self.seasoning_curve = curve;
        self
    }

    /// Get the base CDR.
    pub(crate) fn base_cdr(&self) -> f64 {
        self.base_cdr
    }

    /// Get the copula specification.
    pub(crate) fn copula_spec(&self) -> &CopulaSpec {
        &self.copula_spec
    }

    /// Get the seasoning curve.
    pub(crate) fn seasoning_curve(&self) -> &SeasoningCurve {
        &self.seasoning_curve
    }

    /// Get the seasoning-adjusted CDR at a given month.
    ///
    /// Applies the seasoning curve multiplier to the base CDR.
    pub(crate) fn seasoned_cdr(&self, seasoning_months: u32) -> f64 {
        let multiplier = self.seasoning_curve.multiplier(seasoning_months);
        (self.base_cdr * multiplier).clamp(0.0, 1.0)
    }
}

impl StochasticDefault for CopulaBasedDefault {
    fn conditional_mdr(
        &self,
        seasoning: u32,
        factors: &[f64],
        _macro_factors: &MacroCreditFactors,
    ) -> f64 {
        // Get the seasoning-adjusted annual CDR
        let adjusted_cdr = self.seasoned_cdr(seasoning);

        // Use annual CDR directly for copula threshold (annual-horizon calibration).
        // The copula threshold must use the same horizon as the calibration;
        // converting to MDR first would apply a monthly horizon to an
        // annually-calibrated copula, understating conditional default probability.
        let threshold = self.default_threshold(adjusted_cdr);

        let annual_cond_pd =
            self.copula
                .conditional_default_prob(threshold, factors, self.correlation);

        // Convert conditional annual PD to monthly MDR
        let monthly_pd = 1.0 - (1.0 - annual_cond_pd.clamp(0.0, 1.0)).powf(1.0 / 12.0);

        monthly_pd.clamp(0.0, 1.0)
    }

    fn default_distribution(
        &self,
        n: usize,
        pds: &[f64],
        factors: &[f64],
        correlation: f64,
    ) -> Vec<f64> {
        // For homogeneous pool, use binomial distribution
        // with conditional default probability

        let pd = pds.first().copied().unwrap_or(self.base_cdr);
        let threshold = self.default_threshold(pd);

        let cond_pd = self
            .copula
            .conditional_default_prob(threshold, factors, correlation);

        // Use the core binomial distribution function
        binomial_distribution(n, cond_pd.clamp(0.0, 1.0))
    }

    fn correlation(&self) -> f64 {
        self.correlation
    }

    fn model_name(&self) -> &'static str {
        "Copula-Based Default Model"
    }

    fn expected_mdr(&self, seasoning: u32) -> f64 {
        cdr_to_mdr(self.seasoned_cdr(seasoning))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copula_based_creation() {
        let model = CopulaBasedDefault::gaussian(0.02, 0.20);

        assert!((model.base_cdr() - 0.02).abs() < 1e-10);
        assert!((model.correlation() - 0.20).abs() < 1e-10);
    }

    #[test]
    fn test_conditional_mdr_at_zero_factor() {
        let model = CopulaBasedDefault::gaussian(0.02, 0.20);
        let factors = MacroCreditFactors::default();

        let mdr = model.conditional_mdr(12, &[0.0], &factors);
        let expected = model.expected_mdr(12);

        // At Z=0 with correlation, conditional differs from unconditional
        // The relationship depends on how correlation affects the copula formula
        assert!(mdr > 0.0 && mdr < 1.0, "MDR {} should be in (0, 1)", mdr);
        // Both should be small values (around 0.17% monthly for 2% annual)
        assert!(
            expected > 0.0 && expected < 0.01,
            "Expected MDR {} should be small",
            expected
        );
    }

    #[test]
    fn test_negative_factor_increases_mdr() {
        let model = CopulaBasedDefault::gaussian(0.02, 0.30);
        let factors = MacroCreditFactors::default();

        let mdr_neg = model.conditional_mdr(12, &[-2.0], &factors);
        let mdr_zero = model.conditional_mdr(12, &[0.0], &factors);
        let mdr_pos = model.conditional_mdr(12, &[2.0], &factors);

        // Negative factor (stress) should increase defaults
        assert!(mdr_neg > mdr_zero, "Negative factor should increase MDR");
        assert!(mdr_pos < mdr_zero, "Positive factor should decrease MDR");
    }

    #[test]
    fn test_default_distribution_sums_to_one() {
        let model = CopulaBasedDefault::gaussian(0.05, 0.20);

        let dist = model.default_distribution(10, &[0.05], &[0.0], 0.20);

        let sum: f64 = dist.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_standard_calibrations() {
        let rmbs = CopulaBasedDefault::rmbs_standard();
        assert!((rmbs.base_cdr() - 0.02).abs() < 1e-10);
        assert!((rmbs.correlation() - 0.05).abs() < 1e-10);

        let clo = CopulaBasedDefault::clo_standard();
        assert!((clo.base_cdr() - 0.03).abs() < 1e-10);
        assert!(clo.correlation() > rmbs.correlation());
    }

    #[test]
    fn test_seasoning_curve_flat() {
        let curve = SeasoningCurve::flat();

        assert!((curve.multiplier(0) - 1.0).abs() < 1e-10);
        assert!((curve.multiplier(12) - 1.0).abs() < 1e-10);
        assert!((curve.multiplier(60) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_seasoning_curve_sda() {
        let curve = SeasoningCurve::sda(1.0);

        // Month 0 should be 0
        assert!((curve.multiplier(0) - 0.0).abs() < 1e-10);

        // Ramp up to peak at month 30
        let m15 = curve.multiplier(15);
        let m30 = curve.multiplier(30);
        assert!(m15 > 0.0 && m15 < m30, "Multiplier should ramp up");
        assert!((m30 - 1.0).abs() < 1e-10, "Peak at month 30 should be 1.0");

        // Flat plateau through month 60
        let m45 = curve.multiplier(45);
        let m60 = curve.multiplier(60);
        assert!(
            (m45 - 1.0).abs() < 1e-10,
            "Plateau at month 45 should be 1.0"
        );
        assert!(
            (m60 - 1.0).abs() < 1e-10,
            "Plateau at month 60 should be 1.0"
        );

        // Decline after the plateau
        let m90 = curve.multiplier(90);
        assert!(m90 < m60, "Multiplier should decline after the plateau");
        assert!(
            (m90 - 0.525).abs() < 1e-10,
            "Mid-decline at month 90 should be 0.525"
        );

        // Terminal rate at month 120 and beyond
        let m120 = curve.multiplier(120);
        let m200 = curve.multiplier(200);
        assert!((m120 - 0.05).abs() < 1e-10, "Terminal rate should be 5%");
        assert!((m200 - 0.05).abs() < 1e-10, "Terminal rate stays flat");
    }

    #[test]
    fn test_seasoning_curve_sda_speed_multiplier() {
        let curve_100 = SeasoningCurve::sda(1.0);
        let curve_150 = SeasoningCurve::sda(1.5);

        let m30_100 = curve_100.multiplier(30);
        let m30_150 = curve_150.multiplier(30);

        assert!(
            (m30_150 - 1.5 * m30_100).abs() < 1e-10,
            "150% SDA should be 1.5x"
        );
    }

    #[test]
    fn test_seasoning_affects_mdr() {
        let model =
            CopulaBasedDefault::gaussian(0.02, 0.20).with_seasoning_curve(SeasoningCurve::sda(1.0));

        let factors = MacroCreditFactors::default();

        // Early seasoning should have lower MDR
        let mdr_early = model.conditional_mdr(6, &[0.0], &factors);

        // Peak seasoning should have higher MDR
        let mdr_peak = model.conditional_mdr(30, &[0.0], &factors);

        // Late seasoning should have lower MDR again
        let mdr_late = model.conditional_mdr(120, &[0.0], &factors);

        assert!(mdr_early < mdr_peak, "Early MDR should be less than peak");
        assert!(mdr_late < mdr_peak, "Late MDR should be less than peak");
    }

    #[test]
    fn test_rmbs_standard_has_sda_curve() {
        let rmbs = CopulaBasedDefault::rmbs_standard();

        // RMBS standard should use SDA curve, so seasoned CDR varies
        let cdr_early = rmbs.seasoned_cdr(6);
        let cdr_peak = rmbs.seasoned_cdr(30);
        let cdr_late = rmbs.seasoned_cdr(120);

        assert!(cdr_early < cdr_peak, "Early CDR should be less than peak");
        assert!(cdr_late < cdr_peak, "Late CDR should be less than peak");
    }

    #[test]
    fn test_clo_standard_has_flat_curve() {
        let clo = CopulaBasedDefault::clo_standard();

        // CLO standard should use flat curve
        let cdr_early = clo.seasoned_cdr(6);
        let cdr_mid = clo.seasoned_cdr(30);
        let cdr_late = clo.seasoned_cdr(120);

        assert!(
            (cdr_early - cdr_mid).abs() < 1e-10,
            "CLO CDR should be flat"
        );
        assert!((cdr_mid - cdr_late).abs() < 1e-10, "CLO CDR should be flat");
    }
}
