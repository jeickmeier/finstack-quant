//! Python bindings for `finstack_quant_models::factor`.
//!
//! The module mirrors the Rust crate boundary. Credit hierarchy bindings are
//! registered under `finstack_quant.models.factor.credit`.

use pyo3::prelude::*;
use pyo3::types::PyList;

pub(crate) mod credit;
mod schema;

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
    crate::bindings::portfolio::factor_model::register_credit_forecast(&credit)?;

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
    crate::bindings::portfolio::factor_model::register_risk(&risk)?;
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
