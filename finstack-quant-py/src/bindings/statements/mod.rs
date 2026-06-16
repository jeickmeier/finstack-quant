//! Python bindings for the `finstack-quant-statements` crate.
//!
//! Exposes the financial model specification types, builder, evaluator,
//! DSL parser, and EBITDA normalization engine.

mod adjustments;
mod builder;
pub(crate) mod capital_structure;
mod checks;
mod dsl;
pub(crate) mod evaluator;
pub(crate) mod types;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `statements` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "statements")?;
    m.setattr(
        "__doc__",
        "Financial statement modeling: builders, evaluators, forecasts, DSL, adjustments.",
    )?;

    types::register(py, &m)?;
    capital_structure::register(py, &m)?;
    builder::register(py, &m)?;
    evaluator::register(py, &m)?;
    dsl::register(py, &m)?;
    adjustments::register(py, &m)?;
    checks::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            // Capital structure
            "EcfSweepSpec",
            "PikToggleSpec",
            "WaterfallSpec",
            // Types
            "ForecastMethod",
            "ForecastSpec",
            "NodeType",
            "NodeId",
            "NumericMode",
            "FinancialModelSpec",
            // Builder
            "ModelBuilder",
            "MixedNodeBuilder",
            "MetricRegistry",
            // Evaluator
            "StatementResult",
            "Evaluator",
            // DSL
            "parse_formula",
            "validate_formula",
            // Adjustments
            "NormalizationConfig",
            "normalize",
            // Checks
            "CheckSuiteSpec",
            "CheckReport",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "statements",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}
