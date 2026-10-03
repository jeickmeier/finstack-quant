//! Python bindings for `finstack_quant_models::factor`.
//!
//! The module mirrors the Rust crate boundary. Credit hierarchy bindings are
//! registered under `finstack_quant.models.factor.credit`.

use pyo3::prelude::*;
use pyo3::types::PyList;

mod budget;
pub(crate) mod config;
pub(crate) mod contributions;
pub(crate) mod credit;
mod functions;
pub(crate) mod matrix_input;
mod schema;
mod stress;

use budget::{evaluate_risk_budget, PyPositionBudgetEntry, PyRiskBudgetResult};
use config::{PyDecompositionConfig, PyVolHorizon};
use contributions::{
    PyFactorContribution, PyPositionEsContribution, PyPositionFactorContribution,
    PyPositionResidualContribution, PyPositionRiskDecomposition, PyPositionVarContribution,
    PyRiskDecomposition,
};
use functions::{
    historical_var_decomposition, parametric_es_decomposition, parametric_var_decomposition,
    position_component_var, PyParametricEsDecompositionView, PyPositionEsContributionView,
};
use stress::{
    build_stress_attribution, PyStressAttribution, PyStressPositionEntry, PyTailScenarioBreakdown,
};

/// Register the `models.factor` Python domain.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "factor")?;
    m.setattr(
        "__doc__",
        "Factor-model primitives, credit calibration, and decomposition.",
    )?;

    let credit = crate::bindings::module_utils::new_submodule(&m, "credit")?;
    credit.setattr(
        "__doc__",
        "Credit factor hierarchy artifacts, calibration, and decomposition.",
    )?;
    credit::register(py, &credit)?;
    register_credit_forecast(&credit)?;

    let credit_all = PyList::new(
        py,
        [
            "CreditFactorModel",
            "CreditCalibrator",
            "LevelsAtDate",
            "PeriodDecomposition",
            "FactorCovarianceForecast",
            "FactorCovarianceMatrix",
            "FactorModelConfig",
            "VolHorizon",
            "decompose_levels",
            "decompose_period",
        ],
    )?;
    credit.setattr("__all__", credit_all)?;
    crate::bindings::module_utils::attach_submodule(
        &m,
        &credit,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    let risk = crate::bindings::module_utils::new_submodule(&m, "risk")?;
    risk.setattr(
        "__doc__",
        "Product-independent factor and position risk decomposition kernels.",
    )?;
    register_risk(&risk)?;
    let risk_all = PyList::new(
        py,
        [
            "DEFAULT_UTILIZATION_THRESHOLD",
            "DecompositionConfig",
            "FactorContribution",
            "ParametricEsDecompositionView",
            "PositionBudgetEntry",
            "PositionEsContribution",
            "PositionEsContributionView",
            "PositionFactorContribution",
            "PositionResidualContribution",
            "PositionRiskDecomposition",
            "PositionVarContribution",
            "RiskBudgetResult",
            "RiskDecomposition",
            "StressAttribution",
            "StressPositionEntry",
            "TailScenarioBreakdown",
            "build_stress_attribution",
            "evaluate_risk_budget",
            "historical_var_decomposition",
            "parametric_es_decomposition",
            "parametric_var_decomposition",
            "position_component_var",
        ],
    )?;
    risk.setattr("__all__", risk_all)?;
    crate::bindings::module_utils::attach_submodule(
        &m,
        &risk,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    schema::register(py, &m)?;

    let all = PyList::new(py, ["credit", "risk", "schema"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

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
