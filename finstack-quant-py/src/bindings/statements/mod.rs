//! Python bindings for the `finstack-quant-statements` crate.
//!
//! Exposes the financial model specification types, builder, evaluator,
//! DSL parser, and EBITDA normalization engine.

mod adjustments;
pub(crate) mod builder;
pub(crate) mod capital_structure;
pub(crate) mod checks;
mod dsl;
pub(crate) mod evaluator;
pub(crate) mod monte_carlo;
mod parse;
mod registry;
mod rollup;
mod schema;
pub(crate) mod types;

pub(crate) use parse::{
    PERIOD_GRAMMAR, extract_amount_or_scalar, extract_money_series, extract_period_pairs,
    extract_scalar_series, extract_value_series, parse_period_id, python_literal,
};

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `statements` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "statements")?;
    m.setattr(
        "__doc__",
        "Financial statement modeling: builders, evaluators, forecasts, DSL, adjustments.",
    )?;

    types::register(py, &m)?;
    capital_structure::register(py, &m)?;
    builder::register(py, &m)?;
    registry::register(py, &m)?;
    evaluator::register(py, &m)?;
    monte_carlo::register(py, &m)?;
    rollup::register(py, &m)?;
    dsl::register(py, &m)?;
    adjustments::register(py, &m)?;
    checks::register(py, &m)?;
    schema::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "Adjustment",
            "AppliedAdjustment",
            "CapitalStructureCashflows",
            "CheckConfig",
            "CheckFinding",
            "CheckReport",
            "CheckSuiteSpec",
            "EcfSweepSpec",
            "Evaluator",
            "FinancialModelSpec",
            "ForecastMethod",
            "ForecastSpec",
            "FormulaCheckSpec",
            "MetricDefinition",
            "MixedNodeBuilder",
            "ModelBuilder",
            "MonteCarloConfig",
            "MonteCarloResults",
            "NodeId",
            "NodeSpec",
            "NodeType",
            "NormalizationConfig",
            "NormalizationResult",
            "NumericMode",
            "PaymentClassSpec",
            "PikToggleSpec",
            "Registry",
            "RollupSpec",
            "StatementResult",
            "WaterfallSpec",
            "merge_into_results",
            "normalize",
            "parse_and_compile",
            "parse_formula",
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
