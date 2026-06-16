//! Error types for portfolio operations.

use crate::types::{EntityId, PositionId};
use finstack_quant_core::currency::Currency;
use thiserror::Error;

/// Convenience result type used throughout the portfolio crate.
///
/// This alias helps reduce boilerplate when returning [`enum@Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during portfolio operations.
///
/// Each variant captures the context needed to diagnose failures when building,
/// validating, or valuing a portfolio.
///
/// # Derive policy
///
/// All Finstack Quant domain error types that may cross FFI boundaries (Python/WASM)
/// derive `Serialize`/`Deserialize`. `PartialEq` is included for ergonomic
/// assertions in tests. Infrastructure errors that wrap opaque driver types
/// may opt out of `Serialize` and `PartialEq`.
#[derive(Debug, Clone, PartialEq, Error, serde::Serialize, serde::Deserialize)]
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

    /// Optimization error
    #[error("Optimization error: {0}")]
    OptimizationError(String),

    /// Core error
    #[error(transparent)]
    Core(#[from] finstack_quant_core::Error),

    /// Liquidity calculation error
    #[error("Liquidity error: {0}")]
    LiquidityError(String),

    /// Invalid input data
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl Error {
    /// Create a validation error with context.
    ///
    /// # Arguments
    ///
    /// * `msg` - Human-readable description of the validation failure.
    ///
    /// # Returns
    ///
    /// [`Error::ValidationFailed`] carrying the supplied message.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::ValidationFailed(msg.into())
    }

    /// Create a valuation error with context.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position that triggered the valuation failure.
    /// * `msg` - Human-readable error detail.
    ///
    /// # Returns
    ///
    /// [`Error::ValuationError`] carrying position context and the supplied message.
    pub fn valuation(position_id: impl Into<PositionId>, msg: impl Into<String>) -> Self {
        Self::ValuationError {
            position_id: position_id.into(),
            message: msg.into(),
        }
    }

    /// Create an FX conversion error.
    ///
    /// # Arguments
    ///
    /// * `from` - Source currency.
    /// * `to` - Target currency.
    ///
    /// # Returns
    ///
    /// [`Error::FxConversionFailed`] for the requested currency pair.
    pub fn fx_conversion(from: Currency, to: Currency) -> Self {
        Self::FxConversionFailed { from, to }
    }

    /// Create a missing market data error.
    ///
    /// # Arguments
    ///
    /// * `msg` - Description of the missing market input.
    ///
    /// # Returns
    ///
    /// [`Error::MissingMarketData`] carrying the supplied message.
    pub fn missing_market_data(msg: impl Into<String>) -> Self {
        Self::MissingMarketData(msg.into())
    }

    /// Create an optimization error with context.
    ///
    /// # Arguments
    ///
    /// * `msg` - Description of the optimization failure.
    ///
    /// # Returns
    ///
    /// [`Error::OptimizationError`] carrying the supplied message.
    pub fn optimization_error(msg: impl Into<String>) -> Self {
        Self::OptimizationError(msg.into())
    }

    /// Create a liquidity error with context.
    ///
    /// # Arguments
    ///
    /// * `msg` - Description of the liquidity calculation failure.
    ///
    /// # Returns
    ///
    /// [`Error::LiquidityError`] carrying the supplied message.
    pub fn liquidity(msg: impl Into<String>) -> Self {
        Self::LiquidityError(msg.into())
    }

    /// Create an invalid input error.
    ///
    /// # Arguments
    ///
    /// * `msg` - Description of the bad caller input.
    ///
    /// # Returns
    ///
    /// [`Error::InvalidInput`] carrying the supplied message.
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a scenario error.
    ///
    /// # Arguments
    ///
    /// * `msg` - Description of the scenario-engine failure.
    ///
    /// # Returns
    ///
    /// [`Error::ScenarioError`] carrying the supplied message.
    pub fn scenario_error(msg: impl Into<String>) -> Self {
        Self::ScenarioError(msg.into())
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
        let core: finstack_quant_core::Error =
            Error::fx_conversion(Currency::USD, Currency::EUR).into();
        assert!(matches!(core, finstack_quant_core::Error::Validation(_)));
    }
}
