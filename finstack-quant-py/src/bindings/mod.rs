//! Binding module tree mirroring the Rust umbrella crate structure.
//!
//! Each submodule corresponds to one Rust crate domain and is responsible
//! only for that domain's type conversion and registration.

use pyo3::prelude::*;
use pyo3::types::PyList;

pub mod analytics;
pub mod attribution;
pub mod calibration;
pub mod cashflows;
pub mod core;
pub mod covenants;
pub(crate) mod date_utils;
pub(crate) mod extract;
pub mod features;
pub mod margin;
pub mod models;
pub(crate) mod module_utils;
pub(crate) mod pandas_utils;
pub(crate) mod pickle_support;
pub mod portfolio;
pub(crate) mod repr_support;
pub mod scenarios;
pub mod schema;
pub(crate) mod schema_registry;
pub mod statements;
pub mod statements_analytics;
pub mod valuations;

/// Register all binding domains under the top-level `finstack_quant` module.
pub fn register_root(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.setattr("__package__", module_utils::ROOT_PACKAGE)?;
    // Sourced from the workspace package version at compile time, so the
    // extension can never disagree with the wheel it shipped in.
    m.setattr("__version__", env!("CARGO_PKG_VERSION"))?;

    core::register(py, m)?;
    analytics::register(py, m)?;
    attribution::register(py, m)?;
    calibration::register(py, m)?;
    cashflows::register(py, m)?;
    covenants::register(py, m)?;
    features::register(py, m)?;
    models::register(py, m)?;
    margin::register(py, m)?;
    valuations::register(py, m)?;
    statements::register(py, m)?;
    statements_analytics::register(py, m)?;
    portfolio::register(py, m)?;
    scenarios::register(py, m)?;
    schema::register(py, m)?;

    let all = PyList::new(
        py,
        [
            "core",
            "analytics",
            "attribution",
            "calibration",
            "cashflows",
            "covenants",
            "features",
            "models",
            "margin",
            "valuations",
            "statements",
            "statements_analytics",
            "portfolio",
            "scenarios",
            "schema",
        ],
    )?;
    m.setattr("__all__", all)?;

    Ok(())
}
