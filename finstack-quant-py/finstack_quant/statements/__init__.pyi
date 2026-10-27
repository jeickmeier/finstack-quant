"""
Financial statement modeling: builders, evaluators, forecasts, DSL, adjustments.

Python bindings for the ``finstack-quant-statements`` Rust crate: model specifications,
``ModelBuilder``, ``Evaluator``, formula parsing/validation, and EBITDA-style
normalization helpers.

Examples
--------
>>> from finstack_quant.statements import NodeId
>>> NodeId("revenue").as_str()
'revenue'

"""

from __future__ import annotations

from finstack_quant.statements import schema as schema
from finstack_quant.statements._monte_carlo import (
    MonteCarloConfig as MonteCarloConfig,
    MonteCarloResults as MonteCarloResults,
)
from finstack_quant.statements._rollup import RollupSpec as RollupSpec
from finstack_quant.statements._types import (
    FinancialModelSpec as FinancialModelSpec,
    ForecastMethod as ForecastMethod,
    ForecastSpec as ForecastSpec,
    NodeId as NodeId,
    NodeSpec as NodeSpec,
    NodeType as NodeType,
    NumericMode as NumericMode,
)
from finstack_quant.statements._builder import MixedNodeBuilder as MixedNodeBuilder, ModelBuilder as ModelBuilder
from finstack_quant.statements._registry import MetricDefinition as MetricDefinition, Registry as Registry
from finstack_quant.statements._evaluator import Evaluator as Evaluator, StatementResult as StatementResult
from finstack_quant.statements._dsl import parse_and_compile as parse_and_compile, parse_formula as parse_formula
from finstack_quant.statements._adjustments import (
    Adjustment as Adjustment,
    AppliedAdjustment as AppliedAdjustment,
    NormalizationConfig as NormalizationConfig,
    NormalizationResult as NormalizationResult,
    merge_into_results as merge_into_results,
    normalize as normalize,
)
from finstack_quant.statements._checks import (
    CheckConfig as CheckConfig,
    CheckFinding as CheckFinding,
    CheckReport as CheckReport,
    CheckSuiteSpec as CheckSuiteSpec,
    FormulaCheckSpec as FormulaCheckSpec,
)
from finstack_quant.statements._capital_structure import (
    CapitalStructureCashflows as CapitalStructureCashflows,
    EcfSweepSpec as EcfSweepSpec,
    PaymentClassSpec as PaymentClassSpec,
    PikToggleSpec as PikToggleSpec,
    WaterfallSpec as WaterfallSpec,
)

__all__ = [
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
    "normalize",
    "parse_and_compile",
    "parse_formula",
    "schema",
]
