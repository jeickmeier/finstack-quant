//! Python bindings for vectorized panel feature transforms.

use crate::bindings::module_utils::{py_to_json_value, register_submodule, ParentNameSource};
use crate::errors::core_to_py;
use pyo3::prelude::*;
use pyo3::types::PyList;
use serde_json::Value;

/// Transform a time-series panel column per entity.
#[pyfunction]
#[pyo3(signature = (values, entity, order, op, params=None))]
fn transform_timeseries(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    entity: Vec<String>,
    order: Vec<String>,
    op: &str,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "time-series transform params")?;
    finstack_quant_features::transform_timeseries(&values, &entity, &order, op, params.as_ref())
        .map_err(core_to_py)
}

/// Transform a cross-section per timestamp.
#[pyfunction]
#[pyo3(signature = (values, time_key, op, params=None))]
fn transform_cross_sectional(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: Vec<String>,
    op: &str,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "cross-sectional transform params")?;
    finstack_quant_features::transform_cross_sectional(&values, &time_key, op, params.as_ref())
        .map_err(core_to_py)
}

/// Transform a cross-section within each time/group sub-partition.
#[pyfunction]
#[pyo3(signature = (values, time_key, groups, op, params=None))]
fn transform_cross_sectional_grouped(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: Vec<String>,
    groups: Vec<String>,
    op: &str,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "grouped cross-sectional transform params")?;
    finstack_quant_features::transform_cross_sectional_grouped(
        &values,
        &time_key,
        &groups,
        op,
        params.as_ref(),
    )
    .map_err(core_to_py)
}

/// Remove cross-sectional exposure effects by OLS residualization.
#[pyfunction]
#[pyo3(signature = (values, time_key, exposures, params=None))]
fn neutralize(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: Vec<String>,
    exposures: Vec<Vec<Option<f64>>>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "neutralize params")?;
    finstack_quant_features::neutralize(&values, &time_key, &exposures, params.as_ref())
        .map_err(core_to_py)
}

/// Transform two time-series panel columns per entity.
#[pyfunction]
#[pyo3(signature = (values, other, entity, order, op, params=None))]
fn transform_timeseries_pairwise(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    other: Vec<Option<f64>>,
    entity: Vec<String>,
    order: Vec<String>,
    op: &str,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "pairwise time-series transform params")?;
    finstack_quant_features::transform_timeseries_pairwise(
        &values,
        &other,
        &entity,
        &order,
        op,
        params.as_ref(),
    )
    .map_err(core_to_py)
}

/// Return rolling OLS residuals per entity.
#[pyfunction]
#[pyo3(signature = (values, exposures, entity, order, params=None))]
fn rolling_regression_residual(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    exposures: Vec<Vec<Option<f64>>>,
    entity: Vec<String>,
    order: Vec<String>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "rolling regression residual params")?;
    finstack_quant_features::rolling_regression_residual(
        &values,
        &exposures,
        &entity,
        &order,
        params.as_ref(),
    )
    .map_err(core_to_py)
}

/// Convert a signal to inverse-risk-scaled weights per timestamp.
#[pyfunction]
#[pyo3(signature = (values, time_key, volatility, params=None))]
fn risk_scaled_weights(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: Vec<String>,
    volatility: Vec<Option<f64>>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "risk scaled weights params")?;
    finstack_quant_features::risk_scaled_weights(&values, &time_key, &volatility, params.as_ref())
        .map_err(core_to_py)
}

/// Apply the default signal cleaning pass.
#[pyfunction]
#[pyo3(signature = (values, time_key, params=None))]
fn clean_signal(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: Vec<String>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "clean signal params")?;
    finstack_quant_features::clean_signal(&values, &time_key, params.as_ref()).map_err(core_to_py)
}

/// Normalize a signal cross-sectionally.
#[pyfunction]
#[pyo3(signature = (values, time_key, params=None))]
fn normalize_signal(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: Vec<String>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "normalize signal params")?;
    finstack_quant_features::normalize_signal(&values, &time_key, params.as_ref())
        .map_err(core_to_py)
}

/// Convert ranks into long/short weights.
#[pyfunction]
#[pyo3(signature = (values, time_key, params=None))]
fn rank_to_weights(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: Vec<String>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "rank to weights params")?;
    finstack_quant_features::rank_to_weights(&values, &time_key, params.as_ref())
        .map_err(core_to_py)
}

/// Neutralize a signal and z-score residuals.
#[pyfunction]
#[pyo3(signature = (values, time_key, exposures, params=None))]
fn neutralize_and_zscore(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: Vec<String>,
    exposures: Vec<Vec<Option<f64>>>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let params = parse_params(py, params, "neutralize and zscore params")?;
    finstack_quant_features::neutralize_and_zscore(&values, &time_key, &exposures, params.as_ref())
        .map_err(core_to_py)
}

/// Apply a JSON panel transform pipeline.
#[pyfunction]
fn transform_panel(spec_json: &str) -> PyResult<String> {
    finstack_quant_features::transform_panel(spec_json).map_err(core_to_py)
}

/// Register the features submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "features")?;
    m.setattr("__doc__", "Vectorized panel feature transforms.")?;
    m.add_function(wrap_pyfunction!(transform_timeseries, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_cross_sectional, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_cross_sectional_grouped, &m)?)?;
    m.add_function(wrap_pyfunction!(neutralize, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_timeseries_pairwise, &m)?)?;
    m.add_function(wrap_pyfunction!(rolling_regression_residual, &m)?)?;
    m.add_function(wrap_pyfunction!(risk_scaled_weights, &m)?)?;
    m.add_function(wrap_pyfunction!(clean_signal, &m)?)?;
    m.add_function(wrap_pyfunction!(normalize_signal, &m)?)?;
    m.add_function(wrap_pyfunction!(rank_to_weights, &m)?)?;
    m.add_function(wrap_pyfunction!(neutralize_and_zscore, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_panel, &m)?)?;
    let all = PyList::new(
        py,
        [
            "clean_signal",
            "neutralize",
            "neutralize_and_zscore",
            "normalize_signal",
            "rank_to_weights",
            "risk_scaled_weights",
            "rolling_regression_residual",
            "transform_cross_sectional",
            "transform_cross_sectional_grouped",
            "transform_panel",
            "transform_timeseries",
            "transform_timeseries_pairwise",
        ],
    )?;
    m.setattr("__all__", all)?;
    register_submodule(
        py,
        parent,
        &m,
        "features",
        crate::bindings::module_utils::ROOT_PACKAGE,
        ParentNameSource::Name,
    )
}

fn parse_params(
    py: Python<'_>,
    params: Option<&Bound<'_, PyAny>>,
    label: &str,
) -> PyResult<Option<Value>> {
    params
        .map(|value| py_to_json_value(py, value, label))
        .transpose()
}
