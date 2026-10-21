//! Models-owned factor risk wrappers and pure calculation entry points.
mod budget;
pub(crate) mod contributions;
mod functions;
mod stress;
use budget::{evaluate_risk_budget, PyPositionBudgetEntry, PyRiskBudgetResult};
pub(crate) use contributions::PyRiskDecomposition;
use contributions::{
    PyFactorContribution, PyPositionEsContribution, PyPositionFactorContribution,
    PyPositionResidualContribution, PyPositionRiskDecomposition, PyPositionVarContribution,
};
use functions::{
    decompose_factors, historical_var_decomposition, parametric_es_decomposition,
    parametric_var_decomposition, position_component_var,
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
    m.add(
        "DEFAULT_UTILIZATION_THRESHOLD",
        finstack_quant_factor::risk::DEFAULT_UTILIZATION_THRESHOLD,
    )?;

    m.add_function(wrap_pyfunction!(parametric_var_decomposition, m)?)?;
    m.add_function(wrap_pyfunction!(parametric_es_decomposition, m)?)?;
    m.add_function(wrap_pyfunction!(historical_var_decomposition, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_risk_budget, m)?)?;
    m.add_function(wrap_pyfunction!(build_stress_attribution, m)?)?;
    m.add_function(wrap_pyfunction!(position_component_var, m)?)?;
    m.add_function(wrap_pyfunction!(decompose_factors, m)?)?;
    Ok(())
}
