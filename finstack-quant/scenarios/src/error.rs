//! Errors emitted by the scenarios crate.

use thiserror::Error;

/// Result alias used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during scenario execution.
///
/// # Examples
/// ```rust
/// use finstack_quant_scenarios::error::Error;
///
/// fn classify(err: Error) -> &'static str {
///     match err {
///         Error::MarketDataNotFound { .. } => "market",
///         Error::NodeNotFound { .. } => "statements",
///         _ => "other",
///     }
/// }
///
/// assert_eq!(classify(Error::NodeNotFound { node_id: "Revenue".into() }), "statements");
/// ```
#[derive(Debug, Clone, PartialEq, Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Error {
    /// Market data element not found.
    #[error("Market data not found: {id}")]
    MarketDataNotFound {
        /// Identifier of the missing market data element.
        id: String,
    },

    /// Statement/model node not found (matches [`finstack_quant_statements::error::Error::NodeNotFound`] wording).
    #[error("Node not found: {node_id}")]
    NodeNotFound {
        /// Identifier of the missing statement node.
        node_id: String,
    },

    /// A statement operation was requested without a statement model in the execution context.
    #[error("Statement model required for scenario operation '{operation}'")]
    MissingStatementModel {
        /// Scenario operation that needs a statement model.
        operation: String,
    },

    /// Curve type mismatch.
    #[error("Curve type mismatch: expected {expected}, got {actual}")]
    CurveTypeMismatch {
        /// Expected curve type.
        expected: String,
        /// Actual curve type encountered.
        actual: String,
    },

    /// Unsupported operation for target.
    #[error("Unsupported operation {operation} for target {target}")]
    UnsupportedOperation {
        /// Operation being attempted.
        operation: String,
        /// Target on which the operation is unsupported.
        target: String,
    },

    /// Core library error.
    #[error(transparent)]
    Core(#[from] finstack_quant_core::Error),

    /// Statements library error.
    #[error(transparent)]
    Statements(#[from] finstack_quant_statements::error::Error),

    /// Valuations library error.
    #[error(transparent)]
    Valuations(#[from] finstack_quant_valuations::Error),

    /// General validation error.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Invalid tenor string.
    #[error("Invalid tenor string: {0}")]
    InvalidTenor(String),

    /// Tenor not found in curve.
    #[error("Tenor not found in curve: {tenor} in {curve_id}")]
    TenorNotFound {
        /// Tenor string that was not found.
        tenor: String,
        /// Identifier of the curve.
        curve_id: String,
    },

    /// Invalid time period.
    #[error("Invalid time period: {0}")]
    InvalidPeriod(String),

    /// Instrument not found.
    #[error("Instrument not found: {0}")]
    InstrumentNotFound(String),
}

impl Error {
    /// Create a market data not found error.
    ///
    /// # Arguments
    ///
    /// - `id`: Identifier of the missing market data object.
    pub fn market_data_not_found(id: impl Into<String>) -> Self {
        Self::MarketDataNotFound { id: id.into() }
    }

    /// Create a node not found error.
    ///
    /// # Arguments
    ///
    /// - `node_id`: Identifier of the missing statement node.
    pub fn node_not_found(node_id: impl Into<String>) -> Self {
        Self::NodeNotFound {
            node_id: node_id.into(),
        }
    }

    /// Create an error for statement operations without a model in the execution context.
    ///
    /// # Arguments
    ///
    /// - `operation`: Operation that requires a statement model.
    pub fn missing_statement_model(operation: impl Into<String>) -> Self {
        Self::MissingStatementModel {
            operation: operation.into(),
        }
    }

    /// Create a curve type mismatch error.
    ///
    /// # Arguments
    ///
    /// - `expected`: Curve type expected by the caller.
    /// - `actual`: Curve type that was encountered.
    pub fn curve_type_mismatch(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self::CurveTypeMismatch {
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Create an unsupported operation error.
    ///
    /// # Arguments
    ///
    /// - `operation`: Operation that was attempted.
    /// - `target`: Target object that rejected the operation.
    pub fn unsupported_operation(operation: impl Into<String>, target: impl Into<String>) -> Self {
        Self::UnsupportedOperation {
            operation: operation.into(),
            target: target.into(),
        }
    }

    /// Create a validation error.
    ///
    /// # Arguments
    ///
    /// - `msg`: Human-readable validation message.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    /// Create an internal error.
    ///
    /// # Arguments
    ///
    /// - `msg`: Human-readable internal error message.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Create an invalid tenor error.
    ///
    /// # Arguments
    ///
    /// - `tenor`: Tenor string that failed validation or parsing.
    pub fn invalid_tenor(tenor: impl Into<String>) -> Self {
        Self::InvalidTenor(tenor.into())
    }

    /// Create a tenor not found error.
    ///
    /// # Arguments
    ///
    /// - `tenor`: Tenor string that could not be matched.
    /// - `curve_id`: Curve identifier on which the tenor lookup failed.
    pub fn tenor_not_found(tenor: impl Into<String>, curve_id: impl Into<String>) -> Self {
        Self::TenorNotFound {
            tenor: tenor.into(),
            curve_id: curve_id.into(),
        }
    }

    /// Create an invalid period error.
    ///
    /// # Arguments
    ///
    /// - `period`: Period string that failed validation or parsing.
    pub fn invalid_period(period: impl Into<String>) -> Self {
        Self::InvalidPeriod(period.into())
    }

    /// Create an instrument not found error.
    ///
    /// # Arguments
    ///
    /// - `instrument`: Instrument identifier that could not be found.
    pub fn instrument_not_found(instrument: impl Into<String>) -> Self {
        Self::InstrumentNotFound(instrument.into())
    }
}

impl From<Error> for finstack_quant_core::Error {
    /// Lookup misses (`MarketDataNotFound`, `NodeNotFound`, `TenorNotFound`,
    /// `InstrumentNotFound`) map to core's `InputError::NotFound` so they keep
    /// `ErrorKind::NotFound` (and therefore `KeyError` in Python); `Internal`
    /// stays `Internal`; everything else becomes a validation error.
    fn from(err: Error) -> Self {
        use finstack_quant_core::error::InputError;
        match err {
            Error::Core(core) => core,
            Error::Statements(statements) => statements.into(),
            Error::Valuations(valuations) => valuations.into(),
            Error::Internal(message) => finstack_quant_core::Error::Internal(message),
            Error::MarketDataNotFound { id } | Error::InstrumentNotFound(id) => {
                finstack_quant_core::Error::Input(InputError::NotFound { id })
            }
            Error::NodeNotFound { node_id } => {
                finstack_quant_core::Error::Input(InputError::NotFound { id: node_id })
            }
            Error::TenorNotFound { tenor, curve_id } => {
                finstack_quant_core::Error::Input(InputError::NotFound {
                    id: format!("{tenor} in {curve_id}"),
                })
            }
            other => finstack_quant_core::Error::Validation(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn converts_scenarios_errors_to_core_error() {
        let core: finstack_quant_core::Error = Error::invalid_period("1X").into();
        assert!(matches!(core, finstack_quant_core::Error::Validation(_)));
    }

    #[test]
    fn lookup_misses_keep_not_found_kind() {
        use finstack_quant_core::error::ErrorKind;
        for err in [
            Error::market_data_not_found("USD-OIS"),
            Error::node_not_found("Revenue"),
            Error::tenor_not_found("7Y", "USD-OIS"),
            Error::instrument_not_found("BOND-1"),
        ] {
            let core: finstack_quant_core::Error = err.into();
            assert_eq!(core.kind(), ErrorKind::NotFound, "{core}");
        }
        let internal: finstack_quant_core::Error = Error::internal("boom").into();
        assert!(matches!(internal, finstack_quant_core::Error::Internal(_)));
    }
}
