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

pub(crate) use engine::{PyCovenantBreach, PyCovenantEngine};
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
///   value; ratios in turns, amounts in the engine's reporting currency
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
    let metrics_json = match metrics.extract::<String>() {
        Ok(text) => text,
        Err(_) => {
            let metrics: serde_json::Map<String, serde_json::Value> =
                engine::extract_metric_dict(metrics)?
                    .into_iter()
                    .map(|(key, value)| (key, serde_json::Value::from(value)))
                    .collect();
            serde_json::to_string(&metrics).map_err(crate::errors::display_to_py)?
        }
    };
    let reports = py.detach(|| {
        finstack_quant_covenants::evaluate_engine(engine_json, &metrics_json, &as_of)
            .map_err(crate::errors::core_to_py)
    })?;
    engine::reports_to_pydict(py, reports)
}

/// Standard leveraged-buyout covenant package as a JSON array of specs.
///
/// Typed twin: `lbo_standard`. `initial_leverage` is the maximum gross
/// Debt/EBITDA in turns, `interest_coverage` / `fixed_charge_coverage` are
/// minimum coverage ratios in turns, `max_capex` is a reporting-currency
/// amount. Raises `ValueError` when any input is NaN, infinite or negative.
#[pyfunction]
#[pyo3(text_signature = "(initial_leverage, interest_coverage, fixed_charge_coverage, max_capex)")]
fn lbo_standard_json(
    py: Python<'_>,
    initial_leverage: f64,
    interest_coverage: f64,
    fixed_charge_coverage: f64,
    max_capex: f64,
) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::lbo_standard_json(
            initial_leverage,
            interest_coverage,
            fixed_charge_coverage,
            max_capex,
        )
        .map_err(crate::errors::core_to_py)
    })
}

/// Covenant-lite package as a JSON array of specs (typed twin: `cov_lite`).
///
/// Raises `ValueError` when any input is NaN, infinite or negative.
#[pyfunction]
#[pyo3(text_signature = "(max_leverage, max_senior_leverage)")]
fn cov_lite_json(py: Python<'_>, max_leverage: f64, max_senior_leverage: f64) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::cov_lite_json(max_leverage, max_senior_leverage)
            .map_err(crate::errors::core_to_py)
    })
}

/// Real-estate package as a JSON array of specs (typed twin: `real_estate`).
///
/// `min_debt_yield` and `max_ltv` are decimal fractions (`0.08` = 8%).
/// Raises `ValueError` when any input is NaN, infinite or negative.
#[pyfunction]
#[pyo3(text_signature = "(min_dscr, min_debt_yield, max_ltv)")]
fn real_estate_json(
    py: Python<'_>,
    min_dscr: f64,
    min_debt_yield: f64,
    max_ltv: f64,
) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::real_estate_json(min_dscr, min_debt_yield, max_ltv)
            .map_err(crate::errors::core_to_py)
    })
}

/// Project-finance package as a JSON array of specs (typed twin:
/// `project_finance`).
///
/// Raises `ValueError` when any input is NaN, infinite or negative.
#[pyfunction]
#[pyo3(text_signature = "(min_dscr, distribution_lockup_dscr, min_liquidity, max_net_leverage)")]
fn project_finance_json(
    py: Python<'_>,
    min_dscr: f64,
    distribution_lockup_dscr: f64,
    min_liquidity: f64,
    max_net_leverage: f64,
) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::project_finance_json(
            min_dscr,
            distribution_lockup_dscr,
            min_liquidity,
            max_net_leverage,
        )
        .map_err(crate::errors::core_to_py)
    })
}

/// Register the `finstack_quant.covenants` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "covenants")?;
    m.setattr(
        "__doc__",
        "Typed covenant definitions, the CovenantEngine evaluator, template packages, \
         DataFrame-backed breach forecasting, and the JSON validators / `_json` template twins.",
    )?;

    m.add_class::<PyCovenant>()?;
    m.add_class::<PyCovenantBreach>()?;
    m.add_class::<PyCovenantConsequence>()?;
    m.add_class::<PyCovenantEngine>()?;
    m.add_class::<PyCovenantForecast>()?;
    m.add_class::<PyCovenantForecastConfig>()?;
    m.add_class::<PyCovenantReport>()?;
    m.add_class::<PyCovenantSpec>()?;
    m.add_class::<PyCovenantType>()?;
    m.add_class::<PyCovenantWaiver>()?;
    m.add_class::<PyFutureBreach>()?;
    m.add_class::<PySpringingCondition>()?;
    m.add_class::<PyThresholdSchedule>()?;
    m.add_function(wrap_pyfunction!(forecast::breaches_to_dataframe, &m)?)?;
    m.add_function(wrap_pyfunction!(engine::cov_lite, &m)?)?;
    m.add_function(wrap_pyfunction!(cov_lite_json, &m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_engine, &m)?)?;
    m.add_function(wrap_pyfunction!(forecast::forecast_breaches, &m)?)?;
    m.add_function(wrap_pyfunction!(forecast::forecast_covenant, &m)?)?;
    m.add_function(wrap_pyfunction!(engine::lbo_standard, &m)?)?;
    m.add_function(wrap_pyfunction!(lbo_standard_json, &m)?)?;
    m.add_function(wrap_pyfunction!(engine::project_finance, &m)?)?;
    m.add_function(wrap_pyfunction!(project_finance_json, &m)?)?;
    m.add_function(wrap_pyfunction!(engine::real_estate, &m)?)?;
    m.add_function(wrap_pyfunction!(real_estate_json, &m)?)?;
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
            "CovenantType",
            "CovenantWaiver",
            "FutureBreach",
            "SpringingCondition",
            "ThresholdSchedule",
            "breaches_to_dataframe",
            "cov_lite",
            "cov_lite_json",
            "evaluate_engine",
            "forecast_breaches",
            "forecast_covenant",
            "lbo_standard",
            "lbo_standard_json",
            "project_finance",
            "project_finance_json",
            "real_estate",
            "real_estate_json",
            "reports_to_dataframe",
            "validate_covenant_engine_json",
            "validate_covenant_report_json",
            "validate_covenant_spec_json",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "covenants",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}
