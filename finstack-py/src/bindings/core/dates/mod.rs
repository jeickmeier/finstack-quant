//! Python bindings for the `finstack-core` dates module.

pub mod calendar;
pub mod daycount;
pub mod periods;
pub mod schedule;
pub mod tenor;
pub mod utils;

use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Register the `finstack.core.dates` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "dates")?;
    m.setattr(
        "__doc__",
        "Date, calendar, and schedule utilities from finstack-core.",
    )?;

    daycount::register(&m)?;
    tenor::register(&m)?;
    periods::register(&m)?;
    calendar::register(&m)?;
    schedule::register(&m)?;

    m.add_function(wrap_pyfunction!(py_create_date, &m)?)?;
    m.add_function(wrap_pyfunction!(py_days_since_epoch, &m)?)?;
    m.add_function(wrap_pyfunction!(py_date_from_epoch_days, &m)?)?;

    let mut all_names: Vec<&str> = Vec::new();
    all_names.extend_from_slice(daycount::EXPORTS);
    all_names.extend_from_slice(tenor::EXPORTS);
    all_names.extend_from_slice(periods::EXPORTS);
    all_names.extend_from_slice(calendar::EXPORTS);
    all_names.extend_from_slice(schedule::EXPORTS);
    all_names.extend_from_slice(&["create_date", "days_since_epoch", "date_from_epoch_days"]);

    let all = PyList::new(py, &all_names)?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "dates",
        "finstack.core",
        crate::bindings::module_utils::ParentNameSource::Package,
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
    let m = time::Month::try_from(month)
        .map_err(|_| crate::errors::value_error(format!("invalid month: {month}")))?;
    let date =
        finstack_core::dates::create_date(year, m, day).map_err(crate::errors::core_to_py)?;
    utils::date_to_py(py, date)
}

/// Return the number of days since the Unix epoch (1970-01-01) for a date.
#[pyfunction]
#[pyo3(name = "days_since_epoch", text_signature = "(date)")]
fn py_days_since_epoch(date: &Bound<'_, PyAny>) -> PyResult<i32> {
    let d = utils::py_to_date(date)?;
    Ok(finstack_core::dates::days_since_epoch(d))
}

/// Reconstruct a ``datetime.date`` from epoch days (days since 1970-01-01).
#[pyfunction]
#[pyo3(name = "date_from_epoch_days", text_signature = "(days)")]
fn py_date_from_epoch_days<'py>(py: Python<'py>, days: i32) -> PyResult<Bound<'py, PyAny>> {
    let date = finstack_core::dates::date_from_epoch_days(days).ok_or_else(|| {
        crate::errors::value_error(format!("epoch days {days} out of valid date range"))
    })?;
    utils::date_to_py(py, date)
}
