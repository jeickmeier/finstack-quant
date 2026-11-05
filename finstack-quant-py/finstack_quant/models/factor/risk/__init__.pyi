"""Factor and position risk decomposition kernels.

Examples
--------
>>> from finstack_quant.models.factor.risk import DEFAULT_UTILIZATION_THRESHOLD
>>> DEFAULT_UTILIZATION_THRESHOLD
1.2
"""

from __future__ import annotations
from finstack_quant.models.factor.risk._functions import (
    decompose_factors as decompose_factors,
    position_component_var as position_component_var,
)
from finstack_quant.models.factor.risk._report import (
    PositionRiskReport as PositionRiskReport,
    position_risk_report as position_risk_report,
)
from finstack_quant.models.factor.risk._budget import (
    PositionBudgetEntry as PositionBudgetEntry,
    RiskBudgetResult as RiskBudgetResult,
    evaluate_risk_budget as evaluate_risk_budget,
    evaluate_risk_budget_from_decomposition as evaluate_risk_budget_from_decomposition,
)
from finstack_quant.models.factor.risk._contributions import (
    FactorContribution as FactorContribution,
    PositionEsContribution as PositionEsContribution,
    PositionFactorContribution as PositionFactorContribution,
    PositionResidualContribution as PositionResidualContribution,
    PositionRiskDecomposition as PositionRiskDecomposition,
    PositionVarContribution as PositionVarContribution,
    RiskDecomposition as RiskDecomposition,
)
from finstack_quant.models.factor.risk._stress import (
    StressAttribution as StressAttribution,
    StressPositionEntry as StressPositionEntry,
    TailScenarioBreakdown as TailScenarioBreakdown,
    build_stress_attribution as build_stress_attribution,
)

__all__ = [
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

DEFAULT_UTILIZATION_THRESHOLD: float
"""Default ``actual / target`` utilization ratio (``1.2``) above which
``evaluate_risk_budget`` flags a breach."""
