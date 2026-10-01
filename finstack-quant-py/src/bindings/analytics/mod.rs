//! Python bindings for the `finstack-quant-analytics` crate.
//!
//! The primary Python-callable entry point is [`Performance`]. All analytics —
//! return transforms, return/risk metrics, periodic returns, benchmark
//! comparisons, and basic factor models — are exposed as methods on a
//! `Performance` instance built from a price or return panel. Four scalar
//! free functions (`sharpe`, `sortino`, `volatility`, `max_drawdown`) cover
//! the single-series case without a panel.

mod performance;
mod regression;
mod types;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `analytics` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "analytics")?;
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
    regression::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "AnalyticsError",
            "BetaResult",
            "DatedSeries",
            "DrawdownEpisode",
            "GreeksResult",
            "LookbackReturns",
            "MultiFactorResult",
            "Performance",
            "PeriodStats",
            "RollingGreeks",
            "constrained_least_squares",
            "max_drawdown",
            "sharpe",
            "sortino",
            "volatility",
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
