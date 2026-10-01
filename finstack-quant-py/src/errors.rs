//! Centralized error mapping from Rust crate errors to Python exceptions.
//!
//! All binding callers should route error conversions through [`core_to_py`]
//! (for `finstack_quant_core::Error`) or [`display_to_py`] (for any `Display`-able
//! error). Avoid inline `PyValueError::new_err(e.to_string())` patterns —
//! they bypass this module and break the error-chain-preservation contract
//! the helpers below provide.
//!
//! # Exception hierarchy
//!
//! [`FinstackError`] is the common base for the library's named exceptions, so
//! `except FinstackError` catches all but the one carve-out noted below:
//!
//! ```text
//! ValueError
//! └── FinstackError
//!     ├── AnalyticsError
//!     ├── CholeskyError
//!     ├── PortfolioError
//!     └── ContractValidationError
//!         ├── UnsupportedContractVersionError
//!         ├── MissingContractVersionError
//!         ├── MalformedContractSchemaError
//!         └── ContractLimitExceededError
//! ```
//!
//! `FinstackError` derives from `ValueError` rather than `Exception` so that
//! inserting it above the pre-existing classes cannot break callers: every
//! reparented type keeps `ValueError` in its MRO, so `except ValueError` still
//! catches everything it caught before this base was introduced.
//!
//! `pyo3::create_exception!` accepts exactly one base type (it forwards a single
//! `&Bound<'_, PyType>` to `PyErr::new_type`), so a class cannot derive from both
//! `FinstackError` and an unrelated builtin. One named exception therefore stays
//! outside this tree:
//!
//! - `CalibrationEnvelopeError` (`bindings/valuations/calibration.rs`) derives
//!   from `RuntimeError`. Reparenting it onto `FinstackError` would silently
//!   stop it being a `RuntimeError` and break existing `except RuntimeError`
//!   handlers, so it is deliberately left out until PyO3 can express two bases.
//!
//! # Exception class from the Rust error kind
//!
//! Every typed Rust error reports a Rust-owned
//! [`ErrorKind`](finstack_quant_core::error::ErrorKind), and [`kind_to_py`]
//! turns it into the builtin class: `NotFound` → `KeyError`, `Validation` →
//! `ValueError`, `Computation` → `RuntimeError`. A domain subclass is raised
//! only where its base matches the kind: `AnalyticsError` and
//! `PortfolioError` refine `ValueError`, so they are raised for
//! validation-kind analytics and portfolio errors, and their not-found and
//! computation errors raise `KeyError` / `RuntimeError`. The WASM binding
//! reports the same kind as the error's `kind` property.

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;

pyo3::create_exception!(
    finstack_quant.core,
    FinstackError,
    PyValueError,
    "Base class for the library's named exceptions (inherits ValueError)."
);

pyo3::create_exception!(
    finstack_quant.analytics,
    AnalyticsError,
    FinstackError,
    "Analytics validation or calculation failure (inherits FinstackError, ValueError)."
);

pyo3::create_exception!(
    finstack_quant.portfolio,
    PortfolioError,
    FinstackError,
    "Portfolio validation or calculation failure (inherits FinstackError, ValueError)."
);

pyo3::create_exception!(
    finstack_quant.portfolio,
    ContractValidationError,
    FinstackError,
    "Persisted contract validation failure with structured diagnostics (inherits FinstackError)."
);

pyo3::create_exception!(
    finstack_quant.portfolio,
    UnsupportedContractVersionError,
    ContractValidationError,
    "Unsupported persisted contract version (inherits ContractValidationError)."
);

pyo3::create_exception!(
    finstack_quant.portfolio,
    MissingContractVersionError,
    ContractValidationError,
    "Missing persisted contract version (inherits ContractValidationError)."
);

pyo3::create_exception!(
    finstack_quant.portfolio,
    MalformedContractSchemaError,
    ContractValidationError,
    "Malformed persisted contract schema marker (inherits ContractValidationError)."
);

pyo3::create_exception!(
    finstack_quant.portfolio,
    ContractLimitExceededError,
    ContractValidationError,
    "Persisted contract resource limit exceeded (inherits ContractValidationError)."
);

/// Flatten an error and its `source()` chain into one message (see
/// [`finstack_quant_core::error::format_chain`]).
fn format_chain(err: &(dyn std::error::Error + 'static)) -> String {
    finstack_quant_core::error::format_chain(err)
}

/// Build the builtin exception for a Rust-owned error kind: `NotFound` →
/// `KeyError`, `Validation` → `ValueError`, `Computation` → `RuntimeError`.
pub fn kind_to_py(kind: finstack_quant_core::error::ErrorKind, message: String) -> PyErr {
    use finstack_quant_core::error::ErrorKind;
    match kind {
        ErrorKind::NotFound => PyKeyError::new_err(message),
        ErrorKind::Validation => PyValueError::new_err(message),
        ErrorKind::Computation => PyRuntimeError::new_err(message),
    }
}

/// Convert a `finstack_quant_core::Error` into a Python exception.
///
/// The full error source chain (via [`std::error::Error::source`]) is flattened
/// into the Python message. This preserves context from wrappers like
/// `finstack_quant_valuations::Error::Calibration(core_err)`.
///
/// Exception mapping:
/// - Missing id/lookup failures (`InputError::{MissingCurve, NotFound,
///   CalendarNotFound, FxTriangulationFailed}`) → `KeyError`
/// - Calibration, solver, and operational failures (`Error::{Calibration,
///   Internal, CircularDependency}`, `InputError::{SolverConvergenceFailed,
///   VolatilityConversionFailed, TooLarge}`) → `RuntimeError`
/// - `MetricCalculationFailed` → classified recursively by its underlying cause
/// - Everything else → `ValueError`
pub fn core_to_py(e: finstack_quant_core::Error) -> PyErr {
    kind_to_py(e.kind(), format_chain(&e))
}

/// Convert a `PdCalibrationError` into a Python exception.
///
/// Every variant is a validation failure, so all map to `ValueError`.
pub fn pd_calibration_to_py(e: finstack_quant_models::credit::pd::PdCalibrationError) -> PyErr {
    kind_to_py(e.kind(), format_chain(&e))
}

/// Convert a `MigrationError` into a Python exception by its Rust-owned
/// [`MigrationError::kind`](finstack_quant_models::credit::migration::MigrationError::kind).
pub fn migration_to_py(e: finstack_quant_models::credit::migration::MigrationError) -> PyErr {
    kind_to_py(e.kind(), format_chain(&e))
}

/// Convert an analytics-domain core error into a Python `AnalyticsError`.
///
/// `AnalyticsError` inherits from [`FinstackError`], which inherits from
/// `ValueError`, so existing callers catching `ValueError` remain compatible
/// while analytics users can opt into a narrower exception type.
pub fn analytics_to_py(e: finstack_quant_core::Error) -> PyErr {
    match e.kind() {
        finstack_quant_core::error::ErrorKind::Validation => {
            AnalyticsError::new_err(format_chain(&e))
        }
        kind => kind_to_py(kind, format_chain(&e)),
    }
}

/// Convert a `finstack_quant_scenarios::Error` into a Python exception by its
/// Rust-owned kind. Wrapped core, statements and valuations errors keep their
/// own message rendering.
pub fn scenarios_to_py(e: finstack_quant_scenarios::Error) -> PyErr {
    use finstack_quant_scenarios::Error as SErr;
    match e {
        SErr::Core(core) => core_to_py(core),
        SErr::Statements(inner) => statements_to_py(inner),
        SErr::Valuations(inner) => core_to_py(inner.into()),
        err => kind_to_py(err.kind(), format_chain(&err)),
    }
}

/// Convert a factor-model `DecompositionError` into a Python exception by its
/// Rust-owned kind (unknown issuer → `KeyError`, inconsistent model →
/// `RuntimeError`, otherwise `ValueError`).
pub fn decomposition_error_to_py(
    e: finstack_quant_models::factor::credit::decomposition::DecompositionError,
) -> PyErr {
    kind_to_py(e.kind(), format_chain(&e))
}

/// Convert a `finstack_quant_portfolio::Error` into a Python exception by its
/// Rust-owned kind: validation errors raise `PortfolioError` (a `ValueError`),
/// not-found errors (unknown entity, missing market data or FX rate, a
/// valuation that failed on missing data) raise `KeyError`, and computation
/// failures raise `RuntimeError`.
pub fn portfolio_to_py(e: finstack_quant_portfolio::Error) -> PyErr {
    match e {
        finstack_quant_portfolio::Error::Core(core) => core_to_py(core),
        err => match err.kind() {
            finstack_quant_core::error::ErrorKind::Validation => {
                PortfolioError::new_err(format_chain(&err))
            }
            kind => kind_to_py(kind, format_chain(&err)),
        },
    }
}

/// Convert a typed persisted-contract failure into a Python exception.
///
/// Every [`finstack_quant_core::contract::ContractError`] variant maps without
/// message inspection. Report failures carry a public ``report`` attribute
/// containing a list of diagnostic dictionaries.
pub fn contract_to_py(
    py: Python<'_>,
    error: finstack_quant_core::contract::ContractError,
) -> PyErr {
    use finstack_quant_core::contract::ContractError;

    match error {
        ContractError::UnsupportedVersion { .. } => {
            UnsupportedContractVersionError::new_err(error.to_string())
        }
        ContractError::MissingVersion { .. } => {
            MissingContractVersionError::new_err(error.to_string())
        }
        ContractError::MalformedSchema { .. } => {
            MalformedContractSchemaError::new_err(error.to_string())
        }
        ContractError::LimitExceeded { .. } => {
            ContractLimitExceededError::new_err(error.to_string())
        }
        ContractError::Report(report) => contract_report_to_py(py, &report),
        ContractError::Core(core) => core_to_py(core),
        #[allow(unreachable_patterns)]
        other => ContractValidationError::new_err(other.to_string()),
    }
}

/// Convert a portfolio materialization failure without parsing its message.
pub fn materialization_to_py(py: Python<'_>, error: finstack_quant_portfolio::Error) -> PyErr {
    match error {
        finstack_quant_portfolio::Error::MaterializationFailed(report) => contract_to_py(
            py,
            finstack_quant_core::contract::ContractError::Report(report),
        ),
        error @ finstack_quant_portfolio::Error::ContractLimitExceeded { .. } => {
            ContractLimitExceededError::new_err(error.to_string())
        }
        finstack_quant_portfolio::Error::Core(core) => core_to_py(core),
        other => portfolio_to_py(other),
    }
}

fn contract_report_to_py(
    py: Python<'_>,
    report: &finstack_quant_core::contract::ValidationReport,
) -> PyErr {
    let error = ContractValidationError::new_err(report.summary());
    match diagnostics_to_py(py, report) {
        Ok(diagnostics) => {
            if let Err(setattr_error) = error.value(py).setattr("report", diagnostics) {
                return PyRuntimeError::new_err(format!(
                    "failed to attach 'report' to ContractValidationError: {setattr_error}"
                ));
            }
            error
        }
        Err(conversion_error) => conversion_error,
    }
}

/// Convert a validation report's diagnostics to a Python list of dictionaries.
///
/// Each dictionary is the serde form of the Rust `Diagnostic` (the same keys
/// and enum strings the WASM binding returns), so no label table lives here.
pub(crate) fn diagnostics_to_py(
    py: Python<'_>,
    report: &finstack_quant_core::contract::ValidationReport,
) -> PyResult<Py<PyAny>> {
    crate::bindings::pandas_utils::serde_to_py(py, &report.diagnostics).map(Bound::unbind)
}

/// Convert a `finstack_quant_statements::Error` into a Python exception by its
/// Rust-owned [`kind`](finstack_quant_statements::Error::kind): lookup
/// failures (missing node / data / registry entry) raise `KeyError`, cycles
/// and capital-structure failures `RuntimeError`, and malformed input —
/// including formula evaluation errors and deserialization — `ValueError`.
/// Core errors keep their own mapping; the source chain is preserved.
pub fn statements_to_py(e: finstack_quant_statements::Error) -> PyErr {
    match e {
        finstack_quant_statements::Error::Core(core) => core_to_py(core),
        err => kind_to_py(err.kind(), format_chain(&err)),
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

/// Convert a `CreditScoringError` into a Python exception.
///
/// Every variant (non-finite ratio, non-binary indicator) is a validation
/// failure of the caller's inputs, so all map to `ValueError`.
pub fn scoring_to_py(e: finstack_quant_models::credit::scoring::CreditScoringError) -> PyErr {
    kind_to_py(e.kind(), format_chain(&e))
}

/// Convert a `finstack_quant_models::correlation::Error` into a Python
/// exception through the canonical core taxonomy.
///
/// Iterative failures (nearest-correlation non-convergence, eigendecomposition
/// failure) become `RuntimeError`; every other variant (matrix shape,
/// symmetry, bounds, PSD, volatility, recovery and degrees-of-freedom
/// validation) becomes `ValueError`.
pub fn correlation_to_py(e: finstack_quant_models::correlation::Error) -> PyErr {
    core_to_py(e.into())
}
