//! Python bindings for the `finstack-core` crate.

mod config;
mod credit;
pub(crate) mod currency;
pub mod dates;
pub mod market_data;
mod math;
pub(crate) mod money;
mod rating_scales;
mod types;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `core` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "core")?;
    m.setattr("__doc__", "Bindings for the finstack-core crate.")?;
    m.setattr("__package__", "finstack.core")?;

    config::register(py, &m)?;
    types::register(py, &m)?;
    currency::register(py, &m)?;
    money::register(py, &m)?;
    math::register(py, &m)?;
    dates::register(py, &m)?;
    market_data::register(py, &m)?;
    credit::register(py, &m)?;
    rating_scales::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "config",
            "types",
            "currency",
            "money",
            "math",
            "dates",
            "market_data",
            "credit",
            "rating_scales",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_by_package(
        py, parent, &m, "core", "finstack",
    )?;
    Ok(())
}
