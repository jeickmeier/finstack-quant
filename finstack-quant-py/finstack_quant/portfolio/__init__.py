"""Portfolio construction, valuation, optimization, cashflows, scenarios, and metrics.

Bindings for the ``finstack-quant-portfolio`` Rust crate.

Stability tiers
---------------

The exports below fall into three stability tiers. Treat the tier as a
contract about how disruptive future changes are likely to be.

**Stable** — covered by golden tests and meant to round-trip across releases:

* ``Portfolio``, ``PortfolioBuilder``, ``PortfolioValuation``,
  ``PositionValue``, ``PortfolioResult``, ``PortfolioCashflows`` (the typed
  handles)
* ``parse_portfolio_spec_json``, ``build_portfolio_from_spec_json``
* ``value_portfolio``, ``aggregate_full_cashflows``,
  ``apply_scenario_and_revalue``
* ``aggregate_metrics`` (scalar reads go through ``PortfolioResult``
  methods such as ``total_value`` and ``get_metric``)
* ``replay_portfolio``

**Stable, typed contracts may evolve** — function signatures are stable, but
typed result classes may gain fields between releases:

* ``optimize_portfolio`` (``PortfolioOptimizationSpec`` /
  ``PortfolioOptimizationResult``)

Pure factor-risk decomposition and risk-budget kernels live under
``finstack_quant.models.factor.risk``. Portfolio retains allocation policy,
valuation sensitivities, what-if workflows, and portfolio report adapters.

Bindings should be considered cross-version-compatible only within a single
``finstack-quant-portfolio`` minor release; pin the upstream version when
exporting to downstream services.

Exceptions
----------

``FinstackError`` is the common base for the library's named exceptions, so
``except FinstackError`` catches ``PortfolioError``, ``ContractValidationError``
and their subclasses in one clause. It derives from ``ValueError``, so every
existing ``except ValueError`` keeps working unchanged. Its canonical home is
``finstack_quant.core``; it is re-exported here for convenience.

Examples:
--------
>>> from finstack_quant.portfolio import Portfolio
>>> pf = Portfolio.builder("book", "USD", "2025-01-01").build()
>>> (pf.id, pf.base_currency, len(pf))
('book', 'USD', 0)
"""

import sys as _sys

from finstack_quant.finstack_quant import portfolio as _portfolio

FinstackError = _portfolio.FinstackError
PortfolioError = _portfolio.PortfolioError
ContractValidationError = _portfolio.ContractValidationError
UnsupportedContractVersionError = _portfolio.UnsupportedContractVersionError
MissingContractVersionError = _portfolio.MissingContractVersionError
MalformedContractSchemaError = _portfolio.MalformedContractSchemaError
ContractLimitExceededError = _portfolio.ContractLimitExceededError

Portfolio = _portfolio.Portfolio
PortfolioBuilder = _portfolio.PortfolioBuilder
PositionValue = _portfolio.PositionValue
ReconciliationReport = _portfolio.ReconciliationReport
InstrumentArtifactCache = _portfolio.InstrumentArtifactCache
MaterializationReport = _portfolio.MaterializationReport
PortfolioValuation = _portfolio.PortfolioValuation
ScenarioPnl = _portfolio.ScenarioPnl
ScenarioPnlBatchItem = _portfolio.ScenarioPnlBatchItem
PortfolioResult = _portfolio.PortfolioResult
PortfolioMetrics = _portfolio.PortfolioMetrics
PortfolioCashflows = _portfolio.PortfolioCashflows
PortfolioAttribution = _portfolio.PortfolioAttribution

# Typed attribution/performance result wrappers
BrinsonPeriodResult = _portfolio.BrinsonPeriodResult
CarinoLinkedAttribution = _portfolio.CarinoLinkedAttribution
DurationCellTable = _portfolio.DurationCellTable
ExcessReturnResult = _portfolio.ExcessReturnResult
FactorBrinsonResult = _portfolio.FactorBrinsonResult
FiAttributionResult = _portfolio.FiAttributionResult
FiCarinoLinkedResult = _portfolio.FiCarinoLinkedResult
FiReconciliationReport = _portfolio.FiReconciliationReport
GridAttributionResult = _portfolio.GridAttributionResult
GridCarinoLinkedResult = _portfolio.GridCarinoLinkedResult
LinkedReturn = _portfolio.LinkedReturn
ReplayResult = _portfolio.ReplayResult
WeightAllocationResult = _portfolio.WeightAllocationResult

parse_portfolio_spec_json = _portfolio.parse_portfolio_spec_json
build_portfolio_from_spec_json = _portfolio.build_portfolio_from_spec_json
aggregate_metrics = _portfolio.aggregate_metrics
aggregate_metrics_json = _portfolio.aggregate_metrics_json
value_portfolio = _portfolio.value_portfolio
aggregate_full_cashflows = _portfolio.aggregate_full_cashflows
net_in_currency_by_date = _portfolio.net_in_currency_by_date
apply_scenario_and_revalue = _portfolio.apply_scenario_and_revalue
scenario_pnl = _portfolio.scenario_pnl
scenario_pnl_batch = _portfolio.scenario_pnl_batch
scenario_pnl_batch_json = _portfolio.scenario_pnl_batch_json
attribute_portfolio_pnl = _portfolio.attribute_portfolio_pnl
allocate_weights = _portfolio.allocate_weights
allocate_weights_json = _portfolio.allocate_weights_json
validate_allocation_json = _portfolio.validate_allocation_json
optimize_portfolio = _portfolio.optimize_portfolio
replay_portfolio = _portfolio.replay_portfolio
replay_portfolio_json = _portfolio.replay_portfolio_json
brinson_fachler = _portfolio.brinson_fachler
brinson_fachler_json = _portfolio.brinson_fachler_json
carino_link = _portfolio.carino_link
carino_link_json = _portfolio.carino_link_json
campisi_attribution = _portfolio.campisi_attribution
campisi_attribution_json = _portfolio.campisi_attribution_json
campisi_carino_link = _portfolio.campisi_carino_link
campisi_carino_link_json = _portfolio.campisi_carino_link_json
campisi_carino_link_from_snapshots = _portfolio.campisi_carino_link_from_snapshots
campisi_carino_link_from_snapshots_json = _portfolio.campisi_carino_link_from_snapshots_json
campisi_reconciliation_check = _portfolio.campisi_reconciliation_check
campisi_reconciliation_check_json = _portfolio.campisi_reconciliation_check_json
cell_returns_from_curves = _portfolio.cell_returns_from_curves
cell_returns_from_curves_json = _portfolio.cell_returns_from_curves_json
cell_returns_from_reference = _portfolio.cell_returns_from_reference
cell_returns_from_reference_json = _portfolio.cell_returns_from_reference_json
excess_returns = _portfolio.excess_returns
excess_returns_json = _portfolio.excess_returns_json
factor_brinson_attribution = _portfolio.factor_brinson_attribution
factor_brinson_attribution_json = _portfolio.factor_brinson_attribution_json
grid_attribution = _portfolio.grid_attribution
grid_attribution_json = _portfolio.grid_attribution_json
grid_carino_link = _portfolio.grid_carino_link
grid_carino_link_json = _portfolio.grid_carino_link_json
twrr_modified_dietz = _portfolio.twrr_modified_dietz
twrr_linked = _portfolio.twrr_linked
twrr_linked_json = _portfolio.twrr_linked_json
mwr_xirr = _portfolio.mwr_xirr

# Portfolio factor-model workflow results
FactorRiskDecomposition = _portfolio.FactorRiskDecomposition
SensitivityMatrix = _portfolio.SensitivityMatrix
FactorPnlProfile = _portfolio.FactorPnlProfile
compute_factor_sensitivities = _portfolio.compute_factor_sensitivities
compute_pnl_profiles = _portfolio.compute_pnl_profiles
decompose_factor_risk = _portfolio.decompose_factor_risk
FactorContributionDelta = _portfolio.FactorContributionDelta
WhatIfResult = _portfolio.WhatIfResult
StressResult = _portfolio.StressResult
PositionAssignment = _portfolio.PositionAssignment
UnmatchedEntry = _portfolio.UnmatchedEntry
FactorAssignmentReport = _portfolio.FactorAssignmentReport
LevelVolContribution = _portfolio.LevelVolContribution
PositionVolContribution = _portfolio.PositionVolContribution
CreditVolReport = _portfolio.CreditVolReport
factor_stress = _portfolio.factor_stress
position_what_if = _portfolio.position_what_if
build_credit_vol_report = _portfolio.build_credit_vol_report

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
schema = _portfolio.schema

# `schema` is a real submodule, so `import finstack_quant.portfolio.schema`
# must work as well as attribute access.
if "finstack_quant.portfolio.schema" not in _sys.modules:
    _sys.modules["finstack_quant.portfolio.schema"] = schema

__all__ = [
    "BrinsonPeriodResult",
    "CandidatePosition",
    "CarinoLinkedAttribution",
    "Constraint",
    "ContractLimitExceededError",
    "ContractValidationError",
    "CreditVolReport",
    "DurationCellTable",
    "ExcessReturnResult",
    "FactorAssignmentReport",
    "FactorBrinsonResult",
    "FactorContributionDelta",
    "FactorPnlProfile",
    "FactorRiskDecomposition",
    "FiAttributionResult",
    "FiCarinoLinkedResult",
    "FiReconciliationReport",
    "FinstackError",
    "GridAttributionResult",
    "GridCarinoLinkedResult",
    "Inequality",
    "InstrumentArtifactCache",
    "LevelVolContribution",
    "LinkedReturn",
    "MalformedContractSchemaError",
    "MaterializationReport",
    "MetricExpr",
    "MissingContractVersionError",
    "MissingMetricPolicy",
    "Objective",
    "OptimizationStatus",
    "PerPositionMetric",
    "Portfolio",
    "PortfolioAttribution",
    "PortfolioBuilder",
    "PortfolioCashflows",
    "PortfolioError",
    "PortfolioMetrics",
    "PortfolioOptimizationResult",
    "PortfolioOptimizationSpec",
    "PortfolioResult",
    "PortfolioValuation",
    "PositionAssignment",
    "PositionFilter",
    "PositionValue",
    "PositionVolContribution",
    "ReconciliationReport",
    "ReplayResult",
    "ScenarioPnl",
    "ScenarioPnlBatchItem",
    "SensitivityMatrix",
    "StressResult",
    "TradeDirection",
    "TradeSpec",
    "TradeType",
    "TradeUniverse",
    "UnmatchedEntry",
    "UnsupportedContractVersionError",
    "WeightAllocationResult",
    "WeightingScheme",
    "WhatIfResult",
    "aggregate_full_cashflows",
    "aggregate_metrics",
    "aggregate_metrics_json",
    "allocate_weights",
    "allocate_weights_json",
    "apply_scenario_and_revalue",
    "attribute_portfolio_pnl",
    "brinson_fachler",
    "brinson_fachler_json",
    "build_credit_vol_report",
    "build_portfolio_from_spec_json",
    "campisi_attribution",
    "campisi_attribution_json",
    "campisi_carino_link",
    "campisi_carino_link_from_snapshots",
    "campisi_carino_link_from_snapshots_json",
    "campisi_carino_link_json",
    "campisi_reconciliation_check",
    "campisi_reconciliation_check_json",
    "carino_link",
    "carino_link_json",
    "cell_returns_from_curves",
    "cell_returns_from_curves_json",
    "cell_returns_from_reference",
    "cell_returns_from_reference_json",
    "compute_factor_sensitivities",
    "compute_pnl_profiles",
    "decompose_factor_risk",
    "excess_returns",
    "excess_returns_json",
    "factor_brinson_attribution",
    "factor_brinson_attribution_json",
    "factor_stress",
    "grid_attribution",
    "grid_attribution_json",
    "grid_carino_link",
    "grid_carino_link_json",
    "mwr_xirr",
    "net_in_currency_by_date",
    "optimize_portfolio",
    "parse_portfolio_spec_json",
    "position_what_if",
    "replay_portfolio",
    "replay_portfolio_json",
    "scenario_pnl",
    "scenario_pnl_batch",
    "scenario_pnl_batch_json",
    "schema",
    "twrr_linked",
    "twrr_linked_json",
    "twrr_modified_dietz",
    "validate_allocation_json",
    "value_portfolio",
]
