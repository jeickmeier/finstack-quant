//! Observed underlying fixings shared by the seasoned equity instruments
//! (autocallable, cliquet, equity TRS).
//!
//! Each instrument stores its recorded levels as `(date, level)` pairs in a
//! `past_fixings` field; these helpers look a date up and validate the levels.

use finstack_quant_core::dates::Date;

/// Observed level recorded for `date`, if any.
///
/// # Arguments
///
/// * `fixings` - Recorded `(date, level)` pairs, levels in the underlying's price units.
/// * `date` - Observation, reset or period-start date to look up (exact match).
pub(crate) fn fixing_on(fixings: &[(Date, f64)], date: Date) -> Option<f64> {
    fixings.iter().find(|(d, _)| *d == date).map(|(_, v)| *v)
}

/// Reject any recorded level that is not finite and strictly positive.
///
/// # Arguments
///
/// * `label` - Instrument label that prefixes the error message (for example `Autocallable`).
/// * `fixings` - Recorded `(date, level)` pairs to check.
///
/// # Errors
///
/// Returns a validation error naming the first offending date and level.
pub(crate) fn validate_positive_fixings(
    label: &dyn std::fmt::Display,
    fixings: &[(Date, f64)],
) -> finstack_quant_core::Result<()> {
    for (d, v) in fixings {
        if !v.is_finite() || *v <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{label} past_fixings[{d}] = {v} must be finite and positive"
            )));
        }
    }
    Ok(())
}
