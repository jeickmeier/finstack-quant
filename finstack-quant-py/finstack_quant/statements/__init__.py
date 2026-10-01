"""Financial statement modeling: builders, evaluators, forecasts, DSL, adjustments.

Bindings for the ``finstack-quant-statements`` Rust crate.

Examples:
--------
>>> from finstack_quant.statements import NodeId
>>> NodeId("revenue").as_str()
'revenue'

"""

from finstack_quant.finstack_quant import statements as _statements

Adjustment = _statements.Adjustment
AppliedAdjustment = _statements.AppliedAdjustment
CapitalStructureCashflows = _statements.CapitalStructureCashflows
CheckConfig = _statements.CheckConfig
CheckFinding = _statements.CheckFinding
CheckReport = _statements.CheckReport
CheckSuiteSpec = _statements.CheckSuiteSpec
EcfSweepSpec = _statements.EcfSweepSpec
Evaluator = _statements.Evaluator
FinancialModelSpec = _statements.FinancialModelSpec
ForecastMethod = _statements.ForecastMethod
ForecastSpec = _statements.ForecastSpec
FormulaCheckSpec = _statements.FormulaCheckSpec
MetricDefinition = _statements.MetricDefinition
MixedNodeBuilder = _statements.MixedNodeBuilder
ModelBuilder = _statements.ModelBuilder
MonteCarloConfig = _statements.MonteCarloConfig
MonteCarloResults = _statements.MonteCarloResults
NodeId = _statements.NodeId
NodeSpec = _statements.NodeSpec
NodeType = _statements.NodeType
NormalizationConfig = _statements.NormalizationConfig
NormalizationResult = _statements.NormalizationResult
NumericMode = _statements.NumericMode
PaymentClassSpec = _statements.PaymentClassSpec
PikToggleSpec = _statements.PikToggleSpec
Registry = _statements.Registry
StatementResult = _statements.StatementResult
WaterfallSpec = _statements.WaterfallSpec
normalize = _statements.normalize
normalize_json = _statements.normalize_json
parse_and_compile = _statements.parse_and_compile
parse_formula = _statements.parse_formula

# `schema` is a compiled submodule with no pure-Python shim package; the extension
# registers it as `finstack_quant.statements.schema`.
schema = _statements.schema

__all__: list[str] = [
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
    "StatementResult",
    "WaterfallSpec",
    "normalize",
    "normalize_json",
    "parse_and_compile",
    "parse_formula",
    "schema",
]
