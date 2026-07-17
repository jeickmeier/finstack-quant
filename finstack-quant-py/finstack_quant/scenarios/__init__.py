"""Scenario specification, validation, composition, application, and templates.

Bindings for the ``finstack-quant-scenarios`` Rust crate.

Examples:
--------
>>> import finstack_quant.scenarios as scenarios
>>> scenarios.__name__
'finstack_quant.scenarios'
"""

from __future__ import annotations

from finstack_quant.finstack_quant import scenarios as _scenarios

parse_scenario_spec = _scenarios.parse_scenario_spec
build_scenario_spec = _scenarios.build_scenario_spec
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

# Operation specifications
OperationSpec = _scenarios.OperationSpec
RateBindingSpec = _scenarios.RateBindingSpec
CurveKind = _scenarios.CurveKind
VolSurfaceKind = _scenarios.VolSurfaceKind
TenorMatchMode = _scenarios.TenorMatchMode
TimeRollMode = _scenarios.TimeRollMode
Compounding = _scenarios.Compounding

__all__: list[str] = [
    "Compounding",
    "CurveKind",
    "HorizonResult",
    "OperationSpec",
    "RateBindingSpec",
    "TenorMatchMode",
    "TimeRollMode",
    "VolSurfaceKind",
    "apply_scenario",
    "apply_scenario_to_market",
    "build_from_template",
    "build_scenario_spec",
    "build_template_component",
    "compose_scenarios",
    "compute_horizon_return",
    "list_builtin_template_metadata",
    "list_builtin_templates",
    "list_template_components",
    "parse_scenario_spec",
    "validate_scenario_spec",
]
