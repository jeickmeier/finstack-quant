//! Python bindings for the `finstack-quant-scenarios` crate.
//!
//! Scenarios are spec-based (serde), so this module exposes typed specification
//! construction, validation, template registry discovery, and scenario engine
//! application. Every entry point accepts the typed wrapper or its canonical
//! JSON string; errors map through `crate::errors::scenarios_to_py`.

pub(crate) mod engine;
mod extract;
mod horizon;
mod operation_spec;
mod schema;
pub(crate) mod spec;
mod templates;

use pyo3::prelude::*;
use pyo3::types::PyList;
use spec::{PyScenarioSpec, PyScenarioValidationReport, PyTemplateMetadata};
use templates::{
    build_from_template, build_template_component, compose_scenarios,
    list_builtin_template_metadata, list_builtin_templates, list_template_components,
    parallel_bp_many, validate_scenario_spec,
};

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "scenarios")?;
    m.setattr(
        "__doc__",
        "Scenario specification, validation, composition, application, and built-in templates.",
    )?;

    m.add_class::<PyScenarioSpec>()?;
    m.add_class::<PyScenarioValidationReport>()?;
    m.add_class::<PyTemplateMetadata>()?;
    m.add_function(wrap_pyfunction!(compose_scenarios, &m)?)?;
    m.add_function(wrap_pyfunction!(parallel_bp_many, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_scenario_spec, &m)?)?;
    m.add_function(wrap_pyfunction!(list_builtin_templates, &m)?)?;
    m.add_function(wrap_pyfunction!(list_builtin_template_metadata, &m)?)?;
    m.add_function(wrap_pyfunction!(build_from_template, &m)?)?;
    m.add_function(wrap_pyfunction!(list_template_components, &m)?)?;
    m.add_function(wrap_pyfunction!(build_template_component, &m)?)?;
    engine::register(py, &m)?;
    horizon::register(py, &m)?;
    operation_spec::register(py, &m)?;

    schema::register(py, &m)?;
    // Sorted; must agree with the pure-Python shim and the `.pyi` stub.
    let all = PyList::new(
        py,
        [
            "ApplicationReport",
            "ApplicationResult",
            "Compounding",
            "CurveKind",
            "HierarchyTarget",
            "HorizonResult",
            "OperationSpec",
            "RateBindingSpec",
            "ScenarioSpec",
            "ScenarioValidationReport",
            "TemplateMetadata",
            "TenorMatchMode",
            "TimeRollMode",
            "apply_scenario",
            "build_from_template",
            "build_template_component",
            "compose_scenarios",
            "compute_horizon_return",
            "list_builtin_template_metadata",
            "list_builtin_templates",
            "list_template_components",
            "parallel_bp_many",
            "schema",
            "validate_scenario_spec",
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
