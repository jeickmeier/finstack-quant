//! Calibration execution, dry-run, validation, and exception mapping.

use super::envelope::PyCalibrationEnvelope;
use super::report::PyCalibrationValidationReport;
use super::result::PyCalibrationResult;
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::module_utils::py_to_json_string;
use finstack_quant_calibration::api::engine::{self, ExecuteError};
use finstack_quant_calibration::api::errors::EnvelopeError;
use finstack_quant_calibration::api::schema::CalibrationEnvelope;
use finstack_quant_calibration::api::validate as validate_api;
use pyo3::create_exception;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

create_exception!(
    finstack_quant.calibration,
    CalibrationEnvelopeError,
    PyRuntimeError,
    "Raised when calibration ingestion, validation, context construction, or solving fails.\n\n\
     Carries `kind`, `stage`, `step_id`, `solver_diagnostics` (dict or None), `details` \
     (JSON string) and `diagnostics` (list of strict-load diagnostic dicts with `pointer`, \
     `message`, `code`, `expected_version`, ...) attributes for programmatic handling."
);

/// Attach the structured Rust execution-error payload to `CalibrationEnvelopeError`.
pub(crate) fn execute_error_to_py(py: Python<'_>, err: ExecuteError) -> PyErr {
    let details = err.details();
    let details_json = details.to_json();
    let exc = CalibrationEnvelopeError::new_err(details.cause.clone());
    let value = exc.value(py);
    use crate::bindings::json_bridge::serde_to_py;
    let converted = serde_to_py(py, &details.diagnostics).and_then(|diagnostics| {
        let solver = details
            .solver_diagnostics
            .as_ref()
            .map(|solver| serde_to_py(py, solver))
            .transpose()?;
        Ok((diagnostics, solver))
    });
    let (diagnostics, solver_diagnostics) = match converted {
        Ok(converted) => converted,
        Err(convert_err) => {
            return PyRuntimeError::new_err(format!(
                "failed to convert calibration diagnostics ({}): underlying calibration error: {}",
                convert_err.value(py),
                details.cause
            ));
        }
    };
    let attrs: [(&str, PyResult<()>); 6] = [
        ("kind", value.setattr("kind", details.category)),
        ("stage", value.setattr("stage", details.stage.as_str())),
        ("details", value.setattr("details", details_json)),
        ("step_id", value.setattr("step_id", details.step_id)),
        (
            "solver_diagnostics",
            value.setattr("solver_diagnostics", solver_diagnostics),
        ),
        ("diagnostics", value.setattr("diagnostics", diagnostics)),
    ];
    for (name, result) in attrs {
        if let Err(setattr_err) = result {
            return PyRuntimeError::new_err(format!(
                "failed to attach '{name}' attribute to CalibrationEnvelopeError \
                 ({}): underlying calibration error: {}",
                setattr_err.value(py),
                details.cause
            ));
        }
    }
    exc
}

/// Map an envelope-level error into `CalibrationEnvelopeError`.
pub(crate) fn envelope_error_to_py(py: Python<'_>, err: EnvelopeError) -> PyErr {
    execute_error_to_py(py, ExecuteError::from(err))
}

/// Strictly parse envelope JSON through the Rust loader, surfacing contract
/// diagnostics.
pub(crate) fn parse_envelope(py: Python<'_>, json: &str) -> PyResult<CalibrationEnvelope> {
    validate_api::parse_envelope(json).map_err(|error| envelope_error_to_py(py, error))
}

/// Extract a typed envelope from `CalibrationEnvelope | dict`.
///
/// A dict is serialized with `json.dumps` and goes through the Rust strict
/// loader. A `str` is refused: envelope text loads with
/// `CalibrationEnvelope.from_json`, which is that same loader.
pub(crate) fn extract_envelope(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<CalibrationEnvelope> {
    if let Ok(envelope) = obj.cast::<PyCalibrationEnvelope>() {
        return Ok(envelope.borrow().inner.clone());
    }
    if obj.is_instance_of::<pyo3::types::PyString>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "expected a CalibrationEnvelope or an envelope dict, not a string; load JSON text with CalibrationEnvelope.from_json",
        ));
    }
    let json = py_to_json_string(py, obj, "calibration envelope")?;
    parse_envelope(py, &json)
}

/// Validate a calibration envelope and return it as a typed ``CalibrationEnvelope``.
///
/// Parameters
/// ----------
/// envelope : CalibrationEnvelope | dict
///     Typed envelope or its dict. Load JSON text with
///     ``CalibrationEnvelope.from_json``.
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
pub(super) fn validate_calibration(
    py: Python<'_>,
    envelope: &Bound<'_, PyAny>,
) -> PyResult<PyCalibrationEnvelope> {
    let envelope = extract_envelope(py, envelope)?;
    validate_api::validate_calibration(&envelope)
        .map_err(|error| envelope_error_to_py(py, error))?;
    Ok(PyCalibrationEnvelope::from_inner(envelope))
}

/// Pre-flight envelope validation without invoking the solver.
///
/// Parameters
/// ----------
/// envelope : CalibrationEnvelope | dict
///     Typed envelope or its dict. Load JSON text with
///     ``CalibrationEnvelope.from_json``.
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
pub(super) fn dry_run(
    py: Python<'_>,
    envelope: &Bound<'_, PyAny>,
) -> PyResult<PyCalibrationValidationReport> {
    let envelope = extract_envelope(py, envelope)?;
    Ok(PyCalibrationValidationReport::from_inner(
        validate_api::validate(&envelope),
    ))
}

/// Execute a calibration plan and return the full result.
///
/// Parameters
/// ----------
/// envelope : CalibrationEnvelope | dict
///     Typed envelope or dict containing the plan and market data.
///
/// Returns
/// -------
/// CalibrationResultEnvelope
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
pub(super) fn calibrate(
    py: Python<'_>,
    envelope: &Bound<'_, PyAny>,
) -> PyResult<PyCalibrationResult> {
    let envelope = extract_envelope(py, envelope)?;
    // Release the GIL for the duration of the solver: calibration can run for seconds.
    // `ExecuteError` is a large enum; box it so the closure result stays small.
    let result = py
        .detach(move || engine::calibrate(&envelope).map_err(Box::new))
        .map_err(|e| execute_error_to_py(py, *e))?;
    Ok(PyCalibrationResult::from_inner(result))
}

/// Calibrate the explicit Bermudan LMM loading scale from the market surface.
///
/// Parameters
/// ----------
/// instrument : Swaption | dict
///     Typed Bermudan swaption instrument or its canonical instrument
///     envelope dict (any typed instrument object accepted by the pricing
///     helpers is unwrapped the same way).
/// market : MarketContext
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
pub(super) fn calibrate_bermudan_lmm_base_vol(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market: PyRef<'_, PyMarketContext>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<f64> {
    let instrument_json = crate::bindings::extract::extract_instrument_json(instrument)?;
    let market = market.inner.clone();
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    py.detach(move || {
        finstack_quant_calibration::calibrate_bermudan_lmm_base_vol_from_json(
            &instrument_json,
            &market,
            as_of,
        )
    })
    .map_err(crate::errors::core_to_py)
}
