//! Python bindings for the `finstack-quant-core` dates module.

pub mod calendar;
pub mod date_ext;
pub mod daycount;
mod epoch;
pub mod imm;
pub mod periods;
pub mod schedule;
pub mod sifma;
pub mod tenor;

use epoch::{py_create_date, py_date_from_epoch_days, py_days_since_epoch};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Register the `finstack_quant.core.dates` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "dates")?;
    m.setattr(
        "__doc__",
        "Date, calendar, and schedule utilities from finstack-quant-core.",
    )?;

    daycount::register(&m)?;
    tenor::register(&m)?;
    periods::register(&m)?;
    calendar::register(&m)?;
    schedule::register(&m)?;
    sifma::register(&m)?;
    imm::register(&m)?;
    date_ext::register(&m)?;

    m.add_function(wrap_pyfunction!(py_create_date, &m)?)?;
    m.add_function(wrap_pyfunction!(py_days_since_epoch, &m)?)?;
    m.add_function(wrap_pyfunction!(py_date_from_epoch_days, &m)?)?;

    let mut all_names: Vec<&str> = Vec::new();
    all_names.extend_from_slice(daycount::EXPORTS);
    all_names.extend_from_slice(tenor::EXPORTS);
    all_names.extend_from_slice(periods::EXPORTS);
    all_names.extend_from_slice(calendar::EXPORTS);
    all_names.extend_from_slice(schedule::EXPORTS);
    all_names.extend_from_slice(sifma::EXPORTS);
    all_names.extend_from_slice(imm::EXPORTS);
    all_names.extend_from_slice(date_ext::EXPORTS);
    all_names.extend_from_slice(&["create_date", "days_since_epoch", "date_from_epoch_days"]);
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
