//! Python bindings for P&L attribution.
//!
//! Exposes the JSON-spec attribution pipeline and a `PnlAttribution` wrapper
//! for interactive exploration from Python.

mod dataframe;
mod entry;
mod pnl_attribution;

pub(crate) use pnl_attribution::PyPnlAttribution;

use entry::{
    attribute_pnl, attribute_pnl_from_spec, default_attribution_metrics, default_waterfall_order,
    validate_attribution_json,
};
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the attribution submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "attribution")?;
    m.setattr("__doc__", "P&L attribution across multiple methodologies.")?;
    m.add_class::<PyPnlAttribution>()?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_pnl, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_pnl_from_spec, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_attribution_json, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(default_waterfall_order, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(default_attribution_metrics, &m)?)?;
    let all = PyList::new(
        py,
        [
            "PnlAttribution",
            "attribute_pnl",
            "attribute_pnl_from_spec",
            "validate_attribution_json",
            "default_waterfall_order",
            "default_attribution_metrics",
        ],
    )?;
    m.setattr("__all__", all)?;
    parent.add_submodule(&m)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("finstack.attribution", &m)?;
    Ok(())
}
