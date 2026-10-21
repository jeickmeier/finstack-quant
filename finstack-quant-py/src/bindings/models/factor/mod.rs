//! Python bindings for `finstack_quant_factor`.
//!
//! The module mirrors the Rust crate boundary: `FactorCovarianceMatrix` and
//! `FactorModelConfig` sit on `finstack_quant.models.factor` like their Rust
//! crate-root exports; credit hierarchy bindings are registered under
//! `finstack_quant.models.factor.credit`.

use pyo3::prelude::*;
use pyo3::types::PyList;

pub(crate) mod credit;
pub(crate) mod risk;
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

    let credit_all = PyList::new(
        py,
        [
            "CreditFactorModel",
            "CreditCalibrator",
            "LevelsAtDate",
            "PeriodDecomposition",
            "FactorCovarianceForecast",
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
    risk::register(&risk)?;
    let risk_all = PyList::new(
        py,
        [
            "DEFAULT_UTILIZATION_THRESHOLD",
            "FactorContribution",
            "PositionBudgetEntry",
            "PositionEsContribution",
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
            "decompose_factors",
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

    // Exported at the Rust crate root (`finstack_quant_factor`), so
    // they live on `finstack_quant.models.factor`, not under `credit`.
    m.add_class::<credit::PyFactorCovarianceMatrix>()?;
    m.add_class::<credit::PyFactorModelConfig>()?;

    let all = PyList::new(
        py,
        [
            "FactorCovarianceMatrix",
            "FactorModelConfig",
            "credit",
            "risk",
            "schema",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
