//! Rate-magnitude checks shared by the rates instruments' `validate` methods.

/// Validate that `|value| <= max_abs`.
///
/// # Arguments
///
/// * `value` - Finite or non-finite `f64` whose magnitude is compared; its sign
///   is ignored, and NaN fails the check because `NaN <= max_abs` is false.
/// * `max_abs` - Inclusive bound on the absolute value.
/// * `context` - Name of the value used in the error message.
/// * `units_hint` - Optional unit label appended to the bound in the message.
///
/// # Errors
///
/// Returns `Error::Validation` when `value.abs() > max_abs`.
#[inline]
pub(crate) fn validate_f64_abs_le(
    value: f64,
    max_abs: f64,
    context: &str,
    units_hint: Option<&str>,
) -> finstack_quant_core::Result<()> {
    finstack_quant_core::validation::require_with(value.abs() <= max_abs, || {
        let units = units_hint.unwrap_or("");
        format!("Invalid {context}: |value| exceeds {max_abs}{units} (got {value})")
    })
}

/// Validate that a rate-like value is finite and within `±max_abs`.
///
/// # Arguments
///
/// * `value` - Rate as a decimal (`0.05` is 5 %).
/// * `max_abs` - Inclusive bound on the absolute decimal value.
/// * `context` - Name of the rate used in the error message.
/// * `display_scale` - Multiplier applied to the value and bound in the
///   message (`100.0` to display percent, `10_000.0` for basis points).
/// * `display_units` - Unit label shown after the scaled numbers.
/// * `guidance` - Extra sentence appended to the message; empty for none.
///
/// # Errors
///
/// Returns `Error::Validation` when the value is non-finite or exceeds the bound.
#[inline]
pub(crate) fn validate_rate_magnitude(
    value: f64,
    max_abs: f64,
    context: &str,
    display_scale: f64,
    display_units: &str,
    guidance: &str,
) -> finstack_quant_core::Result<()> {
    finstack_quant_core::validation::validate_f64_finite(value, context)?;
    finstack_quant_core::validation::require_with(value.abs() <= max_abs, || {
        let guidance = if guidance.is_empty() {
            String::new()
        } else {
            format!(" {guidance}")
        };
        format!(
            "Invalid {context}: {:.2}{display_units} exceeds maximum allowed magnitude ({:.0}{display_units}).{guidance}",
            value * display_scale,
            max_abs * display_scale,
        )
    })
}

/// Whether a rate-like value lies outside an inclusive range.
///
/// # Arguments
///
/// * `value` - Rate-like quantity in the same representation as `min` and
///   `max` (for example a decimal rate); NaN is reported as outside the range.
/// * `min` - Inclusive lower bound.
/// * `max` - Inclusive upper bound.
#[inline]
#[must_use]
pub(crate) fn rate_outside_range(value: f64, min: f64, max: f64) -> bool {
    !(min..=max).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_f64_abs_le_rejects_large_values() {
        assert!(validate_f64_abs_le(2.0, 1.0, "test", None).is_err());
        assert!(validate_f64_abs_le(0.5, 1.0, "test", Some("%")).is_ok());
    }

    #[test]
    fn validate_rate_magnitude_rejects_large_values() {
        assert!(validate_rate_magnitude(2.0, 1.0, "test rate", 100.0, "%", "").is_err());
        assert!(validate_rate_magnitude(0.5, 1.0, "test rate", 100.0, "%", "").is_ok());
        assert!(rate_outside_range(2.0, -1.0, 1.0));
        assert!(!rate_outside_range(0.5, -1.0, 1.0));
    }
}
