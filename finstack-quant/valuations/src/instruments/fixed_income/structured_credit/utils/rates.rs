//! Rate conversion utilities for structured credit instruments.
//!
//! Provides standard conversions between different rate conventions:
//! - CPR (Constant Prepayment Rate) ↔ SMM (Single Monthly Mortality)
//! - CDR (Constant Default Rate) ↔ MDR (Monthly Default Rate)
//! - PSA to CPR conversions
//!
//! # Mathematical Foundations
//!
//! ## CPR ↔ SMM
//!
//! CPR is an annualized prepayment rate, while SMM is the monthly equivalent.
//!
//! ```text
//! SMM = 1 - (1 - CPR)^(1/12)
//! CPR = 1 - (1 - SMM)^12
//! ```
//!
//! ## CDR ↔ MDR
//!
//! Similarly, CDR is annualized and MDR is monthly:
//!
//! ```text
//! MDR = 1 - (1 - CDR)^(1/12)
//! CDR = 1 - (1 - MDR)^12
//! ```
//!
//! ## PSA Model
//!
//! The PSA (Public Securities Association) prepayment model defines a standard
//! prepayment curve for residential mortgages:
//!
//! - Months 1-30: CPR ramps linearly from 0% to 6%
//! - Month 30+: CPR stays at 6%
//!
//! PSA speeds are multiples of this curve (e.g., 150% PSA = 1.5x the standard curve).

use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry_or_panic;
use finstack_quant_cashflows::builder::{cdr_to_mdr, cpr_to_smm, mdr_to_cdr, smm_to_cpr};

fn convert_clamped(rate: f64, convert: fn(f64) -> finstack_quant_core::Result<f64>) -> f64 {
    convert(rate.clamp(0.0, 1.0)).unwrap_or(f64::NAN)
}

/// Converts annual CPR to monthly SMM.
///
/// # Arguments
///
/// * `cpr` - Annual constant prepayment rate (as decimal, e.g., 0.06 for 6%)
///
/// # Returns
///
/// Monthly single mortality rate (as decimal). Input is clamped to [0, 1].
///
/// # Formula
///
/// ```text
/// SMM = 1 - (1 - CPR)^(1/12)
/// ```
#[inline]
pub fn clamped_cpr_to_smm(cpr: f64) -> f64 {
    convert_clamped(cpr, cpr_to_smm)
}

/// Converts monthly SMM to annual CPR.
///
/// # Arguments
///
/// * `smm` - Monthly single mortality rate (as decimal, e.g., 0.005 for 0.5%)
///
/// # Returns
///
/// Annual constant prepayment rate (as decimal). Input is clamped to [0, 1].
///
/// # Formula
///
/// ```text
/// CPR = 1 - (1 - SMM)^12
/// ```
#[inline]
pub fn clamped_smm_to_cpr(smm: f64) -> f64 {
    convert_clamped(smm, smm_to_cpr)
}

/// Converts annual CDR to monthly MDR.
///
/// # Arguments
///
/// * `cdr` - Annual constant default rate (as decimal, e.g., 0.02 for 2%)
///
/// # Returns
///
/// Monthly default rate (as decimal). Input is clamped to [0, 1].
///
/// # Formula
///
/// ```text
/// MDR = 1 - (1 - CDR)^(1/12)
/// ```
#[inline]
pub fn clamped_cdr_to_mdr(cdr: f64) -> f64 {
    convert_clamped(cdr, cdr_to_mdr)
}

/// Converts monthly MDR to annual CDR.
///
/// # Arguments
///
/// * `mdr` - Monthly default rate (as decimal, e.g., 0.002 for 0.2%)
///
/// # Returns
///
/// Annual constant default rate (as decimal). Input is clamped to [0, 1].
///
/// # Formula
///
/// ```text
/// CDR = 1 - (1 - MDR)^12
/// ```
#[inline]
pub fn clamped_mdr_to_cdr(mdr: f64) -> f64 {
    convert_clamped(mdr, mdr_to_cdr)
}

/// Converts PSA speed to CPR at a given month.
///
/// # Arguments
///
/// * `psa_speed` - PSA speed multiplier (e.g., 1.0 for 100% PSA, 1.5 for 150% PSA).
///   Negative values are clamped to 0.
/// * `month` - Month number (1-indexed, i.e., month 1 is the first month)
///
/// # Returns
///
/// Annual CPR at the given month (as decimal), clamped to [0, 1].
///
/// # PSA Model
///
/// The standard PSA model (100% PSA):
/// - Month 1: CPR = 0.2%
/// - Month 2: CPR = 0.4%
/// - ...
/// - Month 30: CPR = 6.0%
/// - Month 31+: CPR = 6.0%
///
/// PSA speeds scale this curve linearly. For example, 150% PSA at month 30 = 9% CPR.
pub fn psa_to_cpr(psa_speed: f64, month: u32) -> f64 {
    let psa_speed = psa_speed.max(0.0);
    if month == 0 || psa_speed == 0.0 {
        return 0.0;
    }

    let psa_curve = embedded_registry_or_panic().psa_curve();
    let base_cpr = if month <= psa_curve.ramp_months {
        (month as f64 / psa_curve.ramp_months as f64) * psa_curve.terminal_cpr
    } else {
        psa_curve.terminal_cpr
    };

    (psa_speed * base_cpr).min(1.0)
}

/// Calculate periods per year from a payment frequency.
///
/// # Arguments
///
/// * `freq` - Payment frequency specification
///
/// # Returns
///
/// Number of payment periods per year. Returns 4.0 (quarterly) as fallback
/// for unusual frequency specifications.
///
/// # Examples
///
/// ```text
/// use finstack_quant_core::dates::Tenor;
/// use finstack_quant_valuations::instruments::fixed_income::structured_credit::utils::rates::frequency_periods_per_year;
///
/// // Monthly = 12 periods/year
/// assert_eq!(frequency_periods_per_year(Tenor::monthly()), 12.0);
///
/// // Quarterly = 4 periods/year
/// assert_eq!(frequency_periods_per_year(Tenor::quarterly()), 4.0);
///
/// // Semi-annual = 2 periods/year
/// assert_eq!(frequency_periods_per_year(Tenor::semi_annual()), 2.0);
/// ```
#[inline]
pub(crate) fn frequency_periods_per_year(freq: finstack_quant_core::dates::Tenor) -> f64 {
    use finstack_quant_core::dates::TenorUnit;
    match freq.unit() {
        TenorUnit::Months => {
            if freq.count() > 0 {
                12.0 / freq.count() as f64
            } else {
                4.0
            }
        }
        TenorUnit::Days => {
            if freq.count() > 0 {
                365.0 / freq.count() as f64
            } else {
                4.0
            }
        }

        TenorUnit::Weeks => {
            if freq.count() > 0 {
                52.0 / freq.count() as f64
            } else {
                4.0
            }
        }
        TenorUnit::Years => {
            if freq.count() > 0 {
                1.0 / freq.count() as f64
            } else {
                4.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpr_smm_roundtrip() {
        let test_cprs = vec![0.0, 0.01, 0.05, 0.10, 0.15, 0.20, 0.30];
        for cpr in test_cprs {
            let smm = clamped_cpr_to_smm(cpr);
            let cpr_back = clamped_smm_to_cpr(smm);
            assert!(
                (cpr - cpr_back).abs() < 1e-10,
                "Roundtrip failed for CPR={}: got {}",
                cpr,
                cpr_back
            );
        }
    }

    #[test]
    fn test_cdr_mdr_roundtrip() {
        let test_cdrs = vec![0.0, 0.01, 0.02, 0.05, 0.10];
        for cdr in test_cdrs {
            let mdr = clamped_cdr_to_mdr(cdr);
            let cdr_back = clamped_mdr_to_cdr(mdr);
            assert!(
                (cdr - cdr_back).abs() < 1e-10,
                "Roundtrip failed for CDR={}: got {}",
                cdr,
                cdr_back
            );
        }
    }

    #[test]
    fn test_psa_curve() {
        assert!((psa_to_cpr(1.0, 1) - 0.002).abs() < 1e-10);
        assert!((psa_to_cpr(1.0, 15) - 0.03).abs() < 1e-10);
        assert!((psa_to_cpr(1.0, 30) - 0.06).abs() < 1e-10);
        assert!((psa_to_cpr(1.0, 100) - 0.06).abs() < 1e-10);
        assert!((psa_to_cpr(1.5, 30) - 0.09).abs() < 1e-10);
    }

    #[test]
    fn test_boundary_clamping() {
        assert_eq!(clamped_cpr_to_smm(-0.05), 0.0);
        assert_eq!(clamped_cpr_to_smm(1.5), 1.0);
        assert_eq!(clamped_cdr_to_mdr(-0.02), 0.0);
        assert_eq!(clamped_cdr_to_mdr(1.5), 1.0);
        assert_eq!(psa_to_cpr(-1.0, 15), 0.0);
        assert_eq!(psa_to_cpr(17.0, 30), 1.0);
    }

    #[test]
    fn test_frequency_periods_per_year() {
        use finstack_quant_core::dates::Tenor;

        // Monthly = 12 periods/year
        assert_eq!(frequency_periods_per_year(Tenor::monthly()), 12.0);

        // Quarterly = 4 periods/year
        assert_eq!(frequency_periods_per_year(Tenor::quarterly()), 4.0);

        // Semi-annual = 2 periods/year
        assert_eq!(frequency_periods_per_year(Tenor::semi_annual()), 2.0);

        // Annual = 1 period/year
        assert_eq!(frequency_periods_per_year(Tenor::annual()), 1.0);

        // Bi-monthly (every 2 months) = 6 periods/year
        assert_eq!(
            frequency_periods_per_year(finstack_quant_core::dates::Tenor::new(
                2,
                finstack_quant_core::dates::TenorUnit::Months
            )),
            6.0
        );

        // Daily (252 business days common) -> 365/1 = 365
        assert_eq!(
            frequency_periods_per_year(finstack_quant_core::dates::Tenor::new(
                1,
                finstack_quant_core::dates::TenorUnit::Days
            )),
            365.0
        );

        // Weekly (every 7 days) -> 365/7 ≈ 52.14
        assert!(
            (frequency_periods_per_year(finstack_quant_core::dates::Tenor::new(
                7,
                finstack_quant_core::dates::TenorUnit::Days
            )) - 52.142857)
                .abs()
                < 0.001
        );
    }
}
