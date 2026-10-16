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
        assert!(require_or(true, InputError::Invalid).is_ok());

        let err = require_or(false, InputError::Invalid).expect_err("false condition should fail");
        assert!(matches!(err, Error::Input(InputError::Invalid)));
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
}
