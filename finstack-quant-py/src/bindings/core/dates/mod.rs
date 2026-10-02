//! Python bindings for the `finstack-quant-core` dates module.

pub mod calendar;
pub mod date_ext;
pub mod daycount;
pub mod imm;
pub mod periods;
pub mod schedule;
pub mod sifma;
pub mod tenor;

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

/// Create a ``datetime.date`` from year, month (1-12), and day.
#[pyfunction]
#[pyo3(name = "create_date", text_signature = "(year, month, day)")]
fn py_create_date<'py>(
    py: Python<'py>,
    year: i32,
    month: u8,
    day: u8,
) -> PyResult<Bound<'py, PyAny>> {
    crate::bindings::date_utils::date_to_py(
        py,
        crate::bindings::date_utils::date_from_ymd(year, month, day)?,
    )
}

/// Return the number of days since the Unix epoch (1970-01-01) for a date.
///
/// # Arguments
///
/// * `date` - Calendar date (`datetime.date` or date-like).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date.
#[pyfunction]
#[pyo3(name = "days_since_epoch", text_signature = "(date)")]
fn py_days_since_epoch(date: &Bound<'_, PyAny>) -> PyResult<i32> {
    let d = crate::bindings::date_utils::py_to_date(date)?;
    Ok(finstack_quant_core::dates::days_since_epoch(d))
}

/// Reconstruct a ``datetime.date`` from epoch days (days since 1970-01-01).
#[pyfunction]
#[pyo3(name = "date_from_epoch_days", text_signature = "(days)")]
fn py_date_from_epoch_days<'py>(py: Python<'py>, days: i32) -> PyResult<Bound<'py, PyAny>> {
    let date = finstack_quant_core::dates::date_from_epoch_days(days).ok_or_else(|| {
        crate::errors::value_error(format!("epoch days {days} out of valid date range"))
    })?;
    crate::bindings::date_utils::date_to_py(py, date)
}
