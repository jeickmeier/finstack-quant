//! Unified error hierarchy for the valuations crate.
//!
//! This module defines a single [`Error`] enum that wraps all domain-specific
//! error types, following the same pattern as [`finstack_core::Error`] wrapping
//! [`finstack_core::InputError`].
//!
//! # Design
//!
//! ```text
//! valuations::Error
//! ├── Core(finstack_core::Error)         ← propagated core errors
//! ├── Pricing(PricingError)              ← pricer registry, model failures
//! ├── Correlation(correlation::Error)    ← factor model validation
//! └── WaterfallValidation(ValidationError) ← structured credit waterfall
//! ```
//!
//! All variants convert one-way into [`finstack_core::Error`] via [`From`] for
//! integration with the core error hierarchy.  The `Core` variant
//! enables ergonomic `?` propagation from core functions, consistent with
//! all other Finstack crates (portfolio, statements, scenarios, io).
//! For richer context when mapping core errors inside pricers, see
//! [`crate::pricer::PricingError::from_core`].
//!
//! # Naming Convention
//!
//! Sub-errors use `{Domain}Error` prefixes (`PricingError`,
//! `ValidationError`) so they can be imported
//! alongside `finstack_core::Error` without ambiguity. The unified wrapper
//! is re-exported at crate root as `Error` (matching the standard convention).
//!
//! # Module Layout Convention
//!
//! The error module layout across Finstack crates follows this convention:
//!
//! - **`error/mod.rs`** (directory with submodules): Use when the crate defines
//!   its own rich error hierarchy with multiple enums and helper logic
//!   (e.g., `finstack_core::error` has `Error`, `InputError`, suggestions).
//! - **`error.rs`** (flat file): Use for re-export facades that aggregate errors
//!   defined elsewhere in the crate (this module re-exports `PricingError`,
//!   `ValidationError` from their source modules).
//!
//! # Examples
//!
//! ```ignore
//! use finstack_valuations::error::{Error, PricingError, ValidationError};
//! use finstack_valuations::pricer::InstrumentType;
//!
//! // Domain errors automatically wrap into the unified type
//! let pricing_err: Error = PricingError::type_mismatch(
//!     InstrumentType::Bond,
//!     InstrumentType::Deposit,
//! ).into();
//!
//! // And the unified type converts into finstack_core::Error
//! let core_err: finstack_core::Error = pricing_err.into();
//! ```

pub use crate::instruments::fixed_income::structured_credit::utils::validation::ValidationError;
pub use crate::pricer::{PricingError, PricingErrorContext};
pub use finstack_factor_model::DecompositionError;

/// Unified error type for the valuations crate.
///
/// Wraps domain-specific error types so callers can handle any valuations
/// error through a single type, consistent with [`finstack_core::Error`]
/// wrapping [`finstack_core::InputError`].
///
/// Each variant uses `#[error(transparent)]` to delegate `Display` to the
/// inner error and `#[from]` for ergonomic `?` conversion.
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum Error {
    /// Core library error, enabling `?` propagation from [`finstack_core`] calls.
    ///
    /// Consistent with the `Core` variant in `portfolio`, `statements`,
    /// `scenarios`, and `io` error enums.
    #[error(transparent)]
    Core(#[from] finstack_core::Error),

    /// Pricing model or registry error.
    #[error(transparent)]
    Pricing(#[from] PricingError),

    /// Correlation matrix validation error (factor model).
    #[error(transparent)]
    Correlation(#[from] crate::correlation::Error),

    /// Structured credit waterfall validation error.
    #[error(transparent)]
    WaterfallValidation(#[from] ValidationError),

    /// Credit hierarchy decomposition error.
    #[error(transparent)]
    Decomposition(#[from] DecompositionError),
}

/// Convenience result type used throughout the valuations crate.
pub type Result<T> = std::result::Result<T, Error>;

/// One-way conversion from [`Error`] into [`finstack_core::Error`].
///
/// | `valuations::Error`       | `finstack_core::Error`          |
/// |---------------------------|---------------------------------|
/// | `Core(e)`                 | Pass-through                    |
/// | `Pricing(e)`              | Delegates to `From<PricingError>`|
/// | `Correlation(e)`          | `Validation(e.to_string())`     |
/// | `WaterfallValidation(e)`  | `Validation(e.to_string())`     |
impl From<Error> for finstack_core::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Core(e) => e,
            Error::Pricing(e) => e.into(),
            Error::Correlation(e) => finstack_core::Error::Validation(e.to_string()),
            Error::WaterfallValidation(e) => finstack_core::Error::Validation(e.to_string()),
            Error::Decomposition(e) => finstack_core::Error::Validation(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_error_wraps_into_unified() {
        let core = finstack_core::Error::Validation("test validation".into());
        let unified: Error = core.clone().into();
        assert!(matches!(
            unified,
            Error::Core(finstack_core::Error::Validation(_))
        ));

        // Round-trip back to core should pass through unchanged
        let back: finstack_core::Error = unified.into();
        assert_eq!(back, core);
    }

    #[test]
    fn core_error_propagates_with_question_mark() {
        fn inner() -> std::result::Result<(), finstack_core::Error> {
            Err(finstack_core::Error::Validation("inner failure".into()))
        }

        fn outer() -> std::result::Result<(), Error> {
            inner()?; // This requires From<finstack_core::Error> for Error
            Ok(())
        }

        let err = outer().expect_err("outer() should return an error when inner() fails");
        assert!(matches!(
            err,
            Error::Core(finstack_core::Error::Validation(_))
        ));
    }

    #[test]
    fn pricing_error_wraps_into_unified() {
        let pricing = PricingError::type_mismatch(
            crate::pricer::InstrumentType::Bond,
            crate::pricer::InstrumentType::Deposit,
        );
        let unified: Error = pricing.into();
        assert!(matches!(
            unified,
            Error::Pricing(PricingError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn correlation_error_wraps_into_unified() {
        let corr = crate::correlation::Error::InvalidSize {
            expected: 3,
            actual: 5,
        };
        let unified: Error = corr.into();
        assert!(matches!(
            unified,
            Error::Correlation(crate::correlation::Error::InvalidSize { .. })
        ));
    }

    #[test]
    fn validation_error_wraps_into_unified() {
        let val = ValidationError::DuplicateTierId {
            tier_id: "A".into(),
        };
        let unified: Error = val.into();
        assert!(matches!(
            unified,
            Error::WaterfallValidation(ValidationError::DuplicateTierId { .. })
        ));
    }

    #[test]
    fn unified_converts_to_core_error() {
        // Pricing -> core
        let pricing = PricingError::model_failure_with_context(
            "test failure",
            PricingErrorContext::default(),
        );
        let core_err: finstack_core::Error = Error::Pricing(pricing).into();
        assert!(matches!(core_err, finstack_core::Error::Calibration { .. }));

        // Correlation -> core
        let corr = crate::correlation::Error::NotSymmetric {
            i: 0,
            j: 1,
            diff: 0.01,
        };
        let core_err: finstack_core::Error = Error::Correlation(corr).into();
        assert!(matches!(core_err, finstack_core::Error::Validation(_)));

        // Validation -> core
        let val = ValidationError::EmptyTier {
            tier_id: "B".into(),
        };
        let core_err: finstack_core::Error = Error::WaterfallValidation(val).into();
        assert!(matches!(core_err, finstack_core::Error::Validation(_)));
    }
}
