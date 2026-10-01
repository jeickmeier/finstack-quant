//! Python bindings for the calibration engine.
//!
//! Wraps `finstack_quant_calibration::api::engine::execute` behind a typed
//! envelope-in / rich-result-out API. Envelopes can be authored as typed
//! objects (`CalibrationEnvelope`, `CalibrationPlan`, `CalibrationStep`,
//! quote classes), as dicts, or as canonical JSON strings.

mod config;
mod envelope;
mod hull_white;
mod report;
mod result;
mod schema;

pub(crate) use config::{PyCalibrationConfig, PyRateBounds, PySolverConfig, PyValidationConfig};
pub(crate) use envelope::{
    PyCalibrationEnvelope, PyCalibrationPlan, PyCalibrationStep, PyCdsQuote, PyRateQuote,
    PyVolQuote,
};
pub(crate) use report::{
    PyCalibrationDiagnostics, PyCalibrationReport, PyCalibrationValidationReport, PyQuoteQuality,
};
pub(crate) use result::PyCalibrationResult;

use crate::bindings::module_utils::py_to_json_string;
use finstack_quant_calibration::api::engine::{self, ExecuteError};
use finstack_quant_calibration::api::errors::EnvelopeError;
use finstack_quant_calibration::api::schema::CalibrationEnvelope;
use finstack_quant_calibration::api::validate as validate_api;
use finstack_quant_core::contract::LoadLimits;
use pyo3::create_exception;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

create_exception!(
    finstack_quant.calibration,
    CalibrationEnvelopeError,
    PyRuntimeError,
    "Raised when calibration ingestion, validation, context construction, or solving fails.\n\n\
     Carries `kind`, `stage`, `step_id`, `solver_diagnostics`, `details` (JSON string) and \
     `diagnostics` (list of strict-load diagnostic dicts with `pointer`, `message`, `code`, \
     `expected_version`, ...) attributes for programmatic handling."
);

/// Attach the Rust-owned host-error payload to `CalibrationEnvelopeError`.
pub(crate) fn execute_error_to_py(py: Python<'_>, err: ExecuteError) -> PyErr {
    let host = err.host_error();
    let exc = CalibrationEnvelopeError::new_err(host.message.clone());
    let value = exc.value(py);
    let diagnostics = match strict_load_diagnostics_to_py(py, &host.diagnostics) {
        Ok(list) => list,
        Err(convert_err) => {
            return PyRuntimeError::new_err(format!(
                "failed to convert calibration diagnostics ({}): underlying calibration error: {}",
                convert_err.value(py),
                host.message
            ))
        }
    };
    let attrs: [(&str, PyResult<()>); 6] = [
        ("kind", value.setattr("kind", host.kind)),
        ("stage", value.setattr("stage", host.stage.as_str())),
        ("details", value.setattr("details", host.details)),
        ("step_id", value.setattr("step_id", host.step_id)),
        (
            "solver_diagnostics",
            value.setattr("solver_diagnostics", host.solver_diagnostics),
        ),
        ("diagnostics", value.setattr("diagnostics", diagnostics)),
    ];
    for (name, result) in attrs {
        if let Err(setattr_err) = result {
            return PyRuntimeError::new_err(format!(
                "failed to attach '{name}' attribute to CalibrationEnvelopeError \
                 ({}): underlying calibration error: {}",
                setattr_err.value(py),
                host.message
            ));
        }
    }
    exc
}

/// Render strict-load diagnostics as a list of plain dicts.
fn strict_load_diagnostics_to_py<'py>(
    py: Python<'py>,
    diagnostics: &[finstack_quant_calibration::api::errors::StrictLoadDiagnostic],
) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::empty(py);
    for diagnostic in diagnostics {
        let item = PyDict::new(py);
        item.set_item("code", &diagnostic.code)?;
        item.set_item("phase", &diagnostic.phase)?;
        item.set_item("severity", &diagnostic.severity)?;
        item.set_item("pointer", diagnostic.pointer.as_deref())?;
        item.set_item("message", &diagnostic.message)?;
        item.set_item("contract", diagnostic.contract.as_deref())?;
        item.set_item("expected_version", diagnostic.expected_version)?;
        item.set_item("actual_version", diagnostic.actual_version)?;
        list.append(item)?;
    }
    Ok(list)
}

/// Map an envelope-level error into `CalibrationEnvelopeError`.
pub(crate) fn envelope_error_to_py(py: Python<'_>, err: EnvelopeError) -> PyErr {
    execute_error_to_py(py, ExecuteError::from(err))
}

/// Strictly parse envelope JSON, surfacing contract diagnostics.
pub(crate) fn parse_envelope_json(py: Python<'_>, json: &str) -> PyResult<CalibrationEnvelope> {
    CalibrationEnvelope::from_slice_strict(json.as_bytes(), &LoadLimits::default())
        .map(|(envelope, _report)| envelope)
        .map_err(|error| envelope_error_to_py(py, EnvelopeError::strict_load(&error)))
}

/// Extract a typed envelope from `CalibrationEnvelope | CalibrationPlan | dict | str`.
pub(crate) fn extract_envelope(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<CalibrationEnvelope> {
    if let Ok(envelope) = obj.cast::<PyCalibrationEnvelope>() {
        return Ok(envelope.borrow().inner.clone());
    }
    if let Ok(plan) = obj.cast::<PyCalibrationPlan>() {
        return Ok(plan.borrow().to_envelope(Vec::new(), Vec::new()));
    }
    let json = py_to_json_string(py, obj, "calibration envelope")?;
    parse_envelope_json(py, &json)
}

/// Validate a calibration envelope and return its canonical (pretty-printed) JSON.
///
/// Parameters
/// ----------
/// envelope : CalibrationEnvelope | CalibrationPlan | dict | str
///     Typed envelope, plan (its attached quotes become ``market_data``),
///     dict, or JSON string using schema marker ``finstack_quant.calibration/1``.
///
/// Returns
/// -------
/// str
///     Canonical pretty-printed envelope JSON.
///
/// Raises
/// ------
/// CalibrationEnvelopeError
///     If strict loading or static validation rejects the envelope. Static
///     validation is fail-fast (first error); use ``dry_run`` to list every
///     static error. Strict-load failures carry ``diagnostics``.
#[pyfunction]
#[pyo3(text_signature = "(envelope)")]
fn validate_calibration_json(py: Python<'_>, envelope: &Bound<'_, PyAny>) -> PyResult<String> {
    let envelope = validate_calibration(py, envelope)?.inner;
    envelope
        .to_json_pretty()
        .map_err(|error| envelope_error_to_py(py, error))
}

/// Validate a calibration envelope and return it as a typed ``CalibrationEnvelope``.
///
/// Parameters
/// ----------
/// envelope : CalibrationEnvelope | CalibrationPlan | dict | str
///     Typed envelope, plan, dict, or JSON string.
///
/// Returns
/// -------
/// CalibrationEnvelope
///     The validated envelope (canonical form).
///
/// Raises
/// ------
/// CalibrationEnvelopeError
///     If strict loading or static validation rejects the envelope
///     (fail-fast: first error; ``dry_run`` lists every static error).
#[pyfunction]
#[pyo3(text_signature = "(envelope)")]
fn validate_calibration(
    py: Python<'_>,
    envelope: &Bound<'_, PyAny>,
) -> PyResult<PyCalibrationEnvelope> {
    let envelope = extract_envelope(py, envelope)?;
    if let Some(error) = validate_api::validate(&envelope).errors.into_iter().next() {
        return Err(envelope_error_to_py(py, error));
    }
    Ok(PyCalibrationEnvelope::from_inner(envelope))
}

/// Pre-flight envelope validation without invoking the solver.
///
/// Parameters
/// ----------
/// envelope : CalibrationEnvelope | CalibrationPlan | dict | str
///     Typed envelope, plan, dict, or JSON string.
///
/// Returns
/// -------
/// CalibrationValidationReport
///     Every static error found in a single pass plus the dependency graph.
///     Semantic findings are returned in the report, never raised.
///
/// Raises
/// ------
/// CalibrationEnvelopeError
///     Only if the input cannot be strictly loaded as an envelope (malformed
///     JSON, wrong schema marker, unknown fields, resource limits). Those
///     ``strict_load`` failures carry ``diagnostics`` with JSON pointers.
#[pyfunction]
#[pyo3(text_signature = "(envelope)")]
fn dry_run(py: Python<'_>, envelope: &Bound<'_, PyAny>) -> PyResult<PyCalibrationValidationReport> {
    let envelope = extract_envelope(py, envelope)?;
    Ok(PyCalibrationValidationReport::from_inner(
        validate_api::validate(&envelope),
    ))
}

/// JSON twin of ``dry_run``: the validation report as pretty-printed JSON.
///
/// Parameters
/// ----------
/// envelope : CalibrationEnvelope | CalibrationPlan | dict | str
///     Typed envelope, plan, dict, or JSON string.
///
/// Returns
/// -------
/// str
///     Pretty-printed ``CalibrationValidationReport`` JSON.
///
/// Raises
/// ------
/// CalibrationEnvelopeError
///     If the input cannot be strictly loaded as an envelope.
#[pyfunction]
#[pyo3(text_signature = "(envelope)")]
fn dry_run_json(py: Python<'_>, envelope: &Bound<'_, PyAny>) -> PyResult<String> {
    dry_run(py, envelope)?.to_json_pretty(py)
}

/// Execute a calibration plan and return the full result.
///
/// Parameters
/// ----------
/// envelope : CalibrationEnvelope | CalibrationPlan | dict | str
///     Typed envelope, plan (quotes attached to its steps become the
///     ``market_data``), dict, or JSON string.
///
/// Returns
/// -------
/// CalibrationResult
///     Calibrated market, plan-level report, per-step reports and residuals.
///
/// Raises
/// ------
/// CalibrationEnvelopeError
///     If ingestion, validation, context construction, target construction,
///     solving, or final fit acceptance fails. Static validation is fail-fast
///     (first error); use ``dry_run`` to list every static error.
#[pyfunction]
#[pyo3(text_signature = "(envelope)")]
fn calibrate(py: Python<'_>, envelope: &Bound<'_, PyAny>) -> PyResult<PyCalibrationResult> {
    let envelope = extract_envelope(py, envelope)?;
    // Release the GIL for the duration of the solver: calibration can run for seconds.
    // `ExecuteError` is a large enum; box it so the closure result stays small.
    let result = py
        .detach(move || engine::execute(&envelope).map_err(Box::new))
        .map_err(|e| execute_error_to_py(py, *e))?;
    Ok(PyCalibrationResult::from_inner(result))
}

/// Calibrate the explicit Bermudan LMM loading scale from the market surface.
///
/// Parameters
/// ----------
/// instrument : Swaption | str
///     Typed Bermudan swaption instrument or its canonical instrument JSON
///     envelope (any typed instrument object accepted by the pricing helpers
///     is unwrapped the same way).
/// market : MarketContext | str
///     Market carrying the required discount and swaption-volatility inputs.
/// as_of : datetime.date | str
///     Valuation date used for tenor and expiry construction.
///
/// Returns
/// -------
/// float
///     Positive finite LMM base volatility (annualized decimal) to place in
///     ``model_config.lmm_base_vol``.
///
/// Raises
/// ------
/// ValueError
///     If the instrument is not a Bermudan swaption or inputs are invalid.
/// KeyError
///     If a referenced curve or surface is missing from ``market``.
/// RuntimeError
///     If the Rebonato calibration fails.
#[pyfunction]
#[pyo3(text_signature = "(instrument, market, as_of)")]
fn calibrate_bermudan_lmm_base_vol(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<f64> {
    let instrument_json = crate::bindings::extract::extract_instrument_json(instrument)?;
    let market = crate::bindings::extract::extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date(as_of)?;
    py.detach(move || {
        finstack_quant_calibration::calibrate_bermudan_lmm_base_vol_from_json(
            &instrument_json,
            &market,
            as_of,
        )
    })
    .map_err(crate::errors::core_to_py)
}

/// Register the root-level calibration submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "calibration")?;
    m.add_class::<PyCalibrationConfig>()?;
    m.add_class::<PyCalibrationDiagnostics>()?;
    m.add_class::<PyCalibrationEnvelope>()?;
    m.add_class::<PyCalibrationPlan>()?;
    m.add_class::<PyCalibrationReport>()?;
    m.add_class::<PyCalibrationResult>()?;
    m.add_class::<PyCalibrationStep>()?;
    m.add_class::<PyCalibrationValidationReport>()?;
    m.add_class::<PyCdsQuote>()?;
    m.add_class::<PyQuoteQuality>()?;
    m.add_class::<PyRateBounds>()?;
    m.add_class::<PyRateQuote>()?;
    m.add_class::<PySolverConfig>()?;
    m.add_class::<PyValidationConfig>()?;
    m.add_class::<PyVolQuote>()?;
    m.add(
        "CalibrationEnvelopeError",
        py.get_type::<CalibrationEnvelopeError>(),
    )?;
    m.add_function(pyo3::wrap_pyfunction!(calibrate, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(calibrate_bermudan_lmm_base_vol, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dry_run, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dry_run_json, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_calibration, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_calibration_json, &m)?)?;
    hull_white::register(py, &m)?;
    schema::register(py, &m)?;
    m.setattr(
        "__all__",
        PyList::new(
            py,
            [
                "CalibrationConfig",
                "CalibrationDiagnostics",
                "CalibrationEnvelope",
                "CalibrationEnvelopeError",
                "CalibrationPlan",
                "CalibrationReport",
                "CalibrationResult",
                "CalibrationStep",
                "CalibrationValidationReport",
                "CdsQuote",
                "QuoteQuality",
                "RateBounds",
                "RateQuote",
                "SolverConfig",
                "ValidationConfig",
                "VolQuote",
                "calibrate",
                "calibrate_bermudan_lmm_base_vol",
                "dry_run",
                "dry_run_json",
                "hull_white",
                "schema",
                "validate_calibration",
                "validate_calibration_json",
            ],
        )?,
    )?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
