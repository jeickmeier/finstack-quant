//! Generic validation helpers for checking invariants.
//!
//! These helpers are convention-agnostic: they enforce structural invariants
//! (conditions, ordering, finiteness) without encoding market-specific defaults.

/// Require a condition to be true, otherwise return a validation error.
#[inline]
pub fn require(condition: bool, message: impl Into<String>) -> crate::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(crate::Error::Validation(message.into()))
    }
}

/// Require a condition to be true, otherwise return the provided error.
#[inline]
pub fn require_or(condition: bool, err: impl Into<crate::Error>) -> crate::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(err.into())
    }
}

/// Require a condition to be true, lazily constructing the error message.
#[inline]
pub fn require_with(condition: bool, message: impl FnOnce() -> String) -> crate::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(crate::Error::Validation(message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, InputError};

    #[test]
    fn require_returns_validation_error_when_condition_is_false() {
        assert!(require(true, "ok").is_ok());

        let err = require(false, "missing invariant").expect_err("false condition should fail");
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
}
