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

use pyo3::prelude::*;
use pyo3::types::PyList;
use spec::{PyScenarioSpec, PyTemplateMetadata};

use crate::errors::scenarios_to_py;

fn builtin_registry() -> PyResult<&'static finstack_quant_scenarios::TemplateRegistry> {
    finstack_quant_scenarios::TemplateRegistry::embedded_builtins().map_err(scenarios_to_py)
}

/// Compose several scenario specifications into one.
///
/// Specs are stably sorted by ascending ``priority`` (equal priorities keep
/// input order) and their operations concatenated in that order. Every input
/// must use the same ``hazard_bump_mode``; mixed hazard delivery conventions
/// are rejected rather than silently selecting one.
///
/// Parameters
/// ----------
/// specs : list[ScenarioSpec]
///     Typed scenario specifications; execution order is determined by each
///     spec's ``priority``, not by list position.
///
/// Returns
/// -------
/// ScenarioSpec
///     Typed composed scenario specification.
///
/// Raises
/// ------
/// ValueError
///     If `specs` contain mixed ``hazard_bump_mode`` values or more than one
///     time-roll operation.
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import ScenarioSpec, compose_scenarios
/// >>> rates = ScenarioSpec("rates", [])
/// >>> credit = ScenarioSpec("credit", [])
/// >>> compose_scenarios([rates, credit]).operations
/// []
#[pyfunction]
fn compose_scenarios(specs: Vec<PyScenarioSpec>) -> PyResult<PyScenarioSpec> {
    let specs = specs.into_iter().map(|spec| spec.inner).collect();
    let composed = finstack_quant_scenarios::ScenarioSpec::compose(specs)
        .map_err(crate::errors::scenarios_to_py)?;
    Ok(PyScenarioSpec::from_inner(composed))
}

/// Validate a scenario specification without applying it.
///
/// Parameters
/// ----------
/// scenario : ScenarioSpec | str
///     Typed scenario or JSON-serialized ``ScenarioSpec``.
///
/// Returns
/// -------
/// None
///     Returns nothing on success. An invalid spec raises instead, so
///     ``if validate_scenario_spec(s):`` is not a validity check — call it
///     bare and catch ``ValueError``.
///
/// Raises
/// ------
/// ValueError
///     If the JSON is malformed or the spec fails validation. The message is
///     the same one ``ScenarioSpec.validate()`` raises.
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import ScenarioSpec, validate_scenario_spec
/// >>> validate_scenario_spec(ScenarioSpec("ok", []).to_json()) is None
/// True
#[pyfunction]
fn validate_scenario_spec(scenario: &Bound<'_, PyAny>) -> PyResult<()> {
    extract::extract_scenario_spec(scenario).map(|_| ())
}

/// List the identifiers of every built-in scenario template.
///
/// Templates are the quickest way into the scenarios domain: pick an id here,
/// pass it to :func:`build_from_template`, and apply the resulting spec — no
/// hand-written operation JSON required.
///
/// Returns
/// -------
/// list[str]
///     Template identifiers, e.g. ``"rates_parallel_up_100bp"``.
///
/// See Also
/// --------
/// list_builtin_template_metadata : Names and descriptions for these ids.
/// build_from_template : Turn an id into a runnable spec.
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import list_builtin_templates
/// >>> isinstance(list_builtin_templates(), list)
/// True
#[pyfunction]
fn list_builtin_templates() -> PyResult<Vec<String>> {
    let registry = builtin_registry()?;
    Ok(registry.list().iter().map(|m| m.id.clone()).collect())
}

/// Describe every built-in scenario template.
///
/// Returns
/// -------
/// list[TemplateMetadata]
///     Typed metadata objects in deterministic registry order.
///
/// See Also
/// --------
/// list_builtin_templates : Just the identifiers.
#[pyfunction]
fn list_builtin_template_metadata() -> PyResult<Vec<PyTemplateMetadata>> {
    let registry = builtin_registry()?;
    Ok(registry
        .list()
        .into_iter()
        .cloned()
        .map(PyTemplateMetadata::from_inner)
        .collect())
}

/// Build a complete scenario specification from a built-in template.
///
/// This is the shortest path from nothing to a runnable scenario.
///
/// Parameters
/// ----------
/// template_id : str
///     Identifier from :func:`list_builtin_templates`.
///
/// Returns
/// -------
/// ScenarioSpec
///     Typed scenario specification, accepted directly by
///     :func:`apply_scenario_to_market` and :func:`compute_horizon_return`.
///
/// Raises
/// ------
/// ValueError
///     If ``template_id`` is not a built-in template, or the template fails to
///     build.
///
/// See Also
/// --------
/// list_template_components : Build only part of a template.
#[pyfunction]
fn build_from_template(template_id: &str) -> PyResult<PyScenarioSpec> {
    let spec = builtin_registry()?
        .build(template_id)
        .map_err(|error| crate::errors::value_error(error.to_string()))?;
    Ok(PyScenarioSpec::from_inner(spec))
}

/// List the component identifiers within a built-in template.
///
/// Multi-part templates (e.g. a stress that shocks rates, credit and vol) are
/// decomposable, so a single leg can be applied on its own.
///
/// Parameters
/// ----------
/// template_id : str
///     Identifier from :func:`list_builtin_templates`.
///
/// Returns
/// -------
/// list[str]
///     Component identifiers accepted by :func:`build_template_component`.
///
/// Raises
/// ------
/// ValueError
///     If ``template_id`` is not a built-in template.
#[pyfunction]
fn list_template_components(template_id: &str) -> PyResult<Vec<String>> {
    builtin_registry()?
        .component_ids(template_id)
        .map(|ids| ids.into_iter().map(str::to_string).collect())
        .map_err(|error| crate::errors::value_error(error.to_string()))
}

/// Build a scenario specification from one component of a built-in template.
///
/// Parameters
/// ----------
/// template_id : str
///     Identifier from :func:`list_builtin_templates`.
/// component_id : str
///     Identifier from :func:`list_template_components`.
///
/// Returns
/// -------
/// ScenarioSpec
///     Typed scenario specification covering only that component.
///
/// Raises
/// ------
/// ValueError
///     If either identifier is unknown, or the component fails to build.
#[pyfunction]
fn build_template_component(template_id: &str, component_id: &str) -> PyResult<PyScenarioSpec> {
    let spec = builtin_registry()?
        .build_component(template_id, component_id)
        .map_err(|error| crate::errors::value_error(error.to_string()))?;
    Ok(PyScenarioSpec::from_inner(spec))
}

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "scenarios")?;
    m.setattr(
        "__doc__",
        "Scenario specification, validation, composition, application, and built-in templates.",
    )?;

    m.add_class::<PyScenarioSpec>()?;
    m.add_class::<PyTemplateMetadata>()?;
    m.add_function(wrap_pyfunction!(compose_scenarios, &m)?)?;
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
            "TemplateMetadata",
            "TenorMatchMode",
            "TimeRollMode",
            "apply_scenario",
            "apply_scenario_to_market",
            "build_from_template",
            "build_template_component",
            "compose_scenarios",
            "compute_horizon_return",
            "list_builtin_template_metadata",
            "list_builtin_templates",
            "list_template_components",
            "schema",
            "validate_scenario_spec",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "scenarios",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}
