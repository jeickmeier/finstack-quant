//! Python bindings for `finstack_quant_cashflows::builder`.
//!
//! One Python module (`finstack_quant.cashflows.builder`) mirrors the Rust
//! `builder` re-export surface: spec types, the fluent `CashFlowBuilder`, and
//! `CashFlowSchedule`.

pub(crate) mod orchestrator;
pub(crate) mod schedule;
pub(crate) mod specs;

use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use pyo3::wrap_pyfunction;

/// Convert an annual CPR (constant prepayment rate) to a monthly SMM.
///
/// Uses the standard relationship ``SMM = 1 - (1 - CPR)^(1/12)`` (Fabozzi's
/// MBS handbook).
///
/// Parameters
/// ----------
/// cpr : float
///     Annualized CPR as a decimal in ``[0, 1]`` (``0.06`` means 6%).
///
/// Returns
/// -------
/// float
///     Monthly SMM as a decimal.
///
/// Raises
/// ------
/// ValueError
///     If ``cpr`` is negative, non-finite, or above ``1.0``.
#[pyfunction]
#[pyo3(text_signature = "(cpr)")]
pub(crate) fn cpr_to_smm(cpr: f64) -> PyResult<f64> {
    finstack_quant_cashflows::builder::cpr_to_smm(cpr).map_err(crate::errors::core_to_py)
}

/// Convert a monthly SMM (single monthly mortality) to an annual CPR.
///
/// Uses ``CPR = 1 - (1 - SMM)^12``.
///
/// Parameters
/// ----------
/// smm : float
///     Monthly SMM as a decimal in ``[0, 1]``.
///
/// Returns
/// -------
/// float
///     Annualized CPR as a decimal.
///
/// Raises
/// ------
/// ValueError
///     If ``smm`` is negative, non-finite, or above ``1.0``.
#[pyfunction]
#[pyo3(text_signature = "(smm)")]
pub(crate) fn smm_to_cpr(smm: f64) -> PyResult<f64> {
    finstack_quant_cashflows::builder::smm_to_cpr(smm).map_err(crate::errors::core_to_py)
}

/// Convert an annual CDR (constant default rate) to a monthly MDR.
///
/// Default and prepayment mortality rates share the same annual-to-monthly
/// conversion kernel: ``MDR = 1 - (1 - CDR)^(1/12)``.
///
/// Parameters
/// ----------
/// cdr : float
///     Constant annual default rate as a decimal in ``[0, 1]``.
///
/// Returns
/// -------
/// float
///     Monthly MDR as a decimal.
///
/// Raises
/// ------
/// ValueError
///     If ``cdr`` is negative, non-finite, or above ``1.0``.
#[pyfunction]
#[pyo3(text_signature = "(cdr)")]
pub(crate) fn cdr_to_mdr(cdr: f64) -> PyResult<f64> {
    finstack_quant_cashflows::builder::cdr_to_mdr(cdr).map_err(crate::errors::core_to_py)
}

/// Convert a monthly MDR (monthly default rate) to an annual CDR.
///
/// Uses ``CDR = 1 - (1 - MDR)^12``.
///
/// Parameters
/// ----------
/// mdr : float
///     Monthly default rate as a decimal in ``[0, 1]``.
///
/// Returns
/// -------
/// float
///     Annualized CDR as a decimal.
///
/// Raises
/// ------
/// ValueError
///     If ``mdr`` is negative, non-finite, or above ``1.0``.
#[pyfunction]
#[pyo3(text_signature = "(mdr)")]
pub(crate) fn mdr_to_cdr(mdr: f64) -> PyResult<f64> {
    finstack_quant_cashflows::builder::mdr_to_cdr(mdr).map_err(crate::errors::core_to_py)
}

/// Register the `finstack_quant.cashflows.builder` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "builder")?;
    module.setattr(
        "__doc__",
        "Composable cashflow builder: coupon/fee/amortization specs, CashFlowBuilder, CashFlowSchedule.",
    )?;

    specs::add_classes(&module)?;
    module.add_class::<orchestrator::PyCashFlowBuilder>()?;
    module.add_class::<orchestrator::PyPrincipalEvent>()?;
    module.add_class::<schedule::PyCashFlowMeta>()?;
    module.add_class::<schedule::PyCashFlowSchedule>()?;
    module.add_function(wrap_pyfunction!(
        schedule::py_merge_cashflow_schedules,
        &module
    )?)?;
    module.add_function(wrap_pyfunction!(cdr_to_mdr, &module)?)?;
    module.add_function(wrap_pyfunction!(cpr_to_smm, &module)?)?;
    module.add_function(wrap_pyfunction!(mdr_to_cdr, &module)?)?;
    module.add_function(wrap_pyfunction!(smm_to_cpr, &module)?)?;

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
            "FloatingRateFallback",
            "FloatingRateSpec",
            "Notional",
            "OvernightCompoundingMethod",
            "OvernightIndexConstraintApplication",
            "PrepaymentModelSpec",
            "PrincipalEvent",
            "PrincipalExchange",
            "RecoveryModelSpec",
            "RollRule",
            "ScheduleParams",
            "StepUpCouponSpec",
            "cdr_to_mdr",
            "cpr_to_smm",
            "mdr_to_cdr",
            "merge_cashflow_schedules",
            "smm_to_cpr",
        ],
    )?;
    module.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &module,
        "builder",
        "finstack_quant.cashflows",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
