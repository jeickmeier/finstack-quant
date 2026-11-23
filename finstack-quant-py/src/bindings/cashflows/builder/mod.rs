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
mod rates;

pub(crate) use rates::{abs_to_smm, annual_to_monthly, monthly_to_annual};

/// Register the `finstack_quant.cashflows.builder` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "builder")?;
    module.setattr(
        "__doc__",
        pydoc!("finstack_quant.cashflows.builder", "__module__"),
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
