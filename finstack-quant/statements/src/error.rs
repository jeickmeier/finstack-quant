//! Error types for the statements crate.
//!
//! Uses a flat `error.rs` layout per the project convention — see
//! `docs/CONVENTIONS_ERROR_NAMING.md` for the module layout and naming rules.

use thiserror::Error;

/// Result type alias for statements operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Comprehensive error type for statements operations.
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
    /// Model building error (e.g., invalid period range, missing periods)
    #[error("Model build error: {0}")]
    Build(String),

    /// Formula parsing error
    #[error("Formula parse error: {0}")]
    FormulaParse(String),

    /// Evaluation error (e.g., circular dependency, undefined node)
    #[error("Evaluation error: {0}")]
    Eval(String),

    /// Node not found in model
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// Circular dependency detected
    #[error("Circular dependency detected: {}", format_path(.0))]
    CircularDependency(Vec<String>),

    /// Currency mismatch error
    #[error("Currency mismatch: expected {0}, found {1}")]
    CurrencyMismatch(
        finstack_quant_core::currency::Currency,
        finstack_quant_core::currency::Currency,
    ),

    /// Period validation error
    #[error("Period error: {0}")]
    Period(String),

    /// Missing required data
    #[error("Missing required data: {0}")]
    MissingData(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Forecast method error
    #[error("Forecast error: {0}")]
    Forecast(String),

    /// Registry error
    #[error("Registry error: {0}")]
    Registry(String),

    /// Capital structure error
    #[error("Capital structure error: {0}")]
    CapitalStructure(String),

    /// Serialization/deserialization error (stored as message string for serde compatibility).
    #[error("Serialization error: {0}")]
    Serde(String),

    /// Core crate error
    #[error(transparent)]
    Core(#[from] finstack_quant_core::Error),

    /// I/O error (stored as message string for serde compatibility).
    #[error("I/O error: {0}")]
    Io(String),

    /// Builder construction error
    #[error("Builder error: {0}")]
    BuilderError(String),

    /// Index/collection access error
    #[error("Index error: {0}")]
    IndexError(String),
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serde(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

// Helper function to format circular dependency path
fn format_path(path: &[String]) -> String {
    path.join(" → ")
}

impl Error {
    /// Create a build error with context
    pub fn build(msg: impl Into<String>) -> Self {
        Self::Build(msg.into())
    }

    /// Create an evaluation error with context
    pub fn eval(msg: impl Into<String>) -> Self {
        Self::Eval(msg.into())
    }

    /// Create a formula parse error
    pub fn formula_parse(msg: impl Into<String>) -> Self {
        Self::FormulaParse(msg.into())
    }

    /// Create a period error
    pub fn period(msg: impl Into<String>) -> Self {
        Self::Period(msg.into())
    }

    /// Create a missing data error
    pub fn missing_data(msg: impl Into<String>) -> Self {
        Self::MissingData(msg.into())
    }

    /// Create an invalid input error
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a registry error with context
    pub fn registry(msg: impl Into<String>) -> Self {
        Self::Registry(msg.into())
    }

    /// Create a forecast error with context
    pub fn forecast(msg: impl Into<String>) -> Self {
        Self::Forecast(msg.into())
    }

    /// Create a node not found error
    pub fn node_not_found(node_id: impl Into<String>) -> Self {
        Self::NodeNotFound(node_id.into())
    }

    /// Create a circular dependency error
    pub fn circular_dependency(path: Vec<String>) -> Self {
        Self::CircularDependency(path)
    }

    /// Create a currency mismatch error
    pub fn currency_mismatch(
        expected: finstack_quant_core::currency::Currency,
        found: finstack_quant_core::currency::Currency,
    ) -> Self {
        Self::CurrencyMismatch(expected, found)
    }

    /// Create a capital structure error
    pub fn capital_structure(msg: impl Into<String>) -> Self {
        Self::CapitalStructure(msg.into())
    }

    /// Create a builder error
    pub fn builder_error(msg: impl Into<String>) -> Self {
        Self::BuilderError(msg.into())
    }

    /// Create an index error
    pub fn index_error(msg: impl Into<String>) -> Self {
        Self::IndexError(msg.into())
    }
}

impl From<Error> for finstack_quant_core::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Core(core) => core,
            Error::CurrencyMismatch(expected, actual) => {
                finstack_quant_core::Error::CurrencyMismatch { expected, actual }
            }
            Error::Io(message) | Error::Serde(message) => {
                finstack_quant_core::Error::Internal(message)
            }
            other => finstack_quant_core::Error::Validation(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn converts_statements_errors_to_core_error() {
        let core: finstack_quant_core::Error = Error::invalid_input("bad assumptions").into();
        assert!(matches!(core, finstack_quant_core::Error::Validation(_)));
    }
}
