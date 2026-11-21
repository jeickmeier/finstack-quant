//! Date conversion helpers shared by the Python bindings.
//!
//! Every Python↔`finstack_quant_core::dates::Date` conversion lives here: date-like objects
//! (`py_to_date` / `date_to_py`), ISO-8601 strings (`py_to_date` /
//! `extract_date_iso`), and calendar parts (`month_from_u8` / `date_from_ymd`).

use pyo3::prelude::*;
use pyo3::types::PyDate;

use crate::errors::value_error;

fn parse_iso_date_or_datetime(
    s: &str,
) -> finstack_quant_core::Result<finstack_quant_core::dates::Date> {
    match finstack_quant_core::dates::parse_iso_date(s) {
        Ok(date) => Ok(date),
        Err(orig_err) => {
            let candidate: std::borrow::Cow<'_, str> =
                if s.len() > 10 && s.as_bytes().get(10) == Some(&b' ') {
                    let mut owned = s.to_owned();
                    if owned.is_char_boundary(10) && owned.is_char_boundary(11) {
                        owned.replace_range(10..11, "T");
                    }
                    std::borrow::Cow::Owned(owned)
                } else {
                    std::borrow::Cow::Borrowed(s)
                };
            let fmt = time::format_description::well_known::Iso8601::DEFAULT;
            if let Ok(dt) = time::OffsetDateTime::parse(&candidate, &fmt) {
                return Ok(dt.date());
            }
            if let Ok(dt) = time::PrimitiveDateTime::parse(&candidate, &fmt) {
                return Ok(dt.date());
            }
            Err(orig_err)
        }
    }
}

/// Convert a Python date-like object or ISO-8601 string to a Rust [`finstack_quant_core::dates::Date`].
///
/// Accepts any object exposing integer `year`/`month`/`day` attributes
/// (`datetime.date`, `datetime.datetime`, `pandas.Timestamp`, …),
/// `numpy.datetime64`, as well as ISO-8601 date (`YYYY-MM-DD`) and datetime
/// (`YYYY-MM-DDTHH:MM:SS[Z|+HH:MM]`) strings. Timezone information is ignored:
/// a tz-aware timestamp contributes its wall-clock calendar date with no conversion.
///
/// # Errors
///
/// Returns `TypeError` when `obj` is not date-like, and `ValueError` when a
/// string is invalid ISO 8601 or the input is `NaT` (`pandas.NaT` / `numpy.datetime64('NaT')`).
pub(crate) fn py_to_date(obj: &Bound<'_, PyAny>) -> PyResult<finstack_quant_core::dates::Date> {
    if let Ok(s) = obj.extract::<std::borrow::Cow<'_, str>>() {
        return parse_iso_date_or_datetime(&s).map_err(crate::errors::core_to_py);
    }
    let type_name = obj.get_type().name()?;
    if type_name == "NaTType" {
        return Err(value_error("date cannot be NaT"));
    }
    if type_name == "datetime64" {
        let day_obj = obj
            .call_method1("astype", ("datetime64[D]",))?
            .call_method0("item")?;
        if day_obj.is_none() {
            return Err(value_error("date cannot be NaT"));
        }
        return py_to_date(&day_obj);
    }
    if !(obj.hasattr("year")? && obj.hasattr("month")? && obj.hasattr("day")?) {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "expected a date-like object with year/month/day attributes \
             (datetime.date, datetime.datetime, pandas.Timestamp, numpy.datetime64) or an \
             ISO-8601 'YYYY-MM-DD' string, got {}",
            type_name
        )));
    }
    let year: i32 = obj.getattr("year")?.extract()?;
    let month: u8 = obj.getattr("month")?.extract()?;
    let day: u8 = obj.getattr("day")?.extract()?;
    date_from_ymd(year, month, day)
}

/// Convert a Rust [`finstack_quant_core::dates::Date`] to a Python `datetime.date`.
pub(crate) fn date_to_py<'py>(
    py: Python<'py>,
    date: finstack_quant_core::dates::Date,
) -> PyResult<Bound<'py, PyAny>> {
    PyDate::new(py, date.year(), date.month() as u8, date.day()).map(Bound::into_any)
}

/// Convert a 1-based month number to [`time::Month`], mapping failures to `ValueError`.
pub(crate) fn month_from_u8(value: u8) -> PyResult<time::Month> {
    time::Month::try_from(value).map_err(|_| value_error(format!("invalid month: {value}")))
}

/// Build a [`finstack_quant_core::dates::Date`] from calendar parts, mapping both failures to `ValueError`.
pub(crate) fn date_from_ymd(
    year: i32,
    month: u8,
    day: u8,
) -> PyResult<finstack_quant_core::dates::Date> {
    finstack_quant_core::dates::create_date(year, month_from_u8(month)?, day)
        .map_err(crate::errors::core_to_py)
}

/// Like [`py_to_date`], but yields the ISO 8601 string the canonical crate
/// entry points take.
///
/// A string argument is validated and passed through unchanged, so the only
/// formatting happens on the date-object path.
pub(crate) fn extract_date_iso(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(s) = obj.extract::<std::borrow::Cow<'_, str>>() {
        let parsed = parse_iso_date_or_datetime(&s).map_err(crate::errors::core_to_py)?;
        if s.len() == 10 && s.as_bytes().get(4) == Some(&b'-') && s.as_bytes().get(7) == Some(&b'-')
        {
            return Ok(s.into_owned());
        }
        return Ok(parsed.to_string());
    }
    Ok(py_to_date(obj)?.to_string())
}

/// Convert a slice of `finstack_quant_core::dates::Date` into a list of Python `datetime.date` objects.
pub(crate) fn dates_to_pylist<'py>(
    py: Python<'py>,
    dates: &[finstack_quant_core::dates::Date],
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    dates.iter().map(|&d| date_to_py(py, d)).collect()
}
