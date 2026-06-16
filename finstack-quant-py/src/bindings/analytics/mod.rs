//! Python bindings for the `finstack-quant-analytics` crate.
//!
//! The only Python-callable entry point is [`Performance`]. All analytics —
//! return transforms, return/risk metrics, periodic returns, benchmark
//! comparisons, and basic factor models — are exposed as methods on a
//! `Performance` instance built from a price or return panel.

mod performance;
mod types;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `analytics` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "analytics")?;
    m.setattr(
        "__doc__",
        "Performance analytics centred on the Performance class.",
    )?;
    m.add(
        "AnalyticsError",
        py.get_type::<crate::errors::AnalyticsError>(),
    )?;

    types::register(py, &m)?;
    performance::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "AnalyticsError",
            "Performance",
            "LookbackReturns",
            "PeriodStats",
            "BetaResult",
            "GreeksResult",
            "RollingGreeks",
            "MultiFactorResult",
            "DrawdownEpisode",
            "DatedSeries",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "analytics",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}
