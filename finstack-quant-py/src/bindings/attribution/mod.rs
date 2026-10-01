//! Python bindings for P&L attribution.
//!
//! Exposes the JSON-spec attribution pipeline and a `PnlAttribution` wrapper
//! for interactive exploration from Python.

mod entry;
pub(crate) mod pnl_attribution;
mod return_contribution;
mod schema;

pub(crate) use pnl_attribution::PyPnlAttribution;
pub(crate) use return_contribution::PyReturnContributionResult;

use entry::{
    attribute_pnl, attribute_pnl_envelope_json, attribute_pnl_many, attribute_return_contribution,
    default_attribution_metrics, default_waterfall_order, pnl_bridge, validate_attribution_json,
    validate_return_contribution_json,
};
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the attribution submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "attribution")?;
    m.setattr("__doc__", "P&L attribution across multiple methodologies.")?;
    m.add_class::<PyPnlAttribution>()?;
    m.add_class::<PyReturnContributionResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_pnl, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_pnl_envelope_json, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_pnl_many, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(pnl_bridge, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_return_contribution, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_attribution_json, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(
        validate_return_contribution_json,
        &m
    )?)?;
    m.add_function(pyo3::wrap_pyfunction!(default_waterfall_order, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(default_attribution_metrics, &m)?)?;
    schema::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "PnlAttribution",
            "ReturnContributionResult",
            "attribute_pnl",
            "attribute_pnl_envelope_json",
            "attribute_pnl_many",
            "attribute_return_contribution",
            "default_attribution_metrics",
            "default_waterfall_order",
            "pnl_bridge",
            "schema",
            "validate_attribution_json",
            "validate_return_contribution_json",
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
