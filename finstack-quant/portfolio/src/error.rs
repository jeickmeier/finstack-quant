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
    #[error("Portfolio materialization failed: {0:?}")]
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
    /// * `msg` - Human-readable error detail.
    pub fn valuation(position_id: impl Into<PositionId>, msg: impl Into<String>) -> Self {
        Self::ValuationError {
            position_id: position_id.into(),
            message: msg.into(),
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
    fn from(err: Error) -> Self {
        match err {
            Error::Core(core) => core,
            Error::FxConversionFailed { from, to } => finstack_quant_core::Error::Validation(
                format!("FX conversion failed: {from} to {to}"),
            ),
            other => finstack_quant_core::Error::Validation(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn converts_portfolio_errors_to_core_error() {
        let core: finstack_quant_core::Error = Error::FxConversionFailed {
            from: Currency::USD,
            to: Currency::EUR,
        }
        .into();
        assert!(matches!(core, finstack_quant_core::Error::Validation(_)));
    }
}
