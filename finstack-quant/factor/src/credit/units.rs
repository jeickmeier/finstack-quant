//! Decimal-spread input convention and conversion to basis points.
//!
//! Callers pass **decimal** spreads (`0.01` = 100 bp). Calibration and
//! [`decompose_levels`][crate::credit::decomposition::decompose_levels] convert
//! with `BP_PER_UNIT` (`10_000.0`) at the entry boundary. Artifact internals, factor
//! histories, anchor/decompose outputs, and Σ remain **bp**.

use finstack_quant_core::{Error, Result};

/// Multiplier from a decimal spread (`0.01`) to basis points (`100.0`).
const BP_PER_UNIT: f64 = 10_000.0;

/// Exclusive lower bound of the accepted decimal-spread band.
const DECIMAL_SPREAD_EXCLUSIVE_MIN: f64 = -0.5;

/// Exclusive upper bound of the accepted decimal-spread band.
const DECIMAL_SPREAD_EXCLUSIVE_MAX: f64 = 2.0;

/// Reject non-finite floating-point inputs with a validation error naming `label`.
///
/// # Arguments
///
/// * `label` - Field path or identifier included in the error message.
/// * `value` - Numeric input to check for finiteness.
///
/// # Errors
///
/// Returns [`Error::Validation`] when `value` is `NaN` or infinite.
pub(super) fn ensure_finite(label: impl std::fmt::Display, value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "{label} must be finite, got {value}"
        )))
    }
}

/// Convert a validated decimal spread or generic level to basis points.
///
/// # Arguments
///
/// * `decimal` - Spread or generic factor level in decimal units
///   (`0.01` = 100 bp). Must already have passed
///   [`validate_decimal_spread`]; this function does not re-check the band.
#[must_use]
pub(super) fn decimal_to_bp(decimal: f64) -> f64 {
    decimal * BP_PER_UNIT
}

/// Reject non-finite values and values outside the decimal-spread band.
///
/// Accepted range is the open interval
/// (`DECIMAL_SPREAD_EXCLUSIVE_MIN`, `DECIMAL_SPREAD_EXCLUSIVE_MAX`).
/// Values such as `100.0` fail with a message that the input looks like
/// basis points rather than a decimal spread.
///
/// # Arguments
///
/// * `label` - Field path used in the validation error (issuer id, series
///   index, or `observed_generic`).
/// * `value` - Caller-supplied spread or generic level in **decimal** units.
///
/// # Errors
///
/// Returns [`Error::Validation`] when `value` is non-finite or outside the
/// decimal band.
pub(super) fn validate_decimal_spread(label: impl std::fmt::Display, value: f64) -> Result<()> {
    if !value.is_finite() {
        return Err(Error::Validation(format!(
            "{label} must be a finite decimal spread, got {value}"
        )));
    }
    if value > DECIMAL_SPREAD_EXCLUSIVE_MIN && value < DECIMAL_SPREAD_EXCLUSIVE_MAX {
        return Ok(());
    }
    if value.abs() >= 1.0 {
        return Err(Error::Validation(format!(
            "{label} = {value}: expected a decimal spread (e.g. 0.01 = 100 bp); \
             value looks like basis points"
        )));
    }
    Err(Error::Validation(format!(
        "{label} = {value}: expected a decimal spread in \
         ({DECIMAL_SPREAD_EXCLUSIVE_MIN}, {DECIMAL_SPREAD_EXCLUSIVE_MAX})"
    )))
}
