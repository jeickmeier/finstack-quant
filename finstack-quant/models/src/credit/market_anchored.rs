//! Market-anchored credit-volatility conversions.
//!
//! Credit volatility is quoted one way and consumed another. Index CDS-option
//! markets quote a **fractional** (relative, lognormal) forward-spread
//! volatility — `0.35` meaning 35% of the spread level. The models here need
//! **absolute** parameters: the two-factor callable lattice takes an additive
//! hazard-rate volatility in decimal hazard points per √year, and the
//! revolving-credit CIR process takes a square-root diffusion coefficient.
//! Feeding `0.35` straight into either is roughly an order of magnitude wrong.
//!
//! This module owns that conversion so the callable lattice and the
//! revolving-credit Monte Carlo path derive their parameters from one place.
//!
//! # The credit triangle
//!
//! All conversions rest on the standard first-order approximation relating a
//! par CDS spread to a flat hazard rate and recovery,
//!
//! ```text
//! s ≈ (1 − R) · λ
//! ```
//!
//! which follows from equating the protection and premium legs under a flat
//! hazard and ignoring discounting within the accrual period (O'Kane 2008,
//! §5.4). Differentiating at fixed recovery gives the volatility mapping: a
//! proportional move in the spread is a proportional move in the hazard, so
//!
//! ```text
//! σ_s,abs = σ_fractional · s_ref
//! σ_λ     = σ_fractional · λ_ref = σ_s,abs / (1 − R)
//! ```
//!
//! Recovery cancels from the hazard volatility exactly as it cancels from the
//! level relation — which is why the hazard vol can be recovered from a spread
//! vol without knowing recovery, provided the same triangle is used on both
//! sides.
//!
//! # Scope and limitations
//!
//! This is an explicit **first-order local** mapping, evaluated at one
//! reference hazard over one horizon. It is not a calibration: feeding the
//! resulting `σ_λ` into the callable lattice will not exactly reprice the
//! index option it came from. Term structure of credit volatility, skew, and
//! issuer/index beta are all out of scope — applying an index-derived
//! fractional vol to a single name is a **caller** decision, made explicit by
//! passing the target curve's own reference hazard.
//!
//! # References
//!
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit
//!   Derivatives*. Wiley Finance, §5.4 (credit triangle) and ch. 6. `docs/REFERENCES.md#o-kane-2008`
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models — Theory and
//!   Practice*, 2nd ed. Springer, ch. 22 (CIR intensity dynamics). `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`

use finstack_quant_core::{Error, Result};

/// Conditional average hazard rate over `[start, end]` implied by two survival
/// probabilities.
///
/// ```text
/// λ̄ = −ln(SP(end) / SP(start)) / (end − start)
/// ```
///
/// This is the flat hazard that reproduces the conditional survival across the
/// window, which is the level a horizon-referenced volatility mapping should
/// be anchored at.
///
/// # Arguments
///
/// * `survival_start` - survival probability at the window start, `> 0`
/// * `survival_end` - survival probability at the window end, `> 0`
/// * `horizon_years` - window length in years, `> 0`
///
/// # Errors
///
/// Returns [`Error::Validation`] when either survival probability is
/// non-finite or non-positive, the horizon is non-finite or non-positive, or
/// survival increases across the window (a non-monotonic curve).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::credit::market_anchored::conditional_average_hazard;
///
/// // 3% flat hazard over five years.
/// let sp_start = (-0.03_f64 * 1.0).exp();
/// let sp_end = (-0.03_f64 * 6.0).exp();
/// let lambda = conditional_average_hazard(sp_start, sp_end, 5.0)?;
/// assert!((lambda - 0.03).abs() < 1e-12);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn conditional_average_hazard(
    survival_start: f64,
    survival_end: f64,
    horizon_years: f64,
) -> Result<f64> {
    if !horizon_years.is_finite() || horizon_years <= 0.0 {
        return Err(Error::Validation(format!(
            "credit-volatility horizon must be finite and positive, got {horizon_years}"
        )));
    }
    for (label, sp) in [("start", survival_start), ("end", survival_end)] {
        if !sp.is_finite() || sp <= 0.0 {
            return Err(Error::Validation(format!(
                "survival probability at window {label} must be finite and \
                 positive, got {sp}"
            )));
        }
    }
    if survival_end > survival_start {
        return Err(Error::Validation(format!(
            "survival must not increase across the window: start={survival_start}, \
             end={survival_end}; the hazard curve is non-monotonic"
        )));
    }
    Ok((-(survival_end / survival_start).ln() / horizon_years).max(0.0))
}

/// CIR square-root diffusion coefficient for a spread process anchored at
/// `reference_spread`: `σ_CIR = σ_fractional · √s_ref`.
///
/// The CIR diffusion term is `σ_CIR·√s·dW`, so matching the local lognormal
/// volatility `σ_fractional·s` at `s = s_ref` requires
/// `σ_CIR·√s_ref = σ_fractional·s_ref`.
///
/// # Arguments
///
/// * `fractional_vol` - relative spread volatility, e.g. `0.35`
/// * `reference_spread` - par spread level `s_ref` the process is anchored at
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite or negative volatility or
/// reference spread.
pub fn cir_diffusion_coefficient(fractional_vol: f64, reference_spread: f64) -> Result<f64> {
    validate_volatility(fractional_vol)?;
    validate_level("reference spread", reference_spread)?;
    Ok(fractional_vol * reference_spread.sqrt())
}

fn validate_volatility(vol: f64) -> Result<()> {
    if !vol.is_finite() || vol < 0.0 {
        return Err(Error::Validation(format!(
            "fractional spread volatility must be finite and non-negative, got {vol}"
        )));
    }
    Ok(())
}

fn validate_level(label: &str, level: f64) -> Result<()> {
    if !level.is_finite() || level < 0.0 {
        return Err(Error::Validation(format!(
            "{label} must be finite and non-negative, got {level}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditional_average_hazard_recovers_a_flat_hazard() {
        let lambda = 0.037_f64;
        let sp_start = (-lambda * 2.0).exp();
        let sp_end = (-lambda * 7.0).exp();
        let recovered = conditional_average_hazard(sp_start, sp_end, 5.0).expect("hazard");
        assert!((recovered - lambda).abs() < 1e-12, "got {recovered}");
    }

    #[test]
    fn cir_diffusion_matches_local_lognormal_vol_at_anchor() {
        let fractional = 0.35_f64;
        let ref_spread = 0.6 * 0.03;
        let cir = cir_diffusion_coefficient(fractional, ref_spread).expect("cir");
        assert!((cir * ref_spread.sqrt() - fractional * ref_spread).abs() < 1e-15);
    }

    #[test]
    fn invalid_inputs_fail_explicitly() {
        assert!(
            conditional_average_hazard(1.0, 0.9, 0.0).is_err(),
            "zero horizon"
        );
        assert!(
            conditional_average_hazard(1.0, 0.9, -1.0).is_err(),
            "negative horizon"
        );
        assert!(
            conditional_average_hazard(0.0, 0.9, 5.0).is_err(),
            "zero survival"
        );
        assert!(
            conditional_average_hazard(0.9, 0.95, 5.0).is_err(),
            "increasing survival is a non-monotonic curve"
        );
        assert!(
            cir_diffusion_coefficient(-0.1, 0.03).is_err(),
            "negative volatility"
        );
        assert!(
            cir_diffusion_coefficient(0.35, -0.01).is_err(),
            "negative reference spread"
        );
        assert!(
            cir_diffusion_coefficient(0.35, f64::NAN).is_err(),
            "non-finite reference spread"
        );
    }
}
