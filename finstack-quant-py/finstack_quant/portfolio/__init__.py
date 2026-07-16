"""Portfolio construction, valuation, optimization, cashflows, scenarios, and metrics.

Bindings for the ``finstack-quant-portfolio`` Rust crate.

Stability tiers
---------------

The exports below fall into three stability tiers. Treat the tier as a
contract about how disruptive future changes are likely to be.

**Stable** — covered by golden tests and meant to round-trip across releases:

* ``Portfolio``, ``PortfolioValuation``, ``PortfolioResult``,
  ``PortfolioCashflows`` (the typed handles)
* ``parse_portfolio_spec``, ``build_portfolio_from_spec``
* ``value_portfolio``, ``aggregate_full_cashflows``,
  ``apply_scenario_and_revalue``
* ``aggregate_metrics``, ``portfolio_result_total_value``,
  ``portfolio_result_get_metric``
* ``replay_portfolio``

**Stable, JSON-shape may evolve** — function signatures stable, but the
returned/accepted JSON payload structure can grow new fields (additive,
non-breaking) between releases:

* ``optimize_portfolio`` (``PortfolioOptimizationSpec`` /
  ``PortfolioOptimizationResult`` JSON)
* ``parametric_var_decomposition``, ``parametric_es_decomposition``,
  ``historical_var_decomposition``, ``evaluate_risk_budget``

**Experimental** — calibration constants and convenience defaults still
under review; signatures or default coefficients may change:

* ``lvar_bangia`` — endogenous-cost coefficient is a calibration default
  (see ``LiquidityConfig::endogenous_spread_coef`` in the Rust crate).
* ``almgren_chriss_impact`` — fixes ``delta`` at 0.5; the underlying
  ``optimal_trajectory`` accepts only ``delta = 1`` (linear impact).
* ``kyle_lambda``, ``roll_effective_spread``, ``amihud_illiquidity``,
  ``days_to_liquidate``, ``liquidity_tier`` — small free functions, may be
  re-grouped or renamed.

Bindings should be considered cross-version-compatible only within a single
``finstack-quant-portfolio`` minor release; pin the upstream version when
exporting to downstream services.
"""

from __future__ import annotations

import contextlib

from finstack_quant.finstack_quant import portfolio as _portfolio

with contextlib.suppress(NameError):
    del annotations

PortfolioError = _portfolio.PortfolioError
FinstackValuationError = _portfolio.FinstackValuationError
FinstackFxError = _portfolio.FinstackFxError
FinstackOptimizationError = _portfolio.FinstackOptimizationError

Portfolio = _portfolio.Portfolio
PortfolioValuation = _portfolio.PortfolioValuation
PortfolioResult = _portfolio.PortfolioResult
PortfolioMetrics = _portfolio.PortfolioMetrics
PortfolioCashflows = _portfolio.PortfolioCashflows
PortfolioAttribution = _portfolio.PortfolioAttribution

parse_portfolio_spec = _portfolio.parse_portfolio_spec
build_portfolio_from_spec = _portfolio.build_portfolio_from_spec
portfolio_result_total_value = _portfolio.portfolio_result_total_value
portfolio_result_get_metric = _portfolio.portfolio_result_get_metric
aggregate_metrics = _portfolio.aggregate_metrics
value_portfolio = _portfolio.value_portfolio
aggregate_full_cashflows = _portfolio.aggregate_full_cashflows
apply_scenario_and_revalue = _portfolio.apply_scenario_and_revalue
attribute_portfolio_pnl = _portfolio.attribute_portfolio_pnl
allocate_weights = _portfolio.allocate_weights
validate_allocation_json = _portfolio.validate_allocation_json
optimize_portfolio = _portfolio.optimize_portfolio
replay_portfolio = _portfolio.replay_portfolio
parametric_var_decomposition = _portfolio.parametric_var_decomposition
parametric_es_decomposition = _portfolio.parametric_es_decomposition
historical_var_decomposition = _portfolio.historical_var_decomposition
evaluate_risk_budget = _portfolio.evaluate_risk_budget
roll_effective_spread = _portfolio.roll_effective_spread
amihud_illiquidity = _portfolio.amihud_illiquidity
days_to_liquidate = _portfolio.days_to_liquidate
liquidity_tier = _portfolio.liquidity_tier
lvar_bangia = _portfolio.lvar_bangia
almgren_chriss_impact = _portfolio.almgren_chriss_impact
kyle_lambda = _portfolio.kyle_lambda
brinson_fachler = _portfolio.brinson_fachler
carino_link = _portfolio.carino_link
twrr_modified_dietz = _portfolio.twrr_modified_dietz
twrr_linked = _portfolio.twrr_linked
mwr_xirr = _portfolio.mwr_xirr

# Factor-model decomposition results
FactorContribution = _portfolio.FactorContribution
PositionFactorContribution = _portfolio.PositionFactorContribution
PositionResidualContribution = _portfolio.PositionResidualContribution
RiskDecomposition = _portfolio.RiskDecomposition
FactorRiskDecomposition = _portfolio.FactorRiskDecomposition
SensitivityMatrix = _portfolio.SensitivityMatrix
FactorPnlProfile = _portfolio.FactorPnlProfile
compute_factor_sensitivities = _portfolio.compute_factor_sensitivities
compute_pnl_profiles = _portfolio.compute_pnl_profiles
decompose_factor_risk = _portfolio.decompose_factor_risk
PositionVarContribution = _portfolio.PositionVarContribution
PositionEsContribution = _portfolio.PositionEsContribution
PositionRiskDecomposition = _portfolio.PositionRiskDecomposition
PositionBudgetEntry = _portfolio.PositionBudgetEntry
RiskBudgetResult = _portfolio.RiskBudgetResult
FactorContributionDelta = _portfolio.FactorContributionDelta
WhatIfResult = _portfolio.WhatIfResult
StressResult = _portfolio.StressResult
StressPositionEntry = _portfolio.StressPositionEntry
TailScenarioBreakdown = _portfolio.TailScenarioBreakdown
StressAttribution = _portfolio.StressAttribution
PositionAssignment = _portfolio.PositionAssignment
UnmatchedEntry = _portfolio.UnmatchedEntry
FactorAssignmentReport = _portfolio.FactorAssignmentReport
LevelVolContribution = _portfolio.LevelVolContribution
PositionVolContribution = _portfolio.PositionVolContribution
CreditVolReport = _portfolio.CreditVolReport
VolHorizon = _portfolio.VolHorizon
DecompositionConfig = _portfolio.DecompositionConfig
parametric_var_decomposition_typed = _portfolio.parametric_var_decomposition_typed
historical_var_decomposition_typed = _portfolio.historical_var_decomposition_typed
evaluate_risk_budget_typed = _portfolio.evaluate_risk_budget_typed
factor_stress = _portfolio.factor_stress
position_what_if = _portfolio.position_what_if
build_stress_attribution = _portfolio.build_stress_attribution
build_credit_vol_report = _portfolio.build_credit_vol_report
position_component_var = _portfolio.position_component_var

# Portfolio optimization specifications and results
WeightingScheme = _portfolio.WeightingScheme
MissingMetricPolicy = _portfolio.MissingMetricPolicy
Inequality = _portfolio.Inequality
OptimizationStatus = _portfolio.OptimizationStatus
TradeDirection = _portfolio.TradeDirection
TradeType = _portfolio.TradeType
PerPositionMetric = _portfolio.PerPositionMetric
PositionFilter = _portfolio.PositionFilter
MetricExpr = _portfolio.MetricExpr
Objective = _portfolio.Objective
Constraint = _portfolio.Constraint
TradeSpec = _portfolio.TradeSpec
PortfolioOptimizationSpec = _portfolio.PortfolioOptimizationSpec
PortfolioOptimizationResult = _portfolio.PortfolioOptimizationResult
CandidatePosition = _portfolio.CandidatePosition
TradeUniverse = _portfolio.TradeUniverse
optimize_portfolio_typed = _portfolio.optimize_portfolio_typed

__all__: list[str] = [
    "CandidatePosition",
    "Constraint",
    "CreditVolReport",
    "DecompositionConfig",
    "FactorAssignmentReport",
    "FactorContribution",
    "FactorContributionDelta",
    "FactorPnlProfile",
    "FactorRiskDecomposition",
    "FinstackFxError",
    "FinstackOptimizationError",
    "FinstackValuationError",
    "Inequality",
    "LevelVolContribution",
    "MetricExpr",
    "MissingMetricPolicy",
    "Objective",
    "OptimizationStatus",
    "PerPositionMetric",
    "Portfolio",
    "PortfolioAttribution",
    "PortfolioCashflows",
    "PortfolioError",
    "PortfolioMetrics",
    "PortfolioOptimizationResult",
    "PortfolioOptimizationSpec",
    "PortfolioResult",
    "PortfolioValuation",
    "PositionAssignment",
    "PositionBudgetEntry",
    "PositionEsContribution",
    "PositionFactorContribution",
    "PositionFilter",
    "PositionResidualContribution",
    "PositionRiskDecomposition",
    "PositionVarContribution",
    "PositionVolContribution",
    "RiskBudgetResult",
    "RiskDecomposition",
    "SensitivityMatrix",
    "StressAttribution",
    "StressPositionEntry",
    "StressResult",
    "TailScenarioBreakdown",
    "TradeDirection",
    "TradeSpec",
    "TradeType",
    "TradeUniverse",
    "UnmatchedEntry",
    "VolHorizon",
    "WeightingScheme",
    "WhatIfResult",
    "aggregate_full_cashflows",
    "aggregate_metrics",
    "allocate_weights",
    "almgren_chriss_impact",
    "amihud_illiquidity",
    "apply_scenario_and_revalue",
    "attribute_portfolio_pnl",
    "brinson_fachler",
    "build_credit_vol_report",
    "build_portfolio_from_spec",
    "build_stress_attribution",
    "carino_link",
    "compute_factor_sensitivities",
    "compute_pnl_profiles",
    "days_to_liquidate",
    "decompose_factor_risk",
    "evaluate_risk_budget",
    "evaluate_risk_budget_typed",
    "factor_stress",
    "historical_var_decomposition",
    "historical_var_decomposition_typed",
    "kyle_lambda",
    "liquidity_tier",
    "lvar_bangia",
    "mwr_xirr",
    "optimize_portfolio",
    "optimize_portfolio_typed",
    "parametric_es_decomposition",
    "parametric_var_decomposition",
    "parametric_var_decomposition_typed",
    "parse_portfolio_spec",
    "portfolio_result_get_metric",
    "portfolio_result_total_value",
    "position_component_var",
    "position_what_if",
    "replay_portfolio",
    "roll_effective_spread",
    "twrr_linked",
    "twrr_modified_dietz",
    "validate_allocation_json",
    "value_portfolio",
]
