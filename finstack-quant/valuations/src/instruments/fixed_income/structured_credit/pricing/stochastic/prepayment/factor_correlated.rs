//! Factor-correlated prepayment model.
//!
//! The simplest stochastic prepayment model where CPR is shocked by a
//! systematic factor. This model is suitable when prepayment risk needs
//! to be correlated with other credit events but detailed rate modeling
//! is not required.
//!
//! # Mathematical Model
//!
//! ```text
//! CPR(Z) = base_cpr × exp(β × Z × σ)
//! ```
//!
//! where:
//! - base_cpr is the deterministic CPR from PSA/CPR curve
//! - β is the factor loading
//! - Z ~ N(0,1) is the systematic factor
//! - σ is the CPR volatility
//!
//! # Calibration
//!
//! Typical parameters for RMBS:
//! - Factor loading (β): 0.3-0.5
//! - CPR volatility (σ): 0.15-0.30

#![allow(dead_code)]

use super::traits::StochasticPrepayment;
use crate::cashflow::builder::specs::{PrepaymentCurve, PrepaymentModelSpec};
use crate::instruments::fixed_income::structured_credit::utils::rates::cpr_to_smm;
use finstack_quant_core::types::Percentage;

/// Factor-correlated prepayment model.
///
/// Shocks a base CPR specification by a systematic factor.
/// Used for simple stochastic prepayment correlation.
#[derive(Debug, Clone)]
pub(crate) struct FactorCorrelatedPrepay {
    /// Base deterministic prepayment specification
    base_spec: PrepaymentModelSpec,
    /// Factor loading (sensitivity to systematic factor)
    factor_loading: f64,
    /// CPR volatility (log-normal shock scale)
    cpr_volatility: f64,
}

impl FactorCorrelatedPrepay {
    /// Create a factor-correlated prepayment model.
    ///
    /// # Arguments
    /// * `base_spec` - Base deterministic prepayment model (PSA, CPR curve, etc.)
    /// * `factor_loading` - Sensitivity to systematic factor (typical: 0.3-0.5)
    /// * `cpr_volatility` - CPR volatility (typical: 0.15-0.30)
    pub(crate) fn new(
        base_spec: PrepaymentModelSpec,
        factor_loading: f64,
        cpr_volatility: f64,
    ) -> Self {
        Self {
            base_spec,
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            cpr_volatility: cpr_volatility.clamp(0.0, 1.0),
        }
    }

    /// Create a factor-correlated prepayment model with typed volatility.
    pub(crate) fn new_typed(
        base_spec: PrepaymentModelSpec,
        factor_loading: f64,
        cpr_volatility: Percentage,
    ) -> Self {
        Self {
            base_spec,
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            cpr_volatility: cpr_volatility.as_decimal().clamp(0.0, 1.0),
        }
    }

    /// Create with RMBS-standard calibration.
    ///
    /// Uses 100% PSA as base with:
    /// - Factor loading: 0.4
    /// - CPR volatility: 0.20
    pub(crate) fn rmbs_standard(base_spec: PrepaymentModelSpec) -> Self {
        Self::new(base_spec, 0.4, 0.20)
    }

    /// Create with CLO-standard calibration.
    ///
    /// Uses lower factor loading (loans less rate-sensitive):
    /// - Factor loading: 0.25
    /// - CPR volatility: 0.15
    pub(crate) fn clo_standard(base_spec: PrepaymentModelSpec) -> Self {
        Self::new(base_spec, 0.25, 0.15)
    }

    /// Get the base prepayment specification.
    pub(crate) fn base_spec(&self) -> &PrepaymentModelSpec {
        &self.base_spec
    }

    /// Get the CPR volatility.
    pub(crate) fn cpr_volatility(&self) -> f64 {
        self.cpr_volatility
    }

    /// Get the base CPR at a given seasoning.
    fn base_cpr_at_seasoning(&self, seasoning: u32) -> f64 {
        // Check for no-prepayment case (cpr = 0)
        if self.base_spec.cpr < 1e-10 {
            return 0.0;
        }

        match &self.base_spec.curve {
            None | Some(PrepaymentCurve::Constant) => self.base_spec.cpr,
            Some(PrepaymentCurve::Psa { speed_multiplier }) => {
                // PSA ramp: 0.2% CPR per month up to month 30, then flat at 6% × speed
                let base_cpr = if seasoning < 30 {
                    0.002 * seasoning as f64
                } else {
                    0.06
                };
                base_cpr * speed_multiplier
            }
            Some(PrepaymentCurve::CmbsLockout { lockout_months }) => {
                // Zero prepayment during lockout, then constant CPR
                if seasoning <= *lockout_months {
                    0.0
                } else {
                    self.base_spec.cpr
                }
            }
        }
    }
}

impl StochasticPrepayment for FactorCorrelatedPrepay {
    fn conditional_smm(
        &self,
        seasoning: u32,
        factors: &[f64],
        _market_rate: f64,
        burnout: f64,
    ) -> f64 {
        let base_cpr = self.base_cpr_at_seasoning(seasoning);

        // No prepayment model
        if base_cpr < 1e-10 {
            return 0.0;
        }

        // Apply factor shock: CPR(Z) = base × exp(β × Z × σ)
        let z = factors.first().copied().unwrap_or(0.0);
        let shock = (self.factor_loading * z * self.cpr_volatility).exp();
        let shocked_cpr = (base_cpr * shock * burnout).clamp(0.0, 1.0);

        // Convert CPR to SMM
        cpr_to_smm(shocked_cpr)
    }

    fn expected_smm(&self, seasoning: u32) -> f64 {
        let base_cpr = self.base_cpr_at_seasoning(seasoning);

        // For log-normal shock, E[exp(β × Z × σ)] = exp(0.5 × β² × σ²)
        // But for correlation purposes, we use the base CPR
        cpr_to_smm(base_cpr)
    }

    fn factor_loading(&self) -> f64 {
        self.factor_loading
    }

    fn model_name(&self) -> &'static str {
        "Factor-Correlated Prepayment"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factor_correlated_creation() {
        let base = PrepaymentModelSpec::constant_cpr(0.10);
        let model = FactorCorrelatedPrepay::new(base, 0.4, 0.20);

        assert!((model.factor_loading() - 0.4).abs() < 1e-10);
        assert!((model.cpr_volatility() - 0.20).abs() < 1e-10);
    }

    #[test]
    fn test_conditional_smm_at_zero_factor() {
        let base = PrepaymentModelSpec::constant_cpr(0.10);
        let model = FactorCorrelatedPrepay::new(base, 0.4, 0.20);

        let smm = model.conditional_smm(12, &[0.0], 0.05, 1.0);
        let expected_smm = cpr_to_smm(0.10);

        assert!(
            (smm - expected_smm).abs() < 1e-6,
            "At Z=0, SMM {} should equal base SMM {}",
            smm,
            expected_smm
        );
    }

    #[test]
    fn test_positive_factor_increases_smm() {
        let base = PrepaymentModelSpec::constant_cpr(0.10);
        let model = FactorCorrelatedPrepay::new(base, 0.4, 0.20);

        let smm_neg = model.conditional_smm(12, &[-2.0], 0.05, 1.0);
        let smm_zero = model.conditional_smm(12, &[0.0], 0.05, 1.0);
        let smm_pos = model.conditional_smm(12, &[2.0], 0.05, 1.0);

        assert!(smm_pos > smm_zero, "Positive factor should increase SMM");
        assert!(smm_neg < smm_zero, "Negative factor should decrease SMM");
    }

    #[test]
    fn test_psa_ramp() {
        let base = PrepaymentModelSpec::psa(1.0);
        let model = FactorCorrelatedPrepay::new(base, 0.4, 0.20);

        // Early seasoning should have lower CPR
        let smm_early = model.conditional_smm(6, &[0.0], 0.05, 1.0);
        let smm_late = model.conditional_smm(36, &[0.0], 0.05, 1.0);

        assert!(
            smm_early < smm_late,
            "Later seasoning should have higher SMM"
        );
    }

    #[test]
    fn test_burnout_reduces_smm() {
        let base = PrepaymentModelSpec::constant_cpr(0.10);
        let model = FactorCorrelatedPrepay::new(base, 0.4, 0.20);

        let smm_full = model.conditional_smm(12, &[0.0], 0.05, 1.0);
        let smm_half = model.conditional_smm(12, &[0.0], 0.05, 0.5);

        // Burnout should reduce SMM
        assert!(smm_half < smm_full, "Half burnout should give lower SMM");
        // SMM scales with CPR but not linearly due to the 1-(1-CPR)^(1/12) formula
        // Just verify it's roughly in the right range
        assert!(smm_half > 0.0 && smm_half < smm_full);
    }

    #[test]
    fn test_no_prepayment() {
        let base = PrepaymentModelSpec::constant_cpr(0.0);
        let model = FactorCorrelatedPrepay::new(base, 0.4, 0.20);

        let smm = model.conditional_smm(12, &[2.0], 0.05, 1.0);
        assert!(smm.abs() < 1e-10);
    }

    #[test]
    fn test_standard_calibrations() {
        let base = PrepaymentModelSpec::psa(1.0);

        let rmbs = FactorCorrelatedPrepay::rmbs_standard(base.clone());
        assert!((rmbs.factor_loading() - 0.4).abs() < 1e-10);

        let clo = FactorCorrelatedPrepay::clo_standard(base);
        assert!((clo.factor_loading() - 0.25).abs() < 1e-10);
    }
}
