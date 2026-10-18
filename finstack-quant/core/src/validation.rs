//! Generic validation helpers for checking invariants.
//!
//! These helpers are convention-agnostic: they enforce structural invariants
//! (conditions, ordering, finiteness) without encoding market-specific defaults.

/// Require a condition to be true, otherwise return the provided error.
///
/// Use this when a caller needs to preserve a domain-specific error variant
/// rather than collapse a failed predicate into `Error::Validation`.
///
/// # Arguments
///
/// * `condition` - Invariant or validation predicate that must evaluate to
///   `true` for success.
/// * `err` - Domain-specific error converted and returned when `condition` is
///   false.
///
/// # Errors
///
/// Returns `err.into()` when `condition` is false, preserving the supplied
/// error's category and diagnostic information.
#[inline]
pub fn require_or(condition: bool, err: impl Into<crate::Error>) -> crate::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(err.into())
    }
}

/// Require a condition to be true, lazily constructing the error message.
///
/// The closure is never invoked when the condition holds, making this helper
/// appropriate when an error message needs formatting or data collection.
///
/// # Arguments
///
/// * `condition` - Invariant or validation predicate that must evaluate to
///   `true` for success.
/// * `message` - Closure that lazily constructs validation text only when
///   `condition` is false.
///
/// # Errors
///
/// Invokes `message` and returns [`crate::Error::Validation`] with its result
/// when `condition` is false. If `message` panics, that panic propagates.
#[inline]
pub fn require_with(condition: bool, message: impl FnOnce() -> String) -> crate::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(crate::Error::Validation(message()))
    }
}

/// Validate that a floating-point value is finite.
///
/// # Arguments
///
/// * `value` - Floating-point input that must not be NaN or infinite.
/// * `context` - Field or calculation label included in validation diagnostics.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `value` is not finite.
#[inline]
pub fn validate_f64_finite(value: f64, context: &str) -> crate::Result<()> {
    require_with(value.is_finite(), || {
        format!("Invalid {context}: must be finite.")
    })
}

/// Validate that a floating-point value is finite and strictly positive.
///
/// # Arguments
///
/// * `value` - Floating-point input that must be greater than zero.
/// * `context` - Field or calculation label included in validation diagnostics.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `value` is non-finite or not
/// strictly positive.
#[inline]
pub fn validate_f64_positive(value: f64, context: &str) -> crate::Result<()> {
    require_with(value.is_finite() && value > 0.0, || {
        format!("Invalid {context}: must be positive, got {value}")
    })
}

/// Validate that a floating-point value is finite and non-negative.
///
/// # Arguments
///
/// * `value` - Floating-point input that must be greater than or equal to zero.
/// * `context` - Field or calculation label included in validation diagnostics.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `value` is non-finite or negative.
#[inline]
pub fn validate_f64_non_negative(value: f64, context: &str) -> crate::Result<()> {
    require_with(value.is_finite() && value >= 0.0, || {
        format!("Invalid {context}: must be non-negative, got {value}")
    })
}

/// Validate that a floating-point value is finite and lies in `[0, 1]`.
///
/// # Arguments
///
/// * `value` - Floating-point proportion represented as a decimal fraction.
/// * `context` - Field or calculation label included in validation diagnostics.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `value` is non-finite or outside
/// the closed unit interval.
#[inline]
pub fn validate_f64_unit_interval(value: f64, context: &str) -> crate::Result<()> {
    require_with(value.is_finite() && (0.0..=1.0).contains(&value), || {
        format!("Invalid {context}: must be finite and in [0, 1], got {value}")
    })
}

/// Validate that a text field contains at least one non-whitespace character.
///
/// # Arguments
///
/// * `value` - Text whose trimmed representation must not be empty.
/// * `context` - Field or registry label included in validation diagnostics.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `value` is empty or whitespace.
#[inline]
pub fn validate_non_blank(value: &str, context: &str) -> crate::Result<()> {
    require_with(!value.trim().is_empty(), || {
        format!("Invalid {context}: must not be blank")
    })
}

/// Validate the source label and source version attached to registry records.
///
/// # Arguments
///
/// * `label` - Human-readable record kind used in validation diagnostics.
/// * `source` - Non-blank provenance name for the record.
/// * `source_version` - Non-blank version or publication date for `source`.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when either metadata value is blank.
pub fn validate_source_metadata(
    label: &str,
    source: &str,
    source_version: &str,
) -> crate::Result<()> {
    validate_non_blank(source, &format!("{label} source"))?;
    validate_non_blank(source_version, &format!("{label} source version"))
}

/// Validate non-empty, non-blank, globally unique alias lists.
///
/// # Arguments
///
/// * `registry` - Human-readable registry name used in validation diagnostics.
/// * `kind` - Record kind whose aliases are being validated.
/// * `records` - Alias slices, one per registry record; aliases are compared
///   after trimming surrounding whitespace.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when a record has no aliases, an alias
/// is blank, or the same trimmed alias occurs more than once.
pub fn validate_unique_ids<'a>(
    registry: &str,
    kind: &str,
    records: impl Iterator<Item = &'a [String]>,
) -> crate::Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for ids in records {
        require_with(!ids.is_empty(), || {
            format!("{registry} contains {kind} without an id")
        })?;
        for id in ids {
            let trimmed = id.trim();
            require_with(!trimmed.is_empty(), || {
                format!("{registry} contains blank {kind} id")
            })?;
            require_with(seen.insert(trimmed.to_string()), || {
                format!("{registry} contains duplicate {kind} id '{trimmed}'")
            })?;
        }
    }
    Ok(())
}

/// Validate `end > start` for a date range.
///
/// # Arguments
///
/// * `start` - Range start date.
/// * `end` - Range end date, which must be strictly after `start`.
/// * `context` - Name of the range used in the error message.
///
/// # Errors
///
/// Returns `Error::Validation` when `end <= start`.
#[inline]
pub fn validate_date_range_strict(
    start: crate::dates::Date,
    end: crate::dates::Date,
    context: &str,
) -> crate::Result<()> {
    require_with(end > start, || {
        format!("Invalid {context} date range: end ({end}) must be after start ({start})")
    })
}

/// Validate `end > start` for a date range with a caller-built message.
///
/// # Arguments
///
/// * `start` - Range start date.
/// * `end` - Range end date, which must be strictly after `start`.
/// * `message` - Builds the error message from `(start, end)` on failure.
///
/// # Errors
///
/// Returns `Error::Validation` with `message(start, end)` when `end <= start`.
#[inline]
pub fn validate_date_range_strict_with(
    start: crate::dates::Date,
    end: crate::dates::Date,
    message: impl FnOnce(crate::dates::Date, crate::dates::Date) -> String,
) -> crate::Result<()> {
    require_with(end > start, || message(start, end))
}

/// Validate `end >= start` for a date range.
///
/// # Arguments
///
/// * `start` - Range start date.
/// * `end` - Range end date, which must not be before `start`.
/// * `context` - Name of the range used in the error message.
///
/// # Errors
///
/// Returns `Error::Validation` when `end < start`.
#[inline]
pub fn validate_date_range_non_strict(
    start: crate::dates::Date,
    end: crate::dates::Date,
    context: &str,
) -> crate::Result<()> {
    require_with(end >= start, || {
        format!("{context} start date ({start}) must not be after end date ({end})")
    })
}

/// Validate that a monetary amount is finite.
///
/// # Arguments
///
/// * `money` - Amount to check.
/// * `context` - Name of the amount used in the error message.
///
/// # Errors
///
/// Returns `Error::Validation` when the amount is NaN or infinite.
#[inline]
pub fn validate_money_finite(money: crate::money::Money, context: &str) -> crate::Result<()> {
    require_with(money.amount().is_finite(), || {
        format!("Invalid {context}: amount must be finite.")
    })
}

/// Validate that a monetary amount exceeds a threshold.
///
/// # Arguments
///
/// * `money` - Amount to check.
/// * `min` - Exclusive lower bound in the amount's own currency units.
/// * `context` - Name of the amount used in the error message.
///
/// # Errors
///
/// Returns `Error::Validation` when `money.amount() <= min`.
#[inline]
pub fn validate_money_gt(money: crate::money::Money, min: f64, context: &str) -> crate::Result<()> {
    require_with(money.amount() > min, || {
        format!("Invalid {context}: amount must be > {min}")
    })
}

/// Validate that a monetary amount exceeds a threshold with a caller-built message.
///
/// # Arguments
///
/// * `money` - Amount to check.
/// * `min` - Exclusive lower bound in the amount's own currency units.
/// * `message` - Builds the error message from the offending amount.
///
/// # Errors
///
/// Returns `Error::Validation` with `message(amount)` when `money.amount() <= min`.
#[inline]
pub fn validate_money_gt_with(
    money: crate::money::Money,
    min: f64,
    message: impl FnOnce(f64) -> String,
) -> crate::Result<()> {
    require_with(money.amount() > min, || message(money.amount()))
}

/// Validate that a monetary amount carries the expected currency.
///
/// # Arguments
///
/// * `money` - Amount to check.
/// * `expected` - Currency the amount must be denominated in.
/// * `context` - Name of the amount used in the error message.
///
/// # Errors
///
/// Returns `Error::Validation` when the currencies differ.
#[inline]
pub fn validate_money_currency(
    money: crate::money::Money,
    expected: crate::currency::Currency,
    context: &str,
) -> crate::Result<()> {
    require_with(money.currency() == expected, || {
        format!(
            "Invalid {context}: currency ({}) must match expected ({expected})",
            money.currency()
        )
    })
}

/// Validate that an FX pair has distinct base and quote currencies.
///
/// # Arguments
///
/// * `base` - Base currency of the pair.
/// * `quote` - Quote currency of the pair, which must differ from `base`.
/// * `instrument_name` - Instrument name used in the error message.
///
/// # Errors
///
/// Returns `Error::Validation` when `base == quote`.
#[inline]
pub fn validate_distinct_currencies(
    base: crate::currency::Currency,
    quote: crate::currency::Currency,
    instrument_name: &str,
) -> crate::Result<()> {
    require_with(base != quote, || {
        format!(
            "{instrument_name} base_currency ({base}) must differ from quote_currency ({quote})"
        )
    })
}

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
pub fn validate_f64_abs_le(
    value: f64,
    max_abs: f64,
    context: &str,
    units_hint: Option<&str>,
) -> crate::Result<()> {
    require_with(value.abs() <= max_abs, || {
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
pub fn validate_rate_magnitude(
    value: f64,
    max_abs: f64,
    context: &str,
    display_scale: f64,
    display_units: &str,
    guidance: &str,
) -> crate::Result<()> {
    validate_f64_finite(value, context)?;
    require_with(value.abs() <= max_abs, || {
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
pub fn rate_outside_range(value: f64, min: f64, max: f64) -> bool {
    !(min..=max).contains(&value)
}

/// Validate that a recovery rate is a finite value in `[0, 1)`.
///
/// `R = 1.0` is rejected because LGD `1 - R = 0` makes a protection-leg
/// integrand vanish identically, which breaks hazard-curve bootstrapping and
/// produces meaningless tranche pricing; the ISDA standard convention also
/// reserves `R < 1`.
///
/// # Arguments
///
/// * `recovery_rate` - Recovery as a decimal fraction of notional.
///
/// # Errors
///
/// Returns `Error::Validation` when the rate is non-finite or outside `[0, 1)`.
#[inline]
pub fn validate_recovery_rate(recovery_rate: f64) -> crate::Result<()> {
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
///
/// # Arguments
///
/// * `values` - Dates in the order they were supplied.
/// * `context` - Name of the sequence used in the error message.
///
/// # Errors
///
/// Returns `Error::Validation` when any date is not strictly after its predecessor.
#[inline]
pub fn validate_sorted_strict(values: &[crate::dates::Date], context: &str) -> crate::Result<()> {
    for pair in values.windows(2) {
        if pair[0] >= pair[1] {
            return Err(crate::Error::Validation(format!(
                "{context} must be sorted in ascending order"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, InputError};

    #[test]
    fn require_with_returns_validation_error_when_condition_is_false() {
        assert!(require_with(true, || unreachable!("message is never built")).is_ok());

        let err = require_with(false, || "missing invariant".to_string())
            .expect_err("false condition should fail");
        assert!(matches!(err, Error::Validation(message) if message == "missing invariant"));
    }

    #[test]
    fn require_or_preserves_caller_supplied_error() {
        assert!(require_or(true, InputError::NegativeValue).is_ok());

        let err =
            require_or(false, InputError::NegativeValue).expect_err("false condition should fail");
        assert!(matches!(err, Error::Input(InputError::NegativeValue)));
    }

    #[test]
    fn require_with_is_lazy_on_success_and_builds_message_on_failure() {
        assert!(require_with(true, || panic!("message closure must not run")).is_ok());

        let err =
            require_with(false, || "lazy failure".to_string()).expect_err("false should fail");
        assert!(matches!(err, Error::Validation(message) if message == "lazy failure"));
    }

    #[test]
    fn unique_ids_reject_trimmed_duplicates() {
        let records = [vec!["primary".to_string()], vec![" primary ".to_string()]];
        let err = validate_unique_ids("test registry", "record", records.iter().map(Vec::as_slice))
            .expect_err("trimmed aliases must be unique");
        assert!(matches!(err, Error::Validation(message) if message.contains("duplicate")));
    }

    #[test]
    fn validate_date_range_strict_accepts_valid_order() {
        let start = crate::dates::Date::from_calendar_date(2024, time::Month::January, 1)
            .expect("valid date");
        let end = crate::dates::Date::from_calendar_date(2024, time::Month::February, 1)
            .expect("valid date");
        assert!(validate_date_range_strict(start, end, "test").is_ok());
    }

    #[test]
    fn validate_date_range_strict_rejects_equal_or_reverse() {
        let start = crate::dates::Date::from_calendar_date(2024, time::Month::January, 1)
            .expect("valid date");
        assert!(validate_date_range_strict(start, start, "test").is_err());
        assert!(validate_date_range_strict_with(start, start, |s, e| format!("{s} {e}")).is_err());
    }

    #[test]
    fn validate_date_range_non_strict_accepts_equal() {
        let start = crate::dates::Date::from_calendar_date(2024, time::Month::January, 1)
            .expect("valid date");
        assert!(validate_date_range_non_strict(start, start, "test").is_ok());
    }

    #[test]
    fn validate_money_checks_finite_gt_and_currency() {
        let money = crate::money::Money::from((1_i64, crate::currency::Currency::USD));
        assert!(validate_money_finite(money, "test").is_ok());
        assert!(validate_money_gt(money, 0.0, "test").is_ok());
        assert!(validate_money_gt(money, 2.0, "test").is_err());
        assert!(validate_money_gt_with(money, 2.0, |amount| format!("{amount}")).is_err());
        assert!(validate_money_currency(money, crate::currency::Currency::USD, "test").is_ok());
        assert!(validate_money_currency(money, crate::currency::Currency::EUR, "test").is_err());
        assert!(validate_distinct_currencies(
            crate::currency::Currency::EUR,
            crate::currency::Currency::USD,
            "pair"
        )
        .is_ok());
        assert!(validate_distinct_currencies(
            crate::currency::Currency::USD,
            crate::currency::Currency::USD,
            "pair"
        )
        .is_err());
    }

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

    #[test]
    fn validate_recovery_rate_rejects_one_and_non_finite() {
        assert!(validate_recovery_rate(0.4).is_ok());
        assert!(validate_recovery_rate(1.0).is_err());
        assert!(validate_recovery_rate(f64::NAN).is_err());
    }

    #[test]
    fn validate_sorted_strict_rejects_unsorted() {
        let d1 = crate::dates::Date::from_calendar_date(2024, time::Month::January, 1)
            .expect("valid date");
        let d2 = crate::dates::Date::from_calendar_date(2024, time::Month::February, 1)
            .expect("valid date");
        let d3 = crate::dates::Date::from_calendar_date(2024, time::Month::January, 15)
            .expect("valid date");
        assert!(validate_sorted_strict(&[d1, d2, d3], "dates").is_err());
        assert!(validate_sorted_strict(&[d1, d3, d2], "dates").is_ok());
    }
}
