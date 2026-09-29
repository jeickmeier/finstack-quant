//! Error types for portfolio operations.

use crate::types::{EntityId, PositionId};
use finstack_quant_core::currency::Currency;
use thiserror::Error;

/// Result type used throughout the portfolio crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during portfolio operations.
#[derive(Debug, Clone, PartialEq, Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Error {
    /// Position references an unknown entity
    #[error("Position '{position_id}' references unknown entity '{entity_id}'")]
    UnknownEntity {
        /// Position identifier.
        position_id: PositionId,
        /// Entity identifier that was not found.
        entity_id: EntityId,
    },

    /// Portfolio validation failed
    #[error("Portfolio validation failed: {0}")]
    ValidationFailed(String),

    /// FX conversion failed
    #[error("FX conversion failed: {from} to {to}")]
    FxConversionFailed {
        /// Source currency.
        from: Currency,
        /// Target currency.
        to: Currency,
    },

    /// Valuation error
    #[error("Valuation error for position '{position_id}': {message}")]
    ValuationError {
        /// Position identifier.
        position_id: PositionId,
        /// Error message describing the valuation failure.
        message: String,
        /// Classification of the failure: the wrapped pricing error's kind, or
        /// the kind of the consistency check that failed.
        kind: finstack_quant_core::error::ErrorKind,
    },

    /// Scenario application error
    #[error("Scenario application failed: {0}")]
    ScenarioError(String),

    /// Missing market data
    #[error("Missing market data: {0}")]
    MissingMarketData(String),

    /// Core error
    #[error(transparent)]
    Core(#[from] finstack_quant_core::Error),

    /// Invalid input data
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// A persisted contract exceeded a configured resource bound.
    #[error("input exceeds limit: {what} {found} > {limit}")]
    ContractLimitExceeded {
        /// Resource whose bound was exceeded.
        what: String,
        /// Observed resource count or byte size.
        found: usize,
        /// Configured maximum count or byte size.
        limit: usize,
    },

    /// Structured portfolio materialization diagnostics.
    #[error("Portfolio materialization failed: {}", .0.summary())]
    MaterializationFailed(Box<finstack_quant_core::contract::ValidationReport>),
}

impl Error {
    /// Create a validation error with context.
    ///
    /// # Arguments
    ///
    /// * `msg` - Human-readable description of the validation failure.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::ValidationFailed(msg.into())
    }

    /// Create a valuation error with context.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position that triggered the valuation failure.
    /// * `kind` - Classification of the failure (see [`Error::kind`]): the
    ///   wrapped pricing error's kind, or the kind of the failed check.
    /// * `msg` - Human-readable error detail.
    pub fn valuation(
        position_id: impl Into<PositionId>,
        kind: finstack_quant_core::error::ErrorKind,
        msg: impl Into<String>,
    ) -> Self {
        Self::ValuationError {
            position_id: position_id.into(),
            message: msg.into(),
            kind,
        }
    }

    /// Classify this error for host-language exception mapping.
    ///
    /// | Kind | Variants |
    /// |------|----------|
    /// | [`ErrorKind::NotFound`] | `UnknownEntity`, `MissingMarketData`, `FxConversionFailed` (no rate for the pair) |
    /// | [`ErrorKind::Validation`] | `ValidationFailed`, `ScenarioError`, `InvalidInput`, `ContractLimitExceeded`, `MaterializationFailed` |
    ///
    /// `Core` keeps its own kind and `ValuationError` carries the kind of the
    /// failure it wraps.
    ///
    /// [`ErrorKind::NotFound`]: finstack_quant_core::error::ErrorKind::NotFound
    /// [`ErrorKind::Validation`]: finstack_quant_core::error::ErrorKind::Validation
    #[must_use]
    pub fn kind(&self) -> finstack_quant_core::error::ErrorKind {
        use finstack_quant_core::error::ErrorKind;
        match self {
            Error::Core(core) => core.kind(),
            Error::ValuationError { kind, .. } => *kind,
            Error::UnknownEntity { .. }
            | Error::MissingMarketData(_)
            | Error::FxConversionFailed { .. } => ErrorKind::NotFound,
            Error::ValidationFailed(_)
            | Error::ScenarioError(_)
            | Error::InvalidInput(_)
            | Error::ContractLimitExceeded { .. }
            | Error::MaterializationFailed(_) => ErrorKind::Validation,
        }
    }

    /// Stable machine-readable code for contract failures, surfaced by the
    /// bindings beside [`Error::kind`]: `"limit_exceeded"` for
    /// `ContractLimitExceeded` and `"report"` for `MaterializationFailed`;
    /// `None` for every other variant.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Error::ContractLimitExceeded { .. } => Some("limit_exceeded"),
            Error::MaterializationFailed(_) => Some("report"),
            _ => None,
        }
    }

    /// Create an invalid input error.
    ///
    /// # Arguments
    ///
    /// * `msg` - Description of the bad caller input.
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a typed persisted-contract resource-limit error.
    ///
    /// # Arguments
    ///
    /// * `what` - Stable resource label such as `"bytes"`, `"artifacts"`, or
    ///   `"positions"`.
    /// * `found` - Observed byte or item count that exceeded the bound.
    /// * `limit` - Configured maximum byte or item count.
    pub fn contract_limit_exceeded(what: impl Into<String>, found: usize, limit: usize) -> Self {
        Self::ContractLimitExceeded {
            what: what.into(),
            found,
            limit,
        }
    }
}

impl From<Error> for finstack_quant_core::Error {
    /// Fold a portfolio error into the core taxonomy, keeping [`Error::kind`]:
    /// lookup misses become `InputError::NotFound`, computation failures
    /// `Internal`, and the rest validation errors with the full message.
    fn from(err: Error) -> Self {
        use finstack_quant_core::error::{ErrorKind, InputError};
        if let Error::Core(core) = err {
            return core;
        }
        let message = err.to_string();
        match err.kind() {
            ErrorKind::NotFound => {
                finstack_quant_core::Error::Input(InputError::NotFound { id: message })
            }
            ErrorKind::Computation => finstack_quant_core::Error::Internal(message),
            ErrorKind::Validation => finstack_quant_core::Error::Validation(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn kind_is_preserved_by_the_core_fold() {
        use finstack_quant_core::error::ErrorKind;
        let cases = [
            (
                Error::FxConversionFailed {
                    from: Currency::USD,
                    to: Currency::EUR,
                },
                ErrorKind::NotFound,
            ),
            (
                Error::UnknownEntity {
                    position_id: "P".into(),
                    entity_id: "E".into(),
                },
                ErrorKind::NotFound,
            ),
            (
                Error::MissingMarketData("USD-OIS".into()),
                ErrorKind::NotFound,
            ),
            (
                Error::valuation("P", ErrorKind::NotFound, "missing curve"),
                ErrorKind::NotFound,
            ),
            (
                Error::valuation("P", ErrorKind::Computation, "solver"),
                ErrorKind::Computation,
            ),
            (Error::validation("bad"), ErrorKind::Validation),
            (Error::ScenarioError("bad".into()), ErrorKind::Validation),
            (Error::invalid_input("bad"), ErrorKind::Validation),
            (
                Error::contract_limit_exceeded("bytes", 2, 1),
                ErrorKind::Validation,
            ),
        ];
        for (error, kind) in cases {
            assert_eq!(error.kind(), kind, "{error}");
            let core = finstack_quant_core::Error::from(error);
            assert_eq!(core.kind(), kind, "{core}");
        }
        assert_eq!(
            Error::contract_limit_exceeded("bytes", 2, 1).code(),
            Some("limit_exceeded")
        );
        assert_eq!(Error::invalid_input("bad").code(), None);
    }
}
