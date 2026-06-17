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
    m.add_function(wrap_pyfunction!(transform_panel, &m)?)?;
    let all = PyList::new(
        py,
        [
            "transform_cross_sectional",
            "transform_panel",
            "transform_timeseries",
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
