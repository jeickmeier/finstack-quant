//! Standard calibration constants for structured credit stochastic models.
//!
//! This module provides industry-standard calibration parameters for RMBS,
//! CLO, and other structured credit instruments. Centralizing these constants
//! ensures consistency across default and prepayment models.
//!
//! # Asset Classes
//!
//! - **RMBS (Residential Mortgage-Backed Securities)**: Agency and non-agency
//! - **CLO (Collateralized Loan Obligations)**: Leveraged loan pools
//! - **CMBS (Commercial Mortgage-Backed Securities)**: Commercial loans
//!
//! # References
//!
//! - Moody's Default Study (annual corporate default rates)
//! - PSA Standard Prepayment Model assumptions `docs/REFERENCES.md#richard-roll-1989`
//! - Basel IRB correlation formulas `docs/REFERENCES.md#basel-ii-2006`

use crate::instruments::fixed_income::structured_credit::assumptions::{
    embedded_registry_or_panic, required_assumption,
};
use finstack_quant_cashflows::builder::PrepaymentModelSpec;
use finstack_quant_core::Result;
use finstack_quant_models::credit::pool::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};

/// RMBS standard calibration parameters.
///
/// Suitable for agency and prime non-agency RMBS pools.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RmbsCalibration {
    /// Base annual conditional default rate (CDR)
    pub(crate) base_cdr: f64,
    /// Default correlation (asset correlation)
    pub(crate) default_correlation: f64,
    /// Base annual conditional prepayment rate (CPR)
    pub(crate) base_cpr: f64,
    /// Prepayment factor loading (sensitivity to systematic factor)
    pub(crate) prepay_factor_loading: f64,
    /// CPR volatility
    pub(crate) cpr_volatility: f64,
    /// Default factor sensitivity (for intensity models)
    pub(crate) default_factor_sensitivity: f64,
    /// Default model mean reversion speed
    pub(crate) default_mean_reversion: f64,
    /// Default model volatility
    pub(crate) default_volatility: f64,
    /// Refinancing sensitivity (for Richard-Roll model)
    pub(crate) refi_sensitivity: f64,
    /// Burnout rate (for Richard-Roll model)
    pub(crate) burnout_rate: f64,
}

/// Standard RMBS calibration (prime/agency).
///
/// Based on historical agency RMBS performance:
/// - Low default rates (2% annual CDR)
/// - Low correlation (5%) due to pool diversification
/// - Moderate prepayment (6% CPR base)
/// - Standard PSA-style seasoning
pub(crate) fn rmbs_standard() -> RmbsCalibration {
    required_assumption(embedded_registry_or_panic().rmbs_stochastic_calibration("rmbs_standard"))
}

/// CLO standard calibration parameters.
///
/// Suitable for broadly syndicated loan CLOs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CloCalibration {
    /// Base annual conditional default rate (CDR)
    pub(crate) base_cdr: f64,
    /// Default correlation (asset correlation)
    pub(crate) default_correlation: f64,
    /// Base annual conditional prepayment rate (CPR)
    pub(crate) base_cpr: f64,
    /// Prepayment factor loading
    pub(crate) prepay_factor_loading: f64,
    /// CPR volatility
    pub(crate) cpr_volatility: f64,
    /// Default factor sensitivity
    pub(crate) default_factor_sensitivity: f64,
    /// Default model mean reversion speed
    pub(crate) default_mean_reversion: f64,
    /// Default model volatility
    pub(crate) default_volatility: f64,
}

/// Standard CLO calibration (broadly syndicated loans).
///
/// Based on historical CLO performance:
/// - The registry base case: 2% annual CDR
/// - Higher correlation (20-25%) for corporate exposures
/// - Higher prepayment (20% CPR, the CLO registry base case) due to refinancing
pub(crate) fn clo_standard() -> CloCalibration {
    required_assumption(embedded_registry_or_panic().clo_stochastic_calibration("clo_standard"))
}

/// CMBS standard calibration parameters.
///
/// Suitable for conduit CMBS transactions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CmbsCalibration {
    /// Base annual conditional default rate (CDR)
    pub(crate) base_cdr: f64,
    /// Default correlation (asset correlation)
    pub(crate) default_correlation: f64,
    /// Base annual conditional prepayment rate (CPR)
    pub(crate) base_cpr: f64,
    /// Prepayment factor loading
    pub(crate) prepay_factor_loading: f64,
    /// CPR volatility
    pub(crate) cpr_volatility: f64,
}

/// Standard CMBS calibration (conduit).
///
/// Commercial mortgages have:
/// - Moderate default rates (2.5% annual CDR)
/// - Moderate correlation (15%)
/// - Low prepayment due to lockouts/defeasance (3% CPR)
pub(crate) fn cmbs_standard() -> CmbsCalibration {
    required_assumption(embedded_registry_or_panic().cmbs_stochastic_calibration("cmbs_standard"))
}

pub(crate) fn rmbs_default_spec() -> StochasticDefaultSpec {
    let calibration = rmbs_standard();
    StochasticDefaultSpec::gaussian_copula(calibration.base_cdr, calibration.default_correlation)
}

pub(crate) fn clo_default_spec() -> StochasticDefaultSpec {
    let calibration = clo_standard();
    StochasticDefaultSpec::gaussian_copula(calibration.base_cdr, calibration.default_correlation)
}

pub(crate) fn rmbs_prepay_spec(pool_coupon: f64) -> StochasticPrepaySpec {
    let calibration = rmbs_standard();
    StochasticPrepaySpec::RichardRoll {
        base_cpr: calibration.base_cpr,
        refi_sensitivity: calibration.refi_sensitivity,
        pool_coupon,
        burnout_rate: calibration.burnout_rate,
        factor_loading: calibration.prepay_factor_loading,
        cpr_volatility: calibration.cpr_volatility,
    }
}

pub(crate) fn clo_prepay_spec() -> StochasticPrepaySpec {
    let calibration = clo_standard();
    StochasticPrepaySpec::factor_correlated(
        PrepaymentModelSpec::constant_cpr(calibration.base_cpr),
        calibration.prepay_factor_loading,
        calibration.cpr_volatility,
    )
}

pub(crate) fn rmbs_correlation_structure() -> Result<CorrelationStructure> {
    let calibration = rmbs_standard();
    CorrelationStructure::flat(calibration.default_correlation, -0.30)
}

pub(crate) fn clo_correlation_structure() -> Result<CorrelationStructure> {
    let calibration = clo_standard();
    CorrelationStructure::sectored(calibration.default_correlation + 0.10, 0.10, -0.20)
}

pub(crate) fn cmbs_correlation_structure() -> Result<CorrelationStructure> {
    let calibration = cmbs_standard();
    CorrelationStructure::flat(calibration.default_correlation + 0.05, -0.15)
}

pub(crate) fn abs_auto_correlation_structure() -> Result<CorrelationStructure> {
    CorrelationStructure::flat(0.08, -0.10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rmbs_standard_values() {
        let calibration = rmbs_standard();
        assert!((calibration.base_cdr - 0.02).abs() < 1e-10);
        assert!((calibration.default_correlation - 0.05).abs() < 1e-10);
        assert!((calibration.base_cpr - 0.06).abs() < 1e-10);
    }

    #[test]
    fn test_clo_standard_values() {
        // The stochastic profile shares the deterministic CLO registry
        // record: 2% CDR and 20% CPR.
        let calibration = clo_standard();
        assert!((calibration.base_cdr - 0.02).abs() < 1e-10);
        assert!((calibration.default_correlation - 0.20).abs() < 1e-10);
        assert!((calibration.base_cpr - 0.20).abs() < 1e-10);
    }

    #[test]
    fn test_cmbs_standard_values() {
        let calibration = cmbs_standard();
        assert!((calibration.base_cdr - 0.025).abs() < 1e-10);
        assert!((calibration.default_correlation - 0.15).abs() < 1e-10);
    }

    #[test]
    fn test_clo_higher_correlation_than_rmbs() {
        // Corporate loans have higher correlation than mortgages
        let clo_corr = clo_standard().default_correlation;
        let rmbs_corr = rmbs_standard().default_correlation;
        assert!(
            clo_corr > rmbs_corr,
            "CLO correlation ({}) should exceed RMBS ({})",
            clo_corr,
            rmbs_corr
        );
    }

    #[test]
    fn test_cmbs_lower_prepayment_than_rmbs() {
        // Commercial mortgages have lockouts limiting prepayment
        let cmbs_cpr = cmbs_standard().base_cpr;
        let rmbs_cpr = rmbs_standard().base_cpr;
        assert!(
            cmbs_cpr < rmbs_cpr,
            "CMBS CPR ({}) should be less than RMBS ({})",
            cmbs_cpr,
            rmbs_cpr
        );
    }
}
