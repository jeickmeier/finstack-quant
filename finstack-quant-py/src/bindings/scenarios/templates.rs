//! Built-in scenario templates, composition, validation, and parallel shift helpers.

use super::operation_spec;
use super::spec::{PyScenarioSpec, PyScenarioValidationReport, PyTemplateMetadata};
use crate::bindings::pydoc::pydoc;
use crate::errors::scenarios_to_py;
use pyo3::prelude::*;

fn builtin_registry() -> PyResult<&'static finstack_quant_scenarios::TemplateRegistry> {
    finstack_quant_scenarios::TemplateRegistry::embedded_builtins().map_err(scenarios_to_py)
}

/// Expand a parallel shift into one operation per curve identifier.
///
/// Parameters
/// ----------
/// curve_kind : CurveKind | str
///     Canonical curve family: discount, forward, par_cds or inflation.
/// curve_ids : list[str]
///     Curve identifiers in execution order; empty input produces an empty list.
/// bp : float
///     Additive rate basis points.
/// discount_curve_id : str | None, default None
///     Optional discount curve for ParCDS quote replay, copied onto each operation.
///
/// Returns
/// -------
/// list[OperationSpec]
///     One operation per identifier in the original order.
///
/// Raises
/// ------
/// ValueError
///     If curve_kind is not a canonical family label.
/// TypeError
///     If identifiers are not strings or bp is not numeric.
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import parallel_bp_many
/// >>> len(parallel_bp_many("discount", ["USD-OIS", "EUR-OIS"], 25.0))
/// 2
#[pyfunction]
#[pyo3(signature = (curve_kind, curve_ids, bp, discount_curve_id=None))]
pub(super) fn parallel_bp_many(
    curve_kind: &Bound<'_, PyAny>,
    curve_ids: Vec<String>,
    bp: f64,
    discount_curve_id: Option<String>,
) -> PyResult<Vec<operation_spec::PyOperationSpec>> {
    let kind = operation_spec::extract_curve_kind(curve_kind)?;
    Ok(finstack_quant_scenarios::OperationSpec::parallel_bp_many(
        kind,
        curve_ids,
        bp,
        discount_curve_id.map(Into::into),
    )
    .map_err(scenarios_to_py)?
    .into_iter()
    .map(operation_spec::PyOperationSpec::from_inner)
    .collect())
}

/// Compose several scenario specifications into one.
///
/// Specs are stably sorted by ascending ``priority`` (equal priorities keep
/// input order) and their operations concatenated in that order. Every input
/// must use the same ``resolution_mode``; mixed resolution modes are rejected
/// rather than silently selecting one.
///
/// The composed spec takes its ``id`` and ``name`` from the inputs joined
/// with ``+`` / ``" + "``, its ``description`` from the present input
/// descriptions joined with ``" + "`` (``None`` when no input has one), and
/// the lowest input ``priority`` (``0`` for an empty list), all in
/// execution order.
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
///     If `specs` contain mixed ``resolution_mode`` values or more than one
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
pub(super) fn compose_scenarios(specs: Vec<PyScenarioSpec>) -> PyResult<PyScenarioSpec> {
    let specs = specs.into_iter().map(|spec| spec.inner).collect();
    let composed = finstack_quant_scenarios::ScenarioSpec::compose(specs)
        .map_err(crate::errors::scenarios_to_py)?;
    Ok(PyScenarioSpec::from_inner(composed))
}

#[doc = pydoc!("finstack_quant.scenarios._templates", "validate_scenario_spec")]
#[pyfunction]
pub(super) fn validate_scenario_spec(
    scenario: &Bound<'_, PyAny>,
) -> PyResult<PyScenarioValidationReport> {
    use finstack_quant_scenarios::validate_scenario_spec;
    if let Ok(typed) = scenario.extract::<PyRef<'_, PyScenarioSpec>>() {
        return Ok(PyScenarioValidationReport::from_inner(
            validate_scenario_spec(&typed.inner),
        ));
    }

    let json: String = scenario.extract()?;
    let spec = serde_json::from_str(&json).map_err(|error| {
        crate::errors::serde_json_to_py(error, "Failed to parse ScenarioSpec JSON")
    })?;
    Ok(PyScenarioValidationReport::from_inner(
        validate_scenario_spec(&spec),
    ))
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
///     Template identifiers, e.g. ``"gfc_2008"``.
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
pub(super) fn list_builtin_templates() -> PyResult<Vec<String>> {
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
pub(super) fn list_builtin_template_metadata() -> PyResult<Vec<PyTemplateMetadata>> {
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
///     :func:`apply_scenario` and :func:`compute_horizon_return`.
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
pub(super) fn build_from_template(template_id: &str) -> PyResult<PyScenarioSpec> {
    let spec = builtin_registry()?
        .build(template_id)
        .map_err(crate::errors::scenarios_to_py)?;
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
pub(super) fn list_template_components(template_id: &str) -> PyResult<Vec<String>> {
    builtin_registry()?
        .component_ids(template_id)
        .map(|ids| ids.into_iter().map(str::to_string).collect())
        .map_err(crate::errors::scenarios_to_py)
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
pub(super) fn build_template_component(
    template_id: &str,
    component_id: &str,
) -> PyResult<PyScenarioSpec> {
    let spec = builtin_registry()?
        .build_component(template_id, component_id)
        .map_err(crate::errors::scenarios_to_py)?;
    Ok(PyScenarioSpec::from_inner(spec))
}
