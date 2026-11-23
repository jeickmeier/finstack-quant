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
from finstack_quant.scenarios._spec import (
    ScenarioSpec as ScenarioSpec,
    ScenarioValidationReport as ScenarioValidationReport,
    TemplateMetadata as TemplateMetadata,
)
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

from finstack_quant.scenarios._templates import (
    compose_scenarios as compose_scenarios,
    validate_scenario_spec as validate_scenario_spec,
    list_builtin_templates as list_builtin_templates,
    list_builtin_template_metadata as list_builtin_template_metadata,
    build_from_template as build_from_template,
    list_template_components as list_template_components,
    build_template_component as build_template_component,
    parallel_bp_many as parallel_bp_many,
)

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
    "ScenarioValidationReport",
    "TemplateMetadata",
    "CurveKind",
    "TenorMatchMode",
    "TimeRollMode",
    "Compounding",
    "schema",
]
