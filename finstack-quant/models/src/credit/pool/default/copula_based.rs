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
//! - Li, D. X. (2000). "On Default Correlation: A Copula Function Approach." `docs/REFERENCES.md#li-2000-gaussian-copula`

use super::super::clamped_cdr_to_mdr;
use super::traits::StochasticDefault;
use crate::correlation::copula::{Copula, CopulaSpec};
use finstack_quant_core::math::{standard_normal_inv_cdf, student_t_inv_cdf};

/// Copula-based stochastic default model.
///
/// Uses the shared copula infrastructure for default correlation modeling.
///
/// Dispatches threshold computation based on copula type:
/// - Gaussian/RFL/Multi-factor: Φ⁻¹(PD)
/// - Student-t: t_ν⁻¹(PD)
pub(crate) struct CopulaBasedDefault {
    /// Base annual CDR
    base_cdr: f64,
    /// Copula specification
    copula_spec: CopulaSpec,
    /// Asset correlation
    correlation: f64,
    /// Copula instance
    copula: Box<dyn Copula>,
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

impl CopulaBasedDefault {
    /// Create a copula-based default model.
    ///
    /// # Arguments
    /// * `base_cdr` - Base annual CDR (unconditional)
    /// * `copula_spec` - Copula model specification
    /// * `correlation` - Asset correlation
    ///
    /// # Errors
    ///
    /// Returns an error if the copula spec is invalid.
    pub(crate) fn new(
        base_cdr: f64,
        copula_spec: CopulaSpec,
        correlation: f64,
    ) -> finstack_quant_core::Result<Self> {
        let copula = copula_spec.build().map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "copula construction failed for {copula_spec:?}: {e}"
            ))
        })?;

        Ok(Self {
            base_cdr: base_cdr.clamp(0.0, 1.0),
            copula_spec,
            correlation: correlation.clamp(0.0, 0.99),
            copula,
        })
    }

    /// Compute the default threshold appropriate for the copula type.
    ///
    /// - Gaussian/RFL/Multi-factor: Φ⁻¹(PD)
    /// - Student-t: t_ν⁻¹(PD)
    fn default_threshold(&self, pd: f64) -> f64 {
        let p = pd.clamp(1e-10, 1.0 - 1e-10);
        match &self.copula_spec {
            CopulaSpec::StudentT { degrees_of_freedom } => {
                student_t_inv_cdf(p, *degrees_of_freedom).unwrap_or(f64::NAN)
            }
            _ => standard_normal_inv_cdf(p),
        }
    }
}

impl StochasticDefault for CopulaBasedDefault {
    fn conditional_mdr(&self, _seasoning: u32, factors: &[f64]) -> f64 {
        let threshold = self.default_threshold(self.base_cdr);

        let annual_cond_pd =
            self.copula
                .conditional_default_prob(threshold, factors, self.correlation);

        // Convert conditional annual PD to monthly MDR
        clamped_cdr_to_mdr(annual_cond_pd)
    }

    fn correlation(&self) -> f64 {
        self.correlation
    }

    fn expected_mdr(&self, _seasoning: u32) -> f64 {
        clamped_cdr_to_mdr(self.base_cdr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copula_based_creation() {
        let model = CopulaBasedDefault::new(0.02, CopulaSpec::Gaussian, 0.20)
            .expect("valid Gaussian copula");

        assert!((model.base_cdr - 0.02).abs() < 1e-10);
        assert!((model.correlation() - 0.20).abs() < 1e-10);
    }

    #[test]
    fn test_conditional_mdr_at_zero_factor() {
        let model = CopulaBasedDefault::new(0.02, CopulaSpec::Gaussian, 0.20)
            .expect("valid Gaussian copula");
        let mdr = model.conditional_mdr(12, &[0.0]);
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
        let model = CopulaBasedDefault::new(0.02, CopulaSpec::Gaussian, 0.30)
            .expect("valid Gaussian copula");
        let mdr_neg = model.conditional_mdr(12, &[-2.0]);
        let mdr_zero = model.conditional_mdr(12, &[0.0]);
        let mdr_pos = model.conditional_mdr(12, &[2.0]);

        // Negative factor (stress) should increase defaults
        assert!(mdr_neg > mdr_zero, "Negative factor should increase MDR");
        assert!(mdr_pos < mdr_zero, "Positive factor should decrease MDR");
    }
}
