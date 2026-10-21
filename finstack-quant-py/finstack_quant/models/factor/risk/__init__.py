"""Factor and position risk decomposition kernels.

Examples:
--------
>>> from finstack_quant.models.factor.risk import DEFAULT_UTILIZATION_THRESHOLD
>>> DEFAULT_UTILIZATION_THRESHOLD
1.2
"""

from finstack_quant.finstack_quant import models as _models

_risk = _models.factor.risk

FactorContribution = _risk.FactorContribution
PositionFactorContribution = _risk.PositionFactorContribution
PositionResidualContribution = _risk.PositionResidualContribution
RiskDecomposition = _risk.RiskDecomposition
PositionVarContribution = _risk.PositionVarContribution
PositionEsContribution = _risk.PositionEsContribution
PositionRiskDecomposition = _risk.PositionRiskDecomposition
PositionBudgetEntry = _risk.PositionBudgetEntry
RiskBudgetResult = _risk.RiskBudgetResult
StressPositionEntry = _risk.StressPositionEntry
TailScenarioBreakdown = _risk.TailScenarioBreakdown
StressAttribution = _risk.StressAttribution
DEFAULT_UTILIZATION_THRESHOLD = _risk.DEFAULT_UTILIZATION_THRESHOLD
parametric_var_decomposition = _risk.parametric_var_decomposition
historical_var_decomposition = _risk.historical_var_decomposition
evaluate_risk_budget = _risk.evaluate_risk_budget
build_stress_attribution = _risk.build_stress_attribution
position_component_var = _risk.position_component_var
decompose_factors = _risk.decompose_factors

__all__: list[str] = [
    "DEFAULT_UTILIZATION_THRESHOLD",
    "FactorContribution",
    "PositionBudgetEntry",
    "PositionEsContribution",
    "PositionFactorContribution",
    "PositionResidualContribution",
    "PositionRiskDecomposition",
    "PositionVarContribution",
    "RiskBudgetResult",
    "RiskDecomposition",
    "StressAttribution",
    "StressPositionEntry",
    "TailScenarioBreakdown",
    "build_stress_attribution",
    "decompose_factors",
    "evaluate_risk_budget",
    "historical_var_decomposition",
    "parametric_var_decomposition",
    "position_component_var",
]
