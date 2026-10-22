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
/// stays flat afterwards: `CPR = speed × 6% × min(age, 30) / 30`.
///
/// # Arguments
///
/// * `speed_multiplier` - PSA speed as a multiple of the benchmark (`1.0` =
///   100% PSA, `1.5` = 150% PSA). Not validated here; callers that accept
///   user input check it is finite and non-negative.
/// * `seasoning_months` - Loan age in months; month 0 gives 0% CPR.
///
/// # Returns
///
/// Annual CPR as a decimal. It exceeds 1.0 for speeds above ~16.7× at the
/// plateau; callers decide whether to reject or clamp.
///
/// # Examples
///
/// ```
/// use finstack_quant_cashflows::builder::psa_cpr;
///
/// assert!((psa_cpr(1.0, 15) - 0.03).abs() < 1e-15);
/// assert!((psa_cpr(1.5, 60) - 0.09).abs() < 1e-15);
/// ```
///
/// # References
///
/// - `docs/REFERENCES.md#tuckman-serrat-fixed-income`
#[must_use]
pub fn psa_cpr(speed_multiplier: f64, seasoning_months: u32) -> f64 {
    let base = if seasoning_months <= PSA_RAMP_MONTHS {
        (seasoning_months as f64 / PSA_RAMP_MONTHS as f64) * PSA_TERMINAL_CPR
    } else {
        PSA_TERMINAL_CPR
    };
    base * speed_multiplier
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

    /// 100% PSA: 0.2% CPR per month of age up to 6% at month 30, flat after;
    /// speeds scale linearly (150% PSA at month 20 = 1.5 × 4% = 6%).
    #[test]
    fn psa_cpr_matches_the_benchmark_ramp() {
        for month in 0..=30_u32 {
            assert!((psa_cpr(1.0, month) - 0.002 * f64::from(month)).abs() < 1e-15);
        }
        assert_eq!(psa_cpr(1.0, 31), 0.06);
        assert_eq!(psa_cpr(1.0, 360), 0.06);
        assert!((psa_cpr(1.5, 20) - 0.06).abs() < 1e-15);
        assert_eq!(psa_cpr(0.0, 45), 0.0);
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

/// Convert annual conditional mortality to monthly mortality after clamping.
///
/// Projection loops can use this for shocked or interpolated CPR/CDR values.
///
/// # Arguments
///
/// * `annual` - Annual conditional prepayment or default rate as a decimal.
///   Values below zero (including negative infinity) clamp to zero; values above
///   one (including positive infinity) clamp to one. NaN remains NaN.
///
/// # Returns
///
/// Monthly mortality in `[0, 1]`, or NaN when the input is NaN.
#[inline]
#[must_use]
pub fn annual_to_monthly_clamped(annual: f64) -> f64 {
    annual_to_monthly(annual.clamp(0.0, 1.0)).unwrap_or(f64::NAN)
}

#[cfg(test)]
mod clamped_tests {
    use super::*;

    #[test]
    fn clamped_conversions_stay_on_the_unit_branch() {
        assert_eq!(annual_to_monthly_clamped(0.0), 0.0);
        assert_eq!(annual_to_monthly_clamped(-0.0).to_bits(), 0.0_f64.to_bits());
        assert_eq!(annual_to_monthly_clamped(f64::NEG_INFINITY), 0.0);
        assert_eq!(annual_to_monthly_clamped(f64::INFINITY), 1.0);
        assert!(annual_to_monthly_clamped(f64::NAN).is_nan());
        assert!((annual_to_monthly_clamped(1.5) - 1.0).abs() < 1e-12);
        assert!(annual_to_monthly_clamped(-0.2) == 0.0);
        assert!(
            (annual_to_monthly_clamped(0.02) - annual_to_monthly(0.02).expect("valid CDR")).abs()
                < 1e-15
        );
    }
}
