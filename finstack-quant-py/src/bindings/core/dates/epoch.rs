//! Date creation and epoch-day conversion Python entry points.

use pyo3::prelude::*;

/// Create a ``datetime.date`` from year, month (1-12), and day.
#[pyfunction]
#[pyo3(name = "create_date", text_signature = "(year, month, day)")]
pub(super) fn py_create_date<'py>(
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
pub(super) fn py_days_since_epoch(date: &Bound<'_, PyAny>) -> PyResult<i32> {
    let d = crate::bindings::date_utils::py_to_date(date)?;
    Ok(finstack_quant_core::dates::days_since_epoch(d))
}

/// Reconstruct a ``datetime.date`` from epoch days (days since 1970-01-01).
#[pyfunction]
#[pyo3(name = "date_from_epoch_days", text_signature = "(days)")]
pub(super) fn py_date_from_epoch_days<'py>(
    py: Python<'py>,
    days: i32,
) -> PyResult<Bound<'py, PyAny>> {
    let date = finstack_quant_core::dates::date_from_epoch_days(days)
        .map_err(crate::errors::core_to_py)?;
    crate::bindings::date_utils::date_to_py(py, date)
}
