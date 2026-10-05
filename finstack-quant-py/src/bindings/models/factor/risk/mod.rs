//! Models-owned factor risk wrappers and pure calculation entry points.
mod budget;
pub(crate) mod config;
pub(crate) mod contributions;
mod functions;
mod stress;
use budget::{evaluate_risk_budget, PyPositionBudgetEntry, PyRiskBudgetResult};
use config::PyDecompositionConfig;
pub(crate) use contributions::PyRiskDecomposition;
use contributions::{
    PyFactorContribution, PyPositionEsContribution, PyPositionFactorContribution,
    PyPositionResidualContribution, PyPositionRiskDecomposition, PyPositionVarContribution,
};
use functions::{
    historical_var_decomposition, parametric_es_decomposition, parametric_var_decomposition,
    position_component_var, PyParametricEsDecompositionView, PyPositionEsContributionView,
};
use pyo3::prelude::*;
use stress::{
    build_stress_attribution, PyStressAttribution, PyStressPositionEntry, PyTailScenarioBreakdown,
};
/// Register models-owned factor-risk classes and pure calculation functions.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
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
