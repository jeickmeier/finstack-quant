//! PiT/TtC conversion and central tendency calibration.
//!
//! The Merton-Vasicek single-factor model provides the standard framework
//! for converting between Point-in-Time (PiT) and Through-the-Cycle (TtC)
//! PD estimates, as mandated by Basel II IRB.
//!
//! # References
//!
//! - Vasicek, O. A. (2002). "The Distribution of Loan Portfolio Value."
//!   *Risk*, 15(12), 160-162.
//! - BCBS (2006). "International Convergence of Capital Measurement and
//!   Capital Standards: A Revised Framework (Basel II)." Section 272.

use serde::{Deserialize, Serialize};

use crate::math::{norm_cdf, standard_normal_inv_cdf};

use super::error::PdCalibrationError;

/// Basel IRB minimum one-year PD floor: 3 basis points.
pub const BASEL_IRB_PD_FLOOR: f64 = 0.0003;

/// Apply the Basel IRB 3bp PD floor to a finite PD estimate.
///
/// Generic PD conversion and calibration helpers intentionally do not apply
/// regulatory floors by default. Use this helper explicitly in IRB workflows.
#[must_use]
pub fn apply_basel_irb_pd_floor(pd: f64) -> f64 {
    if pd.is_nan() {
        f64::NAN
    } else {
        pd.max(BASEL_IRB_PD_FLOOR)
    }
}

/// Parameters for the Merton-Vasicek single-factor PiT/TtC conversion.
///
/// Uses the asymptotic single risk factor (ASRF) model:
///
///   PD_PiT = Phi( (Phi^{-1}(PD_TtC) - sqrt(rho) * z) / sqrt(1 - rho) )
///   PD_TtC = Phi( Phi^{-1}(PD_PiT) * sqrt(1 - rho) + sqrt(rho) * z )
///
/// where Phi is the standard normal CDF, rho is the asset correlation,
/// and z is the systematic factor (cycle index).
///
/// # References
///
/// - Vasicek, O. A. (2002). "The Distribution of Loan Portfolio Value."
///   *Risk*, 15(12), 160-162.
/// - BCBS (2006). "International Convergence of Capital Measurement and
///   Capital Standards: A Revised Framework (Basel II)." Section 272.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PdCycleParams {
    /// Asset correlation rho in (0, 1).
    ///
    /// Basel II uses rho in [0.12, 0.24] for corporates.
    pub asset_correlation: f64,
    /// Systematic risk factor (cycle index).
    ///
    /// - z = 0 corresponds to average conditions (PiT == TtC).
    /// - z < 0 corresponds to downturn (stressed PiT > TtC).
    /// - z > 0 corresponds to benign conditions (PiT < TtC).
    pub cycle_index: f64,
}

/// Convert a Through-the-Cycle PD to a Point-in-Time PD.
///
/// PD_PiT = Phi( (Phi^{-1}(PD_TtC) - sqrt(rho) * z) / sqrt(1 - rho) )
///
/// # Errors
///
/// - [`PdCalibrationError::PdOutOfRange`] if `pd_ttc` is not in (0, 1).
/// - [`PdCalibrationError::InvalidCorrelation`] if `rho` is not in (0, 1).
pub fn ttc_to_pit(pd_ttc: f64, params: &PdCycleParams) -> Result<f64, PdCalibrationError> {
    validate_pd(pd_ttc)?;
    validate_correlation(params.asset_correlation)?;
    validate_cycle_index(params.cycle_index)?;

    let rho = params.asset_correlation;
    let z = params.cycle_index;

    let inv_ttc = standard_normal_inv_cdf(pd_ttc);
    let sqrt_rho = rho.sqrt();
    let sqrt_one_minus_rho = (1.0 - rho).sqrt();

    let pd_pit = norm_cdf((inv_ttc - sqrt_rho * z) / sqrt_one_minus_rho);
    Ok(pd_pit)
}

/// Convert a Point-in-Time PD to a Through-the-Cycle PD.
///
/// PD_TtC = Phi( Phi^{-1}(PD_PiT) * sqrt(1 - rho) + sqrt(rho) * z )
///
/// # Errors
///
/// - [`PdCalibrationError::PdOutOfRange`] if `pd_pit` is not in (0, 1).
/// - [`PdCalibrationError::InvalidCorrelation`] if `rho` is not in (0, 1).
pub fn pit_to_ttc(pd_pit: f64, params: &PdCycleParams) -> Result<f64, PdCalibrationError> {
    validate_pd(pd_pit)?;
    validate_correlation(params.asset_correlation)?;
    validate_cycle_index(params.cycle_index)?;

    let rho = params.asset_correlation;
    let z = params.cycle_index;

    let inv_pit = standard_normal_inv_cdf(pd_pit);
    let sqrt_rho = rho.sqrt();
    let sqrt_one_minus_rho = (1.0 - rho).sqrt();

    let pd_ttc = norm_cdf(inv_pit * sqrt_one_minus_rho + sqrt_rho * z);
    Ok(pd_ttc)
}

fn validate_cycle_index(cycle_index: f64) -> Result<(), PdCalibrationError> {
    if !cycle_index.is_finite() {
        return Err(PdCalibrationError::NonFiniteValue { value: cycle_index });
    }
    Ok(())
}

/// Calibrate the central tendency (long-run average PD) from observed
/// default rates over multiple years.
///
/// Computes the arithmetic mean of annual one-year default rates, which is
/// the standard regulatory approach for TtC PD estimation: Basel IRB and
/// EBA GL/2017/16 define the long-run average default rate as the
/// (arithmetic) average of observed one-year default rates.
/// Zero-default years are valid observations and are included in the average.
///
/// # Arguments
///
/// * `annual_default_rates` - Observed default rates per year (each in [0, 1]).
///   Must contain at least one element.
///
/// # Errors
///
/// - [`PdCalibrationError::EmptyInput`] if `annual_default_rates` is empty.
/// - [`PdCalibrationError::ValueOutOfRange`] if any rate is outside [0, 1].
pub fn central_tendency(annual_default_rates: &[f64]) -> Result<f64, PdCalibrationError> {
    if annual_default_rates.is_empty() {
        return Err(PdCalibrationError::EmptyInput);
    }

    for &rate in annual_default_rates {
        if !(0.0..=1.0).contains(&rate) {
            return Err(PdCalibrationError::ValueOutOfRange {
                value: rate,
                min: 0.0,
                max: 1.0,
            });
        }
    }

    let n = annual_default_rates.len() as f64;
    let sum: f64 = annual_default_rates.iter().sum();

    Ok(sum / n)
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn validate_pd(pd: f64) -> Result<(), PdCalibrationError> {
    if pd <= 0.0 || pd >= 1.0 || !pd.is_finite() {
        Err(PdCalibrationError::PdOutOfRange { value: pd })
    } else {
        Ok(())
    }
}

fn validate_correlation(rho: f64) -> Result<(), PdCalibrationError> {
    if rho <= 0.0 || rho >= 1.0 || !rho.is_finite() {
        Err(PdCalibrationError::InvalidCorrelation { value: rho })
    } else {
        Ok(())
    }
}
