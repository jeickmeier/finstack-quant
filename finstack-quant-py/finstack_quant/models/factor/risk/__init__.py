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
PositionRiskReport = _risk.PositionRiskReport
DEFAULT_UTILIZATION_THRESHOLD = _risk.DEFAULT_UTILIZATION_THRESHOLD
position_risk_report = _risk.position_risk_report
evaluate_risk_budget = _risk.evaluate_risk_budget
evaluate_risk_budget_from_decomposition = _risk.evaluate_risk_budget_from_decomposition
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
    "PositionRiskReport",
    "PositionVarContribution",
    "RiskBudgetResult",
    "RiskDecomposition",
    "StressAttribution",
    "StressPositionEntry",
    "TailScenarioBreakdown",
    "build_stress_attribution",
    "decompose_factors",
    "evaluate_risk_budget",
    "evaluate_risk_budget_from_decomposition",
    "position_component_var",
    "position_risk_report",
]
