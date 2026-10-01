//! Python bindings for the [`finstack_quant_core::dates::DateExt`] helpers,
//! exposed as free functions over ``datetime.date`` (or ISO strings).

use crate::bindings::core::dates::calendar::extract_calendar;
use crate::bindings::core::dates::periods::PyFiscalConfig;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_quant_core::dates::DateExt;
use pyo3::prelude::*;
use pyo3::types::PyModule;

/// Public names registered by this module.
pub const EXPORTS: &[&str] = &[
    "add_business_days",
    "add_months",
    "add_weekdays",
    "end_of_month",
    "fiscal_year",
    "is_weekend",
    "months_until",
    "quarter",
];

/// Add (or subtract) ``n`` business days to ``date`` under ``calendar``.
///
/// Skips weekends and holidays according to the calendar. Positive ``n``
/// moves forward, negative ``n`` backward; ``0`` returns ``date`` unchanged
/// even when it is not itself a business day.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Anchor date.
/// n : int
///     Signed 32-bit number of business days to move.
/// calendar : HolidayCalendar | str
///     Holiday calendar object or registry id (``"usny"``; ``"nyse+gblo"``
///     joins calendars).
///
/// Returns
/// -------
/// datetime.date
///     The shifted business day.
///
/// Raises
/// ------
/// KeyError
///     If ``calendar`` names an unknown calendar.
/// ValueError
///     If ``date`` is invalid, the shift exceeds the supported date range,
///     or no business day is found within the bounded (100-day) search window.
///
/// OverflowError
///     If the offset is outside the signed 32-bit integer range.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.dates import add_business_days
/// >>> add_business_days(datetime.date(2025, 6, 27), 3, "target2")
/// datetime.date(2025, 7, 2)
#[pyfunction(name = "add_business_days")]
#[pyo3(text_signature = "(date, n, calendar)")]
fn py_add_business_days<'py>(
    py: Python<'py>,
    date: &Bound<'py, PyAny>,
    n: i32,
    calendar: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let cal = extract_calendar(calendar)?;
    let shifted = py_to_date(date)?
        .add_business_days(n, cal)
        .map_err(core_to_py)?;
    date_to_py(py, shifted)
}

/// Add (or subtract) ``n`` weekdays to ``date``, skipping only Saturdays and Sundays.
///
/// Holidays are *not* considered; use ``add_business_days`` with a calendar
/// for holiday-aware arithmetic.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Anchor date.
/// n : int
///     Signed 32-bit number of weekdays to move; ``0`` returns ``date`` unchanged.
///
/// Returns
/// -------
/// datetime.date
///     The shifted weekday.
///
/// Raises
/// ------
/// ValueError
///     If ``date`` is invalid or the shifted date exceeds the supported calendar range.
///
/// OverflowError
///     If the offset is outside the signed 32-bit integer range.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.dates import add_weekdays
/// >>> add_weekdays(datetime.date(2025, 1, 3), 1)
/// datetime.date(2025, 1, 6)
#[pyfunction(name = "add_weekdays")]
#[pyo3(text_signature = "(date, n)")]
fn py_add_weekdays<'py>(
    py: Python<'py>,
    date: &Bound<'py, PyAny>,
    n: i32,
) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(py, py_to_date(date)?.add_weekdays(n).map_err(core_to_py)?)
}

/// Add ``months`` to ``date``, clamping to the last valid day of the target month.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Anchor date.
/// months : int
///     Signed 32-bit number of calendar months (Jan 31 + 1 gives Feb 28/29).
///
/// Returns
/// -------
/// datetime.date
///     The shifted date.
///
/// Raises
/// ------
/// ValueError
///     If ``date`` is invalid or the shifted date exceeds the supported calendar range.
///
/// OverflowError
///     If the offset is outside the signed 32-bit integer range.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.dates import add_months
/// >>> add_months(datetime.date(2024, 1, 31), 1)
/// datetime.date(2024, 2, 29)
#[pyfunction(name = "add_months")]
#[pyo3(text_signature = "(date, months)")]
fn py_add_months<'py>(
    py: Python<'py>,
    date: &Bound<'py, PyAny>,
    months: i32,
) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        py_to_date(date)?.add_months(months).map_err(core_to_py)?,
    )
}

/// Last day of the month containing ``date``.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Any date in the month.
///
/// Returns
/// -------
/// datetime.date
///     The month-end date.
///
/// Raises
/// ------
/// ValueError
///     If ``date`` is not a valid calendar date or ISO string.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import end_of_month
/// >>> end_of_month("2024-02-15")
/// datetime.date(2024, 2, 29)
#[pyfunction(name = "end_of_month")]
#[pyo3(text_signature = "(date)")]
fn py_end_of_month<'py>(py: Python<'py>, date: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(py, py_to_date(date)?.end_of_month())
}

/// Whether ``date`` falls on a Saturday or Sunday.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Date to test.
///
/// Returns
/// -------
/// bool
///     ``True`` for Saturday or Sunday.
///
/// Raises
/// ------
/// ValueError
///     If ``date`` is not a valid calendar date or ISO string.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import is_weekend
/// >>> is_weekend("2025-01-04")
/// True
#[pyfunction(name = "is_weekend")]
#[pyo3(text_signature = "(date)")]
fn py_is_weekend(date: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(py_to_date(date)?.is_weekend())
}

/// Calendar quarter (1-4) containing ``date``.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Date to classify.
///
/// Returns
/// -------
/// int
///     Quarter number from ``1`` (Jan-Mar) to ``4`` (Oct-Dec).
///
/// Raises
/// ------
/// ValueError
///     If ``date`` is not a valid calendar date or ISO string.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import quarter
/// >>> quarter("2025-08-15")
/// 3
#[pyfunction(name = "quarter")]
#[pyo3(text_signature = "(date)")]
fn py_quarter(date: &Bound<'_, PyAny>) -> PyResult<u8> {
    Ok(py_to_date(date)?.quarter())
}

/// Fiscal year label of ``date`` under a fiscal-year configuration.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Date to classify.
/// config : FiscalConfig
///     Fiscal-year start (e.g. ``FiscalConfig.us_federal()`` starts October 1,
///     so 2024-10-15 belongs to fiscal year 2025).
///
/// Returns
/// -------
/// int
///     Fiscal year label (the calendar year in which the fiscal year ends).
///
/// Raises
/// ------
/// ValueError
///     If ``date`` is not a valid calendar date or ISO string.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import FiscalConfig, fiscal_year
/// >>> fiscal_year("2024-10-15", FiscalConfig.us_federal())
/// 2025
#[pyfunction(name = "fiscal_year")]
#[pyo3(text_signature = "(date, config)")]
fn py_fiscal_year(date: &Bound<'_, PyAny>, config: &PyFiscalConfig) -> PyResult<i32> {
    Ok(py_to_date(date)?.fiscal_year(config.inner))
}

/// Whole months from ``date`` to ``other`` (``0`` when ``other`` is earlier).
///
/// Counts complete months: ``(other.year - date.year) * 12 + (other.month -
/// date.month)``, less one when ``other``'s day-of-month has not yet reached
/// ``date``'s (month-end to month-end counts as whole). This is the
/// loan-seasoning convention used by structured-credit models.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Start date.
/// other : datetime.date | str
///     End date.
///
/// Returns
/// -------
/// int
///     Non-negative month count.
///
/// Raises
/// ------
/// ValueError
///     If either argument is not a valid calendar date or ISO string.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import months_until
/// >>> months_until("2020-01-15", "2022-03-10")
/// 25
#[pyfunction(name = "months_until")]
#[pyo3(text_signature = "(date, other)")]
fn py_months_until(date: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<u32> {
    Ok(py_to_date(date)?.months_until(py_to_date(other)?))
}

/// Register the date-extension free functions on the dates module.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(py_add_business_days, module)?)?;
    module.add_function(wrap_pyfunction!(py_add_weekdays, module)?)?;
    module.add_function(wrap_pyfunction!(py_add_months, module)?)?;
    module.add_function(wrap_pyfunction!(py_end_of_month, module)?)?;
    module.add_function(wrap_pyfunction!(py_is_weekend, module)?)?;
    module.add_function(wrap_pyfunction!(py_quarter, module)?)?;
    module.add_function(wrap_pyfunction!(py_fiscal_year, module)?)?;
    module.add_function(wrap_pyfunction!(py_months_until, module)?)?;
    Ok(())
}
