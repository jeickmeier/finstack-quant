//! Python bindings for `finstack_core::credit`.

mod lgd;
mod migration;
mod pd;
mod scoring;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `credit` submodule on the parent `core` module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "credit")?;
    m.setattr(
        "__doc__",
        "Credit risk models: academic scoring (Altman, Ohlson, Zmijewski), PD calibration, and LGD / EAD.",
    )?;

    let qual = crate::bindings::module_utils::set_submodule_package_by_package(
        parent,
        &m,
        "credit",
        "finstack.core",
    )?;

    scoring::register(py, &m)?;
    pd::register(py, &m)?;
    lgd::register(py, &m)?;
    migration::register(py, &m)?;

    let all = PyList::new(py, ["scoring", "pd", "lgd", "migration"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;

    Ok(())
}
