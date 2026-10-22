//! Python bindings for `finstack_quant_cashflows::builder`.
//!
//! One Python module (`finstack_quant.cashflows.builder`) mirrors the Rust
//! `builder` re-export surface: spec types, the fluent `CashFlowBuilder`, and
//! `CashFlowSchedule`.

pub(crate) mod orchestrator;
pub(crate) mod schedule;
pub(crate) mod specs;

use crate::bindings::pydoc::pydoc;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use pyo3::wrap_pyfunction;

/// Convert an ABS speed to the single-month mortality for a seasoning month.
///
/// The ABS convention (auto-loan and consumer ABS) prepays a constant share
/// ``speed`` of the *original* balance each month, so
/// ``SMM_t = speed / (1 - speed * (t - 1))``. Month 0 is treated as month 1;
/// once the original balance is exhausted the result is capped at ``1.0``.
///
/// Parameters
/// ----------
/// speed : float
///     Monthly prepayment as a decimal fraction of the original balance
///     (``0.015`` = 1.5% ABS), in ``[0, 1]``.
/// month : int
///     Seasoning month counted from origination (non-negative).
///
/// Returns
/// -------
/// float
///     Single-month mortality as a decimal in ``[0, 1]``.
///
/// Raises
/// ------
/// ValueError
///     If ``speed`` is non-finite or outside ``[0, 1]``.
#[pyfunction]
#[pyo3(text_signature = "(speed, month)")]
pub(crate) fn abs_to_smm(speed: f64, month: u32) -> PyResult<f64> {
    finstack_quant_cashflows::builder::abs_to_smm(speed, month).map_err(crate::errors::core_to_py)
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "annual_to_monthly")]
#[pyfunction]
#[pyo3(text_signature = "(annual)")]
pub(crate) fn annual_to_monthly(annual: f64) -> PyResult<f64> {
    finstack_quant_cashflows::builder::annual_to_monthly(annual).map_err(crate::errors::core_to_py)
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "monthly_to_annual")]
#[pyfunction]
#[pyo3(text_signature = "(monthly)")]
pub(crate) fn monthly_to_annual(monthly: f64) -> PyResult<f64> {
    finstack_quant_cashflows::builder::monthly_to_annual(monthly).map_err(crate::errors::core_to_py)
}

/// Register the `finstack_quant.cashflows.builder` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "builder")?;
    module.setattr(
        "__doc__",
        "Composable cashflow builder: coupon/fee/amortization specs, CashFlowBuilder, CashFlowSchedule.",
    )?;

    specs::add_classes(&module)?;
    module.add_class::<orchestrator::PyCashFlowBuilder>()?;
    module.add_class::<schedule::PyCashFlowMeta>()?;
    module.add_class::<schedule::PyCashFlowSchedule>()?;
    module.add_function(wrap_pyfunction!(
        schedule::merge_cashflow_schedules,
        &module
    )?)?;
    module.add_function(wrap_pyfunction!(abs_to_smm, &module)?)?;
    module.add_function(wrap_pyfunction!(annual_to_monthly, &module)?)?;
    module.add_function(wrap_pyfunction!(monthly_to_annual, &module)?)?;

    let all = PyList::new(
        py,
        [
            "AmortizationSpec",
            "CashFlowBuilder",
            "CashFlowMeta",
            "CashFlowSchedule",
            "CouponType",
            "DefaultModelSpec",
            "FeeAccrualBasis",
            "FeeBase",
            "FeeSpec",
            "FixedCouponSpec",
            "FloatingCouponSpec",
            "FloatingLegCompounding",
            "FloatingRateFallback",
            "FloatingRateSpec",
            "Notional",
            "OvernightIndexConstraintApplication",
            "PrepaymentModelSpec",
            "PrincipalExchange",
            "RecoveryModelSpec",
            "RollRule",
            "ScheduleParams",
            "StepUpCouponSpec",
            "abs_to_smm",
            "annual_to_monthly",
            "monthly_to_annual",
            "merge_cashflow_schedules",
        ],
    )?;
    module.setattr("__all__", all)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}
