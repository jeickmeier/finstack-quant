//! Python bindings for the `finstack-quant-core` crate.

pub(crate) mod config;
pub(crate) mod currency;
pub mod dates;
pub mod market_data;
mod math;
pub(crate) mod money;
mod rating_scales;
mod schema;
pub(crate) mod table;
pub(crate) mod types;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `core` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "core")?;
    m.setattr("__doc__", "Bindings for the finstack-quant-core crate.")?;

    config::register(py, &m)?;
    types::register(py, &m)?;
    currency::register(py, &m)?;
    money::register(py, &m)?;
    math::register(py, &m)?;
    dates::register(py, &m)?;
    market_data::register(py, &m)?;
    rating_scales::register(py, &m)?;
    table::register(py, &m)?;

    // `FinstackError` is declared with `module = finstack_quant.core`, so this
    // is its canonical home. Exporting it here is what makes that declaration
    // true: without it `repr()` names an unreachable module and pickling an
    // exception instance cannot resolve the class.
    m.setattr(
        "FinstackError",
        py.get_type::<crate::errors::FinstackError>(),
    )?;

    schema::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "FinstackError",
            "config",
            "currency",
            "dates",
            "market_data",
            "math",
            "money",
            "rating_scales",
            "schema",
            "table",
            "types",
        ],
    )?;
    m.setattr("__all__", all)?;
    // `Exposure::Python`: the pure-Python package `finstack_quant/core/__init__.py`
    // owns `finstack_quant.core`; registering the compiled module there would
    // leave that package (its docstring, doctest and sorted `__all__`) unreachable.
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
