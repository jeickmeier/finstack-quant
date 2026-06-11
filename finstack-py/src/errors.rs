//! Centralized error mapping from Rust crate errors to Python exceptions.
//!
//! All binding callers should route error conversions through [`core_to_py`]
//! (for `finstack_core::Error`) or [`display_to_py`] (for any `Display`-able
//! error). Avoid inline `PyValueError::new_err(e.to_string())` patterns —
//! they bypass this module and break the error-chain-preservation contract
//! the helpers below provide.

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;

pyo3::create_exception!(
    finstack.analytics,
    AnalyticsError,
    PyValueError,
    "Analytics validation or calculation failure (inherits ValueError)."
);

pyo3::create_exception!(
    finstack.portfolio,
    PortfolioError,
    PyValueError,
    "Portfolio validation or calculation failure (inherits ValueError)."
);

pyo3::create_exception!(
    finstack.portfolio,
    FinstackValuationError,
    PortfolioError,
    "Portfolio valuation failure (inherits PortfolioError)."
);

pyo3::create_exception!(
    finstack.portfolio,
    FinstackFxError,
    PortfolioError,
    "Portfolio FX conversion or market-data failure (inherits PortfolioError)."
);

pyo3::create_exception!(
    finstack.portfolio,
    FinstackOptimizationError,
    PortfolioError,
    "Portfolio optimization failure (inherits PortfolioError)."
);

/// Format an error and its full `source()` chain into a single string.
///
/// PyO3 exceptions only carry a message, not a structured cause chain, so we
/// concatenate every level of context (`top: first cause: deeper cause: …`)
/// into the message. This mirrors `anyhow`'s default display and lets quants
/// see the full diagnostic context in Python without dropping back to Rust.
fn format_chain(err: &dyn std::error::Error) -> String {
    let mut out = err.to_string();
    let mut src = err.source();
    while let Some(cause) = src {
        // De-duplicate when a wrapper already included the inner display.
        let msg = cause.to_string();
        if !out.ends_with(&msg) {
            out.push_str(": ");
            out.push_str(&msg);
        }
        src = cause.source();
    }
    out
}

/// Convert a `finstack_core::Error` into a Python exception.
///
/// The full error source chain (via [`std::error::Error::source`]) is flattened
/// into the Python message. This preserves context from wrappers like
/// `finstack_valuations::Error::Calibration(core_err)`.
///
/// Exception mapping:
/// - Missing id/lookup failures (`InputError::{MissingCurve, NotFound,
///   CalendarNotFound, FxTriangulationFailed}`) → `KeyError`
/// - Calibration, solver, and operational failures (`Error::{Calibration,
///   Internal}`, `InputError::SolverConvergenceFailed`) → `RuntimeError`
/// - Everything else → `ValueError`
pub fn core_to_py(e: finstack_core::Error) -> PyErr {
    use finstack_core::error::InputError;
    use finstack_core::Error;

    let message = format_chain(&e);
    match &e {
        Error::Input(
            InputError::MissingCurve { .. }
            | InputError::NotFound { .. }
            | InputError::CalendarNotFound { .. }
            | InputError::FxTriangulationFailed { .. },
        ) => PyKeyError::new_err(message),
        Error::Calibration { .. }
        | Error::Internal(_)
        | Error::Input(InputError::SolverConvergenceFailed { .. }) => {
            PyRuntimeError::new_err(message)
        }
        _ => PyValueError::new_err(message),
    }
}

/// Convert a `PdCalibrationError` into a Python exception.
///
/// Mirrors [`core_to_py`]: unknown-rating lookups raise `KeyError`, all other
/// (validation) failures raise `ValueError`.
pub fn pd_calibration_to_py(e: finstack_core::credit::pd::PdCalibrationError) -> PyErr {
    use finstack_core::credit::pd::PdCalibrationError as E;
    let message = format_chain(&e);
    match &e {
        E::UnknownRating { .. } => PyKeyError::new_err(message),
        _ => PyValueError::new_err(message),
    }
}

/// Convert a `MigrationError` into a Python exception.
///
/// Mirrors [`core_to_py`]: label/state lookup misses raise `KeyError`,
/// numerical/operational failures raise `RuntimeError`, validation failures
/// raise `ValueError`.
pub fn migration_to_py(e: finstack_core::credit::migration::MigrationError) -> PyErr {
    use finstack_core::credit::migration::MigrationError as E;
    let message = format_chain(&e);
    match &e {
        E::UnknownState { .. } | E::NoWarfFactor { .. } => PyKeyError::new_err(message),
        E::NoValidGenerator { .. }
        | E::ComplexEigenvalues
        | E::RoundTripError { .. }
        | E::SingularMatrix
        | E::Internal(_) => PyRuntimeError::new_err(message),
        _ => PyValueError::new_err(message),
    }
}

/// Convert an analytics-domain core error into a Python `AnalyticsError`.
///
/// `AnalyticsError` inherits from `ValueError`, so existing callers catching
/// `ValueError` remain compatible while analytics users can opt into a narrower
/// exception type.
pub fn analytics_to_py(e: finstack_core::Error) -> PyErr {
    AnalyticsError::new_err(format_chain(&e))
}

/// Convert a `finstack_portfolio::Error` into a portfolio-domain Python exception.
///
/// The named subclasses preserve compatibility with callers catching
/// `ValueError` or `PortfolioError`, while letting portfolio users distinguish
/// valuation, FX/market-data, and optimization failures.
pub fn portfolio_to_py(e: finstack_portfolio::Error) -> PyErr {
    match e {
        finstack_portfolio::Error::Core(core) => core_to_py(core),
        err @ finstack_portfolio::Error::ValuationError { .. } => {
            FinstackValuationError::new_err(format_chain(&err))
        }
        err @ (finstack_portfolio::Error::FxConversionFailed { .. }
        | finstack_portfolio::Error::MissingMarketData(_)) => {
            FinstackFxError::new_err(format_chain(&err))
        }
        err @ finstack_portfolio::Error::OptimizationError(_) => {
            FinstackOptimizationError::new_err(format_chain(&err))
        }
        err => PortfolioError::new_err(format_chain(&err)),
    }
}

/// Convert any `Display`-able error into a Python `ValueError`.
///
/// If the error implements `std::error::Error`, the full source chain is
/// flattened into the message; otherwise only the top-level `Display`
/// string is used.
pub fn display_to_py<E>(e: E) -> PyErr
where
    E: std::fmt::Display,
{
    // We can't generically detect `std::error::Error` without specialization,
    // so callers with rich chains should prefer `core_to_py` or the helpers
    // that accept `&dyn Error`. This path still beats the inline
    // `PyValueError::new_err(e.to_string())` pattern it replaces because it
    // routes through one place.
    PyValueError::new_err(e.to_string())
}

/// Construct a Python `ValueError` through the centralized binding error path.
pub fn value_error(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(message.into())
}

/// Convert a `serde_json::Error` into a Python `ValueError`, with a caller-
/// supplied context prefix that names what was being (de)serialized.
///
/// Use at every `serde_json::{to_string, from_str, to_value, from_value}`
/// boundary in bindings so the resulting Python messages stay consistent:
///
/// ```ignore
/// serde_json::from_str(json).map_err(|err| serde_json_to_py(err, "invalid config JSON"))?;
/// ```
pub fn serde_json_to_py(err: serde_json::Error, context: &str) -> PyErr {
    PyValueError::new_err(format!("{context}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::fmt;

    #[derive(Debug)]
    struct Wrapper(Box<dyn Error + Send + Sync>);
    impl fmt::Display for Wrapper {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "calibration failed")
        }
    }
    impl Error for Wrapper {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&*self.0)
        }
    }

    #[derive(Debug)]
    struct Leaf;
    impl fmt::Display for Leaf {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "solver diverged after 1000 iterations")
        }
    }
    impl Error for Leaf {}

    #[test]
    fn format_chain_flattens_sources() {
        let err = Wrapper(Box::new(Leaf));
        let formatted = format_chain(&err);
        assert_eq!(
            formatted,
            "calibration failed: solver diverged after 1000 iterations"
        );
    }

    #[test]
    fn format_chain_avoids_duplication() {
        // If a wrapper already includes the inner message verbatim at the end,
        // we don't append it again.
        #[derive(Debug)]
        struct Echo(Box<dyn Error + Send + Sync>);
        impl fmt::Display for Echo {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "outer: {}", self.0)
            }
        }
        impl Error for Echo {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(&*self.0)
            }
        }

        let err = Echo(Box::new(Leaf));
        let formatted = format_chain(&err);
        assert_eq!(formatted, "outer: solver diverged after 1000 iterations");
    }
}
