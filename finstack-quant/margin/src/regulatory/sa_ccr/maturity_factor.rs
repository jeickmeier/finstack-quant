//! Maturity factor computation for SA-CCR.
//!
//! The maturity factor adjusts the effective notional for the time
//! horizon over which exposure can build.

/// Compute the maturity factor for an unmargined trade.
///
/// `MF_i = sqrt(min(M_i, 1 year) / 1 year)`
/// where `M_i = max(10 business days / 250, remaining maturity)`.
///
/// # Arguments
///
/// * `remaining_maturity_years` - Remaining maturity in years.
///
/// # Returns
///
/// Unmargined SA-CCR maturity factor.
///
/// # References
///
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
#[must_use]
pub fn maturity_factor_unmargined(remaining_maturity_years: f64) -> f64 {
    let m = f64::max(remaining_maturity_years, 10.0 / 250.0); // 10 business days floor
    f64::min(m, 1.0).sqrt()
}

/// Compute the maturity factor for a margined netting set.
///
/// `MF = 3/2 * sqrt(MPOR / 250)`
/// MPOR = margin period of risk in business days (floor: 10 bilateral, 5 cleared).
///
/// # Arguments
///
/// * `mpor_days` - Margin period of risk in business days.
///
/// # Returns
///
/// Margined SA-CCR maturity factor.
///
/// # References
///
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
#[must_use]
pub fn maturity_factor_margined(mpor_days: u32) -> f64 {
    let mpor_years = f64::from(mpor_days) / 250.0;
    1.5 * mpor_years.sqrt()
}
