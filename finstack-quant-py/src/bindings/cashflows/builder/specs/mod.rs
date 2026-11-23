//! Spec-type bindings for `finstack_quant_cashflows::builder`.

mod amortization;
mod coupon;
mod default;
mod fees;
mod prepayment;
mod recovery;
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

mod convert;

use pyo3::prelude::*;

pub(crate) use convert::{
    DateMoneyRows, date_decimal_pairs, date_money_to_py, decimal_bps_from_any, decimal_from_any,
    decimal_opt_to_py, decimal_rate_from_any, fields_with_decimal_from_py,
};

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
