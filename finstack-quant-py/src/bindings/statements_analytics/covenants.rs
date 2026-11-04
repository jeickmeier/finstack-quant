//! Covenant evaluation and breach forecasting over evaluated statement results.
//!
//! Wraps `finstack_quant_statements_analytics::analysis::{evaluate_covenants,
//! forecast_breaches}`: the statement results come in as a typed object or
//! JSON, the engine and forecast configuration as the
//! `finstack_quant.covenants` classes.

use crate::bindings::covenants::{
    PyCovenantEngine, PyCovenantForecastConfig, PyFutureBreach, dated_reports_frame,
};
use crate::bindings::extract::extract_results_ref;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_covenants::CovenantForecastConfig;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.statements_analytics._covenants", "evaluate_covenants")]
#[pyfunction]
#[pyo3(text_signature = "(results, covenants)")]
fn evaluate_covenants<'py>(
    py: Python<'py>,
    results: &Bound<'py, PyAny>,
    covenants: PyRef<'py, PyCovenantEngine>,
) -> PyResult<Bound<'py, PyAny>> {
    let results = extract_results_ref(results)?;
    let series = finstack_quant_statements_analytics::analysis::evaluate_covenants(
        &results,
        &covenants.inner,
    )
    .map_err(core_to_py)?;
    dated_reports_frame(py, series)
}

#[doc = pydoc!("finstack_quant.statements_analytics._covenants", "forecast_breaches")]
#[pyfunction]
#[pyo3(
    signature = (results, covenants, config=None),
    text_signature = "(results, covenants, config=None)"
)]
fn forecast_breaches(
    results: &Bound<'_, PyAny>,
    covenants: PyRef<'_, PyCovenantEngine>,
    config: Option<PyRef<'_, PyCovenantForecastConfig>>,
) -> PyResult<Vec<PyFutureBreach>> {
    let results = extract_results_ref(results)?;
    let config = config.map_or_else(CovenantForecastConfig::default, |c| c.inner.clone());
    finstack_quant_statements_analytics::analysis::forecast_breaches(
        &results,
        &covenants.inner,
        config,
    )
    .map(|breaches| {
        breaches
            .into_iter()
            .map(PyFutureBreach::from_inner)
            .collect()
    })
    .map_err(core_to_py)
}

/// Register the statement covenant functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(evaluate_covenants, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(forecast_breaches, m)?)?;
    Ok(())
}
