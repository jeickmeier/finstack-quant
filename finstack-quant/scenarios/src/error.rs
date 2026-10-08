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
}

impl Error {
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
}

impl Error {
    /// Classify this error for host-language exception mapping; agrees with
    /// the `From<Error> for finstack_quant_core::Error` fold.
    ///
    /// Wrapped `Core`, `Statements` and `Valuations` errors keep their own
    /// kind. `MarketDataNotFound`, `NodeNotFound`, `TenorNotFound` are not-found errors, `Internal` a computation
    /// error, and every other variant a validation error.
    #[must_use]
    pub fn kind(&self) -> finstack_quant_core::error::ErrorKind {
        use finstack_quant_core::error::ErrorKind;
        match self {
            Error::Core(error) => error.kind(),
            Error::Statements(error) => error.kind(),
            Error::Valuations(error) => error.kind(),
            Error::MarketDataNotFound { .. }
            | Error::NodeNotFound { .. }
            | Error::TenorNotFound { .. } => ErrorKind::NotFound,
            Error::Internal(_) => ErrorKind::Computation,
            Error::MissingStatementModel { .. }
            | Error::Validation(_)
            | Error::InvalidTenor(_)
            | Error::InvalidPeriod(_) => ErrorKind::Validation,
        }
    }
}

impl From<Error> for finstack_quant_core::Error {
    /// Lookup misses (`MarketDataNotFound`, `NodeNotFound`, `TenorNotFound`) map to core's `InputError::NotFound` so they keep
    /// `ErrorKind::NotFound` (and therefore `KeyError` in Python); `Internal`
    /// stays `Internal`; everything else becomes a validation error.
    fn from(err: Error) -> Self {
        use finstack_quant_core::error::InputError;
        match err {
            Error::Core(core) => core,
            Error::Statements(statements) => statements.into(),
            Error::Valuations(valuations) => valuations.into(),
            Error::Internal(message) => finstack_quant_core::Error::Internal(message),
            Error::MarketDataNotFound { id } => {
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
        let core: finstack_quant_core::Error = Error::InvalidPeriod("1X".into()).into();
        assert!(matches!(core, finstack_quant_core::Error::Validation(_)));
    }

    #[test]
    fn lookup_misses_keep_not_found_kind() {
        use finstack_quant_core::error::ErrorKind;
        for err in [
            Error::MarketDataNotFound {
                id: "USD-OIS".into(),
            },
            Error::NodeNotFound {
                node_id: "Revenue".into(),
            },
            Error::TenorNotFound {
                tenor: "7Y".into(),
                curve_id: "USD-OIS".into(),
            },
        ] {
            let core: finstack_quant_core::Error = err.into();
            assert_eq!(core.kind(), ErrorKind::NotFound, "{core}");
        }
        let internal: finstack_quant_core::Error = Error::internal("boom").into();
        assert!(matches!(internal, finstack_quant_core::Error::Internal(_)));
    }
}
