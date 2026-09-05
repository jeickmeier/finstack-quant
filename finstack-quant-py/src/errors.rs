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
//!     │   ├── ValuationError
//!     │   └── FxError
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
//! The bare `ValueError`/`KeyError`/`RuntimeError` values produced by
//! [`core_to_py`], [`value_error`], and friends are deliberately left
//! unclassified — reclassifying them would change the exception type of every
//! existing call site.

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

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
    ValuationError,
    PortfolioError,
    "Portfolio valuation failure (inherits PortfolioError)."
);

pyo3::create_exception!(
    finstack_quant.portfolio,
    FxError,
    PortfolioError,
    "Portfolio FX conversion or market-data failure (inherits PortfolioError)."
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
    use finstack_quant_core::error::ErrorKind;

    let message = format_chain(&e);
    match e.kind() {
        ErrorKind::NotFound => PyKeyError::new_err(message),
        // Numerical non-convergence and allocation-limit breaches are
        // operational states, not bad user input.
        ErrorKind::Computation => PyRuntimeError::new_err(message),
        ErrorKind::Validation => PyValueError::new_err(message),
    }
}

/// Convert a `PdCalibrationError` into a Python exception.
///
/// Every variant is a validation failure, so all map to `ValueError`.
pub fn pd_calibration_to_py(e: finstack_quant_models::credit::pd::PdCalibrationError) -> PyErr {
    PyValueError::new_err(format_chain(&e))
}

/// Convert a `MigrationError` into a Python exception.
///
/// Mirrors [`core_to_py`]: label/state lookup misses raise `KeyError`,
/// numerical/operational failures raise `RuntimeError`, validation failures
/// raise `ValueError`.
pub fn migration_to_py(e: finstack_quant_models::credit::migration::MigrationError) -> PyErr {
    use finstack_quant_models::credit::migration::MigrationError as E;
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
/// `AnalyticsError` inherits from [`FinstackError`], which inherits from
/// `ValueError`, so existing callers catching `ValueError` remain compatible
/// while analytics users can opt into a narrower exception type.
pub fn analytics_to_py(e: finstack_quant_core::Error) -> PyErr {
    use finstack_quant_core::error::ErrorKind;

    let message = format_chain(&e);
    match e.kind() {
        ErrorKind::NotFound => PyKeyError::new_err(message),
        ErrorKind::Computation => PyRuntimeError::new_err(message),
        ErrorKind::Validation => AnalyticsError::new_err(message),
    }
}

/// Convert a `finstack_quant_scenarios::Error` into a Python exception.
///
/// Lookup misses (`MarketDataNotFound`, `NodeNotFound`, `TenorNotFound`,
/// `InstrumentNotFound`) raise `KeyError`; engine/computation failures
/// (`Internal`) raise `RuntimeError`; wrapped core/statements/valuations
/// errors delegate to their own mappers; everything else (validation,
/// unsupported operation, malformed tenor/period) raises `ValueError`.
pub fn scenarios_to_py(e: finstack_quant_scenarios::Error) -> PyErr {
    use finstack_quant_scenarios::Error as SErr;
    match e {
        SErr::Core(core) => core_to_py(core),
        SErr::Statements(inner) => statements_to_py(inner),
        SErr::Valuations(inner) => core_to_py(inner.into()),
        err @ (SErr::MarketDataNotFound { .. }
        | SErr::NodeNotFound { .. }
        | SErr::TenorNotFound { .. }
        | SErr::InstrumentNotFound(_)) => PyKeyError::new_err(format_chain(&err)),
        err @ SErr::Internal(_) => PyRuntimeError::new_err(format_chain(&err)),
        err => PyValueError::new_err(format_chain(&err)),
    }
}

/// Convert a factor-model `DecompositionError` into a Python exception.
///
/// `UnknownIssuer` is a lookup miss and raises `KeyError`; `ModelInconsistent`
/// is an operational invariant failure and raises `RuntimeError`; the
/// remaining shape/tag validation failures raise `ValueError`.
pub fn decomposition_error_to_py(
    e: finstack_quant_models::factor::credit::decomposition::DecompositionError,
) -> PyErr {
    use finstack_quant_models::factor::credit::decomposition::DecompositionError as E;
    let message = format_chain(&e);
    match &e {
        E::UnknownIssuer { .. } => PyKeyError::new_err(message),
        E::ModelInconsistent { .. } => PyRuntimeError::new_err(message),
        _ => PyValueError::new_err(message),
    }
}

/// Convert a `finstack_quant_portfolio::Error` into a portfolio-domain Python exception.
///
/// The named subclasses preserve compatibility with callers catching
/// `ValueError` or `PortfolioError`, while letting portfolio users distinguish
/// valuation and FX/market-data failures.
pub fn portfolio_to_py(e: finstack_quant_portfolio::Error) -> PyErr {
    match e {
        finstack_quant_portfolio::Error::Core(core) => core_to_py(core),
        err @ finstack_quant_portfolio::Error::ValuationError { .. } => {
            ValuationError::new_err(format_chain(&err))
        }
        err @ (finstack_quant_portfolio::Error::FxConversionFailed { .. }
        | finstack_quant_portfolio::Error::MissingMarketData(_)) => {
            FxError::new_err(format_chain(&err))
        }
        err => PortfolioError::new_err(format_chain(&err)),
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
    let error = ContractValidationError::new_err(format!(
        "validation failed with {} structured diagnostic(s)",
        report.diagnostics.len()
    ));
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

/// Convert a validation report to a Python list of diagnostic dictionaries.
pub(crate) fn diagnostics_to_py(
    py: Python<'_>,
    report: &finstack_quant_core::contract::ValidationReport,
) -> PyResult<Py<PyAny>> {
    let diagnostics = PyList::empty(py);
    for diagnostic in &report.diagnostics {
        let item = PyDict::new(py);
        item.set_item("code", &diagnostic.code)?;
        item.set_item("phase", load_phase_name(diagnostic.phase))?;
        item.set_item("severity", severity_name(diagnostic.severity))?;
        item.set_item("pointer", diagnostic.pointer.as_deref())?;
        item.set_item("message", &diagnostic.message)?;
        item.set_item("contract", diagnostic.contract.as_deref())?;
        item.set_item("expected_version", diagnostic.expected_version)?;
        item.set_item("actual_version", diagnostic.actual_version)?;
        item.set_item("artifact_hash", diagnostic.artifact_hash.as_deref())?;
        item.set_item("revision_id", diagnostic.revision_id.as_deref())?;
        item.set_item("instrument_id", diagnostic.instrument_id.as_deref())?;
        item.set_item("position_id", diagnostic.position_id.as_deref())?;
        diagnostics.append(item)?;
    }
    Ok(diagnostics.into_any().unbind())
}

fn load_phase_name(phase: finstack_quant_core::contract::LoadPhase) -> &'static str {
    use finstack_quant_core::contract::LoadPhase;

    match phase {
        LoadPhase::Parse => "parse",
        LoadPhase::Version => "version",
        LoadPhase::Structure => "structure",
        LoadPhase::Semantic => "semantic",
        LoadPhase::Canonicalize => "canonicalize",
        LoadPhase::Hash => "hash",
        LoadPhase::Build => "build",
        #[allow(unreachable_patterns)]
        _ => "unknown",
    }
}

fn severity_name(severity: finstack_quant_core::contract::Severity) -> &'static str {
    use finstack_quant_core::contract::Severity;

    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        #[allow(unreachable_patterns)]
        _ => "unknown",
    }
}

/// Convert a `finstack_quant_statements::Error` into a Python exception.
///
/// Follows the binding error contract: lookup failures (missing node / data /
/// registry metric) raise `KeyError`; core errors delegate to [`core_to_py`]
/// (so e.g. a missing curve stays a `KeyError`); operational failures
/// (I/O, capital-structure, index) raise `RuntimeError`; malformed input —
/// including deserialization (`Serde`) — and the remaining validation/argument
/// errors raise `ValueError`. The full source chain is preserved via
/// [`format_chain`].
pub fn statements_to_py(e: finstack_quant_statements::Error) -> PyErr {
    use finstack_quant_statements::Error as SErr;
    match e {
        SErr::Core(core) => core_to_py(core),
        err @ (SErr::NodeNotFound(_) | SErr::MissingData(_) | SErr::RegistryNotFound(_)) => {
            PyKeyError::new_err(format_chain(&err))
        }
        // `Serde` is malformed *input*, not an operational failure: bad JSON at
        // one entry point (`MetricRegistry.load_from_json_str`) must raise the
        // same `ValueError` as bad JSON at another (`FinancialModelSpec.from_json`),
        // so a caller catching `ValueError` for bad config handles both.
        err @ SErr::CapitalStructure(_) => PyRuntimeError::new_err(format_chain(&err)),
        err => PyValueError::new_err(format_chain(&err)),
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
    PyValueError::new_err(format_chain(&e))
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
