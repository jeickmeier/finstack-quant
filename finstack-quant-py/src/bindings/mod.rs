//! Binding module tree mirroring the Rust umbrella crate structure.
//!
//! Each submodule corresponds to one Rust crate domain and is responsible
//! only for that domain's type conversion and registration.

use pyo3::prelude::*;
use pyo3::types::PyList;

pub mod analytics;
pub mod attribution;
pub mod cashflows;
pub mod core;
pub mod covenants;
pub(crate) mod date_utils;
pub(crate) mod extract;
pub mod factor_model;
pub mod margin;
pub(crate) mod module_utils;
pub mod monte_carlo;
pub(crate) mod pandas_utils;
pub mod portfolio;
pub mod scenarios;
pub mod statements;
pub mod statements_analytics;
pub mod valuations;

/// Register all binding domains under the top-level `finstack_quant` module.
pub fn register_root(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.setattr("__package__", "finstack_quant")?;

    core::register(py, m)?;
    analytics::register(py, m)?;
    attribution::register(py, m)?;
    cashflows::register(py, m)?;
    covenants::register(py, m)?;
    factor_model::register(py, m)?;
    monte_carlo::register(py, m)?;
    margin::register(py, m)?;
    valuations::register(py, m)?;
    statements::register(py, m)?;
    statements_analytics::register(py, m)?;
    portfolio::register(py, m)?;
    scenarios::register(py, m)?;

    let all = PyList::new(
        py,
        [
            "core",
            "analytics",
            "attribution",
            "cashflows",
            "covenants",
            "factor_model",
            "monte_carlo",
            "margin",
            "valuations",
            "statements",
            "statements_analytics",
            "portfolio",
            "scenarios",
        ],
    )?;
    m.setattr("__all__", all)?;

    Ok(())
}
