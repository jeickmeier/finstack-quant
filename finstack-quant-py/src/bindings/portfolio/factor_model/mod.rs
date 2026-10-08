//! Typed `#[pyclass]` wrappers for `finstack_quant_portfolio::factor_model` result types.
//!
//! The decomposition helpers return structured ``#[pyclass]`` wrappers around
//! the Rust result types. The module also exposes the full set of result
//! classes for callers that want to inspect a
//! ``RiskDecomposition``, ``WhatIfResult``, ``StressResult``, ``CreditVolReport``,
//! or ``FactorAssignmentReport`` without serializing through JSON.
//!
//! ``FactorModel`` is a stateful handle built once from a ``FactorModelConfig``
//! (Rust ``FactorModel::from_config``) and reused for assignment,
//! sensitivities, risk decomposition, position what-if and factor stress. The

mod assignment;
mod budget_whatif;
mod credit_vol;
mod model;
mod stress;

use pyo3::prelude::*;

use assignment::{PyFactorAssignmentReport, PyPositionAssignment, PyUnmatchedEntry};
use budget_whatif::{PyFactorContributionDelta, PyWhatIfResult};
use credit_vol::{
    build_credit_vol_report, PyCreditVolReport, PyLevelVolContribution, PyPositionVolContribution,
};
use model::PyFactorModel;
use stress::{PyStressPnl, PyStressResult};

/// Register factor_model typed result classes and typed-sibling functions on
/// the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFactorContributionDelta>()?;
    m.add_class::<PyWhatIfResult>()?;
    m.add_class::<PyStressResult>()?;
    m.add_class::<PyStressPnl>()?;
    m.add_class::<PyFactorModel>()?;
    m.add_class::<PyPositionAssignment>()?;
    m.add_class::<PyUnmatchedEntry>()?;
    m.add_class::<PyFactorAssignmentReport>()?;
    m.add_class::<PyLevelVolContribution>()?;
    m.add_class::<PyPositionVolContribution>()?;
    m.add_class::<PyCreditVolReport>()?;
    m.add_function(wrap_pyfunction!(build_credit_vol_report, m)?)?;

    Ok(())
}
