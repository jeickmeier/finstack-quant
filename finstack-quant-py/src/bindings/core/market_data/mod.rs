//! Python bindings for `finstack_quant_core::market_data` term structures and context.

pub mod context;
pub mod curves;
pub mod fx;
pub mod scalars;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `finstack_quant.core.market_data` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "market_data")?;
    m.setattr(
        "__doc__",
        "Bindings for finstack-quant-core market data: curves, vol surfaces, FX, and market context.",
    )?;

    curves::register(&m)?;
    fx::register(&m)?;
    scalars::register(&m)?;
    context::register(&m)?;

    let mut all_names = Vec::with_capacity(
        curves::EXPORTS.len() + fx::EXPORTS.len() + scalars::EXPORTS.len() + context::EXPORTS.len(),
    );
    all_names.extend_from_slice(curves::EXPORTS);
    all_names.extend_from_slice(fx::EXPORTS);
    all_names.extend_from_slice(scalars::EXPORTS);
    all_names.extend_from_slice(context::EXPORTS);

    all_names.sort_unstable();
    let all = PyList::new(py, &all_names)?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
