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
/// Returns `Error::Validation("cpr must be a decimal in [0,1]; got …")` if
/// CPR is negative, non-finite, or above 100%.
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
    check_unit_interval("cpr", cpr)?;
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
/// Returns `Error::Validation("smm must be a decimal in [0,1]; got …")` for
/// negative, non-finite, or above-`1.0` inputs.
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
    check_unit_interval("smm", smm)?;
    if smm == 0.0 {
        return Ok(0.0);
    }
    Ok(1.0 - (1.0 - smm).powi(12))
}

/// Reject non-finite or out-of-range mortality rates with a self-describing
/// message (`<name> must be a decimal in [0,1]; got <value>`).
fn check_unit_interval(name: &str, value: f64) -> finstack_quant_core::Result<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{name} must be a decimal in [0,1]; got {value}"
        )));
    }
    Ok(())
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

/// Convert an ABS speed to the single-month mortality for a seasoning month.
///
/// The ABS convention (auto-loan and consumer ABS) prepays a constant share
/// `speed` of the *original* balance every month, so the mortality on the
/// remaining balance rises with seasoning:
///
/// `SMM_t = speed / (1 − speed · (t − 1))`, `t ≥ 1`
///
/// Month 0 is treated as month 1. Once the original balance is exhausted
/// (`1 − speed·(t − 1) ≤ speed`) the whole remaining balance prepays and
/// the result is capped at 1.0.
///
/// # Arguments
///
/// * `speed` - Monthly prepayment as a decimal fraction of the original
///   balance (`0.015` = 1.5% ABS), in `[0, 1]`.
/// * `month` - Seasoning month of the prepayment, counted from origination.
///
/// # Returns
///
/// Single-month mortality as a decimal in `[0, 1]`.
///
/// # Errors
///
/// Returns `Error::Validation` if `speed` is non-finite or outside `[0, 1]`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::abs_to_smm;
///
/// assert!((abs_to_smm(0.015, 1)? - 0.015).abs() < 1e-15);
/// // Month 36: 0.015 / (1 − 0.015 × 35) = 3.158%
/// assert!((abs_to_smm(0.015, 36)? - 0.015 / 0.475).abs() < 1e-15);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - Fabozzi, F. J. (ed.), *The Handbook of Fixed Income Securities*,
///   auto-loan ABS prepayment conventions.
pub fn abs_to_smm(speed: f64, month: u32) -> finstack_quant_core::Result<f64> {
    check_unit_interval("abs speed", speed)?;
    if speed == 0.0 {
        return Ok(0.0);
    }
    let elapsed = f64::from(month.max(1) - 1);
    let remaining = 1.0 - speed * elapsed;
    if remaining <= speed * (1.0 + 1e-9) {
        return Ok(1.0);
    }
    Ok((speed / remaining).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn conversion_boundaries_and_invalid_inputs() {
        let conversions = [
            (
                "CPR to SMM",
                cpr_to_smm as fn(f64) -> finstack_quant_core::Result<f64>,
            ),
            (
                "SMM to CPR",
                smm_to_cpr as fn(f64) -> finstack_quant_core::Result<f64>,
            ),
        ];

        for (name, convert) in conversions {
            for (input, expected) in [(0.0, 0.0), (1.0, 1.0)] {
                assert_eq!(
                    convert(input).expect("boundary rate should succeed"),
                    expected,
                    "{name} returned the wrong boundary value"
                );
            }

            for invalid in [-0.01, 1.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(
                    convert(invalid).is_err(),
                    "{name} accepted invalid rate {invalid}"
                );
            }
        }
    }
}
