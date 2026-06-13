//! Netting and collateral logic for XVA exposure calculations.
//!
//! Implements close-out netting under ISDA master agreements and
//! CSA collateral reduction for counterparty credit exposure.
//!
//! # Close-Out Netting
//!
//! Under a valid ISDA master agreement, upon default all transactions
//! are terminated and a single net amount is determined:
//!
//! ```text
//! Net exposure = max(Σᵢ Vᵢ, 0)
//! ```
//!
//! This is significantly less than the sum of individual positive exposures:
//! ```text
//! Gross exposure = Σᵢ max(Vᵢ, 0) ≥ Net exposure
//! ```
//!
//! # References
//!
//! - ISDA (2002). "2002 ISDA Master Agreement." Section 6 (Close-Out Netting).
//! - Gregory, J. (2020). *The xVA Challenge*, Chapter 6.
//! - BCBS 279 (2014). SA-CCR: "The standardised approach for measuring
//!   counterparty credit risk exposures."

use super::types::CsaTerms;
use finstack_core::math::neumaier_sum;

/// Apply close-out netting to a set of instrument mark-to-market values.
///
/// Under a valid ISDA master agreement, the exposure is computed on the
/// net portfolio value rather than summing individual positive exposures.
///
/// # Arguments
///
/// * `instrument_values` - Individual instrument MtM values (positive or negative)
///
/// # Returns
///
/// Net positive exposure: `max(Σᵢ Vᵢ, 0)`.
///
/// # Examples
///
/// ```
/// use finstack_margin::xva::netting::apply_netting;
///
/// // Two offsetting trades: net exposure is reduced
/// let values = [100.0, -80.0];
/// assert!((apply_netting(&values) - 20.0).abs() < 1e-12);
///
/// // All negative: no exposure
/// let values = [-50.0, -30.0];
/// assert!((apply_netting(&values)).abs() < 1e-12);
/// ```
#[inline]
pub fn apply_netting(instrument_values: &[f64]) -> f64 {
    let net = neumaier_sum(instrument_values.iter().copied());
    net.max(0.0)
}

/// Apply CSA collateral terms to reduce gross exposure.
///
/// Models the collateral mechanics of a Credit Support Annex:
///
/// ```text
/// unsecured_exposure = min(exposure, threshold + MTA)
/// net_exposure = max(unsecured_exposure - IA, 0)
/// ```
///
/// The independent amount (IA) is additional collateral posted by the
/// counterparty that further reduces credit exposure beyond the
/// variation margin collateral call.
///
/// # Arguments
///
/// * `gross_exposure` - Portfolio exposure before collateral (non-negative)
/// * `csa` - CSA terms governing collateral exchange
///
/// # Returns
///
/// Net exposure after collateral, always non-negative.
///
/// # Examples
///
/// ```
/// use finstack_margin::xva::netting::apply_collateral;
/// use finstack_margin::xva::types::CsaTerms;
///
/// let csa = CsaTerms {
///     threshold: 10.0,
///     mta: 1.0,
///     mpor_days: 10,
///     independent_amount: 0.0,
/// };
///
/// // Exposure below threshold: no collateral called
/// assert!((apply_collateral(8.0, &csa) - 8.0).abs() < 1e-12);
///
/// // Exposure above threshold + MTA: the residual exposure is threshold + MTA
/// assert!((apply_collateral(20.0, &csa) - 11.0).abs() < 1e-12);
/// ```
#[inline]
pub fn apply_collateral(gross_exposure: f64, csa: &CsaTerms) -> f64 {
    let unsecured_exposure = if gross_exposure > csa.threshold + csa.mta {
        csa.threshold + csa.mta
    } else {
        gross_exposure
    };
    (unsecured_exposure - csa.independent_amount).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Netting tests ──────────────────────────────────────────────

    #[test]
    fn netting_reduces_exposure() {
        // Offsetting trades should reduce net exposure
        let gross: f64 = [100.0_f64, -80.0].iter().filter(|v| **v > 0.0).sum::<f64>();
        let net = apply_netting(&[100.0, -80.0]);
        assert!(
            net < gross,
            "Netting should reduce exposure: net={net}, gross={gross}"
        );
        assert!((net - 20.0).abs() < 1e-12);
    }

    #[test]
    fn netting_all_positive() {
        // All positive values: net equals sum
        let values = [10.0, 20.0, 30.0];
        assert!((apply_netting(&values) - 60.0).abs() < 1e-12);
    }

    #[test]
    fn netting_all_negative_gives_zero() {
        // All negative: no exposure
        let values = [-10.0, -20.0, -30.0];
        assert!(apply_netting(&values).abs() < 1e-12);
    }

    #[test]
    fn netting_empty_gives_zero() {
        assert!(apply_netting(&[]).abs() < 1e-12);
    }

    #[test]
    fn netting_single_positive() {
        assert!((apply_netting(&[42.0]) - 42.0).abs() < 1e-12);
    }

    #[test]
    fn netting_single_negative_gives_zero() {
        assert!(apply_netting(&[-42.0]).abs() < 1e-12);
    }

    #[test]
    fn netting_mixed_magnitude_cancellation_preserves_small_residual() {
        let values = [1e16_f64, 1.0, -1e16];
        assert!((apply_netting(&values) - 1.0).abs() < 1e-10);
    }

    // ── Collateral tests ───────────────────────────────────────────

    fn make_csa(threshold: f64, mta: f64, ia: f64) -> CsaTerms {
        CsaTerms {
            threshold,
            mta,
            mpor_days: 10,
            independent_amount: ia,
        }
    }

    #[test]
    fn collateral_below_threshold_unchanged() {
        let csa = make_csa(10.0, 1.0, 0.0);
        assert!((apply_collateral(8.0, &csa) - 8.0).abs() < 1e-12);
    }

    #[test]
    fn collateral_between_threshold_and_mta() {
        // Over threshold by 0.5, but below MTA (1.0) → no collateral called
        let csa = make_csa(10.0, 1.0, 0.0);
        assert!((apply_collateral(10.5, &csa) - 10.5).abs() < 1e-12);
    }

    #[test]
    fn collateral_above_threshold_plus_mta() {
        // Exposure = 20, threshold = 10, MTA = 1
        // residual unsecured exposure = threshold + MTA = 11
        let csa = make_csa(10.0, 1.0, 0.0);
        assert!((apply_collateral(20.0, &csa) - 11.0).abs() < 1e-12);
    }

    #[test]
    fn collateral_with_independent_amount() {
        // IA reduces the net exposure (additional collateral posted by counterparty)
        let csa = make_csa(10.0, 1.0, 5.0);
        // Residual exposure = threshold + MTA = 11; net = max(11 - 5, 0) = 6.
        assert!((apply_collateral(20.0, &csa) - 6.0).abs() < 1e-12);
    }

    #[test]
    fn collateral_zero_threshold() {
        // Zero threshold CSA (bilateral VM): MTA remains as unsecured residual.
        let csa = make_csa(0.0, 0.5, 0.0);
        assert!((apply_collateral(100.0, &csa) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn collateral_zero_exposure() {
        let csa = make_csa(10.0, 1.0, 0.0);
        assert!(apply_collateral(0.0, &csa).abs() < 1e-12);
    }

    #[test]
    fn collateral_never_negative() {
        // Even with large IA on zero exposure, result is floored at zero
        let csa = make_csa(0.0, 0.0, 100.0);
        let result = apply_collateral(0.0, &csa);
        assert!(
            result.abs() < 1e-12,
            "Collateralized exposure should be zero when IA exceeds exposure, got {result}"
        );
    }

    #[test]
    fn collateral_ia_reduces_to_zero() {
        // Large IA should reduce exposure to zero (floored)
        let csa = make_csa(0.0, 0.0, 1000.0);
        let result = apply_collateral(50.0, &csa);
        assert!(
            result.abs() < 1e-12,
            "Large IA should reduce exposure to zero, got {result}"
        );
    }
}
