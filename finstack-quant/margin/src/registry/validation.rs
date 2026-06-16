//! Generic value-range validators used during registry parsing.
//!
//! These small predicates are split out of [`super`]'s 1.6k-line module
//! so adding a new bound check (e.g. for a future SIMM v2.7 parameter)
//! does not require navigating around the much larger SIMM-specific
//! correlation-matrix validators that live next to the parser code.

use finstack_quant_core::{Error, Result};

/// Reject negative, NaN, or infinite rates.
pub(super) fn validate_rate(name: &str, v: f64) -> Result<()> {
    if !v.is_finite() || v < 0.0 {
        return Err(Error::Validation(format!(
            "invalid rate '{name}': must be finite and >= 0"
        )));
    }
    Ok(())
}

/// Reject probabilities outside the closed unit interval `[0, 1]`.
pub(super) fn validate_probability(name: &str, v: f64) -> Result<()> {
    if !v.is_finite() || !(0.0..=1.0).contains(&v) {
        return Err(Error::Validation(format!(
            "invalid probability '{name}': must be in [0,1]"
        )));
    }
    Ok(())
}

/// Reject negative, NaN, or infinite numeric inputs (risk weights,
/// vega weights, concentration thresholds, etc.).
pub(super) fn validate_non_negative(name: &str, v: f64) -> Result<()> {
    if !v.is_finite() || v < 0.0 {
        return Err(Error::Validation(format!(
            "invalid value '{name}': must be finite and >= 0"
        )));
    }
    Ok(())
}

/// Reject haircuts outside the closed unit interval `[0, 1]`. Haircuts
/// outside this range either understate (negative) or fully consume
/// (>1) collateral value and are always operator errors.
pub(super) fn validate_haircut(name: &str, v: f64) -> Result<()> {
    if !v.is_finite() || !(0.0..=1.0).contains(&v) {
        return Err(Error::Validation(format!(
            "invalid haircut '{name}': must be in [0,1]"
        )));
    }
    Ok(())
}

/// Wrap a `serde_json::Error` in [`Error::Validation`] with the
/// margin-registry context.
pub(super) fn to_validation(err: serde_json::Error) -> Error {
    Error::Validation(format!("Failed to parse margin registry: {err}"))
}
