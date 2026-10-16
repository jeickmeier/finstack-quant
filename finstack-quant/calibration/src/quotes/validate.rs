//! Shared numeric checks for concrete market-quote validation.

use finstack_quant_core::{Error, Result};

/// Reject non-finite values.
pub(crate) fn finite(value: f64, field: &str) -> Result<()> {
    if !value.is_finite() {
        return Err(Error::Validation(format!(
            "{field} must be finite; got {value}"
        )));
    }
    Ok(())
}

/// Reject non-finite or non-positive values.
pub(crate) fn positive(value: f64, field: &str) -> Result<()> {
    finite(value, field)?;
    if value <= 0.0 {
        return Err(Error::Validation(format!(
            "{field} must be positive; got {value}"
        )));
    }
    Ok(())
}

/// Reject non-finite or negative values.
pub(crate) fn non_negative(value: f64, field: &str) -> Result<()> {
    finite(value, field)?;
    if value < 0.0 {
        return Err(Error::Validation(format!(
            "{field} must be non-negative; got {value}"
        )));
    }
    Ok(())
}

/// Reject values outside the closed unit interval.
pub(crate) fn unit_interval(value: f64, field: &str) -> Result<()> {
    finite(value, field)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(Error::Validation(format!(
            "{field} must be in [0, 1]; got {value}"
        )));
    }
    Ok(())
}
