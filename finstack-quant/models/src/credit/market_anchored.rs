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

/// Smallest reference spread/hazard treated as non-degenerate.
///
/// Below this the credit triangle stops being informative: a fractional
/// volatility of a vanishing spread carries no absolute information, and the
/// CIR square-root coefficient would collapse.
pub const MIN_REFERENCE_LEVEL: f64 = 1e-8;

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

/// Reference par spread implied by a hazard rate under the credit triangle:
/// `s ≈ (1 − R) · λ`.
///
/// # Arguments
///
/// * `hazard` - annualized default intensity `λ`
/// * `recovery` - recovery rate `R`, in `[0, 1)`
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite or negative hazard, or a
/// recovery outside `[0, 1)`.
pub fn reference_spread(hazard: f64, recovery: f64) -> Result<f64> {
    validate_hazard(hazard)?;
    validate_recovery(recovery)?;
    Ok((1.0 - recovery) * hazard)
}

/// Convert a fractional (relative) spread volatility to an absolute one at a
/// reference spread level: `σ_s,abs = σ_fractional · s_ref`.
///
/// # Arguments
///
/// * `fractional_vol` - relative spread volatility, e.g. `0.35`
/// * `reference_spread` - par spread level the volatility is anchored at
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite or negative volatility or
/// reference spread.
fn absolute_spread_volatility(fractional_vol: f64, reference_spread: f64) -> Result<f64> {
    validate_volatility(fractional_vol)?;
    validate_level("reference spread", reference_spread)?;
    Ok(fractional_vol * reference_spread)
}

/// Convert a fractional spread volatility to the **additive hazard**
/// volatility the two-factor rates-credit lattice consumes:
///
/// ```text
/// σ_λ = σ_fractional · λ_ref
/// ```
///
/// Equivalently `σ_s,abs / (1 − R)` under the same triangle — recovery
/// cancels, so this needs only the reference hazard.
///
/// # Arguments
///
/// * `fractional_vol` - relative spread volatility, e.g. `0.35`
/// * `reference_hazard` - hazard level `λ_ref` the volatility is anchored at
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite or negative volatility or
/// reference hazard.
fn additive_hazard_volatility(fractional_vol: f64, reference_hazard: f64) -> Result<f64> {
    validate_volatility(fractional_vol)?;
    validate_level("reference hazard", reference_hazard)?;
    Ok(fractional_vol * reference_hazard)
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

/// Every quantity used and produced by a market-anchored conversion.
///
/// Returned alongside the converted volatility so a caller can see the level
/// the conversion was anchored at, not just the answer. That is what stops a
/// relative CDS-option quote such as `0.35` from being dropped into an
/// additive hazard lattice unnoticed: the diagnostics show the absolute
/// output next to the fractional input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreditVolatilityConversion {
    /// Horizon in years the reference level was averaged over.
    pub horizon_years: f64,
    /// Recovery rate used for the credit triangle.
    pub recovery: f64,
    /// Conditional average hazard over the horizon.
    pub reference_hazard: f64,
    /// Reference par spread implied by the triangle.
    pub reference_spread: f64,
    /// Input fractional (relative) spread volatility.
    pub fractional_spread_volatility: f64,
    /// Output absolute spread volatility.
    pub absolute_spread_volatility: f64,
    /// Output absolute hazard volatility — the additive `σ_λ` the callable
    /// lattice consumes.
    pub hazard_volatility: f64,
    /// CIR diffusion coefficient for a square-root spread process.
    pub cir_diffusion: f64,
}

impl CreditVolatilityConversion {
    /// Build the full conversion from a fractional spread volatility anchored
    /// on a survival window.
    ///
    /// # Arguments
    ///
    /// * `fractional_spread_volatility` - relative spread vol, e.g. `0.35`
    /// * `survival_start` - survival probability at the window start on the
    ///   **target** credit curve
    /// * `survival_end` - survival probability at the window end on the same
    ///   curve
    /// * `horizon_years` - window length in years
    /// * `recovery` - recovery rate for the credit triangle, in `[0, 1)`
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for any invalid horizon, survival
    /// probability, recovery, or volatility, and when the anchored hazard is
    /// too small for the mapping to carry information
    /// (see [`MIN_REFERENCE_LEVEL`]).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_models::credit::market_anchored::CreditVolatilityConversion;
    ///
    /// // A 35% relative CDS-option vol on a 3% hazard is a 1.05% absolute
    /// // hazard vol — not 35%.
    /// let sp_start = 1.0_f64;
    /// let sp_end = (-0.03_f64 * 5.0).exp();
    /// let conversion =
    ///     CreditVolatilityConversion::from_survival_window(0.35, sp_start, sp_end, 5.0, 0.4)?;
    /// assert!((conversion.reference_hazard - 0.03).abs() < 1e-12);
    /// assert!((conversion.hazard_volatility - 0.0105).abs() < 1e-12);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn from_survival_window(
        fractional_spread_volatility: f64,
        survival_start: f64,
        survival_end: f64,
        horizon_years: f64,
        recovery: f64,
    ) -> Result<Self> {
        validate_volatility(fractional_spread_volatility)?;
        validate_recovery(recovery)?;
        let reference_hazard =
            conditional_average_hazard(survival_start, survival_end, horizon_years)?;
        Self::from_reference_hazard(
            fractional_spread_volatility,
            reference_hazard,
            horizon_years,
            recovery,
        )
    }

    /// Build the conversion from an already-computed reference hazard.
    ///
    /// Use this when the anchoring level comes from somewhere other than a
    /// survival ratio (a quoted flat hazard, say).
    ///
    /// # Errors
    ///
    /// As [`Self::from_survival_window`].
    fn from_reference_hazard(
        fractional_spread_volatility: f64,
        reference_hazard: f64,
        horizon_years: f64,
        recovery: f64,
    ) -> Result<Self> {
        validate_volatility(fractional_spread_volatility)?;
        validate_recovery(recovery)?;
        validate_hazard(reference_hazard)?;
        if !horizon_years.is_finite() || horizon_years <= 0.0 {
            return Err(Error::Validation(format!(
                "credit-volatility horizon must be finite and positive, got {horizon_years}"
            )));
        }
        // A zero volatility maps to zero everywhere and is always meaningful;
        // only a positive volatility needs a non-degenerate anchor.
        if fractional_spread_volatility > 0.0 && reference_hazard < MIN_REFERENCE_LEVEL {
            return Err(Error::Validation(format!(
                "reference hazard {reference_hazard:.3e} is below \
                 {MIN_REFERENCE_LEVEL:.0e}: a fractional spread volatility of a \
                 vanishing spread carries no absolute information, so it cannot \
                 be converted to an additive hazard volatility. Anchor on a \
                 horizon where the curve implies a positive hazard."
            )));
        }

        let reference_spread = reference_spread(reference_hazard, recovery)?;
        Ok(Self {
            horizon_years,
            recovery,
            reference_hazard,
            reference_spread,
            fractional_spread_volatility,
            absolute_spread_volatility: absolute_spread_volatility(
                fractional_spread_volatility,
                reference_spread,
            )?,
            hazard_volatility: additive_hazard_volatility(
                fractional_spread_volatility,
                reference_hazard,
            )?,
            cir_diffusion: cir_diffusion_coefficient(
                fractional_spread_volatility,
                reference_spread,
            )?,
        })
    }
}

fn validate_volatility(vol: f64) -> Result<()> {
    if !vol.is_finite() || vol < 0.0 {
        return Err(Error::Validation(format!(
            "fractional spread volatility must be finite and non-negative, got {vol}"
        )));
    }
    Ok(())
}

fn validate_recovery(recovery: f64) -> Result<()> {
    if !recovery.is_finite() || !(0.0..1.0).contains(&recovery) {
        return Err(Error::Validation(format!(
            "recovery rate must be finite and in [0, 1), got {recovery}"
        )));
    }
    Ok(())
}

fn validate_hazard(hazard: f64) -> Result<()> {
    if !hazard.is_finite() || hazard < 0.0 {
        return Err(Error::Validation(format!(
            "reference hazard must be finite and non-negative, got {hazard}"
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

    /// Recovery cancels from the hazard-volatility mapping: computing it
    /// directly from the hazard and computing it from the absolute spread vol
    /// must agree for any recovery.
    #[test]
    fn recovery_cancels_from_the_hazard_volatility_mapping() {
        let (fractional, lambda) = (0.35_f64, 0.03_f64);
        for recovery in [0.0, 0.2, 0.4, 0.7, 0.95] {
            let conversion = CreditVolatilityConversion::from_reference_hazard(
                fractional, lambda, 5.0, recovery,
            )
            .expect("conversion");
            assert!(
                (conversion.hazard_volatility - fractional * lambda).abs() < 1e-15,
                "recovery {recovery}: sigma_lambda must not depend on recovery"
            );
            assert!(
                (conversion.hazard_volatility
                    - conversion.absolute_spread_volatility / (1.0 - recovery))
                    .abs()
                    < 1e-15,
                "recovery {recovery}: sigma_lambda must equal sigma_s_abs / (1 - R)"
            );
        }
    }

    #[test]
    fn zero_volatility_maps_exactly_to_zero() {
        let conversion =
            CreditVolatilityConversion::from_reference_hazard(0.0, 0.03, 5.0, 0.4).expect("zero");
        assert_eq!(conversion.absolute_spread_volatility, 0.0);
        assert_eq!(conversion.hazard_volatility, 0.0);
        assert_eq!(conversion.cir_diffusion, 0.0);

        // A zero vol stays valid even at a degenerate hazard: nothing to scale.
        CreditVolatilityConversion::from_reference_hazard(0.0, 0.0, 5.0, 0.4)
            .expect("zero vol needs no anchor");
    }

    /// The conversion is what stops a relative CDS-option quote from being
    /// dropped into an additive lattice: 35% relative on a 3% hazard is a
    /// 1.05% absolute hazard vol, over 30x smaller.
    #[test]
    fn fractional_quote_becomes_a_much_smaller_absolute_hazard_vol() {
        let conversion =
            CreditVolatilityConversion::from_reference_hazard(0.35, 0.03, 5.0, 0.4).expect("conv");
        assert!((conversion.hazard_volatility - 0.0105).abs() < 1e-15);
        assert!(
            conversion.fractional_spread_volatility / conversion.hazard_volatility > 30.0,
            "the diagnostics must make the scale difference visible"
        );
        // Reference spread follows the triangle.
        assert!((conversion.reference_spread - 0.6 * 0.03).abs() < 1e-15);
        // CIR coefficient matches the local lognormal vol at the anchor.
        assert!(
            (conversion.cir_diffusion * conversion.reference_spread.sqrt()
                - conversion.fractional_spread_volatility * conversion.reference_spread)
                .abs()
                < 1e-15
        );
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
            CreditVolatilityConversion::from_reference_hazard(-0.1, 0.03, 5.0, 0.4).is_err(),
            "negative volatility"
        );
        assert!(
            CreditVolatilityConversion::from_reference_hazard(0.35, 0.03, 5.0, 1.0).is_err(),
            "recovery of 1 leaves no loss given default"
        );
        assert!(
            CreditVolatilityConversion::from_reference_hazard(0.35, f64::NAN, 5.0, 0.4).is_err(),
            "non-finite hazard"
        );
        let err = CreditVolatilityConversion::from_reference_hazard(0.35, 1e-12, 5.0, 0.4)
            .expect_err("degenerate anchor with positive vol");
        assert!(
            err.to_string().contains("carries no absolute information"),
            "error should explain why: {err}"
        );
    }
}
