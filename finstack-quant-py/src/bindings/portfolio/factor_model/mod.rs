//! Typed `#[pyclass]` wrappers for `finstack_quant_portfolio::factor_model` result types.
//!
//! The decomposition helpers return structured ``#[pyclass]`` wrappers around
//! the Rust result types. The module also exposes the full set of result
//! classes for callers that want to inspect a
//! ``RiskDecomposition``, ``WhatIfResult``, ``StressResult``, ``CreditVolReport``,
//! or ``FactorAssignmentReport`` without serializing through JSON.
//!
//! Engine and builder types (``FactorModel``, ``FactorModelBuilder``,
//! ``ParametricPositionDecomposer``, ``HistoricalPositionDecomposer``,
//! ``WhatIfEngine``, ``FactorCovarianceForecast``) are intentionally left for
//! a future slice — they hold borrowed handles or trait objects that do not
//! map cleanly to a JSON-first PyO3 surface and are not required by the
//! result-type contract this slice fulfils.

mod assignment;
mod budget_whatif;
pub(crate) mod config;
mod contributions;
mod credit_vol;
mod functions;
mod stress;

use pyo3::prelude::*;

use assignment::{PyFactorAssignmentReport, PyPositionAssignment, PyUnmatchedEntry};
use budget_whatif::{
    evaluate_risk_budget, position_what_if, PyFactorContributionDelta, PyPositionBudgetEntry,
    PyRiskBudgetResult, PyWhatIfResult,
};
use config::{PyDecompositionConfig, PyVolHorizon};
pub(crate) use contributions::PyRiskDecomposition;
use contributions::{
    PyFactorContribution, PyPositionEsContribution, PyPositionFactorContribution,
    PyPositionResidualContribution, PyPositionRiskDecomposition, PyPositionVarContribution,
};
use credit_vol::{
    build_credit_vol_report, PyCreditVolReport, PyLevelVolContribution, PyPositionVolContribution,
};
use functions::{
    historical_var_decomposition, parametric_es_decomposition, parametric_var_decomposition,
    position_component_var, PyParametricEsDecompositionView, PyPositionEsContributionView,
};
use stress::{
    build_stress_attribution, factor_stress, PyStressAttribution, PyStressPositionEntry,
    PyStressResult, PyTailScenarioBreakdown,
};

/// Register factor_model typed result classes and typed-sibling functions on
/// the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFactorContributionDelta>()?;
    m.add_class::<PyWhatIfResult>()?;
    m.add_class::<PyStressResult>()?;
    m.add_class::<PyPositionAssignment>()?;
    m.add_class::<PyUnmatchedEntry>()?;
    m.add_class::<PyFactorAssignmentReport>()?;
    m.add_class::<PyLevelVolContribution>()?;
    m.add_class::<PyPositionVolContribution>()?;
    m.add_class::<PyCreditVolReport>()?;
    m.add_function(wrap_pyfunction!(factor_stress, m)?)?;
    m.add_function(wrap_pyfunction!(position_what_if, m)?)?;
    m.add_function(wrap_pyfunction!(build_credit_vol_report, m)?)?;

    Ok(())
}

/// Register models-owned factor-risk classes and pure calculation functions.
pub(crate) fn register_risk(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFactorContribution>()?;
    m.add_class::<PyPositionFactorContribution>()?;
    m.add_class::<PyPositionResidualContribution>()?;
    m.add_class::<PyRiskDecomposition>()?;
    m.add_class::<PyPositionVarContribution>()?;
    m.add_class::<PyPositionEsContribution>()?;
    m.add_class::<PyPositionRiskDecomposition>()?;
    m.add_class::<PyPositionBudgetEntry>()?;
    m.add_class::<PyRiskBudgetResult>()?;
    m.add_class::<PyStressPositionEntry>()?;
    m.add_class::<PyTailScenarioBreakdown>()?;
    m.add_class::<PyStressAttribution>()?;
    m.add_class::<PyDecompositionConfig>()?;
    m.add_class::<PyPositionEsContributionView>()?;
    m.add_class::<PyParametricEsDecompositionView>()?;
    m.add(
        "DEFAULT_UTILIZATION_THRESHOLD",
        finstack_quant_models::factor::risk::DEFAULT_UTILIZATION_THRESHOLD,
    )?;

    m.add_function(wrap_pyfunction!(parametric_var_decomposition, m)?)?;
    m.add_function(wrap_pyfunction!(parametric_es_decomposition, m)?)?;
    m.add_function(wrap_pyfunction!(historical_var_decomposition, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_risk_budget, m)?)?;
    m.add_function(wrap_pyfunction!(build_stress_attribution, m)?)?;
    m.add_function(wrap_pyfunction!(position_component_var, m)?)?;
    Ok(())
}

/// Register the models-owned credit forecast horizon wrapper.
pub(crate) fn register_credit_forecast(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVolHorizon>()?;
    Ok(())
}
