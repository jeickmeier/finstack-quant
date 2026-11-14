//! Credit rate conversions.
//!
//! Utilities for converting between monthly and annual rate conventions.
//! These conversions apply to both prepayment rates (CPR/SMM) and
//! default rates (CDR/MDR), as they use identical mathematical formulas.

/// Convert an annual conditional prepayment or default rate to its monthly rate.
///
/// Uses `monthly = 1 - (1 - annual)^(1/12)` for both CPR/SMM and CDR/MDR.
///
/// # Arguments
///
/// * `annual` - Finite annual conditional mortality as a decimal in `[0, 1]`;
///   `0.06` represents 6% CPR or CDR. No calendar day-count adjustment is applied.
///
/// # Returns
///
/// Monthly conditional mortality as a decimal. Zero and one map to themselves.
///
/// # Errors
///
/// Returns `Error::Validation` if `annual` is non-finite or outside `[0, 1]`.
///
/// # Examples
///
/// ```
/// use finstack_quant_cashflows::builder::annual_to_monthly;
/// assert!((annual_to_monthly(0.06)? - 0.0051430128).abs() < 1e-10);
/// assert_eq!(annual_to_monthly(1.0)?, 1.0);
/// assert!(annual_to_monthly(-0.05).is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn annual_to_monthly(annual: f64) -> finstack_quant_core::Result<f64> {
    check_unit_interval("annual", annual)?;
    if annual == 0.0 {
        return Ok(0.0);
    }
    // Retain precision for small mortality rates and finite-difference risks.
    Ok(-((1.0 - annual).ln() / 12.0).exp_m1())
}

/// Length of the PSA benchmark seasoning ramp, in months.
pub const PSA_RAMP_MONTHS: u32 = 30;

/// Terminal annual CPR of the 100% PSA benchmark (6%).
pub const PSA_TERMINAL_CPR: f64 = 0.06;

/// Peak annual CDR of the 100% SDA benchmark (0.60%), reached at month 30.
///
/// [`DefaultModelSpec::Sda`](crate::builder::DefaultModelSpec::Sda) scales
/// this curve by its `speed_multiplier`.
pub const SDA_PEAK_CDR: f64 = 0.006;

/// Annual CPR of the PSA benchmark curve at a loan age, scaled by speed.
///
/// 100% PSA ramps linearly from 0.2% CPR at month 1 to 6% at month 30 and
/// stays flat afterwards: `CPR = min(1.0, speed × 6% × min(age, 30) / 30)`.
///
/// # Arguments
///
/// * `speed_multiplier` - Non-negative, finite PSA speed as a multiple of the
///   benchmark (`1.0` = 100% PSA, `1.5` = 150% PSA).
/// * `seasoning_months` - Loan age in months (`>= 1`).
///
/// # Returns
///
/// Annual CPR as a decimal in `[0, 1]`, capped at `1.0` for high PSA speeds.
///
/// # Errors
///
/// Returns `Error::Validation` if `speed_multiplier` is non-finite or
/// negative, or if `seasoning_months == 0`.
///
/// # Examples
///
/// ```
/// use finstack_quant_cashflows::builder::psa_cpr;
///
/// assert!((psa_cpr(1.0, 15)? - 0.03).abs() < 1e-15);
/// assert!((psa_cpr(1.5, 60)? - 0.09).abs() < 1e-15);
/// assert_eq!(psa_cpr(20.0, 30)?, 1.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - `docs/REFERENCES.md#tuckman-serrat-fixed-income`
pub fn psa_cpr(speed_multiplier: f64, seasoning_months: u32) -> finstack_quant_core::Result<f64> {
    if !speed_multiplier.is_finite() || speed_multiplier < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "PSA speed_multiplier ({speed_multiplier}) must be finite and non-negative"
        )));
    }
    if seasoning_months == 0 {
        return Err(finstack_quant_core::Error::Validation(
            "seasoning_months must be >= 1".to_string(),
        ));
    }
    let base = if seasoning_months <= PSA_RAMP_MONTHS {
        (f64::from(seasoning_months) / f64::from(PSA_RAMP_MONTHS)) * PSA_TERMINAL_CPR
    } else {
        PSA_TERMINAL_CPR
    };
    Ok((base * speed_multiplier).min(1.0))
}

/// Convert a monthly conditional prepayment or default rate to its annual rate.
///
/// Uses `annual = 1 - (1 - monthly)^12` for both SMM/CPR and MDR/CDR.
///
/// # Arguments
///
/// * `monthly` - Finite monthly conditional mortality as a decimal in `[0, 1]`;
///   `0.01` represents 1% SMM or MDR. Annualization uses twelve monthly periods.
///
/// # Returns
///
/// Annual conditional mortality as a decimal. Zero and one map to themselves.
///
/// # Errors
///
/// Returns `Error::Validation` if `monthly` is non-finite or outside `[0, 1]`.
///
/// # Examples
///
/// ```
/// use finstack_quant_cashflows::builder::{annual_to_monthly, monthly_to_annual};
/// let monthly = annual_to_monthly(0.06)?;
/// assert!((monthly_to_annual(monthly)? - 0.06).abs() < 1e-14);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn monthly_to_annual(monthly: f64) -> finstack_quant_core::Result<f64> {
    check_unit_interval("monthly", monthly)?;
    if monthly == 0.0 {
        return Ok(0.0);
    }
    // Keep precision for small mortality rates with expm1/ln_1p.
    Ok(-(12.0 * (-monthly).ln_1p()).exp_m1())
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

/// Convert an ABS speed to the single-month mortality for a seasoning month.
///
/// The ABS convention (auto-loan and consumer ABS) prepays a constant share
/// `speed` of the *original* balance every month, so the mortality on the
/// remaining balance rises with seasoning:
///
/// `SMM_t = speed / (1 − speed · (t − 1))`, `t ≥ 1`
///
/// Once the original balance is exhausted (`1 − speed·(t − 1) ≤ speed`) the
/// whole remaining balance prepays and the result is capped at 1.0.
///
/// # Arguments
///
/// * `speed` - Monthly prepayment as a decimal fraction of the original
///   balance (`0.015` = 1.5% ABS), in `[0, 1]`.
/// * `month` - Seasoning month of the prepayment, counted from origination (`>= 1`).
///
/// # Returns
///
/// Single-month mortality as a decimal in `[0, 1]`.
///
/// # Errors
///
/// Returns `Error::Validation` if `speed` is non-finite or outside `[0, 1]`, or
/// if `month == 0`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::abs_to_smm;
///
/// assert!((abs_to_smm(0.015, 1)? - 0.015).abs() < 1e-15);
/// // Month 36: 0.015 / (1 − 0.015 × 35) = 3.158%
/// assert!((abs_to_smm(0.015, 36)? - 0.015 / 0.475).abs() < 1e-15);
/// assert!(abs_to_smm(0.015, 0).is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - Fabozzi, F. J. (ed.), *The Handbook of Fixed Income Securities*,
///   auto-loan ABS prepayment conventions.
pub fn abs_to_smm(speed: f64, month: u32) -> finstack_quant_core::Result<f64> {
    check_unit_interval("abs speed", speed)?;
    if month == 0 {
        return Err(finstack_quant_core::Error::Validation(
            "month must be >= 1".to_string(),
        ));
    }
    if speed == 0.0 {
        return Ok(0.0);
    }
    let elapsed = f64::from(month - 1);
    let remaining = 1.0 - speed * elapsed;
    if remaining <= speed * (1.0 + 1e-9) {
        return Ok(1.0);
    }
    Ok((speed / remaining).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 100% PSA: 0.2% CPR per month of age up to 6% at month 30, flat after;
    /// speeds scale linearly and cap at 1.0 above 16.67x.
    #[test]
    fn psa_cpr_matches_the_benchmark_ramp() {
        assert!(psa_cpr(1.0, 0).is_err());
        assert!(psa_cpr(-0.1, 1).is_err());
        assert!(psa_cpr(f64::NAN, 1).is_err());
        for month in 1..=30_u32 {
            assert!(
                (psa_cpr(1.0, month).expect("valid month") - 0.002 * f64::from(month)).abs()
                    < 1e-15
            );
        }
        assert_eq!(psa_cpr(1.0, 31).expect("valid"), 0.06);
        assert_eq!(psa_cpr(1.0, 360).expect("valid"), 0.06);
        assert!((psa_cpr(1.5, 20).expect("valid") - 0.06).abs() < 1e-15);
        assert_eq!(psa_cpr(0.0, 45).expect("valid"), 0.0);
        assert_eq!(psa_cpr(20.0, 30).expect("high PSA caps at 1.0"), 1.0);
    }

    #[test]
    fn rate_errors_name_the_rate_the_caller_passed() {
        for (result, name) in [
            (annual_to_monthly(1.2), "annual"),
            (monthly_to_annual(-0.1), "monthly"),
            (annual_to_monthly(f64::INFINITY), "annual"),
            (monthly_to_annual(f64::NAN), "monthly"),
        ] {
            let message = result.expect_err("out-of-range rate").to_string();
            assert!(
                message.contains(&format!("{name} must be a decimal in [0,1]")),
                "{name}: {message}"
            );
        }
    }

    #[test]
    fn annual_to_monthly_uses_stable_kernel() {
        let annual = 0.02;
        let expected = -((1.0_f64 - annual).ln() / 12.0).exp_m1();
        assert_eq!(
            annual_to_monthly(annual).expect("valid annual rate"),
            expected
        );
    }

    #[test]
    fn conversion_boundaries_and_invalid_inputs() {
        let conversions = [
            (
                "annual to monthly",
                annual_to_monthly as fn(f64) -> finstack_quant_core::Result<f64>,
            ),
            (
                "monthly to annual",
                monthly_to_annual as fn(f64) -> finstack_quant_core::Result<f64>,
            ),
        ];

        for (name, convert) in conversions {
            for (input, expected) in [(-0.0, 0.0_f64), (0.0, 0.0), (1.0, 1.0)] {
                assert_eq!(
                    convert(input)
                        .expect("boundary rate should succeed")
                        .to_bits(),
                    expected.to_bits(),
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
