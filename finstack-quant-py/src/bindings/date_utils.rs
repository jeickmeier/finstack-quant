//! Date conversion helpers shared by the Python bindings.
//!
//! Every Python↔`time::Date` conversion lives here: date-like objects
//! (`py_to_date` / `date_to_py`), ISO-8601 strings (`extract_date` /
//! `extract_date_iso`), and calendar parts (`month_from_u8` / `date_from_ymd`).

use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::errors::{display_to_py, value_error};

/// Convert a Python date-like object or ISO-8601 string to a Rust [`time::Date`].
///
/// Accepts any object exposing integer `year`/`month`/`day` attributes
/// (`datetime.date`, `datetime.datetime`, `pandas.Timestamp`, …) as well as an
/// ISO-8601 `YYYY-MM-DD` string. Timezone information is ignored: a tz-aware
/// timestamp contributes its wall-clock calendar date with no conversion.
///
/// # Errors
///
/// Returns `TypeError` when `obj` is neither a string nor date-like, and
/// `ValueError` when a string is not valid ISO 8601.
pub(crate) fn py_to_date(obj: &Bound<'_, PyAny>) -> PyResult<time::Date> {
    if let Ok(s) = obj.extract::<std::borrow::Cow<'_, str>>() {
        return finstack_quant_core::dates::parse_iso_date(&s).map_err(crate::errors::core_to_py);
    }
    if !(obj.hasattr("year")? && obj.hasattr("month")? && obj.hasattr("day")?) {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "expected a date-like object with year/month/day attributes \
             (datetime.date, datetime.datetime, or pandas.Timestamp) or an \
             ISO-8601 'YYYY-MM-DD' string, got {}",
            obj.get_type().name()?
        )));
    }
    let year: i32 = obj.getattr("year")?.extract()?;
    let month: u8 = obj.getattr("month")?.extract()?;
    let day: u8 = obj.getattr("day")?.extract()?;
    time::Date::from_calendar_date(year, month_from_u8(month)?, day).map_err(display_to_py)
}

/// Convert a Rust [`time::Date`] to a Python `datetime.date`.
pub(crate) fn date_to_py<'py>(py: Python<'py>, date: time::Date) -> PyResult<Bound<'py, PyAny>> {
    let datetime = PyModule::import(py, "datetime")?;
    let date_class = datetime.getattr("date")?;
    date_class.call1((date.year(), date.month() as u8, date.day()))
}

/// Convert a 1-based month number to [`time::Month`], mapping failures to `ValueError`.
pub(crate) fn month_from_u8(value: u8) -> PyResult<time::Month> {
    time::Month::try_from(value).map_err(|_| value_error(format!("invalid month: {value}")))
}

/// Build a [`time::Date`] from calendar parts, mapping both failures to `ValueError`.
pub(crate) fn date_from_ymd(year: i32, month: u8, day: u8) -> PyResult<time::Date> {
    finstack_quant_core::dates::create_date(year, month_from_u8(month)?, day)
        .map_err(crate::errors::core_to_py)
}

/// Accept either an ISO 8601 string or a Python date-like object.
///
/// Date-valued parameters split into two historically disjoint groups: typed
/// constructors take `datetime.date` (via [`py_to_date`]) while `as_of`
/// parameters took only strings. Quants hit that seam constantly, because the
/// object they already hold is whichever one the *other* group wanted. This
/// accepts both.
///
/// The string extraction is attempted first because it is a cheap type check,
/// not a parse: a `datetime.date`, `datetime.datetime` or `pandas.Timestamp`
/// fails it immediately and falls through to the attribute probe.
///
/// # Errors
///
/// Returns `TypeError` when `obj` is neither a string nor date-like, and
/// `ValueError` when a string is not valid ISO 8601.
pub(crate) fn extract_date(obj: &Bound<'_, PyAny>) -> PyResult<time::Date> {
    py_to_date(obj)
}

/// Like [`extract_date`], but yields the ISO 8601 string the canonical crate
/// entry points take.
///
/// A string argument is validated and passed through unchanged, so the only
/// formatting happens on the date-object path.
pub(crate) fn extract_date_iso(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(s) = obj.extract::<std::borrow::Cow<'_, str>>() {
        finstack_quant_core::dates::parse_iso_date(&s).map_err(crate::errors::core_to_py)?;
        return Ok(s.into_owned());
    }
    Ok(py_to_date(obj)?.to_string())
}
