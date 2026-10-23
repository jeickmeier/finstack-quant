//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use rust_decimal::Decimal;

use crate::bindings::core::money::{decimal_from_py, decimal_to_py, is_python_decimal, PyMoney};
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
