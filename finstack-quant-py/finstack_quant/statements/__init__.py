"""Financial statement modeling: builders, evaluators, forecasts, DSL, adjustments.

Bindings for the ``finstack-quant-statements`` Rust crate.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import statements as _statements

ForecastMethod = _statements.ForecastMethod
ForecastSpec = _statements.ForecastSpec
NodeType = _statements.NodeType
NodeId = _statements.NodeId
NumericMode = _statements.NumericMode
FinancialModelSpec = _statements.FinancialModelSpec
ModelBuilder = _statements.ModelBuilder
MixedNodeBuilder = _statements.MixedNodeBuilder
MetricRegistry = _statements.MetricRegistry
StatementResult = _statements.StatementResult
Evaluator = _statements.Evaluator
parse_formula = _statements.parse_formula
validate_formula = _statements.validate_formula
NormalizationConfig = _statements.NormalizationConfig
normalize = _statements.normalize
CheckSuiteSpec = _statements.CheckSuiteSpec
CheckReport = _statements.CheckReport
EcfSweepSpec = _statements.EcfSweepSpec
PikToggleSpec = _statements.PikToggleSpec
WaterfallSpec = _statements.WaterfallSpec

__all__: list[str] = [
    "CheckReport",
    "CheckSuiteSpec",
    "EcfSweepSpec",
    "Evaluator",
    "FinancialModelSpec",
    "ForecastMethod",
    "ForecastSpec",
    "MetricRegistry",
    "MixedNodeBuilder",
    "ModelBuilder",
    "NodeId",
    "NodeType",
    "NormalizationConfig",
    "NumericMode",
    "PikToggleSpec",
    "StatementResult",
    "WaterfallSpec",
    "normalize",
    "parse_formula",
    "validate_formula",
]
