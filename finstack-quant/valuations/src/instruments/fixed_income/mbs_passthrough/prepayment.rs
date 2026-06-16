//! Prepayment model integration for agency MBS.
//!
//! This module provides wrappers around the cashflow builder's prepayment
//! infrastructure, exposing both deterministic (PSA/CPR) and stochastic
//! (factor-correlated, Richard-Roll) prepayment models.

use crate::cashflow::builder::specs::PrepaymentModelSpec;
use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry;
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::prepayment::{
    RichardRollPrepay, StochasticPrepayment,
};

/// Agency MBS prepayment model wrapper.
///
/// Combines deterministic prepayment curves (PSA, constant CPR) with optional
/// stochastic extensions for factor correlation and refinancing incentive.
///
/// # Model Types
///
/// 1. **Deterministic (PSA/CPR)**: Uses `PrepaymentModelSpec` for predictable
///    prepayment curves based on seasoning.
///
/// 2. **Stochastic (Factor-Correlated)**: Adds systematic factor shocks to
///    the base prepayment rate for Monte Carlo simulations.
///
/// 3. **Richard-Roll**: Full refinancing incentive model with seasoning ramp,
///    burnout effects, and seasonality patterns.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::prepayment::AgencyPrepaymentModel;
/// use finstack_quant_cashflows::builder::specs::PrepaymentModelSpec;
///
/// // Standard 100% PSA
/// let model = AgencyPrepaymentModel::from_spec(PrepaymentModelSpec::psa(1.0));
/// let smm = model.smm(24); // 24 months seasoning
///
/// // Richard-Roll with refi incentive
/// let refi_model = AgencyPrepaymentModel::richard_roll(0.045, 2.0).expect("calibration loads");
/// ```
#[derive(Debug, Clone)]
pub struct AgencyPrepaymentModel {
    /// Base deterministic prepayment specification.
    base_spec: PrepaymentModelSpec,
    /// Optional stochastic prepayment model.
    stochastic: Option<Box<dyn StochasticPrepaymentClone>>,
}

/// Helper trait to allow cloning of boxed stochastic models.
pub trait StochasticPrepaymentClone: StochasticPrepayment {
    /// Clone into a boxed trait object.
    fn clone_box(&self) -> Box<dyn StochasticPrepaymentClone>;
}

impl<T: StochasticPrepayment + Clone + 'static> StochasticPrepaymentClone for T {
    fn clone_box(&self) -> Box<dyn StochasticPrepaymentClone> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn StochasticPrepaymentClone> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl AgencyPrepaymentModel {
    /// Create from a deterministic prepayment spec.
    pub fn from_spec(spec: PrepaymentModelSpec) -> Self {
        Self {
            base_spec: spec,
            stochastic: None,
        }
    }

    /// Create standard 100% PSA model.
    pub fn psa_100() -> Self {
        Self::from_spec(PrepaymentModelSpec::psa(1.0))
    }

    /// Create PSA model with custom speed multiplier.
    ///
    /// # Arguments
    ///
    /// * `speed` - PSA speed multiplier (1.0 = 100% PSA, 2.0 = 200% PSA)
    pub fn psa(speed: f64) -> Self {
        Self::from_spec(PrepaymentModelSpec::psa(speed))
    }

    /// Create constant CPR model.
    ///
    /// # Arguments
    ///
    /// * `cpr` - Constant prepayment rate (annual, e.g., 0.06 for 6% CPR)
    pub fn constant_cpr(cpr: f64) -> Self {
        Self::from_spec(PrepaymentModelSpec::constant_cpr(cpr))
    }

    /// Create Richard-Roll prepayment model with refinancing incentive.
    ///
    /// # Arguments
    ///
    /// * `pool_coupon` - Pool weighted average coupon (WAC)
    /// * `refi_sensitivity` - Refinancing sensitivity parameter (typically 2.0-3.0)
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded RMBS calibration assets fail to load.
    pub fn richard_roll(
        pool_coupon: f64,
        refi_sensitivity: f64,
    ) -> finstack_quant_core::Result<Self> {
        let (base_cpr, burnout_rate) = standard_rmbs_prepayment_defaults()?;
        let rr = RichardRollPrepay::new(base_cpr, refi_sensitivity, pool_coupon, burnout_rate);

        Ok(Self {
            base_spec: PrepaymentModelSpec::constant_cpr(base_cpr),
            stochastic: Some(Box::new(rr)),
        })
    }

    /// Create agency-standard Richard-Roll model.
    ///
    /// Uses calibrated parameters typical for conforming agency MBS.
    pub fn agency_standard(pool_coupon: f64) -> Self {
        let rr = RichardRollPrepay::agency_standard(pool_coupon);
        Self {
            base_spec: PrepaymentModelSpec::psa(1.0),
            stochastic: Some(Box::new(rr)),
        }
    }

    /// Get base prepayment spec.
    pub fn base_spec(&self) -> &PrepaymentModelSpec {
        &self.base_spec
    }

    /// Check if model has stochastic component.
    pub fn has_stochastic(&self) -> bool {
        self.stochastic.is_some()
    }

    /// Get SMM (single monthly mortality) for given seasoning.
    ///
    /// Uses the deterministic base spec. For stochastic rates, use
    /// `conditional_smm` with factor realizations.
    pub fn smm(&self, seasoning_months: u32) -> finstack_quant_core::Result<f64> {
        self.base_spec.smm(seasoning_months)
    }

    /// Get conditional SMM with factor realizations (stochastic mode).
    ///
    /// # Arguments
    ///
    /// * `seasoning_months` - Months since origination
    /// * `factors` - Systematic factor realizations
    /// * `market_rate` - Current mortgage rate (for refi incentive)
    /// * `burnout` - Burnout factor in [0, 1] (1 = no burnout)
    ///
    /// # Returns
    ///
    /// SMM conditional on factors if stochastic model is set,
    /// otherwise returns deterministic SMM.
    pub fn conditional_smm(
        &self,
        seasoning_months: u32,
        factors: &[f64],
        market_rate: f64,
        burnout: f64,
    ) -> finstack_quant_core::Result<f64> {
        if let Some(ref stoch) = self.stochastic {
            Ok(stoch.conditional_smm(seasoning_months, factors, market_rate, burnout))
        } else {
            self.smm(seasoning_months)
        }
    }

    /// Get expected (unconditional) SMM at given seasoning.
    ///
    /// For stochastic models, this is E[SMM(t)] integrated over the
    /// factor distribution.
    pub fn expected_smm(&self, seasoning_months: u32) -> finstack_quant_core::Result<f64> {
        if let Some(ref stoch) = self.stochastic {
            Ok(stoch.expected_smm(seasoning_months))
        } else {
            self.smm(seasoning_months)
        }
    }
}

fn standard_rmbs_prepayment_defaults() -> finstack_quant_core::Result<(f64, f64)> {
    let calibration = embedded_registry()?.rmbs_stochastic_calibration("rmbs_standard")?;
    Ok((calibration.base_cpr, calibration.burnout_rate))
}

/// Convert CPR (constant prepayment rate) to SMM (single monthly mortality).
///
/// SMM = 1 - (1 - CPR)^(1/12)
///
/// # Errors
///
/// Returns `Error::Validation` if `cpr` is not in `[0.0, 1.0]` or is non-finite.
pub fn cpr_to_smm(cpr: f64) -> finstack_quant_core::Result<f64> {
    if !cpr.is_finite() || !(0.0..=1.0).contains(&cpr) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cpr_to_smm: CPR must be a finite value in [0.0, 1.0], got {cpr}"
        )));
    }
    Ok(1.0 - (1.0 - cpr).powf(1.0 / 12.0))
}

/// Convert SMM (single monthly mortality) to CPR (constant prepayment rate).
///
/// CPR = 1 - (1 - SMM)^12
///
/// # Errors
///
/// Returns `Error::Validation` if `smm` is not in `[0.0, 1.0]` or is non-finite.
pub fn smm_to_cpr(smm: f64) -> finstack_quant_core::Result<f64> {
    if !smm.is_finite() || !(0.0..=1.0).contains(&smm) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "smm_to_cpr: SMM must be a finite value in [0.0, 1.0], got {smm}"
        )));
    }
    Ok(1.0 - (1.0 - smm).powi(12))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psa_100_smm() {
        let model = AgencyPrepaymentModel::psa_100();

        // At 0 months: 0% CPR = 0 SMM
        let smm_0 = model.smm(0).expect("smm(0)");
        assert!(smm_0.abs() < 1e-10);

        // At 30 months: 6% CPR (terminal PSA)
        let smm_30 = model.smm(30).expect("smm(30)");
        let expected_smm_30 = cpr_to_smm(0.06).expect("valid CPR");
        assert!((smm_30 - expected_smm_30).abs() < 1e-6);

        // At 60 months: still 6% CPR (post-ramp)
        let smm_60 = model.smm(60).expect("smm(60)");
        assert!((smm_60 - expected_smm_30).abs() < 1e-6);
    }

    #[test]
    fn test_psa_200_smm() {
        let model = AgencyPrepaymentModel::psa(2.0);

        // At 30 months: 12% CPR (200% PSA)
        let smm_30 = model.smm(30).expect("smm(30)");
        let expected = cpr_to_smm(0.12).expect("valid CPR");
        assert!((smm_30 - expected).abs() < 1e-6);
    }

    #[test]
    fn test_constant_cpr() {
        let model = AgencyPrepaymentModel::constant_cpr(0.08);

        // Should be constant regardless of seasoning
        let smm_0 = model.smm(0).expect("smm(0)");
        let smm_30 = model.smm(30).expect("smm(30)");
        let expected = cpr_to_smm(0.08).expect("valid CPR");

        assert!((smm_0 - expected).abs() < 1e-6);
        assert!((smm_30 - expected).abs() < 1e-6);
    }

    #[test]
    fn test_richard_roll_rate_sensitivity() {
        let model = AgencyPrepaymentModel::richard_roll(0.045, 2.0).expect("calibration loads");
        assert!(model.has_stochastic());

        // When market rate is below pool coupon (refi incentive)
        let smm_low_rate = model
            .conditional_smm(36, &[0.0], 0.03, 1.0)
            .expect("valid conditional_smm");
        let smm_at_coupon = model
            .conditional_smm(36, &[0.0], 0.045, 1.0)
            .expect("valid conditional_smm");
        let smm_high_rate = model
            .conditional_smm(36, &[0.0], 0.06, 1.0)
            .expect("valid conditional_smm");

        // Lower rates should increase prepayment
        assert!(smm_low_rate > smm_at_coupon);
        assert!(smm_high_rate < smm_at_coupon);
    }

    #[test]
    fn test_cpr_smm_conversion() {
        // 6% CPR
        let cpr = 0.06;
        let smm = cpr_to_smm(cpr).expect("valid CPR");
        let cpr_back = smm_to_cpr(smm).expect("valid SMM");

        assert!((cpr_back - cpr).abs() < 1e-10);

        // SMM should be approximately CPR/12 for small rates
        // 6%/12 = 0.5%
        assert!((smm - 0.005).abs() < 0.001);
    }

    #[test]
    fn test_cpr_smm_rejects_negative_and_out_of_range() {
        assert!(cpr_to_smm(-0.01).is_err());
        assert!(cpr_to_smm(1.01).is_err());
        assert!(cpr_to_smm(f64::NAN).is_err());
        assert!(smm_to_cpr(-0.01).is_err());
        assert!(smm_to_cpr(1.01).is_err());
        assert!(smm_to_cpr(f64::NAN).is_err());
    }

    #[test]
    fn test_model_clone() {
        let model = AgencyPrepaymentModel::richard_roll(0.045, 2.0).expect("calibration loads");
        let cloned = model.clone();

        // Both should produce same SMM
        let smm1 = model
            .conditional_smm(24, &[0.0], 0.04, 1.0)
            .expect("valid conditional_smm");
        let smm2 = cloned
            .conditional_smm(24, &[0.0], 0.04, 1.0)
            .expect("valid conditional_smm");

        assert!((smm1 - smm2).abs() < 1e-10);
    }
}
