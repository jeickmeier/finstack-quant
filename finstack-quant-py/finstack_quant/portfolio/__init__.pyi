"""
Portfolio construction, valuation, optimization, cashflows, scenarios, and metrics.

Examples
--------
>>> from finstack_quant.portfolio import Portfolio
>>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
>>> (Portfolio.from_spec(spec).id, len(Portfolio.from_spec(spec)))
('empty', 0)
"""

from __future__ import annotations

from typing import Any
from finstack_quant.core import FinstackError
from finstack_quant.portfolio import schema as schema
from finstack_quant.portfolio._materialization import (
    InstrumentArtifactCache as InstrumentArtifactCache,
    MaterializationReport as MaterializationReport,
)
from finstack_quant.portfolio._container import (
    Portfolio as Portfolio,
    PortfolioBuilder as PortfolioBuilder,
    parse_portfolio_spec_json as parse_portfolio_spec_json,
)
from finstack_quant.portfolio._valuation import (
    PortfolioValuation as PortfolioValuation,
    PositionValue as PositionValue,
    value_portfolio as value_portfolio,
)
from finstack_quant.portfolio._attribution import (
    PortfolioAttribution as PortfolioAttribution,
    ReconciliationReport as ReconciliationReport,
    attribute_portfolio_pnl as attribute_portfolio_pnl,
)
from finstack_quant.portfolio._cashflows import (
    PortfolioCashflows as PortfolioCashflows,
    aggregate_full_cashflows as aggregate_full_cashflows,
    net_in_currency_by_date as net_in_currency_by_date,
)
from finstack_quant.portfolio._margin import (
    NettingSetMargin as NettingSetMargin,
    PortfolioMarginResult as PortfolioMarginResult,
    portfolio_margin as portfolio_margin,
)
from finstack_quant.portfolio._results import PortfolioResult as PortfolioResult
from finstack_quant.portfolio._metrics import (
    PortfolioMetrics as PortfolioMetrics,
    aggregate_metrics as aggregate_metrics,
)
from finstack_quant.portfolio._scenarios import (
    ScenarioPnl as ScenarioPnl,
    ScenarioPnlBatchItem as ScenarioPnlBatchItem,
    apply_scenario_and_revalue as apply_scenario_and_revalue,
    scenario_pnl as scenario_pnl,
    scenario_pnl_batch as scenario_pnl_batch,
)
from finstack_quant.portfolio._allocation import (
    WeightAllocationResult as WeightAllocationResult,
    allocate_weights as allocate_weights,
)
from finstack_quant.portfolio._replay import ReplayResult as ReplayResult, replay_portfolio as replay_portfolio
from finstack_quant.portfolio._brinson import (
    BrinsonPeriodResult as BrinsonPeriodResult,
    CarinoLinkedAttribution as CarinoLinkedAttribution,
    brinson_fachler as brinson_fachler,
    carino_link as carino_link,
)
from finstack_quant.portfolio._fi_attribution import (
    FiAttributionResult as FiAttributionResult,
    FiCarinoLinkedResult as FiCarinoLinkedResult,
    campisi_attribution as campisi_attribution,
    campisi_carino_link as campisi_carino_link,
)
from finstack_quant.portfolio._excess_return import (
    DurationCellTable as DurationCellTable,
    ExcessReturnResult as ExcessReturnResult,
    cell_returns_from_curves as cell_returns_from_curves,
    cell_returns_from_reference as cell_returns_from_reference,
    excess_returns as excess_returns,
)
from finstack_quant.portfolio._grid_attribution import (
    GridAttributionResult as GridAttributionResult,
    GridCarinoLinkedResult as GridCarinoLinkedResult,
    grid_attribution as grid_attribution,
    grid_carino_link as grid_carino_link,
)
from finstack_quant.portfolio._factor_brinson import (
    FactorBrinsonResult as FactorBrinsonResult,
    factor_brinson_attribution as factor_brinson_attribution,
)
from finstack_quant.portfolio._performance import (
    LinkedReturn as LinkedReturn,
    mwr_xirr as mwr_xirr,
    twrr_linked as twrr_linked,
    twrr_modified_dietz as twrr_modified_dietz,
)
from finstack_quant.portfolio._factor_model.whatif import (
    FactorContributionDelta as FactorContributionDelta,
    StressPnl as StressPnl,
    StressResult as StressResult,
    WhatIfResult as WhatIfResult,
)
from finstack_quant.portfolio._factor_model.model import FactorModel as FactorModel
from finstack_quant.portfolio._primitive import (
    PortfolioPrimitiveExposureReport as PortfolioPrimitiveExposureReport,
    primitive_exposures as primitive_exposures,
)
from finstack_quant.portfolio._factor_model.assignment import (
    FactorAssignmentReport as FactorAssignmentReport,
    PositionAssignment as PositionAssignment,
    UnmatchedEntry as UnmatchedEntry,
)
from finstack_quant.portfolio._factor_model.credit_vol_report import (
    CreditVolReport as CreditVolReport,
    LevelVolContribution as LevelVolContribution,
    PositionVolContribution as PositionVolContribution,
    build_credit_vol_report as build_credit_vol_report,
)
from finstack_quant.portfolio._optimization.enums import (
    Inequality as Inequality,
    MissingMetricPolicy as MissingMetricPolicy,
    TradeDirection as TradeDirection,
    TradeType as TradeType,
    WeightingScheme as WeightingScheme,
)
from finstack_quant.portfolio._optimization.expressions import (
    Constraint as Constraint,
    MetricExpr as MetricExpr,
    Objective as Objective,
    PerPositionMetric as PerPositionMetric,
    PositionFilter as PositionFilter,
)
from finstack_quant.portfolio._optimization.spec_result import (
    CandidatePosition as CandidatePosition,
    PortfolioOptimizationResult as PortfolioOptimizationResult,
    PortfolioOptimizationSpec as PortfolioOptimizationSpec,
    TradeUniverse as TradeUniverse,
)
from finstack_quant.portfolio._optimization.status_trade import (
    OptimizationStatus as OptimizationStatus,
    TradeSpec as TradeSpec,
)
from finstack_quant.portfolio._optimization.optimize import (
    optimize_portfolio as optimize_portfolio,
    rebalance_from_spec as rebalance_from_spec,
)
from finstack_quant.portfolio._sensitivity import (
    FactorPnlProfile as FactorPnlProfile,
    SensitivityMatrix as SensitivityMatrix,
    compute_factor_sensitivities as compute_factor_sensitivities,
    compute_pnl_profiles as compute_pnl_profiles,
)

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
    "FactorModel",
    "FactorPnlProfile",
    "FiAttributionResult",
    "FiCarinoLinkedResult",
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
    "NettingSetMargin",
    "Objective",
    "OptimizationStatus",
    "PerPositionMetric",
    "Portfolio",
    "PortfolioAttribution",
    "PortfolioBuilder",
    "PortfolioCashflows",
    "PortfolioError",
    "PortfolioMarginResult",
    "PortfolioMetrics",
    "PortfolioOptimizationResult",
    "PortfolioOptimizationSpec",
    "PortfolioPrimitiveExposureReport",
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
    "StressPnl",
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
    "allocate_weights",
    "apply_scenario_and_revalue",
    "attribute_portfolio_pnl",
    "brinson_fachler",
    "build_credit_vol_report",
    "campisi_attribution",
    "campisi_carino_link",
    "carino_link",
    "cell_returns_from_curves",
    "cell_returns_from_reference",
    "compute_factor_sensitivities",
    "compute_pnl_profiles",
    "excess_returns",
    "factor_brinson_attribution",
    "grid_attribution",
    "grid_carino_link",
    "mwr_xirr",
    "net_in_currency_by_date",
    "optimize_portfolio",
    "parse_portfolio_spec_json",
    "portfolio_margin",
    "primitive_exposures",
    "rebalance_from_spec",
    "replay_portfolio",
    "scenario_pnl",
    "scenario_pnl_batch",
    "schema",
    "twrr_linked",
    "twrr_modified_dietz",
    "value_portfolio",
]

class PortfolioError(FinstackError):
    """
    Portfolio validation failure (a ``ValueError``).

    Portfolio errors are classified by their Rust-owned kind: validation
    failures raise ``PortfolioError``, not-found failures (unknown entity,
    missing market data or FX rate, a valuation that failed on missing data)
    raise ``KeyError``, and computation failures raise ``RuntimeError``.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioError
    >>> str(PortfolioError("invalid portfolio"))
    'invalid portfolio'
    """

class ContractValidationError(FinstackError):
    """
    Persisted contract validation failure with structured diagnostics.

    ``report`` is a list of dictionaries with stable ``code`` and optional
    RFC 6901 ``pointer`` entries suitable for form and ingestion error UIs.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError
    >>> issubclass(ContractValidationError, ValueError)
    True
    """

    report: list[dict[str, Any]]

class UnsupportedContractVersionError(ContractValidationError):
    """
    Persisted payload uses an unsupported positive contract version.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError, UnsupportedContractVersionError
    >>> issubclass(UnsupportedContractVersionError, ContractValidationError)
    True
    """

class MissingContractVersionError(ContractValidationError):
    """
    Strict persisted payload omits its required contract version.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError, MissingContractVersionError
    >>> issubclass(MissingContractVersionError, ContractValidationError)
    True
    """

class MalformedContractSchemaError(ContractValidationError):
    """
    Persisted payload has a malformed or mismatched schema marker.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError, MalformedContractSchemaError
    >>> issubclass(MalformedContractSchemaError, ContractValidationError)
    True
    """

class ContractLimitExceededError(ContractValidationError):
    """
    Persisted payload exceeds a configured byte, count, or depth bound.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError, ContractLimitExceededError
    >>> issubclass(ContractLimitExceededError, ContractValidationError)
    True
    """
