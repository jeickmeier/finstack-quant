//! Python bindings for the `finstack-quant-covenants` crate.
//!
//! Typed covenant definitions (`Covenant`, `CovenantType`, `CovenantSpec`,
//! `ThresholdSchedule`, `CovenantWaiver`, ...), the `CovenantEngine`
//! evaluator, template packages, DataFrame-backed breach forecasting, and the
//! JSON validators / `_json` template twins shared with WASM.

mod engine;
mod forecast;
mod report;
mod spec;

pub(crate) use engine::{PyCovenantBreach, PyCovenantEngine, dated_reports_frame};
pub(crate) use forecast::{PyCovenantForecast, PyCovenantForecastConfig, PyFutureBreach};
pub(crate) use report::PyCovenantReport;
pub(crate) use spec::{
    PyCovenant, PyCovenantConsequence, PyCovenantSpec, PyCovenantType, PyCovenantWaiver,
    PySpringingCondition, PyThresholdSchedule,
};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

/// Validate and canonicalize a covenant spec JSON string.
///
/// # Arguments
///
/// * `spec_json` - JSON-encoded `CovenantSpec`
///
/// # Returns
///
/// Canonical JSON string (object keys sorted recursively) after validation.
///
/// # Errors
///
/// Raises `ValueError` when the spec fails schema or semantic validation.
#[pyfunction]
#[pyo3(text_signature = "(spec_json)")]
fn validate_covenant_spec_json(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::validate_covenant_spec_json(spec_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Validate and canonicalize a covenant report JSON string.
///
/// Returns canonical JSON with object keys sorted recursively. Raises
/// `ValueError` when the report JSON is malformed.
#[pyfunction]
#[pyo3(text_signature = "(report_json)")]
fn validate_covenant_report_json(py: Python<'_>, report_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::validate_covenant_report_json(report_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Validate and canonicalize a covenant engine JSON string.
///
/// Only `specs` is required in the document; `breach_history`, `windows` and
/// `waivers` default to empty. Returns canonical JSON with object keys sorted
/// recursively. Raises `ValueError` when the engine fails schema or semantic
/// validation.
#[pyfunction]
#[pyo3(text_signature = "(engine_json)")]
fn validate_covenant_engine_json(py: Python<'_>, engine_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::validate_covenant_engine_json(engine_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Evaluate a covenant engine JSON document against a metric mapping.
///
/// # Arguments
///
/// * `engine_json` - Serialized covenant engine (`CovenantEngine.to_json()`
///   or a hand-written document; only `specs` is required)
/// * `metrics` - `dict[str, float]` or JSON-object string mapping metric id to
///   value; ratios in turns, amounts in the engine's reporting currency. A
///   dict reaches Rust as numbers (`evaluate_engine_with_source`), so a
///   non-finite value fails only when a covenant reads it; a string is parsed
///   by Rust `HashMapMetricSource::from_json`
/// * `as_of` - Evaluation date, either a date-like object (`datetime.date`,
///   `pandas.Timestamp`) or an ISO 8601 string
///
/// # Returns
///
/// A dict mapping the covenant instance key (label) to a typed
/// `CovenantReport`, in spec order.
///
/// # Errors
///
/// Raises `KeyError` when a required metric is missing and `ValueError` when
/// the engine document or a metric value is invalid.
#[pyfunction]
#[pyo3(text_signature = "(engine_json, metrics, as_of)")]
fn evaluate_engine<'py>(
    py: Python<'py>,
    engine_json: &str,
    metrics: &Bound<'py, PyAny>,
    as_of: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let reports = match metrics.extract::<String>() {
        Ok(metrics_json) => py.detach(|| {
            finstack_quant_covenants::evaluate_engine(engine_json, &metrics_json, &as_of)
        }),
        Err(_) => {
            let source = finstack_quant_covenants::HashMapMetricSource::from_pairs(
                engine::extract_metric_dict(metrics)?,
            );
            py.detach(|| {
                finstack_quant_covenants::evaluate_engine_with_source(engine_json, &source, &as_of)
            })
        }
    }
    .map_err(crate::errors::core_to_py)?;
    engine::reports_to_pydict(py, reports)
}

/// Register the `finstack_quant.covenants` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "covenants")?;
    m.setattr(
        "__doc__",
        "Typed covenant definitions, the CovenantEngine evaluator, CovenantTemplate packages, \
         DataFrame-backed breach forecasting, and the JSON validators.",
    )?;

    m.add_class::<PyCovenant>()?;
    m.add_class::<PyCovenantBreach>()?;
    m.add_class::<PyCovenantConsequence>()?;
    m.add_class::<PyCovenantEngine>()?;
    m.add_class::<PyCovenantForecast>()?;
    m.add_class::<PyCovenantForecastConfig>()?;
    m.add_class::<PyCovenantReport>()?;
    m.add_class::<PyCovenantSpec>()?;
    m.add_class::<engine::PyCovenantTemplate>()?;
    m.add_class::<PyCovenantType>()?;
    m.add_class::<PyCovenantWaiver>()?;
    m.add_class::<PyFutureBreach>()?;
    m.add_class::<PySpringingCondition>()?;
    m.add_class::<PyThresholdSchedule>()?;
    m.add_function(wrap_pyfunction!(forecast::breaches_to_dataframe, &m)?)?;
    m.add_function(wrap_pyfunction!(engine::covenant_template, &m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_engine, &m)?)?;
    m.add_function(wrap_pyfunction!(forecast::forecast_breaches, &m)?)?;
    m.add_function(wrap_pyfunction!(forecast::forecast_covenant, &m)?)?;
    m.add_function(wrap_pyfunction!(engine::reports_to_dataframe, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_covenant_engine_json, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_covenant_report_json, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_covenant_spec_json, &m)?)?;

    let all = PyList::new(
        py,
        [
            "Covenant",
            "CovenantBreach",
            "CovenantConsequence",
            "CovenantEngine",
            "CovenantForecast",
            "CovenantForecastConfig",
            "CovenantReport",
            "CovenantSpec",
            "CovenantTemplate",
            "CovenantType",
            "CovenantWaiver",
            "FutureBreach",
            "SpringingCondition",
            "ThresholdSchedule",
            "breaches_to_dataframe",
            "covenant_template",
            "evaluate_engine",
            "forecast_breaches",
            "forecast_covenant",
            "reports_to_dataframe",
            "validate_covenant_engine_json",
            "validate_covenant_report_json",
            "validate_covenant_spec_json",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
