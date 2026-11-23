//! Decimal and date-money Python conversion helpers for cashflow builder specs.

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyInt};
use rust_decimal::Decimal;

use crate::bindings::core::money::{PyMoney, decimal_from_py, decimal_to_py, is_python_decimal};
use crate::bindings::core::types::{PyBps, PyRate};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

pub(crate) type DateMoneyRows<'py> = Vec<(Bound<'py, PyAny>, PyMoney)>;

/// Extract a `rust_decimal::Decimal` from `decimal.Decimal`, `float`, `int`,
/// or a numeric `str` (e.g. ``"0.05"``), rejecting `bool`.
///
/// `str`, `int`, and `decimal.Decimal` all go through exact parsing via
/// `finstack_quant_core::decimal::parse_decimal` (or `decimal_from_py`), so
/// large integers never lose precision through `f64` and non-representable text
/// is rejected rather than rounded.
pub(crate) fn decimal_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Decimal> {
    if is_python_decimal(obj)? {
        return decimal_from_py(obj);
    }
    if obj.is_instance_of::<PyBool>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "value cannot be a bool",
        ));
    }
    if obj.is_instance_of::<PyInt>() {
        return finstack_quant_core::decimal::parse_decimal(obj.str()?.to_str()?)
            .map_err(core_to_py);
    }
    if let Ok(text) = obj.extract::<String>() {
        return finstack_quant_core::decimal::parse_decimal(&text).map_err(core_to_py);
    }
    let value: f64 = obj.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("expected decimal.Decimal, float, int, or str")
    })?;
    finstack_quant_core::decimal::f64_to_decimal(value).map_err(|e| {
        crate::errors::value_error(format!(
            "value {value} is not representable as Decimal: {e}"
        ))
    })
}

/// Normalize a contractual decimal rate in its declared units.
pub(crate) fn decimal_rate_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Decimal> {
    if let Ok(rate) = obj.cast::<PyRate>() {
        return finstack_quant_core::decimal::f64_to_decimal(rate.borrow().inner.as_decimal())
            .map_err(core_to_py);
    }
    if let Ok(bps) = obj.cast::<PyBps>() {
        return Ok(Decimal::new(i64::from(bps.borrow().inner.as_bp()), 4));
    }
    decimal_from_any(obj)
}

/// Normalize a contractual quote expressed in basis points.
pub(crate) fn decimal_bps_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Decimal> {
    if let Ok(bps) = obj.cast::<PyBps>() {
        return Ok(Decimal::from(bps.borrow().inner.as_bp()));
    }
    if obj.is_instance_of::<PyRate>() {
        return decimal_rate_from_any(obj)?
            .checked_mul(Decimal::from(10_000))
            .ok_or_else(|| crate::errors::value_error("basis-point quote exceeds Decimal range"));
    }
    decimal_from_any(obj)
}

/// Normalize one unit-sensitive field without changing the caller's mapping.
pub(crate) fn fields_with_decimal_from_py<T: serde::de::DeserializeOwned + Send>(
    py: Python<'_>,
    fields: Option<&Bound<'_, PyDict>>,
    label: &str,
    field: &str,
    convert: fn(&Bound<'_, PyAny>) -> PyResult<Decimal>,
) -> PyResult<T> {
    let Some(fields) = fields else {
        return crate::bindings::module_utils::fields_from_py(py, None, label);
    };
    let Some(value) = fields.get_item(field)? else {
        return crate::bindings::module_utils::fields_from_py(py, Some(fields), label);
    };
    let normalized = fields.copy()?;
    normalized.set_item(field, convert(&value)?.to_string())?;
    crate::bindings::module_utils::fields_from_py(py, Some(&normalized), label)
}

/// Extract a list of `(date, Decimal)` pairs.
pub(crate) fn date_decimal_pairs(
    items: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
) -> PyResult<Vec<(Date, Decimal)>> {
    items
        .iter()
        .map(|(d, v)| Ok((py_to_date(d)?, decimal_from_any(v)?)))
        .collect()
}

pub(crate) fn date_money_to_py<'py>(
    py: Python<'py>,
    items: &[(Date, Money)],
) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
    items
        .iter()
        .map(|(d, m)| Ok((date_to_py(py, *d)?, PyMoney::from_inner(*m))))
        .collect()
}

pub(crate) fn decimal_opt_to_py<'py>(
    py: Python<'py>,
    value: Option<Decimal>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    value.map(|v| decimal_to_py(py, v)).transpose()
}
