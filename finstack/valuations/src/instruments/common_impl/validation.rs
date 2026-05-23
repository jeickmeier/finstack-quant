//! Shared validation helpers for instrument invariants.
//!
//! These helpers are intentionally **convention-agnostic**: they do not encode
//! market-specific defaults (spot lags, business day conventions, rate bounds).
//! Use them to enforce structural invariants (ordering, finiteness, positivity),
//! and keep market-standard checks in instrument-specific validation.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;

// Re-export generic validation helpers from core.
pub(crate) use finstack_core::validation::{require_or, require_with};

/// Validate `end > start` for a date range.
#[inline]
pub(crate) fn validate_date_range_strict(
    start: Date,
    end: Date,
    context: &str,
) -> finstack_core::Result<()> {
    require_with(end > start, || {
        format!(
            "Invalid {} date range: end ({}) must be after start ({})",
            context, end, start
        )
    })
}

/// Validate `end > start` using a custom error message.
#[inline]
pub(crate) fn validate_date_range_strict_with(
    start: Date,
    end: Date,
    message: impl FnOnce(Date, Date) -> String,
) -> finstack_core::Result<()> {
    require_with(end > start, || message(start, end))
}

/// Validate `end >= start` for a date range.
#[inline]
pub(crate) fn validate_date_range_non_strict(
    start: Date,
    end: Date,
    context: &str,
) -> finstack_core::Result<()> {
    require_with(end >= start, || {
        format!(
            "{} start date ({}) must not be after end date ({})",
            context, start, end
        )
    })
}

/// Validate that a money amount is finite.
#[inline]
pub(crate) fn validate_money_finite(money: Money, context: &str) -> finstack_core::Result<()> {
    require_with(money.amount().is_finite(), || {
        format!("Invalid {}: amount must be finite.", context)
    })
}

/// Validate that a money amount is greater than a threshold.
#[inline]
pub(crate) fn validate_money_gt(
    money: Money,
    min: f64,
    context: &str,
) -> finstack_core::Result<()> {
    require_with(money.amount() > min, || {
        format!("Invalid {}: amount must be > {}", context, min)
    })
}

/// Validate that a money amount is greater than a threshold using a custom message.
#[inline]
pub(crate) fn validate_money_gt_with(
    money: Money,
    min: f64,
    message: impl FnOnce(f64) -> String,
) -> finstack_core::Result<()> {
    require_with(money.amount() > min, || message(money.amount()))
}

/// Validate that a money amount has the expected currency.
#[inline]
pub(crate) fn validate_money_currency(
    money: Money,
    expected: Currency,
    context: &str,
) -> finstack_core::Result<()> {
    require_with(money.currency() == expected, || {
        format!(
            "Invalid {}: currency ({}) must match expected ({})",
            context,
            money.currency(),
            expected
        )
    })
}

/// Validate that an FX pair has distinct base and quote currencies.
#[inline]
pub(crate) fn validate_distinct_currencies(
    base: Currency,
    quote: Currency,
    instrument_name: &str,
) -> finstack_core::Result<()> {
    require_with(base != quote, || {
        format!(
            "{instrument_name} base_currency ({base}) must differ from quote_currency ({quote})"
        )
    })
}

/// Validate that a floating-point value is finite.
#[inline]
pub(crate) fn validate_f64_finite(value: f64, context: &str) -> finstack_core::Result<()> {
    require_with(value.is_finite(), || {
        format!("Invalid {}: must be finite.", context)
    })
}

/// Validate that a floating-point value is positive (> 0).
#[inline]
pub(crate) fn validate_f64_positive(value: f64, context: &str) -> finstack_core::Result<()> {
    require_with(value > 0.0, || {
        format!("Invalid {}: must be positive, got {}", context, value)
    })
}

/// Validate that a floating-point value is non-negative (>= 0).
#[inline]
pub(crate) fn validate_f64_non_negative(value: f64, context: &str) -> finstack_core::Result<()> {
    require_with(value >= 0.0, || {
        format!("Invalid {}: must be non-negative, got {}", context, value)
    })
}

/// Validate that `|value| <= max_abs` (useful for rate magnitude guards).
#[inline]
pub(crate) fn validate_f64_abs_le(
    value: f64,
    max_abs: f64,
    context: &str,
    units_hint: Option<&str>,
) -> finstack_core::Result<()> {
    require_with(value.abs() <= max_abs, || {
        let units = units_hint.unwrap_or("");
        format!(
            "Invalid {}: |value| exceeds {}{} (got {})",
            context, max_abs, units, value
        )
    })
}

/// Validate that a rate-like value is finite and within `±max_abs`.
#[inline]
pub(crate) fn validate_rate_magnitude(
    value: f64,
    max_abs: f64,
    context: &str,
    display_scale: f64,
    display_units: &str,
    guidance: &str,
) -> finstack_core::Result<()> {
    validate_f64_finite(value, context)?;
    require_with(value.abs() <= max_abs, || {
        let guidance = if guidance.is_empty() {
            String::new()
        } else {
            format!(" {guidance}")
        };
        format!(
            "Invalid {}: {:.2}{} exceeds maximum allowed magnitude ({:.0}{}).{}",
            context,
            value * display_scale,
            display_units,
            max_abs * display_scale,
            display_units,
            guidance
        )
    })
}

/// Return true when a rate-like value is outside an inclusive range.
#[inline]
pub(crate) fn rate_outside_range(value: f64, min: f64, max: f64) -> bool {
    !(min..=max).contains(&value)
}

/// Validate that a recovery rate is within valid bounds \[0, 1\).
///
/// # Errors
///
/// Returns an error if recovery rate is not finite or is not in `[0.0, 1.0)`.
///
/// `R = 1.0` is rejected because LGD = (1 - R) = 0 makes the protection-leg
/// integrand vanish identically, which breaks hazard-curve bootstrapping
/// (no spread-to-hazard inversion is possible) and produces meaningless
/// tranche pricing. ISDA's standard convention also reserves `R < 1`.
#[inline]
pub(crate) fn validate_recovery_rate(recovery_rate: f64) -> finstack_core::Result<()> {
    require_with(
        recovery_rate.is_finite() && (0.0..1.0).contains(&recovery_rate),
        || {
            format!(
                "Recovery rate must be a finite value in [0.0, 1.0), got {recovery_rate}. \
                 R = 1.0 implies zero LGD, which makes protection legs degenerate."
            )
        },
    )
}

/// Validate that dates are strictly increasing.
#[inline]
pub(crate) fn validate_sorted_strict(values: &[Date], context: &str) -> finstack_core::Result<()> {
    for i in 1..values.len() {
        if values[i - 1] >= values[i] {
            return Err(finstack_core::Error::Validation(format!(
                "{} must be sorted in ascending order",
                context
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::money::Money;
    use time::Month;

    #[test]
    fn validate_date_range_strict_accepts_valid_order() {
        let start = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2024, Month::February, 1).expect("valid date");
        assert!(validate_date_range_strict(start, end, "test").is_ok());
    }

    #[test]
    fn validate_date_range_strict_rejects_equal_or_reverse() {
        let start = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        assert!(validate_date_range_strict(start, end, "test").is_err());
    }

    #[test]
    fn validate_date_range_non_strict_accepts_equal() {
        let start = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        assert!(validate_date_range_non_strict(start, end, "test").is_ok());
    }

    #[test]
    fn validate_money_checks_finite_and_gt() {
        let money = Money::new(1.0, Currency::USD);
        assert!(validate_money_finite(money, "test").is_ok());
        assert!(validate_money_gt(money, 0.0, "test").is_ok());
        assert!(validate_money_gt(money, 2.0, "test").is_err());
    }

    #[test]
    fn validate_f64_abs_le_rejects_large_values() {
        assert!(validate_f64_abs_le(2.0, 1.0, "test", None).is_err());
        assert!(validate_f64_abs_le(0.5, 1.0, "test", None).is_ok());
    }

    #[test]
    fn validate_rate_magnitude_rejects_large_values() {
        assert!(validate_rate_magnitude(2.0, 1.0, "test rate", 100.0, "%", "").is_err());
        assert!(validate_rate_magnitude(0.5, 1.0, "test rate", 100.0, "%", "").is_ok());
        assert!(rate_outside_range(2.0, -1.0, 1.0));
        assert!(!rate_outside_range(0.5, -1.0, 1.0));
    }

    #[test]
    fn validate_sorted_strict_rejects_unsorted() {
        let d1 = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let d2 = Date::from_calendar_date(2024, Month::February, 1).expect("valid date");
        let d3 = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
        assert!(validate_sorted_strict(&[d1, d2, d3], "dates").is_err());
        assert!(validate_sorted_strict(&[d1, d3, d2], "dates").is_ok());
    }
}
