//! Python bindings for IMM, CDS-roll, and listed-option expiry dates.

use finstack_quant_core::dates::{
    imm_option_expiry, is_cds_date, is_imm_date, next_cds_date, next_imm, next_imm_option_expiry,
    next_semiannual_cds_maturity, next_third_friday, prev_cds_date, prev_cds_semiannual_roll,
    third_friday, third_wednesday,
};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

/// Public names registered by this module.
pub const EXPORTS: &[&str] = &[
    "third_wednesday",
    "third_friday",
    "next_imm",
    "is_imm_date",
    "is_cds_date",
    "next_cds_date",
    "prev_cds_date",
    "prev_cds_semiannual_roll",
    "next_semiannual_cds_maturity",
    "imm_option_expiry",
    "next_imm_option_expiry",
    "next_third_friday",
];

/// Third Wednesday of the given month — the IMM date convention.
///
/// # Arguments
///
/// * `month` - Gregorian month number from 1 through 12.
/// * `year` - Calendar year; the selected date must fit Python years 1 through 9999.
///
/// # Errors
///
/// Returns `ValueError` for an invalid month or an unrepresentable year/date.
#[pyfunction(name = "third_wednesday")]
#[pyo3(text_signature = "(month, year)")]
fn py_third_wednesday<'py>(py: Python<'py>, month: u8, year: i32) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        third_wednesday(crate::bindings::date_utils::month_from_u8(month)?, year)
            .map_err(core_to_py)?,
    )
}

/// Third Friday of the given month — the listed-equity-option expiry
/// convention.
///
/// # Arguments
///
/// * `month` - Gregorian month number from 1 through 12.
/// * `year` - Calendar year; the selected date must fit Python years 1 through 9999.
///
/// # Errors
///
/// Returns `ValueError` for an invalid month or an unrepresentable year/date.
#[pyfunction(name = "third_friday")]
#[pyo3(text_signature = "(month, year)")]
fn py_third_friday<'py>(py: Python<'py>, month: u8, year: i32) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        third_friday(crate::bindings::date_utils::month_from_u8(month)?, year)
            .map_err(core_to_py)?,
    )
}

/// Next quarterly IMM date strictly after `date`.
///
/// # Arguments
///
/// * `date` - Reference calendar date (`datetime.date`, `datetime.datetime`,
///   or any object with integer `year`/`month`/`day` attributes).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date or no selected roll date is
/// representable in Python years 1 through 9999.
#[pyfunction(name = "next_imm")]
#[pyo3(text_signature = "(date)")]
fn py_next_imm<'py>(py: Python<'py>, date: &Bound<'_, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(py, next_imm(py_to_date(date)?).map_err(core_to_py)?)
}

/// True when `date` is a quarterly IMM date.
///
/// # Arguments
///
/// * `date` - Candidate calendar date (`datetime.date` or date-like).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date.
#[pyfunction(name = "is_imm_date")]
#[pyo3(text_signature = "(date)")]
fn py_is_imm_date(date: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(is_imm_date(py_to_date(date)?))
}

/// True when `date` is a standard CDS roll date.
///
/// # Arguments
///
/// * `date` - Candidate calendar date (`datetime.date` or date-like).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date.
#[pyfunction(name = "is_cds_date")]
#[pyo3(text_signature = "(date)")]
fn py_is_cds_date(date: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(is_cds_date(py_to_date(date)?))
}

/// Next standard CDS roll date strictly after `date`.
///
/// # Arguments
///
/// * `date` - Reference calendar date (`datetime.date` or date-like).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date or no selected roll date is
/// representable in Python years 1 through 9999.
#[pyfunction(name = "next_cds_date")]
#[pyo3(text_signature = "(date)")]
fn py_next_cds_date<'py>(py: Python<'py>, date: &Bound<'_, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(py, next_cds_date(py_to_date(date)?).map_err(core_to_py)?)
}

/// Most recent standard CDS roll date strictly before `date`.
///
/// # Arguments
///
/// * `date` - Reference calendar date (`datetime.date` or date-like).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date or no selected roll date is
/// representable in Python years 1 through 9999.
#[pyfunction(name = "prev_cds_date")]
#[pyo3(text_signature = "(date)")]
fn py_prev_cds_date<'py>(py: Python<'py>, date: &Bound<'_, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(py, prev_cds_date(py_to_date(date)?).map_err(core_to_py)?)
}

/// Most recent semi-annual (March / September) CDS roll on or before `date`.
///
/// # Arguments
///
/// * `date` - Reference calendar date (`datetime.date` or date-like).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date or no selected roll date is
/// representable in Python years 1 through 9999.
#[pyfunction(name = "prev_cds_semiannual_roll")]
#[pyo3(text_signature = "(date)")]
fn py_prev_cds_semiannual_roll<'py>(
    py: Python<'py>,
    date: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        prev_cds_semiannual_roll(py_to_date(date)?).map_err(core_to_py)?,
    )
}

/// Next semi-annual CDS maturity date on or after `date`.
///
/// # Arguments
///
/// * `date` - Reference calendar date (`datetime.date` or date-like).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date or no selected roll date is
/// representable in Python years 1 through 9999.
#[pyfunction(name = "next_semiannual_cds_maturity")]
#[pyo3(text_signature = "(date)")]
fn py_next_semiannual_cds_maturity<'py>(
    py: Python<'py>,
    date: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        next_semiannual_cds_maturity(py_to_date(date)?).map_err(core_to_py)?,
    )
}

/// Expiry of the option on the IMM future for the given month.
///
/// # Arguments
///
/// * `month` - Gregorian month number from 1 through 12.
/// * `year` - Calendar year; the selected date must fit Python years 1 through 9999.
///
/// # Errors
///
/// Returns `ValueError` for an invalid month or an unrepresentable year/date.
#[pyfunction(name = "imm_option_expiry")]
#[pyo3(text_signature = "(month, year)")]
fn py_imm_option_expiry<'py>(py: Python<'py>, month: u8, year: i32) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        imm_option_expiry(crate::bindings::date_utils::month_from_u8(month)?, year)
            .map_err(core_to_py)?,
    )
}

/// Next quarterly IMM option expiry strictly after `date`.
///
/// # Arguments
///
/// * `date` - Reference calendar date (`datetime.date` or date-like).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date or no selected roll date is
/// representable in Python years 1 through 9999.
#[pyfunction(name = "next_imm_option_expiry")]
#[pyo3(text_signature = "(date)")]
fn py_next_imm_option_expiry<'py>(
    py: Python<'py>,
    date: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        next_imm_option_expiry(py_to_date(date)?).map_err(core_to_py)?,
    )
}

/// Next unadjusted monthly third Friday strictly after `date`.
///
/// Exchange holidays are not applied. This primitive does not determine a
/// listed option's expiration or last trading date.
///
/// # Arguments
///
/// * `date` - Reference calendar date (`datetime.date` or date-like).
///
/// # Errors
///
/// Returns `TypeError` if `date` is not date-like, or `ValueError` if those
/// attributes do not form a valid calendar date or no selected roll date is
/// representable in Python years 1 through 9999.
#[pyfunction(name = "next_third_friday")]
#[pyo3(text_signature = "(date)")]
fn py_next_third_friday<'py>(
    py: Python<'py>,
    date: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        next_third_friday(py_to_date(date)?).map_err(core_to_py)?,
    )
}

pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(py_third_wednesday, module)?)?;
    module.add_function(wrap_pyfunction!(py_third_friday, module)?)?;
    module.add_function(wrap_pyfunction!(py_next_imm, module)?)?;
    module.add_function(wrap_pyfunction!(py_is_imm_date, module)?)?;
    module.add_function(wrap_pyfunction!(py_is_cds_date, module)?)?;
    module.add_function(wrap_pyfunction!(py_next_cds_date, module)?)?;
    module.add_function(wrap_pyfunction!(py_prev_cds_date, module)?)?;
    module.add_function(wrap_pyfunction!(py_prev_cds_semiannual_roll, module)?)?;
    module.add_function(wrap_pyfunction!(py_next_semiannual_cds_maturity, module)?)?;
    module.add_function(wrap_pyfunction!(py_imm_option_expiry, module)?)?;
    module.add_function(wrap_pyfunction!(py_next_imm_option_expiry, module)?)?;
    module.add_function(wrap_pyfunction!(py_next_third_friday, module)?)?;
    Ok(())
}
