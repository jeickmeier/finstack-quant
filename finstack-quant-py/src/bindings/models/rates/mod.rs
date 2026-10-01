//! Python bindings for product-independent interest-rate models.

pub mod dtsm;
pub mod hull_white;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `finstack_quant.models.rates` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "rates")?;
    module.setattr(
        "__doc__",
        "Product-independent interest-rate models and statistical term-structure engines.",
    )?;

    dtsm::register(py, &module)?;
    hull_white::register(py, &module)?;
    module.setattr("__all__", PyList::new(py, ["dtsm", "hull_white"])?)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
