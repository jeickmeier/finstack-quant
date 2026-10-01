"""Scenario specification, validation, composition, application, and templates.

Bindings for the ``finstack-quant-scenarios`` Rust crate.

Examples:
--------
>>> from finstack_quant.scenarios import list_builtin_templates
>>> list_builtin_templates()[:2]
['gfc_2008', 'covid_2020']
"""

from finstack_quant.finstack_quant import scenarios as _scenarios

compose_scenarios = _scenarios.compose_scenarios
validate_scenario_spec = _scenarios.validate_scenario_spec
list_builtin_templates = _scenarios.list_builtin_templates
list_builtin_template_metadata = _scenarios.list_builtin_template_metadata
build_from_template = _scenarios.build_from_template
list_template_components = _scenarios.list_template_components
build_template_component = _scenarios.build_template_component
apply_scenario = _scenarios.apply_scenario
apply_scenario_to_market = _scenarios.apply_scenario_to_market
compute_horizon_return = _scenarios.compute_horizon_return
HorizonResult = _scenarios.HorizonResult
ApplicationReport = _scenarios.ApplicationReport
ApplicationResult = _scenarios.ApplicationResult
ScenarioSpec = _scenarios.ScenarioSpec
TemplateMetadata = _scenarios.TemplateMetadata

# Operation specifications
OperationSpec = _scenarios.OperationSpec
RateBindingSpec = _scenarios.RateBindingSpec
HierarchyTarget = _scenarios.HierarchyTarget
CurveKind = _scenarios.CurveKind
TenorMatchMode = _scenarios.TenorMatchMode
TimeRollMode = _scenarios.TimeRollMode
Compounding = _scenarios.Compounding
schema = _scenarios.schema

__all__: list[str] = [
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
]
