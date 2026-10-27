"""
Scenario specification, validation, composition, application, and built-in templates.

Examples
--------
>>> from finstack_quant.scenarios import list_builtin_templates
>>> list_builtin_templates()[:2]
['gfc_2008', 'covid_2020']
"""

from __future__ import annotations

from finstack_quant.scenarios import schema as schema
from finstack_quant.scenarios._spec import ScenarioSpec as ScenarioSpec, TemplateMetadata as TemplateMetadata
from finstack_quant.scenarios._engine import (
    ApplicationReport as ApplicationReport,
    ApplicationResult as ApplicationResult,
    apply_scenario as apply_scenario,
)
from finstack_quant.scenarios._horizon import (
    HorizonResult as HorizonResult,
    compute_horizon_return as compute_horizon_return,
)
from finstack_quant.scenarios._operation_spec.kinds import (
    Compounding as Compounding,
    CurveKind as CurveKind,
    TenorMatchMode as TenorMatchMode,
    TimeRollMode as TimeRollMode,
)
from finstack_quant.scenarios._operation_spec.rate_binding import RateBindingSpec as RateBindingSpec
from finstack_quant.scenarios._operation_spec.hierarchy import HierarchyTarget as HierarchyTarget
from finstack_quant.scenarios._operation_spec.spec import OperationSpec as OperationSpec

__all__ = [
    "compose_scenarios",
    "parallel_bp_many",
    "validate_scenario_spec",
    "list_builtin_templates",
    "list_builtin_template_metadata",
    "build_from_template",
    "list_template_components",
    "build_template_component",
    "apply_scenario",
    "compute_horizon_return",
    "ApplicationReport",
    "ApplicationResult",
    "HierarchyTarget",
    "HorizonResult",
    "OperationSpec",
    "RateBindingSpec",
    "ScenarioSpec",
    "TemplateMetadata",
    "CurveKind",
    "TenorMatchMode",
    "TimeRollMode",
    "Compounding",
    "schema",
]

def compose_scenarios(specs: list[ScenarioSpec]) -> ScenarioSpec:
    """
    Merge multiple scenario specs using the scenario engine composer.

    Specs are stably sorted by ascending ``priority`` (equal priorities keep
    input order) and their operations concatenated in that order. Every input
    must use the same ``hazard_bump_mode``.

    Parameters
    ----------
    specs : list[ScenarioSpec]
        Typed scenario specifications to compose; execution order is determined
        by each spec's ``priority``, not by list position.

    Returns
    -------
    ScenarioSpec
        Typed composed scenario specification.

    Raises
    ------
    ValueError
        If scenario composition fails.

    Examples
    --------
    >>> from finstack_quant.scenarios import compose_scenarios
    >>> compose_scenarios([]).operations
    []
    """
    ...

def validate_scenario_spec(scenario: ScenarioSpec | str) -> None:
    """
    Validate a scenario specification without applying it.

    Parameters
    ----------
    scenario : ScenarioSpec | str
        Typed scenario or JSON-serialized ``ScenarioSpec``.

    Returns
    -------
    None
        Returns nothing on success. An invalid spec raises instead, so
        ``if validate_scenario_spec(s):`` is not a validity check.

    Raises
    ------
    ValueError
        If ``scenario`` is not valid JSON or fails validation; the message is
        the one ``ScenarioSpec.validate()`` raises.

    Examples
    --------
    >>> from finstack_quant.scenarios import validate_scenario_spec
    >>> spec = '{"id":"s","name":"S","operations":[]}'
    >>> validate_scenario_spec(spec) is None
    True
    """
    ...

def list_builtin_templates() -> list[str]:
    """
    List template IDs from the embedded built-in registry.

    Returns
    -------
    list[str]
        Template identifier strings.

    Raises
    ------
    ValueError
        If the example or catalog cannot be produced.

    Examples
    --------
    >>> from finstack_quant.scenarios import list_builtin_templates
    >>> list_builtin_templates()[:2]
    ['gfc_2008', 'covid_2020']
    """
    ...

def list_builtin_template_metadata() -> list[TemplateMetadata]:
    """
    Return typed metadata for all built-in templates.

    Returns
    -------
    list[TemplateMetadata]
        Metadata in deterministic registry order.

    Raises
    ------
    ValueError
        If the example or catalog cannot be produced.

    Examples
    --------
    >>> from finstack_quant.scenarios import list_builtin_template_metadata
    >>> list_builtin_template_metadata()[0].id
    'gfc_2008'
    """
    ...

def build_from_template(template_id: str) -> ScenarioSpec:
    """
    Instantiate a ``ScenarioSpec`` from a built-in template.

    Parameters
    ----------
    template_id : str
        Registry key for the template.

    Returns
    -------
    ScenarioSpec
        Typed scenario specification built from the template.

    Raises
    ------
    ValueError
        If ``template_id`` is not found in the registry.

    Examples
    --------
    >>> from finstack_quant.scenarios import build_from_template
    >>> build_from_template("gfc_2008").id
    'gfc_2008'
    """
    ...

def list_template_components(template_id: str) -> list[str]:
    """
    List sub-component IDs for composite templates.

    Parameters
    ----------
    template_id : str
        Parent template identifier.

    Returns
    -------
    list[str]
        Component identifiers.

    Raises
    ------
    ValueError
        If ``template_id`` is not found in the registry.

    Examples
    --------
    >>> from finstack_quant.scenarios import list_template_components
    >>> list_template_components("gfc_2008")[:2]
    ['gfc_2008_rates', 'gfc_2008_credit']
    """
    ...

def build_template_component(template_id: str, component_id: str) -> ScenarioSpec:
    """
    Build a single component spec from a composite template.

    Parameters
    ----------
    template_id : str
        Parent template identifier.
    component_id : str
        Component key inside the template.

    Returns
    -------
    ScenarioSpec
        Typed component scenario specification.

    Raises
    ------
    ValueError
        If ``template_id`` or ``component_id`` is not found.

    Examples
    --------
    >>> from finstack_quant.scenarios import build_template_component
    >>> component = build_template_component("gfc_2008", "gfc_2008_rates")
    >>> component.id
    'gfc_2008_rates'
    """
    ...

def parallel_bp_many(
    curve_kind: CurveKind | str,
    curve_ids: list[str],
    bp: float,
    discount_curve_id: str | None = None,
) -> list[OperationSpec]:
    """Expand a parallel shift into one operation per curve identifier.

    Parameters
    ----------
    curve_kind : CurveKind | str
        Typed family or canonical discount, forward, par_cds, inflation or commodity family.
    curve_ids : list[str]
        Identifiers in execution order; an empty list produces no operations.
    bp : float
        Additive basis points, or percent of forward for commodity curves.
    discount_curve_id : str | None, default None
        Optional ParCDS discount curve copied onto each operation.

    Returns
    -------
    list[OperationSpec]
        One operation per curve in the supplied order.

    Raises
    ------
    ValueError
        If the curve family label is unknown.
    TypeError
        If identifiers are not strings or bp is not numeric.

    Examples
    --------
    >>> from finstack_quant.scenarios import parallel_bp_many
    >>> len(parallel_bp_many("discount", ["USD-OIS", "EUR-OIS"], 25.0))
    2
    """
    ...
