//! Credit rate conversions.
//!
//! Utilities for converting between monthly and annual rate conventions.
//! These conversions apply to both prepayment rates (CPR/SMM) and
//! default rates (CDR/MDR), as they use identical mathematical formulas.

/// Convert annual CPR (constant prepayment rate) to monthly SMM (single monthly mortality).
///
/// Uses the standard relationship (per Fabozzi's MBS handbook):
/// `SMM = 1 - (1 - CPR)^(1/12)`.
///
/// # Arguments
///
/// * `cpr` - Annualized CPR or CDR as a decimal, for example `0.06` for 6%.
///
/// # Returns
///
/// Monthly SMM or MDR as a decimal.
///
/// # Edge Cases
///
/// - CPR = 0: Returns 0.0 (no prepayment)
/// - CPR = 100%: Returns 100% SMM.
///
/// # Errors
///
/// Returns `InputError::NegativeValue` if CPR is negative and
/// `InputError::Invalid` if CPR is non-finite or above 100%.
///
/// # Examples
///
/// ```
/// use finstack_quant_cashflows::builder::cpr_to_smm;
///
/// // Convert 6% CPR to SMM
/// let cpr = 0.06;
/// let smm = cpr_to_smm(cpr).unwrap();
/// assert!((smm - 0.005143).abs() < 0.0001); // Approximately 0.5143% monthly
///
/// // Edge case: 100% CPR maps to 100% SMM
/// let smm_100 = cpr_to_smm(1.0).unwrap();
/// assert_eq!(smm_100, 1.0);
///
/// // Negative CPR is rejected
/// assert!(cpr_to_smm(-0.05).is_err());
/// ```
pub fn cpr_to_smm(cpr: f64) -> finstack_quant_core::Result<f64> {
    if !cpr.is_finite() {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::Invalid,
        ));
    }
    if cpr < 0.0 {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::NegativeValue,
        ));
    }
    if cpr > 1.0 {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::Invalid,
        ));
    }
    if cpr == 0.0 {
        return Ok(0.0);
    }
    // `expm1` retains precision for small mortality rates. This matters for
    // finite-difference risk measures, where rounding in two nearby PVs can be
    // amplified by the final subtraction.
    Ok(-((1.0 - cpr).ln() / 12.0).exp_m1())
}

/// Convert monthly SMM to annual CPR.
///
/// # Formula
///
/// `annual = 1 - (1 - monthly)^12`
///
/// # Arguments
///
/// * `smm` - Monthly SMM or MDR as a decimal in `[0, 1]`.
///
/// # Returns
///
/// Annualized CPR or CDR as a decimal.
///
/// # Errors
///
/// Returns `InputError::NegativeValue` for negative inputs and
/// `InputError::Invalid` for values above `1.0`.
///
/// # Examples
///
/// ```
/// use finstack_quant_cashflows::builder::{cpr_to_smm, smm_to_cpr};
///
/// // Roundtrip conversion
/// let cpr = 0.06;
/// let smm = cpr_to_smm(cpr).unwrap();
/// let cpr_back = smm_to_cpr(smm).unwrap();
/// assert!((cpr - cpr_back).abs() < 1e-10);
/// ```
pub fn smm_to_cpr(smm: f64) -> finstack_quant_core::Result<f64> {
    if !smm.is_finite() {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::Invalid,
        ));
    }
    if smm < 0.0 {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::NegativeValue,
        ));
    }
    if smm > 1.0 {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::Invalid,
        ));
    }
    if smm == 0.0 {
        return Ok(0.0);
    }
    Ok(1.0 - (1.0 - smm).powi(12))
}

/// Convert annual CDR (constant default rate) to monthly MDR.
///
/// Default and prepayment mortality rates use the same annual-to-monthly
/// conversion. This named entry point keeps domain terminology explicit while
/// delegating to the single checked kernel.
///
/// Computes `MDR = 1 - (1 - CDR)^(1/12)`, with rates represented as decimal
/// probabilities (`0.06` means 6% annually).
///
/// # Arguments
///
/// * `cdr` - Constant annual default rate as a finite decimal probability in
///   `[0, 1]`, where `0.06` represents 6% per year.
///
/// # Errors
///
/// Returns an error if `cdr` is non-finite or outside `[0, 1]`.
pub fn cdr_to_mdr(cdr: f64) -> finstack_quant_core::Result<f64> {
    cpr_to_smm(cdr)
}

/// Convert monthly MDR to annual CDR.
///
/// Default and prepayment mortality rates use the same monthly-to-annual
/// conversion. This named entry point keeps domain terminology explicit while
/// delegating to the single checked kernel.
///
/// Computes `CDR = 1 - (1 - MDR)^12`, with rates represented as decimal
/// probabilities (`0.01` means 1% monthly).
///
/// # Arguments
///
/// * `mdr` - Monthly default rate as a finite decimal probability in `[0, 1]`,
///   where `0.01` represents 1% for one month.
///
/// # Errors
///
/// Returns an error if `mdr` is non-finite or outside `[0, 1]`.
pub fn mdr_to_cdr(mdr: f64) -> finstack_quant_core::Result<f64> {
    smm_to_cpr(mdr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annual_to_monthly_conversion() {
        let annual = 0.06; // 6% annual
        let monthly = cpr_to_smm(annual).expect("valid CPR");

        // 6% annual should be approximately 0.5143% monthly
        assert!((monthly - 0.005143).abs() < 0.0001);
        assert!(monthly > 0.0);
        assert!(monthly < annual); // Monthly should be less than annual
    }

    #[test]
    fn test_monthly_to_annual_conversion() {
        let monthly = 0.01; // 1% monthly
        let annual = smm_to_cpr(monthly).expect("valid SMM");

        // Should be positive and greater than monthly
        assert!(annual > monthly);
        assert!(annual < 1.0);
    }

    #[test]
    fn test_roundtrip_conversion() {
        let original = 0.06;
        let monthly = cpr_to_smm(original).expect("valid CPR");
        let back = smm_to_cpr(monthly).expect("valid SMM");

        // Should roundtrip with high precision
        assert!((original - back).abs() < 1e-10);
    }

    #[test]
    fn default_rate_names_share_the_checked_kernel() {
        let annual = 0.02;
        let monthly = cdr_to_mdr(annual).expect("valid CDR");
        assert_eq!(monthly, cpr_to_smm(annual).expect("valid annual rate"));
        assert_eq!(
            mdr_to_cdr(monthly).expect("valid MDR"),
            smm_to_cpr(monthly).expect("valid monthly rate")
        );
    }

    #[test]
    fn annual_to_monthly_uses_stable_kernel() {
        let annual = 0.02;
        let expected = -((1.0_f64 - annual).ln() / 12.0).exp_m1();
        assert_eq!(cpr_to_smm(annual).expect("valid annual rate"), expected);
    }

    #[test]
    fn test_zero_rate() {
        assert_eq!(cpr_to_smm(0.0).expect("zero CPR should succeed"), 0.0);
        assert_eq!(smm_to_cpr(0.0).expect("zero SMM should succeed"), 0.0);
    }

    #[test]
    fn test_consistency_across_rates() {
        // Test that prepayment (CPR) and default (CDR) use the same formula
        let rate = 0.05;
        let monthly_prepay = cpr_to_smm(rate).expect("valid CPR");
        let monthly_default = cpr_to_smm(rate).expect("valid CPR");

        // Should be identical
        assert!((monthly_prepay - monthly_default).abs() < 1e-15);
    }

    #[test]
    fn test_cpr_smm_roundtrip_via_new_names() {
        let cpr = 0.06;
        let smm = cpr_to_smm(cpr).expect("valid CPR");
        let cpr_back = smm_to_cpr(smm).expect("valid SMM");
        assert!((cpr - cpr_back).abs() < 1e-10);
    }

    #[test]
    fn test_smm_to_cpr_rejects_invalid_inputs() {
        assert!(
            smm_to_cpr(-0.01).is_err(),
            "negative SMM should be rejected"
        );
        assert!(
            smm_to_cpr(1.01).is_err(),
            "SMM above 100% should be rejected"
        );
    }

    #[test]
    fn test_cpr_100_percent_maps_to_100_percent_smm() {
        let smm = cpr_to_smm(1.0).expect("100% CPR should succeed");
        assert_eq!(smm, 1.0);
    }

    #[test]
    fn test_cpr_above_100_percent_rejected() {
        assert!(cpr_to_smm(1.5).is_err());
    }

    #[test]
    fn test_non_finite_rates_rejected() {
        assert!(cpr_to_smm(f64::NAN).is_err());
        assert!(cpr_to_smm(f64::INFINITY).is_err());
        assert!(smm_to_cpr(f64::NAN).is_err());
        assert!(smm_to_cpr(f64::INFINITY).is_err());
    }

    #[test]
    fn test_cpr_to_smm_rejects_negative() {
        let result = cpr_to_smm(-0.05);
        assert!(result.is_err(), "Negative CPR should return error");
    }

    #[test]
    fn test_cpr_to_smm_positive() {
        let result = cpr_to_smm(0.06).expect("positive CPR should succeed");
        // 6% CPR -> SMM ~ 0.005143
        assert!((result - 0.005143).abs() < 0.0001);
    }
}
