//! Python bindings for `finstack_quant_factor`.
//!
//! The module mirrors the Rust crate boundary: `FactorCovarianceMatrix` and
//! `FactorModelConfig` sit on `finstack_quant.models.factor` like their Rust
//! crate-root exports; credit hierarchy bindings are registered under
//! `finstack_quant.models.factor.credit`.

use pyo3::prelude::*;
use pyo3::types::PyList;

pub(crate) mod config;
pub(crate) mod credit;
pub(crate) mod definitions;
pub(crate) mod risk;
mod schema;

/// Register the `models.factor` Python domain.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "factor")?;
    m.setattr(
        "__doc__",
        crate::bindings::pydoc::pydoc!("finstack_quant.models.factor", "__module__"),
    )?;

    let credit = crate::bindings::module_utils::new_submodule(&m, "credit")?;
    credit.setattr(
        "__doc__",
        crate::bindings::pydoc::pydoc!("finstack_quant.models.factor.credit", "__module__"),
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
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    let risk = crate::bindings::module_utils::new_submodule(&m, "risk")?;
    risk.setattr(
        "__doc__",
        crate::bindings::pydoc::pydoc!("finstack_quant.models.factor.risk", "__module__"),
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
            "PositionRiskBudget",
            "PositionRiskDecomposition",
            "PositionRiskReport",
            "PositionRiskRequest",
            "PositionRiskSource",
            "PositionVarContribution",
            "RiskBudgetResult",
            "RiskDecomposition",
            "RiskMeasure",
            "StressAttribution",
            "StressPositionEntry",
            "TailScenarioBreakdown",
            "build_stress_attribution",
            "decompose_factors",
            "evaluate_risk_budget",
            "evaluate_risk_budget_from_decomposition",
            "position_component_var",
            "position_risk_report",
        ],
    )?;
    risk.setattr("__all__", risk_all)?;
    crate::bindings::module_utils::attach_submodule(
        &m,
        &risk,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    schema::register(py, &m)?;

    // Exported at the Rust crate root (`finstack_quant_factor`), so
    // they live on `finstack_quant.models.factor`, not under `credit`.
    m.add_class::<config::PyFactorCovarianceMatrix>()?;
    m.add_class::<config::PyFactorModelConfig>()?;
    m.add_class::<definitions::PyFactorDefinition>()?;
    m.add_class::<definitions::PyMarketMapping>()?;

    let all = PyList::new(
        py,
        [
            "FactorCovarianceMatrix",
            "FactorModelConfig",
            "FactorDefinition",
            "MarketMapping",
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
