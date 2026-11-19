//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyInt, PyModule};
use rust_decimal::Decimal;

use crate::bindings::core::money::{PyMoney, decimal_from_py, decimal_to_py, is_python_decimal};
use crate::bindings::core::types::{PyBps, PyRate};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

type DateMoneyRows<'py> = Vec<(Bound<'py, PyAny>, PyMoney)>;

mod amortization;
mod coupon;
mod default;
mod fees;
mod prepayment;
mod recovery;
/// Extract a `rust_decimal::Decimal` from `decimal.Decimal`, `float`, `int`,
/// or a numeric `str` (e.g. ``"0.05"``).
///
/// `str` and `decimal.Decimal` both go through
/// `finstack_quant_core::decimal::parse_decimal` (the parser behind
/// `Money::from_decimal_str`), so text that is not exactly representable as a
/// Rust `Decimal` is rejected rather than rounded.
mod schedule;

pub(crate) use amortization::PyAmortizationSpec;
pub(crate) use amortization::PyNotional;
pub(crate) use coupon::PyCouponType;
pub(crate) use coupon::PyFixedCouponSpec;
pub(crate) use coupon::PyFloatingCouponSpec;
pub(crate) use coupon::PyFloatingLegCompounding;
pub(crate) use coupon::PyFloatingRateFallback;
pub(crate) use coupon::PyFloatingRateSpec;
pub(crate) use coupon::PyOvernightIndexConstraintApplication;
pub(crate) use coupon::PyStepUpCouponSpec;
pub(crate) use default::PyDefaultModelSpec;
pub(crate) use fees::PyFeeAccrualBasis;
pub(crate) use fees::PyFeeBase;
pub(crate) use fees::PyFeeSpec;
pub(crate) use prepayment::PyPrepaymentModelSpec;
pub(crate) use recovery::PyRecoveryModelSpec;
pub(crate) use schedule::PyPrincipalExchange;
pub(crate) use schedule::PyRollRule;
pub(crate) use schedule::PyScheduleParams;

pub(crate) fn decimal_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Decimal> {
    if is_python_decimal(obj)? {
        return decimal_from_py(obj);
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
    decimal_number_from_any(obj)
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
    decimal_number_from_any(obj)
}

fn decimal_number_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Decimal> {
    if obj.is_instance_of::<PyBool>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "rate cannot be a bool",
        ));
    }
    if obj.is_instance_of::<PyInt>() {
        return finstack_quant_core::decimal::parse_decimal(obj.str()?.to_str()?)
            .map_err(core_to_py);
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

fn decimal_opt_to_py<'py>(
    py: Python<'py>,
    value: Option<Decimal>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    value.map(|v| decimal_to_py(py, v)).transpose()
}

pub(crate) fn add_classes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRollRule>()?;
    module.add_class::<PyPrincipalExchange>()?;
    module.add_class::<PyCouponType>()?;
    module.add_class::<PyFloatingLegCompounding>()?;
    module.add_class::<PyOvernightIndexConstraintApplication>()?;
    module.add_class::<PyFloatingRateFallback>()?;
    module.add_class::<PyFeeAccrualBasis>()?;
    module.add_class::<PyScheduleParams>()?;
    module.add_class::<PyFixedCouponSpec>()?;
    module.add_class::<PyFloatingRateSpec>()?;
    module.add_class::<PyFloatingCouponSpec>()?;
    module.add_class::<PyStepUpCouponSpec>()?;
    module.add_class::<PyAmortizationSpec>()?;
    module.add_class::<PyNotional>()?;
    module.add_class::<PyFeeBase>()?;
    module.add_class::<PyFeeSpec>()?;
    module.add_class::<PyPrepaymentModelSpec>()?;
    module.add_class::<PyDefaultModelSpec>()?;
    module.add_class::<PyRecoveryModelSpec>()?;
    Ok(())
}
