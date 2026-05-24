//! Date conversion helpers between Python `datetime.date` and `time::Date`.

use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::PyModule;

/// Convert a Python `datetime.date` to a Rust [`time::Date`].
pub fn py_to_date(obj: &Bound<'_, PyAny>) -> PyResult<time::Date> {
    let year: i32 = obj.getattr("year")?.extract()?;
    let month: u8 = obj.getattr("month")?.extract()?;
    let day: u8 = obj.getattr("day")?.extract()?;
    let m = time::Month::try_from(month)
        .map_err(|_| crate::errors::value_error(format!("invalid month: {month}")))?;
    time::Date::from_calendar_date(year, m, day).map_err(display_to_py)
}

/// Convert a Rust [`time::Date`] to a Python `datetime.date`.
pub fn date_to_py<'py>(py: Python<'py>, date: time::Date) -> PyResult<Bound<'py, PyAny>> {
    let datetime = PyModule::import(py, "datetime")?;
    let date_class = datetime.getattr("date")?;
    date_class.call1((date.year(), date.month() as u8, date.day()))
}
