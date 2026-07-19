"""
Portfolio construction, valuation, optimization, cashflows, scenarios, and metrics.

Examples
--------
>>> import finstack_quant.portfolio as portfolio
>>> portfolio.__name__
'finstack_quant.portfolio'
"""

from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt
import pandas as pd

from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.factor_model.credit import CreditFactorModel

__all__ = [
    "FinstackFxError",
    "FinstackOptimizationError",
    "FinstackValuationError",
    "Portfolio",
    "PortfolioAttribution",
    "PortfolioCashflows",
    "PortfolioError",
    "PortfolioMetrics",
    "PortfolioResult",
    "PortfolioValuation",
    "aggregate_full_cashflows",
    "aggregate_metrics",
    "almgren_chriss_impact",
    "amihud_illiquidity",
    "apply_scenario_and_revalue",
    "attribute_portfolio_pnl",
    "allocate_weights",
    "brinson_fachler",
    "build_credit_vol_report",
    "build_portfolio_from_spec",
    "build_stress_attribution",
    "carino_link",
    "days_to_liquidate",
    "evaluate_risk_budget",
    "factor_stress",
    "historical_var_decomposition",
    "kyle_lambda",
    "liquidity_tier",
    "lvar_bangia",
    "mwr_xirr",
    "optimize_portfolio",
    "parametric_es_decomposition",
    "parametric_var_decomposition",
    "parse_portfolio_spec",
    "portfolio_result_get_metric",
    "portfolio_result_total_value",
    "replay_portfolio",
    "position_what_if",
    "roll_effective_spread",
    "scenario_pnl",
    "scenario_pnl_batch",
    "twrr_linked",
    "twrr_modified_dietz",
    "validate_allocation_json",
    "value_portfolio",
    "value_portfolio_typed",
    # factor_model typed result classes
    "FactorContribution",
    "PositionFactorContribution",
    "PositionResidualContribution",
    "RiskDecomposition",
    "FactorRiskDecomposition",
    "SensitivityMatrix",
    "FactorPnlProfile",
    "compute_factor_sensitivities",
    "compute_pnl_profiles",
    "decompose_factor_risk",
    "PositionVarContribution",
    "PositionEsContribution",
    "PositionRiskDecomposition",
    "PositionBudgetEntry",
    "RiskBudgetResult",
    "FactorContributionDelta",
    "WhatIfResult",
    "StressResult",
    "StressPositionEntry",
    "TailScenarioBreakdown",
    "StressAttribution",
    "PositionAssignment",
    "UnmatchedEntry",
    "FactorAssignmentReport",
    "LevelVolContribution",
    "PositionVolContribution",
    "CreditVolReport",
    "VolHorizon",
    "DecompositionConfig",
    "parametric_var_decomposition_typed",
    "historical_var_decomposition_typed",
    "evaluate_risk_budget_typed",
    "position_component_var",
    # optimization spec/result classes
    "WeightingScheme",
    "MissingMetricPolicy",
    "Inequality",
    "TradeDirection",
    "TradeType",
    "PerPositionMetric",
    "PositionFilter",
    "MetricExpr",
    "Objective",
    "Constraint",
    "CandidatePosition",
    "TradeUniverse",
    "OptimizationStatus",
    "TradeSpec",
    "PortfolioOptimizationSpec",
    "PortfolioOptimizationResult",
    "optimize_portfolio_typed",
]

class PortfolioError(ValueError):
    """
    Portfolio validation or calculation failure.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioError
    >>> PortfolioError.__name__
    'PortfolioError'
    """

class FinstackValuationError(PortfolioError):
    """
    Portfolio valuation failure.

    Examples
    --------
    >>> from finstack_quant.portfolio import FinstackValuationError
    >>> FinstackValuationError.__name__
    'FinstackValuationError'
    """

class FinstackFxError(PortfolioError):
    """
    Portfolio FX conversion or market-data failure.

    Examples
    --------
    >>> from finstack_quant.portfolio import FinstackFxError
    >>> FinstackFxError.__name__
    'FinstackFxError'
    """

class FinstackOptimizationError(PortfolioError):
    """
    Portfolio optimization failure.

    Examples
    --------
    >>> from finstack_quant.portfolio import FinstackOptimizationError
    >>> FinstackOptimizationError.__name__
    'FinstackOptimizationError'
    """

class Portfolio:
    """
    Built runtime portfolio. Cheap to clone; pass directly to pipeline functions.

    Build once with :meth:`from_spec` and reuse across ``value_portfolio``,
    ``aggregate_full_cashflows``, ``aggregate_metrics``, and
    ``apply_scenario_and_revalue`` to skip the per-call spec parse + index
    rebuild.

    Examples
    --------
    >>> from finstack_quant.portfolio import Portfolio
    >>> Portfolio.__name__
    'Portfolio'
    """

    @staticmethod
    def from_spec(spec_json: str) -> Portfolio:
        """
        Parse a ``PortfolioSpec`` JSON string into a runtime portfolio.

        Parameters
        ----------
        spec_json : str
            Portfolio specification JSON, including positions, base currency,
            and as-of date, to validate and compile for reuse.

        Returns
        -------
        Portfolio
            Result of from spec for this `Portfolio` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import Portfolio
        >>> callable(Portfolio.from_spec)
        True
        """
        ...

    @property
    def id(self) -> str:
        """
        Portfolio identifier.
        Returns
        -------
        str
            The id exposed by this `Portfolio`.
        """
        ...

    @property
    def as_of(self) -> str:
        """
        Portfolio as-of date as an ISO 8601 string.
        Returns
        -------
        str
            The as of exposed by this `Portfolio`.
        """
        ...

    @property
    def base_ccy(self) -> str:
        """
        Base currency code used for valuation and aggregation.
        Returns
        -------
        str
            The base ccy exposed by this `Portfolio`.
        """
        ...

    def __len__(self) -> int:
        """Number of positions in the built portfolio.
        Returns
        -------
        int
        """
        ...

    def to_spec_json(self) -> str:
        """
        Serialize the portfolio back to its canonical ``PortfolioSpec`` JSON.
        Returns
        -------
        str
            Result of to spec json for this `Portfolio` in the annotated representation.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioAttribution:
    """
    Rust-computed portfolio P&L attribution in portfolio base currency.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioAttribution
    >>> PortfolioAttribution.__name__
    'PortfolioAttribution'
    """

    def to_json(self) -> str:
        """
        Serialize the complete canonical attribution payload.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioAttribution`, suitable for a matching `from_json` call.
        """
        ...

    def by_position_json(self) -> str:
        """
        Serialize nested attribution in Rust ``IndexMap`` position order.

        Returns
        -------
        str
            Result of by position json for this `PortfolioAttribution` in the annotated representation.
        """
        ...

    def reconciliation_check(self, tolerance: float) -> dict[str, float | bool]:
        """
        Reconcile aggregate factor P&L to total P&L.

        Parameters
        ----------
        tolerance : float
            Absolute base-currency difference tolerated before reconciliation
            is reported as failing.

        Returns
        -------
        dict[str, float | bool]
            Result of reconciliation check for this `PortfolioAttribution` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def total_pnl(self) -> Money:
        """
        Return the total pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The total pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def carry(self) -> Money:
        """
        Return the carry for `PortfolioAttribution`.

        Returns
        -------
        Money
            The carry exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def rates_curves_pnl(self) -> Money:
        """
        Return the rates curves pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The rates curves pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def credit_curves_pnl(self) -> Money:
        """
        Return the credit curves pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The credit curves pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def inflation_curves_pnl(self) -> Money:
        """
        Return the inflation curves pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The inflation curves pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def correlations_pnl(self) -> Money:
        """
        Return the correlations pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The correlations pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def fx_pnl(self) -> Money:
        """
        Return the fx pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The fx pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def fx_translation_pnl(self) -> Money:
        """
        Return the fx translation pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The fx translation pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def cross_factor_pnl(self) -> Money:
        """
        Return the cross factor pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The cross factor pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def vol_pnl(self) -> Money:
        """
        Return the vol pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The vol pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def model_params_pnl(self) -> Money:
        """
        Return the model params pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The model params pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def market_scalars_pnl(self) -> Money:
        """
        Return the market scalars pnl for `PortfolioAttribution`.

        Returns
        -------
        Money
            The market scalars pnl exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def residual(self) -> Money:
        """
        Return the residual for `PortfolioAttribution`.

        Returns
        -------
        Money
            The residual exposed by this `PortfolioAttribution`.
        """
        ...

    @property
    def result_invalid(self) -> bool:
        """
        Return the result invalid for `PortfolioAttribution`.

        Returns
        -------
        bool
            The result invalid exposed by this `PortfolioAttribution`.
        """
        ...

    def __repr__(self) -> str: ...

class PortfolioValuation:
    """
    Typed wrapper around a ``PortfolioValuation`` result.

    Wrap the JSON returned by :func:`value_portfolio` once and pass the typed
    object to :func:`aggregate_metrics` to skip re-parsing.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioValuation
    >>> PortfolioValuation.__name__
    'PortfolioValuation'
    """

    @staticmethod
    def from_json(valuation_json: str) -> PortfolioValuation:
        """
        Deserialize a ``PortfolioValuation`` from JSON.

        Parameters
        ----------
        valuation_json : str
            Canonical valuation payload returned by ``value_portfolio`` or an
            equivalent serialized portfolio valuation.

        Returns
        -------
        PortfolioValuation
            Validated `PortfolioValuation` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioValuation
        >>> callable(PortfolioValuation.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this valuation to canonical JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioValuation`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def total_value(self) -> float:
        """
        Total portfolio value in ``base_ccy``.
        Returns
        -------
        float
            The total value exposed by this `PortfolioValuation`.
        """
        ...

    @property
    def base_ccy(self) -> str:
        """
        Base currency code for this valuation.
        Returns
        -------
        str
            The base ccy exposed by this `PortfolioValuation`.
        """
        ...

    @property
    def as_of(self) -> str:
        """
        Valuation date as an ISO 8601 string.
        Returns
        -------
        str
            The as of exposed by this `PortfolioValuation`.
        """
        ...

    def __len__(self) -> int:
        """Number of valued positions.
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioCashflows:
    """
    Typed wrapper around a ``PortfolioCashflows`` ladder.

    Returned by :func:`aggregate_full_cashflows`; survives multiple drill-in
    calls (``events_json``, ``by_date_json``, ``issues_json``,
    :meth:`collapse_to_base_by_date_kind`) without re-parsing.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioCashflows
    >>> PortfolioCashflows.__name__
    'PortfolioCashflows'
    """

    @staticmethod
    def from_json(cashflows_json: str) -> PortfolioCashflows:
        """
        Deserialize a cashflow ladder from JSON.

        Parameters
        ----------
        cashflows_json : str
            Canonical classified-cashflow payload returned by the aggregation
            API or an equivalent serialized ladder.

        Returns
        -------
        PortfolioCashflows
            Validated `PortfolioCashflows` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioCashflows
        >>> callable(PortfolioCashflows.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the full cashflow ladder to canonical JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioCashflows`, suitable for a matching `from_json` call.
        """
        ...

    def events_json(self) -> str:
        """
        Return all dated cashflow events as JSON.
        Returns
        -------
        str
            Result of events json for this `PortfolioCashflows` in the annotated representation.
        """
        ...

    def by_date_json(self) -> str:
        """
        Return cashflows grouped by date as JSON.
        Returns
        -------
        str
            Result of by date json for this `PortfolioCashflows` in the annotated representation.
        """
        ...

    def issues_json(self) -> str:
        """
        Return cashflow aggregation or FX-conversion issues as JSON.
        Returns
        -------
        str
            Result of issues json for this `PortfolioCashflows` in the annotated representation.
        """
        ...

    def num_positions(self) -> int:
        """
        Number of positions represented in the ladder.
        Returns
        -------
        int
            Result of num positions for this `PortfolioCashflows` in the annotated representation.
        """
        ...

    def num_issues(self) -> int:
        """
        Number of diagnostic issues recorded on the ladder.
        Returns
        -------
        int
            Result of num issues for this `PortfolioCashflows` in the annotated representation.
        """
        ...

    def collapse_to_base_by_date_kind(
        self,
        market: MarketContext | str,
        base_ccy: str,
        as_of: str,
    ) -> str:
        """
        Collapse the ladder to a base-currency ``(date, kind) → Money`` JSON.

        Uses **spot-equivalent** FX at each payment date. ``as_of`` is the
        valuation/run date used to flag far-future conversions.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON providing FX data for payment-date
            base-currency conversion.
        base_ccy : str
            ISO currency code into which each classified cashflow is converted.
        as_of : str
            ISO-8601 valuation date used for conversion diagnostics and limits.

        Returns
        -------
        str
            Result of collapse to base by date kind for this `PortfolioCashflows` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def __len__(self) -> int:
        """Number of cashflow events.
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioResult:
    """
    Typed wrapper around a ``PortfolioResult`` envelope.

    Use the scalar accessors (``total_value``, ``get_metric``) to read single
    values without re-parsing the JSON envelope.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioResult
    >>> PortfolioResult.__name__
    'PortfolioResult'
    """

    @staticmethod
    def from_json(result_json: str) -> PortfolioResult:
        """
        Deserialize a portfolio result envelope from JSON.

        Parameters
        ----------
        result_json : str
            Canonical portfolio-result JSON containing total value and metrics.

        Returns
        -------
        PortfolioResult
            Validated `PortfolioResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioResult
        >>> callable(PortfolioResult.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result envelope to canonical JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioResult`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def total_value(self) -> float:
        """
        Total value stored in the result envelope.
        Returns
        -------
        float
            The total value exposed by this `PortfolioResult`.
        """
        ...

    def get_metric(self, metric_id: str) -> float | None:
        """
        Return a metric value, or ``None`` when it is absent.

        Parameters
        ----------
        metric_id : str
            Fully qualified metric key, such as ``"pv01::usd_ois"``.

        Returns
        -------
        float | None
            Requested metric resolved from this `PortfolioResult` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def require_metric(self, metric_id: str) -> float:
        """
        Return a metric value, raising ``KeyError`` when it is absent.

        Parameters
        ----------
        metric_id : str
            Fully qualified metric key that must be present in the result.

        Returns
        -------
        float
            Result of require metric for this `PortfolioResult` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioMetrics:
    """
    Typed wrapper around Rust-aggregated portfolio metrics.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioMetrics
    >>> PortfolioMetrics.__name__
    'PortfolioMetrics'
    """

    @staticmethod
    def from_json(metrics_json: str) -> PortfolioMetrics:
        """
        Deserialize canonical ``PortfolioMetrics`` JSON.

        Parameters
        ----------
        metrics_json : str
            Canonical metric payload returned by ``aggregate_metrics``.

        Returns
        -------
        PortfolioMetrics
            Validated `PortfolioMetrics` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioMetrics
        >>> callable(PortfolioMetrics.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize canonical ``PortfolioMetrics`` JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioMetrics`, suitable for a matching `from_json` call.
        """
        ...

    def metric_series(
        self,
        base: str,
    ) -> list[tuple[list[str], float, dict[str, float]]]:
        """
        Return decoded components, total, and entity values in wire order.

        Entity mappings preserve Rust ``IndexMap`` insertion order. Malformed
        legacy escapes remain literal; decoded coordinate collisions use
        literal wire components so no aggregate entry is lost.

        Parameters
        ----------
        base : str
            Metric namespace prefix used to select matching metric series.

        Returns
        -------
        list[tuple[list[str], float, dict[str, float]]]
            Result of metric series for this `PortfolioMetrics` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def __repr__(self) -> str: ...

def parse_portfolio_spec(json_str: str) -> str:
    """
    Parse and canonicalize a ``PortfolioSpec`` from JSON.

    Parameters
    ----------
    json_str : str
        Portfolio specification JSON to validate and normalize into canonical
        Rust serialization.

    Returns
    -------
    str
        Result of parse portfolio spec for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import parse_portfolio_spec
    >>> callable(parse_portfolio_spec)
    True
    """
    ...

def build_portfolio_from_spec(spec_json: str) -> str:
    """
    Build a runtime portfolio from JSON and return the round-tripped spec.

    Prefer :meth:`Portfolio.from_spec` for real work — it returns the typed
    object that pipeline functions reuse without rebuilding.

    Parameters
    ----------
    spec_json : str
        Portfolio specification JSON to validate, compile, and serialize back.

    Returns
    -------
    str
        Result of build portfolio from spec for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import build_portfolio_from_spec
    >>> callable(build_portfolio_from_spec)
    True
    """
    ...

def portfolio_result_total_value(result: PortfolioResult | str) -> float:
    """
    Read total portfolio value from a ``PortfolioResult`` envelope.

    Accepts a typed :class:`PortfolioResult` (O(1)) or a JSON string
    (O(size-of-envelope)).

    Parameters
    ----------
    result : PortfolioResult or str
        Typed result envelope or canonical result JSON containing total value.

    Returns
    -------
    float
        Result of portfolio result total value for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import portfolio_result_total_value
    >>> callable(portfolio_result_total_value)
    True
    """
    ...

def portfolio_result_get_metric(result: PortfolioResult | str, metric_id: str) -> float | None:
    """
    Read one metric from a ``PortfolioResult``.

    Accepts a typed :class:`PortfolioResult` or a JSON string.

    Parameters
    ----------
    result : PortfolioResult or str
        Typed result envelope or canonical JSON containing portfolio metrics.
    metric_id : str
        Fully qualified metric key, such as ``"cs01::BOND_A"``.

    Returns
    -------
    float | None
        Result of portfolio result get metric for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import portfolio_result_get_metric
    >>> callable(portfolio_result_get_metric)
    True
    """
    ...

def aggregate_metrics(
    valuation: PortfolioValuation | str,
    base_ccy: str,
    market: MarketContext | str,
    as_of: str,
) -> str:
    """
    Aggregate portfolio metrics from a valuation.

    Accepts a typed :class:`PortfolioValuation` (fast path) or a JSON string.

    Parameters
    ----------
    valuation : PortfolioValuation or str
        Typed valuation or canonical valuation JSON to aggregate.
    base_ccy : str
        ISO base-currency code in which aggregate values and metrics are stated.
    market : MarketContext or str
        Market context object or JSON supplying conversion and market inputs.
    as_of : str
        ISO-8601 valuation date used to resolve date-dependent market data.

    Returns
    -------
    str
        Result of aggregate metrics for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import aggregate_metrics
    >>> callable(aggregate_metrics)
    True
    """
    ...

def value_portfolio(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    strict_risk: bool = False,
    metrics: list[str] | None = None,
) -> str:
    """
    Value a portfolio.

    Accepts either a typed :class:`Portfolio` (no rebuild) or a JSON
    ``PortfolioSpec`` string, and either a typed ``MarketContext`` or a JSON
    string. Returns JSON for backwards compatibility — wrap with
    :meth:`PortfolioValuation.from_json` once to enable the fast downstream
    path into ``aggregate_metrics``.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to value.
    market : MarketContext or str
        Market context object or JSON supplying curves, quotes, and FX data.
    strict_risk : bool
        Whether absent or failed risk calculations are treated as errors rather
        than diagnostic output; defaults to ``False``.
    metrics : list[str] or None, default None
        Exact metric identifiers to compute. ``None`` requests the standard
        portfolio risk set; an empty list performs PV-only valuation.

    Returns
    -------
    str
        Result of value portfolio for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import value_portfolio
    >>> callable(value_portfolio)
    True
    """
    ...

def value_portfolio_typed(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    strict_risk: bool = False,
    metrics: list[str] | None = None,
) -> PortfolioValuation:
    """
    Value a portfolio and return a typed result without JSON serialization.

    This is the preferred entry point for in-process Python pipelines. Typed
    ``Portfolio`` and ``MarketContext`` inputs avoid rebuilding runtime state,
    while the typed result can be passed directly to :func:`aggregate_metrics`.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to value.
    market : MarketContext or str
        Market context object or JSON supplying curves, quotes, and FX data.
    strict_risk : bool, default False
        Whether absent or failed risk calculations abort the valuation rather
        than being recorded as diagnostics.
    metrics : list[str] or None, default None
        Exact metric identifiers to compute. ``None`` requests the standard
        portfolio risk set; an empty list performs PV-only valuation.

    Returns
    -------
    PortfolioValuation
        Typed valuation result backed directly by the Rust calculation.

    Raises
    ------
    PortfolioError
        If portfolio construction, market lookup, FX conversion, pricing, or
        strict risk evaluation fails.

    Examples
    --------
    >>> from finstack_quant.portfolio import value_portfolio_typed
    >>> callable(value_portfolio_typed)
    True
    """
    ...

def aggregate_full_cashflows(portfolio: Portfolio | str, market: MarketContext | str) -> PortfolioCashflows:
    """
    Build the full classified cashflow ladder for the portfolio.

    Returns a typed :class:`PortfolioCashflows` wrapper; call ``to_json()``
    to get the raw ladder or use the typed accessors to drill in.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to expand.
    market : MarketContext or str
        Market context object or JSON needed for instrument cashflow generation.

    Returns
    -------
    PortfolioCashflows
        Result of aggregate full cashflows for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import aggregate_full_cashflows
    >>> callable(aggregate_full_cashflows)
    True
    """
    ...

def apply_scenario_and_revalue(
    portfolio: Portfolio | str,
    scenario_json: str,
    market: MarketContext | str,
) -> tuple[str, str]:
    """
    Apply a scenario and revalue the portfolio.

    Returns ``(valuation_json, report_json)``.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to revalue.
    scenario_json : str
        Canonical scenario specification JSON describing market-data shocks.
    market : MarketContext or str
        Base market context object or JSON to shock before revaluation.

    Returns
    -------
    tuple[str, str]
        Result of apply scenario and revalue for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import apply_scenario_and_revalue
    >>> callable(apply_scenario_and_revalue)
    True
    """
    ...

def scenario_pnl(
    portfolio: Portfolio | str,
    scenario_json: str,
    market: MarketContext | str,
) -> tuple[str, str]:
    """
    Compute the profit and loss attributable to a scenario.

    Values the portfolio against the unshocked market and against the
    scenario-shocked market, then reports the base-currency difference per
    position and in total. Positions added or removed by the scenario are
    zero-filled against the missing side, so ``by_position`` always sums to
    ``total``.

    Returns ``(pnl_json, report_json)``.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON valued on both the
        unshocked and shocked legs.
    scenario_json : str
        Canonical scenario specification JSON describing the market-data shocks
        whose profit-and-loss impact is measured.
    market : MarketContext or str
        Unshocked market context object or JSON used as the base leg and as the
        source the scenario operations are applied to.

    Returns
    -------
    tuple[str, str]
        ``(pnl_json, report_json)`` — the ``ScenarioPnl`` ladder (base-currency
        ``total`` and ``by_position`` amounts) and the scenario application
        report carrying which operations were applied.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or
        domain constraints, or if a position's shocked and base values carry
        different currencies.

    Examples
    --------
    >>> from finstack_quant.portfolio import scenario_pnl
    >>> callable(scenario_pnl)
    True
    """
    ...

def scenario_pnl_batch(
    portfolio: Portfolio | str,
    scenarios_json: str,
    market: MarketContext | str,
) -> str:
    """
    Compute ordered scenario P&L results while reusing one base valuation.

    The Rust portfolio engine values the unshocked portfolio once, applies
    each scenario independently, and returns results in the same order as the
    input JSON array. It is the batch counterpart to :func:`scenario_pnl`;
    use it when more than one scenario is evaluated against one portfolio and
    market snapshot.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON. The typed form
        avoids rebuilding the portfolio for the batch.
    scenarios_json : str
        Canonical JSON array of ``ScenarioSpec`` objects. Array order is
        preserved exactly. ``"[]"`` returns ``"[]"`` without valuation.
    market : MarketContext or str
        Unshocked market context or canonical market JSON used for the shared
        base valuation and each scenario application.

    Returns
    -------
    str
        Canonical JSON array. Each item has ``scenario_id``, ``pnl`` (the same
        base-currency ``ScenarioPnl`` shape returned by :func:`scenario_pnl`),
        and ``report`` (the corresponding scenario application report).

    Raises
    ------
    ValueError
        If ``scenarios_json`` is malformed or cannot deserialize to an ordered
        array of valid ``ScenarioSpec`` values.
    PortfolioError
        If scenario application, valuation, or base-currency P&L differencing
        fails. The reported error is for the earliest failing input scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import scenario_pnl_batch
    >>> callable(scenario_pnl_batch)
    True
    """
    ...

def attribute_portfolio_pnl(
    portfolio: Portfolio | str,
    market_t0: MarketContext | str,
    market_t1: MarketContext | str,
    as_of_t0: str,
    as_of_t1: str,
    method: str | dict[str, Any],
    config: dict[str, Any] | str | None = None,
) -> PortfolioAttribution:
    """
    Attribute portfolio P&L with Rust-owned aggregation and FX translation.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON being attributed.
    market_t0 : MarketContext or str
        Opening market context object or JSON at the start of the P&L interval.
    market_t1 : MarketContext or str
        Closing market context object or JSON at the end of the P&L interval.
    as_of_t0 : str
        ISO-8601 opening valuation date associated with ``market_t0``.
    as_of_t1 : str
        ISO-8601 closing valuation date associated with ``market_t1``.
    method : str or dict[str, Any]
        Attribution-method name or method configuration understood by Rust.
    config : dict[str, Any] or str or None
        Optional attribution configuration mapping or canonical JSON payload.

    Returns
    -------
    PortfolioAttribution
        Result of attribute portfolio pnl for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import attribute_portfolio_pnl
    >>> callable(attribute_portfolio_pnl)
    True
    """
    ...

def allocate_weights(spec_json: str) -> str:
    """
    Allocate strategy weights from a JSON specification.

    The specification contains a ``scheme`` (for example
    ``"inverse_volatility"``), ``total_capital``, and a list of strategy
    objects with ``id`` and return/history fields required by the selected
    scheme. The Rust allocator computes normalized weights and money amounts;
    Python only passes the JSON through.

    Parameters
    ----------
    spec_json : str
        JSON-serialized allocation specification.

    Returns
    -------
    str
        JSON allocation result with the selected scheme and per-strategy
        ``id``, ``weight``, and allocated capital fields.

    Raises
    ------
    ValueError
        If the JSON is malformed, required fields are missing, the
        scheme is unsupported, or the selected scheme cannot be evaluated.

    Examples
    --------
    >>> from finstack_quant.portfolio import allocate_weights
    >>> callable(allocate_weights)
    True
    """
    ...

def validate_allocation_json(spec_json: str) -> None:
    """
    Validate a strategy allocation JSON specification.

    Performs the same Rust-side parse and semantic validation used by
    :func:`allocate_weights` without computing allocations.

    Parameters
    ----------
    spec_json : str
        JSON-serialized allocation specification.

    Raises
    ------
    ValueError
        If the specification is malformed or invalid.

    Examples
    --------
    >>> from finstack_quant.portfolio import validate_allocation_json
    >>> callable(validate_allocation_json)
    True
    """
    ...

def optimize_portfolio(spec_json: str, market: MarketContext | str) -> str:
    """
    Optimize portfolio weights using the LP-based optimizer.

    Parameters
    ----------
    spec_json : str
        JSON-encoded ``PortfolioOptimizationSpec`` combining the portfolio
        definition, objective, constraints, weighting scheme, and optional trade
        universe.
    market : MarketContext or str
        Typed market context or serialized JSON with curves and scalars required
        by metric expressions in the spec.

    Returns
    -------
    str
        Compact JSON-encoded ``PortfolioOptimizationResult``. Use
        ``json.dumps(json.loads(...), indent=2)`` to pretty-print.

    Raises
    ------
    FinstackOptimizationError
        If the spec is infeasible, unbounded, or the solver fails.
    FinstackValuationError
        If a required metric cannot be valued for a candidate position.
    ValueError
        If ``spec_json`` or market JSON is malformed.

    Examples
    --------
    >>> from finstack_quant.portfolio import optimize_portfolio
    >>> result_json = optimize_portfolio(spec_json, market)  # doctest: +SKIP
    """
    ...

def replay_portfolio(
    portfolio: Portfolio | str,
    snapshots_json: str,
    config_json: str,
) -> str:
    """
    Replay a portfolio through dated market snapshots.

    Parameters
    ----------
    portfolio : Portfolio or str
        Typed :class:`Portfolio` or JSON ``PortfolioSpec``.
    snapshots_json : str
        JSON array or envelope of market snapshots.
    config_json : str
        JSON replay configuration controlling dates, valuation
        options, and output detail.

    Returns
    -------
    str
        JSON replay result containing dated valuations and diagnostics.

    Raises
    ------
    PortfolioError
        If the portfolio, snapshots, or replay config are
        invalid, or if a snapshot valuation fails.

    Examples
    --------
    >>> from finstack_quant.portfolio import replay_portfolio
    >>> callable(replay_portfolio)
    True
    """
    ...

def parametric_var_decomposition(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]] | npt.NDArray[np.float64],
    confidence: float = 0.95,
) -> dict[str, object]:
    """
    Decompose portfolio parametric VaR across positions.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers aligned with ``weights``.
    weights : list[float]
        Portfolio weights or exposures.
    covariance : list[list[float]] or numpy.ndarray
        Square covariance matrix aligned with ``position_ids``. C-contiguous
        ``float64`` arrays use the direct buffer path.
    confidence : float, default 0.95
        VaR confidence level in ``(0, 1)``.

    Returns
    -------
    dict[str, object]
        Dict containing portfolio VaR and per-position component, marginal, and
        relative VaR contributions.

    Raises
    ------
    ValueError
        If dimensions do not match, covariance is malformed, or the
        confidence level is invalid.

    Examples
    --------
    >>> from finstack_quant.portfolio import parametric_var_decomposition
    >>> callable(parametric_var_decomposition)
    True
    """
    ...

def parametric_es_decomposition(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]] | npt.NDArray[np.float64],
    confidence: float = 0.95,
) -> dict[str, object]:
    """
    Decompose portfolio parametric expected shortfall across positions.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers aligned with ``weights``.
    weights : list[float]
        Portfolio weights or exposures.
    covariance : list[list[float]] or numpy.ndarray
        Square covariance matrix aligned with ``position_ids``. C-contiguous
        ``float64`` arrays use the direct buffer path.
    confidence : float, default 0.95
        ES confidence level in ``(0, 1)``.

    Returns
    -------
    dict[str, object]
        Dict containing portfolio ES and per-position component, marginal, and
        relative ES contributions.

    Raises
    ------
    ValueError
        If dimensions do not match, covariance is malformed, or the
        confidence level is invalid.

    Examples
    --------
    >>> from finstack_quant.portfolio import parametric_es_decomposition
    >>> callable(parametric_es_decomposition)
    True
    """
    ...

def historical_var_decomposition(
    position_ids: list[str],
    position_pnls: list[list[float]] | npt.NDArray[np.float64],
    confidence: float = 0.95,
) -> dict[str, object]:
    """
    Decompose historical VaR from scenario or realized position P&Ls.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers.
    position_pnls : list[list[float]] or numpy.ndarray
        Position-major matrix of P&Ls shaped
        ``len(position_ids) x n_scenarios``. C-contiguous ``float64`` arrays
        use the direct buffer path.
    confidence : float, default 0.95
        Historical VaR confidence level in ``(0, 1)``.

    Returns
    -------
    dict[str, object]
        Dict containing portfolio historical VaR and per-position contribution
        estimates.

    Raises
    ------
    ValueError
        If the P&L matrix is empty, ragged, dimensionally
        inconsistent, or the confidence level is invalid.

    Examples
    --------
    >>> from finstack_quant.portfolio import historical_var_decomposition
    >>> callable(historical_var_decomposition)
    True
    """
    ...

def evaluate_risk_budget(
    position_ids: list[str],
    actual_var: list[float],
    target_var_pct: list[float],
    portfolio_var: float,
    utilization_threshold: float = 1.20,
) -> dict[str, object]:
    """
    Compare actual position VaR against target risk-budget shares.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers aligned with ``actual_var`` and
        ``target_var_pct``.
    actual_var : list[float]
        Position component VaR amounts.
    target_var_pct : list[float]
        Target share of total portfolio VaR per position.
    portfolio_var : float
        Total portfolio VaR used to convert target percentages
        into target VaR amounts.
    utilization_threshold : float, default 1.20
        Breach threshold for actual / target utilization.

    Returns
    -------
    dict[str, object]
        Dict with per-position utilization, excess VaR, total over-budget risk,
        and breach flag.

    Raises
    ------
    ValueError
        If input lengths differ or risk-budget inputs are invalid.

    Examples
    --------
    >>> from finstack_quant.portfolio import evaluate_risk_budget
    >>> callable(evaluate_risk_budget)
    True
    """
    ...

def roll_effective_spread(returns: list[float]) -> float | None:
    """
    Estimate the Roll effective bid-ask spread from returns.

    Returns ``None`` when there are too few observations or the first-order
    autocovariance does not imply a positive spread.

    Parameters
    ----------
    returns : list[float]
        Ordered simple decimal returns sampled at a consistent observation
        frequency.

    Returns
    -------
    float | None
        Result of roll effective spread for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import roll_effective_spread
    >>> callable(roll_effective_spread)
    True
    """
    ...

def amihud_illiquidity(returns: list[float], volumes: list[float]) -> float | None:
    """
    Compute Amihud illiquidity from absolute returns and traded volumes.

    Parameters
    ----------
    returns : list[float]
        Period returns.
    volumes : list[float]
        Traded volumes aligned with ``returns``.

    Returns
    -------
    float or None
        Average ``abs(return) / volume`` over positive-volume observations, or
        ``None`` when no valid observations are available.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import amihud_illiquidity
    >>> callable(amihud_illiquidity)
    True
    """
    ...

def days_to_liquidate(
    position_value: float,
    avg_daily_volume: float,
    participation_rate: float,
) -> float:
    """
    Estimate liquidation horizon in trading days.

    Parameters
    ----------
    position_value : float
        Position market value to liquidate.
    avg_daily_volume : float
        Average daily market volume in the same notional units.
    participation_rate : float
        Maximum fraction of daily volume the liquidation may consume.

    Returns
    -------
    float
        ``position_value / (avg_daily_volume * participation_rate)``.

    Raises
    ------
    ValueError
        If volume or participation inputs are non-positive.

    Examples
    --------
    >>> from finstack_quant.portfolio import days_to_liquidate
    >>> callable(days_to_liquidate)
    True
    """
    ...

def liquidity_tier(days_to_liquidate: float) -> str:
    """
    Classify liquidation horizon into the Rust liquidity-tier labels.

    Parameters
    ----------
    days_to_liquidate : float
        Estimated trading-day horizon required to fully liquidate the position.

    Returns
    -------
    str
        Result of liquidity tier for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import liquidity_tier
    >>> callable(liquidity_tier)
    True
    """
    ...

def lvar_bangia(
    var: float,
    spread_mean: float,
    spread_vol: float,
    confidence: float,
    position_value: float,
) -> dict[str, float]:
    """
    Compute Bangia-style liquidity-adjusted VaR.

    Parameters
    ----------
    var : float
        Base market VaR.
    spread_mean : float
        Mean bid-ask spread.
    spread_vol : float
        Spread volatility.
    confidence : float
        Confidence level for the liquidity adjustment.
    position_value : float
        Position value used to scale spread cost.

    Returns
    -------
    dict[str, float]
        Dict containing the base VaR, liquidity add-on, and adjusted LVaR.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import lvar_bangia
    >>> callable(lvar_bangia)
    True
    """
    ...

def almgren_chriss_impact(
    position_size: float,
    avg_daily_volume: float,
    volatility: float,
    execution_horizon_days: float,
    permanent_impact_coef: float,
    temporary_impact_coef: float,
    reference_price: float | None = None,
) -> dict[str, float]:
    """
    Estimate Almgren-Chriss execution impact components.

    Parameters
    ----------
    position_size : float
        Trade size in shares or notional units.
    avg_daily_volume : float
        Average daily volume in matching units.
    volatility : float
        Asset volatility used for risk scaling.
    execution_horizon_days : float
        Execution horizon in trading days.
    permanent_impact_coef : float
        Permanent impact coefficient.
    temporary_impact_coef : float
        Temporary impact coefficient.
    reference_price : float, optional
        Optional price used to convert share impact to notional impact.

    Returns
    -------
    dict[str, float]
        Dict of permanent, temporary, and total impact estimates.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import almgren_chriss_impact
    >>> callable(almgren_chriss_impact)
    True
    """
    ...

def kyle_lambda(volumes: list[float], returns: list[float]) -> float | None:
    """
    Estimate Kyle's lambda from volume and return observations.

    Returns ``None`` when the aligned sample has too few valid observations or
    cannot support the regression-style estimate.

    Parameters
    ----------
    volumes : list[float]
        Ordered trading-volume observations in consistent notional or share units.
    returns : list[float]
        Ordered simple decimal returns aligned one-for-one with ``volumes``.

    Returns
    -------
    float | None
        Result of kyle lambda for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import kyle_lambda
    >>> callable(kyle_lambda)
    True
    """
    ...

def brinson_fachler(sectors_json: str) -> str:
    """
    Compute single-period Brinson-Fachler attribution from sector JSON.

    Parameters
    ----------
    sectors_json : str
        JSON array of ``SectorPeriod`` objects with ``sector``,
        ``portfolio_weight``, ``benchmark_weight``, ``portfolio_return``, and
        ``benchmark_return`` fields. Returns are simple decimal returns for the
        period (e.g. ``0.02`` for +2%).

    Returns
    -------
    str
        JSON-serialized ``BrinsonPeriodResult`` with allocation, selection, and
        interaction effects plus total active return.

    Raises
    ------
    PortfolioError
        If sector weights do not sum to one or returns are invalid.
    ValueError
        If ``sectors_json`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#brinson-fachler-1985``.

    Examples
    --------
    >>> from finstack_quant.portfolio import brinson_fachler
    >>> result_json = brinson_fachler(sectors_json)  # doctest: +SKIP
    """
    ...

def carino_link(periods_json: str) -> str:
    """
    Compute Carino-linked multi-period Brinson attribution from period JSON.

    Parameters
    ----------
    periods_json : str
        JSON array of periods, where each period is an array of ``SectorPeriod``
        objects (same schema as :func:`brinson_fachler`).

    Returns
    -------
    str
        JSON-serialized ``CarinoLinkedAttribution`` with linked allocation,
        selection, and interaction effects across periods.

    Raises
    ------
    PortfolioError
        If any period fails Brinson validation.
    ValueError
        If ``periods_json`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#carino-1999``.

    Examples
    --------
    >>> from finstack_quant.portfolio import carino_link
    >>> linked_json = carino_link(periods_json)  # doctest: +SKIP
    """
    ...

def twrr_modified_dietz(period_json: str) -> float | None:
    """
    Compute a Modified-Dietz TWRR sub-period return from period JSON.

    Parameters
    ----------
    period_json : str
        JSON-encoded ``TwrrPeriod`` with beginning market value, external
        cashflows, and ending market value for the sub-period.

    Returns
    -------
    float or None
        Sub-period time-weighted return as a decimal, or ``None`` when the
        period cannot be computed (e.g. zero denominator).

    Raises
    ------
    ValueError
        If ``period_json`` is malformed.

    Examples
    --------
    >>> from finstack_quant.portfolio import twrr_modified_dietz
    >>> r = twrr_modified_dietz(period_json)  # doctest: +SKIP
    """
    ...

def twrr_linked(returns_json: str, horizon_years: float) -> str | None:
    """
    Geometrically link TWRR sub-period returns over a horizon.

    Parameters
    ----------
    returns_json : str
        JSON array of sub-period decimal returns (e.g. from Modified Dietz).
    horizon_years : float
        Reporting horizon in years used to annualize the linked return.

    Returns
    -------
    str or None
        JSON-encoded linked return result, or ``None`` when linking fails (e.g.
        empty return series).

    Raises
    ------
    ValueError
        If ``returns_json`` is malformed.

    Examples
    --------
    >>> from finstack_quant.portfolio import twrr_linked
    >>> linked = twrr_linked(returns_json, horizon_years=1.0)  # doctest: +SKIP
    """
    ...

def mwr_xirr(cashflows_json: str) -> float:
    """
    Compute money-weighted return via XIRR from dated cashflow JSON.

    Parameters
    ----------
    cashflows_json : str
        JSON array of ``DatedCashflow`` objects with ISO dates and signed amounts
        (investments negative, distributions positive).

    Returns
    -------
    float
        Internal rate of return as a decimal annualized rate.

    Raises
    ------
    PortfolioError
        If XIRR does not converge or cashflows lack a sign change.
    ValueError
        If ``cashflows_json`` is malformed.

    Examples
    --------
    >>> from finstack_quant.portfolio import mwr_xirr
    >>> irr = mwr_xirr(cashflows_json)  # doctest: +SKIP
    """
    ...

# ---------------------------------------------------------------------------
# factor_model typed result classes (Slice 8)
# ---------------------------------------------------------------------------

class FactorContribution:
    """
    Aggregate contribution of a single factor to portfolio risk.

    Examples
    --------
    >>> from finstack_quant.portfolio import FactorContribution
    >>> FactorContribution.__name__
    'FactorContribution'
    """

    @classmethod
    def from_json(cls, json_str: str) -> FactorContribution:
        """
        Deserialize a factor contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized factor contribution, normally produced by
            ``FactorContribution.to_json``.

        Returns
        -------
        FactorContribution
            Validated `FactorContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorContribution
        >>> callable(FactorContribution.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this factor contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `FactorContribution`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def factor_id(self) -> str:
        """
        Factor identifier.
        Returns
        -------
        str
            The factor id exposed by this `FactorContribution`.
        """
        ...

    @property
    def absolute_risk(self) -> float:
        """
        Absolute risk contribution.
        Returns
        -------
        float
            The absolute risk exposed by this `FactorContribution`.
        """
        ...

    @property
    def relative_risk(self) -> float:
        """
        Share of total portfolio risk.
        Returns
        -------
        float
            The relative risk exposed by this `FactorContribution`.
        """
        ...

    @property
    def marginal_risk(self) -> float:
        """
        Marginal risk contribution.
        Returns
        -------
        float
            The marginal risk exposed by this `FactorContribution`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionFactorContribution:
    """
    Per-position contribution to a specific factor bucket.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionFactorContribution
    >>> PositionFactorContribution.__name__
    'PositionFactorContribution'
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionFactorContribution:
        """
        Deserialize a position-factor contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized position-factor contribution, normally
            produced by ``PositionFactorContribution.to_json``.

        Returns
        -------
        PositionFactorContribution
            Validated `PositionFactorContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFactorContribution
        >>> callable(PositionFactorContribution.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position-factor contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionFactorContribution`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `PositionFactorContribution`.
        """
        ...

    @property
    def factor_id(self) -> str:
        """
        Factor identifier.
        Returns
        -------
        str
            The factor id exposed by this `PositionFactorContribution`.
        """
        ...

    @property
    def risk_contribution(self) -> float:
        """
        Risk contribution for this position-factor pair.
        Returns
        -------
        float
            The risk contribution exposed by this `PositionFactorContribution`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionResidualContribution:
    """
    Annualized residual variance contributed by a single position.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionResidualContribution
    >>> PositionResidualContribution.__name__
    'PositionResidualContribution'
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionResidualContribution:
        """
        Deserialize a residual contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized residual contribution, normally produced by
            ``PositionResidualContribution.to_json``.

        Returns
        -------
        PositionResidualContribution
            Validated `PositionResidualContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionResidualContribution
        >>> callable(PositionResidualContribution.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this residual contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionResidualContribution`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `PositionResidualContribution`.
        """
        ...

    @property
    def residual_variance(self) -> float:
        """
        Residual variance assigned to this position.
        Returns
        -------
        float
            The residual variance exposed by this `PositionResidualContribution`.
        """
        ...

    @property
    def source_kind(self) -> str:
        """
        Source category used to derive residual risk.
        Returns
        -------
        str
            The source kind exposed by this `PositionResidualContribution`.
        """
        ...

    @property
    def source_issuer_id(self) -> str | None:
        """
        Issuer identifier for issuer-sourced residual risk, if present.
        Returns
        -------
        str or None
            The source issuer id exposed by this `PositionResidualContribution`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class RiskDecomposition:
    """
    Portfolio-level risk decomposition across factors and residuals.

    Examples
    --------
    >>> from finstack_quant.portfolio import RiskDecomposition
    >>> RiskDecomposition.__name__
    'RiskDecomposition'
    """

    @classmethod
    def from_json(cls, json_str: str) -> RiskDecomposition:
        """
        Deserialize a risk decomposition from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized factor-and-residual decomposition, normally
            produced by ``RiskDecomposition.to_json``.

        Returns
        -------
        RiskDecomposition
            Validated `RiskDecomposition` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import RiskDecomposition
        >>> callable(RiskDecomposition.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this risk decomposition to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `RiskDecomposition`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def total_risk(self) -> float:
        """
        Total portfolio risk under the decomposition measure.
        Returns
        -------
        float
            The total risk exposed by this `RiskDecomposition`.
        """
        ...

    @property
    def measure_json(self) -> str:
        """
        Risk measure specification as JSON.
        Returns
        -------
        str
            The measure json exposed by this `RiskDecomposition`.
        """
        ...

    @property
    def residual_risk(self) -> float:
        """
        Residual risk not explained by factor contributions.
        Returns
        -------
        float
            The residual risk exposed by this `RiskDecomposition`.
        """
        ...

    @property
    def factor_contributions(self) -> list[FactorContribution]:
        """
        Factor-level risk contributions.
        Returns
        -------
        list[FactorContribution]
        """
        ...

    @property
    def position_factor_contributions(self) -> list[PositionFactorContribution]:
        """
        Position-by-factor risk contributions.
        Returns
        -------
        list[PositionFactorContribution]
        """
        ...

    @property
    def position_residual_contributions(self) -> list[PositionResidualContribution]:
        """
        Per-position residual risk contributions.
        Returns
        -------
        list[PositionResidualContribution]
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionVarContribution:
    """
    Per-position component / marginal VaR.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionVarContribution
    >>> PositionVarContribution.__name__
    'PositionVarContribution'
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionVarContribution:
        """
        Deserialize a position VaR contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized component and marginal VaR contribution,
            normally produced by ``PositionVarContribution.to_json``.

        Returns
        -------
        PositionVarContribution
            Validated `PositionVarContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionVarContribution
        >>> callable(PositionVarContribution.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position VaR contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionVarContribution`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `PositionVarContribution`.
        """
        ...

    @property
    def component_var(self) -> float:
        """
        Component VaR assigned to this position.
        Returns
        -------
        float
            The component var exposed by this `PositionVarContribution`.
        """
        ...

    @property
    def relative_var(self) -> float:
        """
        Share of total portfolio VaR.
        Returns
        -------
        float
            The relative var exposed by this `PositionVarContribution`.
        """
        ...

    @property
    def marginal_var(self) -> float | None:
        """
        Marginal VaR, if computed.
        Returns
        -------
        float or None
            The marginal var exposed by this `PositionVarContribution`.
        """
        ...

    @property
    def incremental_var(self) -> float | None:
        """
        Incremental VaR, if requested in the decomposition config.
        Returns
        -------
        float or None
            The incremental var exposed by this `PositionVarContribution`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionEsContribution:
    """
    Per-position component / marginal ES.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionEsContribution
    >>> PositionEsContribution.__name__
    'PositionEsContribution'
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionEsContribution:
        """
        Deserialize a position ES contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized component and marginal expected-shortfall
            contribution, normally produced by ``PositionEsContribution.to_json``.

        Returns
        -------
        PositionEsContribution
            Validated `PositionEsContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionEsContribution
        >>> callable(PositionEsContribution.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position ES contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionEsContribution`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `PositionEsContribution`.
        """
        ...

    @property
    def component_es(self) -> float:
        """
        Component expected shortfall assigned to this position.
        Returns
        -------
        float
            The component es exposed by this `PositionEsContribution`.
        """
        ...

    @property
    def relative_es(self) -> float:
        """
        Share of total portfolio expected shortfall.
        Returns
        -------
        float
            The relative es exposed by this `PositionEsContribution`.
        """
        ...

    @property
    def marginal_es(self) -> float | None:
        """
        Marginal expected shortfall, if computed.
        Returns
        -------
        float or None
            The marginal es exposed by this `PositionEsContribution`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionRiskDecomposition:
    """
    Complete position-level risk decomposition.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionRiskDecomposition
    >>> PositionRiskDecomposition.__name__
    'PositionRiskDecomposition'
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionRiskDecomposition:
        """
        Deserialize a position risk decomposition from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized VaR/ES decomposition, normally produced by
            ``PositionRiskDecomposition.to_json``.

        Returns
        -------
        PositionRiskDecomposition
            Validated `PositionRiskDecomposition` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionRiskDecomposition
        >>> callable(PositionRiskDecomposition.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position risk decomposition to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionRiskDecomposition`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def portfolio_var(self) -> float:
        """
        Return the portfolio var for `PositionRiskDecomposition`.
        Portfolio VaR.
        Returns
        -------
        float
            The portfolio var exposed by this `PositionRiskDecomposition`.
        """
        ...

    @property
    def portfolio_es(self) -> float:
        """
        Portfolio expected shortfall.
        Returns
        -------
        float
            The portfolio es exposed by this `PositionRiskDecomposition`.
        """
        ...

    @property
    def confidence(self) -> float:
        """
        Confidence level used for VaR/ES.
        Returns
        -------
        float
            The confidence exposed by this `PositionRiskDecomposition`.
        """
        ...

    @property
    def n_positions(self) -> int:
        """
        Number of positions included in the decomposition.
        Returns
        -------
        int
            The n positions exposed by this `PositionRiskDecomposition`.
        """
        ...

    @property
    def method(self) -> str:
        """
        Decomposition method label.
        Returns
        -------
        str
            The method exposed by this `PositionRiskDecomposition`.
        """
        ...

    @property
    def euler_residual(self) -> float | None:
        """
        Euler allocation residual, if reported.
        Returns
        -------
        float or None
            The euler residual exposed by this `PositionRiskDecomposition`.
        """
        ...

    @property
    def var_contributions(self) -> list[PositionVarContribution]:
        """
        Per-position VaR contributions.
        Returns
        -------
        list[PositionVarContribution]
        """
        ...

    @property
    def es_contributions(self) -> list[PositionEsContribution]:
        """
        Per-position expected shortfall contributions.
        Returns
        -------
        list[PositionEsContribution]
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionBudgetEntry:
    """
    Per-position budget comparison entry.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionBudgetEntry
    >>> PositionBudgetEntry.__name__
    'PositionBudgetEntry'
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionBudgetEntry:
        """
        Deserialize a risk-budget entry from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized per-position budget comparison, normally
            produced by ``PositionBudgetEntry.to_json``.

        Returns
        -------
        PositionBudgetEntry
            Validated `PositionBudgetEntry` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionBudgetEntry
        >>> callable(PositionBudgetEntry.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this risk-budget entry to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionBudgetEntry`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `PositionBudgetEntry`.
        """
        ...

    @property
    def actual_component_var(self) -> float:
        """
        Actual component VaR for this position.
        Returns
        -------
        float
            The actual component var exposed by this `PositionBudgetEntry`.
        """
        ...

    @property
    def target_component_var(self) -> float:
        """
        Target component VaR for this position.
        Returns
        -------
        float
            The target component var exposed by this `PositionBudgetEntry`.
        """
        ...

    @property
    def utilization(self) -> float:
        """
        Actual-to-target utilization ratio.
        Returns
        -------
        float
            The utilization exposed by this `PositionBudgetEntry`.
        """
        ...

    @property
    def excess(self) -> float:
        """
        Actual component VaR less target component VaR.
        Returns
        -------
        float
            The excess exposed by this `PositionBudgetEntry`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class RiskBudgetResult:
    """
    Budget evaluation result across positions.

    Examples
    --------
    >>> from finstack_quant.portfolio import RiskBudgetResult
    >>> RiskBudgetResult.__name__
    'RiskBudgetResult'
    """

    @classmethod
    def from_json(cls, json_str: str) -> RiskBudgetResult:
        """
        Deserialize a risk-budget result from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized portfolio risk-budget result, normally
            produced by ``RiskBudgetResult.to_json``.

        Returns
        -------
        RiskBudgetResult
            Validated `RiskBudgetResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import RiskBudgetResult
        >>> callable(RiskBudgetResult.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this risk-budget result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `RiskBudgetResult`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def total_overbudget(self) -> float:
        """
        Total amount above target risk budgets.
        Returns
        -------
        float
            The total overbudget exposed by this `RiskBudgetResult`.
        """
        ...

    @property
    def has_breach(self) -> bool:
        """
        Whether any position exceeds the utilization threshold.
        Returns
        -------
        bool
            Whether this `RiskBudgetResult` has breach.
        """
        ...

    @property
    def positions(self) -> list[PositionBudgetEntry]:
        """
        Per-position risk-budget entries.
        Returns
        -------
        list[PositionBudgetEntry]
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class FactorContributionDelta:
    """
    Per-factor contribution change between a baseline and a scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import FactorContributionDelta
    >>> FactorContributionDelta.__name__
    'FactorContributionDelta'
    """

    @classmethod
    def from_json(cls, json_str: str) -> FactorContributionDelta:
        """
        Deserialize a factor contribution delta from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized baseline-to-scenario factor delta, normally
            produced by ``FactorContributionDelta.to_json``.

        Returns
        -------
        FactorContributionDelta
            Validated `FactorContributionDelta` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorContributionDelta
        >>> callable(FactorContributionDelta.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this factor contribution delta to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `FactorContributionDelta`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def factor_id(self) -> str:
        """
        Factor identifier.
        Returns
        -------
        str
            The factor id exposed by this `FactorContributionDelta`.
        """
        ...

    @property
    def absolute_change(self) -> float:
        """
        Absolute contribution change.
        Returns
        -------
        float
            The absolute change exposed by this `FactorContributionDelta`.
        """
        ...

    @property
    def relative_change(self) -> float:
        """
        Relative contribution change.
        Returns
        -------
        float
            The relative change exposed by this `FactorContributionDelta`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class WhatIfResult:
    """
    Result of a position what-if scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import WhatIfResult
    >>> WhatIfResult.__name__
    'WhatIfResult'
    """

    @classmethod
    def from_json(cls, json_str: str) -> WhatIfResult:
        """
        Deserialize a what-if result from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized before-and-after risk result, normally
            produced by ``WhatIfResult.to_json``.

        Returns
        -------
        WhatIfResult
            Validated `WhatIfResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import WhatIfResult
        >>> callable(WhatIfResult.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this what-if result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `WhatIfResult`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def before(self) -> RiskDecomposition:
        """
        Baseline risk decomposition.
        Returns
        -------
        RiskDecomposition
        """
        ...

    @property
    def after(self) -> RiskDecomposition:
        """
        Post-scenario risk decomposition.
        Returns
        -------
        RiskDecomposition
        """
        ...

    @property
    def delta(self) -> list[FactorContributionDelta]:
        """
        Per-factor contribution changes.
        Returns
        -------
        list[FactorContributionDelta]
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class StressResult:
    """
    Result of a factor-stress scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import StressResult
    >>> StressResult.__name__
    'StressResult'
    """

    @classmethod
    def from_json(cls, json_str: str) -> StressResult:
        """
        Deserialize a stress result from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized stressed P&L and decomposition result, normally
            produced by ``StressResult.to_json``.

        Returns
        -------
        StressResult
            Validated `StressResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import StressResult
        >>> callable(StressResult.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `StressResult`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def total_pnl(self) -> float:
        """
        Total portfolio P&L under the stress scenario.
        Returns
        -------
        float
            The total pnl exposed by this `StressResult`.
        """
        ...

    @property
    def position_pnl(self) -> list[tuple[str, float]]:
        """
        Per-position P&L pairs.
        Returns
        -------
        list[tuple[str, float]]
        """
        ...

    @property
    def stressed_decomposition(self) -> RiskDecomposition:
        """
        Risk decomposition after applying the stress.
        Returns
        -------
        RiskDecomposition
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class StressPositionEntry:
    """
    Single position's contribution to tail stress.

    Examples
    --------
    >>> from finstack_quant.portfolio import StressPositionEntry
    >>> StressPositionEntry.__name__
    'StressPositionEntry'
    """

    @classmethod
    def from_json(cls, json_str: str) -> StressPositionEntry:
        """
        Deserialize a stress position entry from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized per-position tail-stress contribution, normally
            produced by ``StressPositionEntry.to_json``.

        Returns
        -------
        StressPositionEntry
            Validated `StressPositionEntry` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import StressPositionEntry
        >>> callable(StressPositionEntry.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress position entry to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `StressPositionEntry`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `StressPositionEntry`.
        """
        ...

    @property
    def avg_tail_pnl(self) -> float:
        """
        Average P&L across tail scenarios.
        Returns
        -------
        float
            The avg tail pnl exposed by this `StressPositionEntry`.
        """
        ...

    @property
    def pct_of_tail_loss(self) -> float:
        """
        Share of aggregate tail loss.
        Returns
        -------
        float
            The pct of tail loss exposed by this `StressPositionEntry`.
        """
        ...

    @property
    def worst_scenario_pnl(self) -> float:
        """
        Worst single-scenario P&L for this position.
        Returns
        -------
        float
            The worst scenario pnl exposed by this `StressPositionEntry`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TailScenarioBreakdown:
    """
    Breakdown of a single tail scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import TailScenarioBreakdown
    >>> TailScenarioBreakdown.__name__
    'TailScenarioBreakdown'
    """

    @classmethod
    def from_json(cls, json_str: str) -> TailScenarioBreakdown:
        """
        Deserialize a tail scenario breakdown from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized tail-scenario P&L breakdown, normally
            produced by ``TailScenarioBreakdown.to_json``.

        Returns
        -------
        TailScenarioBreakdown
            Validated `TailScenarioBreakdown` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import TailScenarioBreakdown
        >>> callable(TailScenarioBreakdown.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this tail scenario breakdown to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `TailScenarioBreakdown`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def scenario_index(self) -> int:
        """
        Scenario index in the source P&L matrix.
        Returns
        -------
        int
            The scenario index exposed by this `TailScenarioBreakdown`.
        """
        ...

    @property
    def portfolio_pnl(self) -> float:
        """
        Portfolio P&L for this tail scenario.
        Returns
        -------
        float
            The portfolio pnl exposed by this `TailScenarioBreakdown`.
        """
        ...

    @property
    def position_pnls(self) -> list[float]:
        """
        Per-position P&L for this scenario, index-aligned to
        ``StressAttribution.position_ids`` (entry ``i`` is the P&L for
        ``position_ids[i]``).
        Returns
        -------
        list[float]
            The position pnls exposed by this `TailScenarioBreakdown`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class StressAttribution:
    """
    Per-position attribution of portfolio losses in tail scenarios.

    Examples
    --------
    >>> from finstack_quant.portfolio import StressAttribution
    >>> StressAttribution.__name__
    'StressAttribution'
    """

    @classmethod
    def from_json(cls, json_str: str) -> StressAttribution:
        """
        Deserialize stress attribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized tail-loss attribution, normally produced by
            ``StressAttribution.to_json``.

        Returns
        -------
        StressAttribution
            Validated `StressAttribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import StressAttribution
        >>> callable(StressAttribution.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress attribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `StressAttribution`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def var_threshold(self) -> float:
        """
        VaR threshold used to select tail scenarios.
        Returns
        -------
        float
            The var threshold exposed by this `StressAttribution`.
        """
        ...

    @property
    def n_tail_scenarios(self) -> int:
        """
        Number of scenarios classified as tail scenarios.
        Returns
        -------
        int
            The n tail scenarios exposed by this `StressAttribution`.
        """
        ...

    @property
    def position_ids(self) -> list[str]:
        """
        Canonical position ordering shared by every ``tail_scenarios`` entry.
        ``tail_scenarios[k].position_pnls[i]`` is the P&L for ``position_ids[i]``.
        Returns
        -------
        list[str]
            The position ids exposed by this `StressAttribution`.
        """
        ...

    @property
    def position_contributions(self) -> list[StressPositionEntry]:
        """
        Per-position tail-loss contributions.
        Returns
        -------
        list[StressPositionEntry]
        """
        ...

    @property
    def tail_scenarios(self) -> list[TailScenarioBreakdown]:
        """
        Detailed tail scenario breakdowns.
        Returns
        -------
        list[TailScenarioBreakdown]
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionAssignment:
    """
    Matched factor assignments for a single portfolio position.

    The full ``(dependency, factor_id)`` pairs are available as JSON via
    :meth:`mappings_json`; matched factor identifiers are accessible directly
    via the :attr:`factor_ids` property.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionAssignment
    >>> PositionAssignment.__name__
    'PositionAssignment'
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionAssignment:
        """
        Deserialize a position assignment from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized factor assignment for one position, normally
            produced by ``PositionAssignment.to_json``.

        Returns
        -------
        PositionAssignment
            Validated `PositionAssignment` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionAssignment
        >>> callable(PositionAssignment.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position assignment to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionAssignment`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `PositionAssignment`.
        """
        ...

    @property
    def n_mappings(self) -> int:
        """
        Number of dependency-to-factor mappings.
        Returns
        -------
        int
            The n mappings exposed by this `PositionAssignment`.
        """
        ...

    def mappings_json(self) -> str:
        """
        Return detailed dependency-to-factor mappings as JSON.
        Returns
        -------
        str
            Result of mappings json for this `PositionAssignment` in the annotated representation.
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """
        Matched factor identifiers.
        Returns
        -------
        list[str]
            The factor ids exposed by this `PositionAssignment`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class UnmatchedEntry:
    """
    Single unmatched dependency surfaced during assignment.

    Examples
    --------
    >>> from finstack_quant.portfolio import UnmatchedEntry
    >>> UnmatchedEntry.__name__
    'UnmatchedEntry'
    """

    @classmethod
    def from_json(cls, json_str: str) -> UnmatchedEntry:
        """
        Deserialize an unmatched dependency entry from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized unmatched-factor diagnostic, normally produced
            by ``UnmatchedEntry.to_json``.

        Returns
        -------
        UnmatchedEntry
            Validated `UnmatchedEntry` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import UnmatchedEntry
        >>> callable(UnmatchedEntry.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this unmatched entry to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `UnmatchedEntry`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `UnmatchedEntry`.
        """
        ...

    def dependency_json(self) -> str:
        """
        Return the unmatched dependency payload as JSON.
        Returns
        -------
        str
            Result of dependency json for this `UnmatchedEntry` in the annotated representation.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class FactorAssignmentReport:
    """
    Assignment results for a portfolio-level factor mapping pass.

    Examples
    --------
    >>> from finstack_quant.portfolio import FactorAssignmentReport
    >>> FactorAssignmentReport.__name__
    'FactorAssignmentReport'
    """

    @classmethod
    def from_json(cls, json_str: str) -> FactorAssignmentReport:
        """
        Deserialize a factor assignment report from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized factor-model assignment report, normally
            produced by ``FactorAssignmentReport.to_json``.

        Returns
        -------
        FactorAssignmentReport
            Validated `FactorAssignmentReport` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorAssignmentReport
        >>> callable(FactorAssignmentReport.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this factor assignment report to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `FactorAssignmentReport`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def assignments(self) -> list[PositionAssignment]:
        """
        Matched assignments by position.
        Returns
        -------
        list[PositionAssignment]
        """
        ...

    @property
    def unmatched(self) -> list[UnmatchedEntry]:
        """
        Dependencies that could not be mapped to factors.
        Returns
        -------
        list[UnmatchedEntry]
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class LevelVolContribution:
    """
    Aggregated risk contribution for a single hierarchy level.

    Examples
    --------
    >>> from finstack_quant.portfolio import LevelVolContribution
    >>> LevelVolContribution.__name__
    'LevelVolContribution'
    """

    @property
    def level_name(self) -> str:
        """
        Hierarchy level name.
        Returns
        -------
        str
            The level name exposed by this `LevelVolContribution`.
        """
        ...

    @property
    def total(self) -> float:
        """
        Total contribution for this level.
        Returns
        -------
        float
            The total exposed by this `LevelVolContribution`.
        """
        ...

    @property
    def by_bucket(self) -> dict[str, float]:
        """
        Contribution by hierarchy bucket.
        Returns
        -------
        dict[str, float]
            The by bucket exposed by this `LevelVolContribution`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionVolContribution:
    """
    Per-position vol breakdown under :class:`CreditVolReport`.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionVolContribution
    >>> PositionVolContribution.__name__
    'PositionVolContribution'
    """

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `PositionVolContribution`.
        """
        ...

    @property
    def factor_total(self) -> float:
        """
        Factor-driven volatility contribution.
        Returns
        -------
        float
            The factor total exposed by this `PositionVolContribution`.
        """
        ...

    @property
    def idiosyncratic(self) -> float:
        """
        Idiosyncratic volatility contribution.
        Returns
        -------
        float
            The idiosyncratic exposed by this `PositionVolContribution`.
        """
        ...

    @property
    def total(self) -> float:
        """
        Total position volatility contribution.
        Returns
        -------
        float
            The total exposed by this `PositionVolContribution`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class CreditVolReport:
    """
    Aggregated vol report grouped by hierarchy level.

    Examples
    --------
    >>> from finstack_quant.portfolio import CreditVolReport
    >>> CreditVolReport.__name__
    'CreditVolReport'
    """

    @property
    def total(self) -> float:
        """
        Total portfolio volatility under the report measure.
        Returns
        -------
        float
            The total exposed by this `CreditVolReport`.
        """
        ...

    @property
    def measure_json(self) -> str:
        """
        Risk measure specification as JSON.
        Returns
        -------
        str
            The measure json exposed by this `CreditVolReport`.
        """
        ...

    @property
    def generic(self) -> float:
        """
        Generic factor contribution.
        Returns
        -------
        float
            The generic exposed by this `CreditVolReport`.
        """
        ...

    @property
    def idiosyncratic_total(self) -> float:
        """
        Aggregate idiosyncratic contribution.
        Returns
        -------
        float
            The idiosyncratic total exposed by this `CreditVolReport`.
        """
        ...

    @property
    def by_level(self) -> list[LevelVolContribution]:
        """
        Volatility contribution by hierarchy level.
        Returns
        -------
        list[LevelVolContribution]
        """
        ...

    @property
    def by_position(self) -> list[PositionVolContribution] | None:
        """
        Optional per-position volatility contributions.
        Returns
        -------
        list[PositionVolContribution] or None
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class VolHorizon:
    """
    Forecast horizon used to scale a calibrated `Sample` vol estimate.

    Examples
    --------
    >>> from finstack_quant.portfolio import VolHorizon
    >>> VolHorizon.__name__
    'VolHorizon'
    """

    @classmethod
    def one_step(cls) -> VolHorizon:
        """
        Use the calibrated one-step forecast horizon.

        Returns
        -------
        VolHorizon
            Result of one step for this `VolHorizon` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import VolHorizon
        >>> callable(VolHorizon.one_step)
        True
        """
        ...

    @classmethod
    def unconditional(cls) -> VolHorizon:
        """
        Use the unconditional long-run forecast horizon.

        Returns
        -------
        VolHorizon
            Result of unconditional for this `VolHorizon` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import VolHorizon
        >>> callable(VolHorizon.unconditional)
        True
        """
        ...

    @classmethod
    def n_steps(cls, n: int) -> VolHorizon:
        """
        Scale the forecast to a fixed number of discrete steps.

        Parameters
        ----------
        n : int
            Positive number of calibrated sampling periods to forecast ahead.

        Returns
        -------
        VolHorizon
            Result of n steps for this `VolHorizon` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import VolHorizon
        >>> callable(VolHorizon.n_steps)
        True
        """
        ...

    @classmethod
    def years(cls, years: float) -> VolHorizon:
        """
        Scale the forecast to a year fraction.

        Parameters
        ----------
        years : float
            Positive forecast horizon in years, converted using the calibrated
            model's observation frequency.

        Returns
        -------
        VolHorizon
            Result of years for this `VolHorizon` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import VolHorizon
        >>> callable(VolHorizon.years)
        True
        """
        ...

    @classmethod
    def parse(cls, s: str) -> VolHorizon:
        """
        Parse a horizon string accepted by the Rust factor model.

        Parameters
        ----------
        s : str
            Horizon expression such as ``"one_step"``, ``"unconditional"``,
            a step count, or a year-based form accepted by the model.

        Returns
        -------
        VolHorizon
            Result of parse for this `VolHorizon` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import VolHorizon
        >>> callable(VolHorizon.parse)
        True
        """
        ...

    @property
    def kind(self) -> str:
        """
        Horizon variant label.
        Returns
        -------
        str
            The kind exposed by this `VolHorizon`.
        """
        ...

    @property
    def n(self) -> int | None:
        """
        Step count for ``n_steps`` horizons.
        Returns
        -------
        int or None
            The n exposed by this `VolHorizon`.
        """
        ...

    @property
    def years_value(self) -> float | None:
        """
        Year fraction for ``years`` horizons.
        Returns
        -------
        float or None
            The years value exposed by this `VolHorizon`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class DecompositionConfig:
    """
    Configuration for position-level VaR decomposition.

    Examples
    --------
    >>> from finstack_quant.portfolio import DecompositionConfig
    >>> DecompositionConfig.__name__
    'DecompositionConfig'
    """

    @classmethod
    def parametric_95(cls) -> DecompositionConfig:
        """
        Default 95% parametric VaR decomposition config.

        Returns
        -------
        DecompositionConfig
            Result of parametric 95 for this `DecompositionConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import DecompositionConfig
        >>> callable(DecompositionConfig.parametric_95)
        True
        """
        ...

    @classmethod
    def parametric_99(cls) -> DecompositionConfig:
        """
        Default 99% parametric VaR decomposition config.

        Returns
        -------
        DecompositionConfig
            Result of parametric 99 for this `DecompositionConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import DecompositionConfig
        >>> callable(DecompositionConfig.parametric_99)
        True
        """
        ...

    @classmethod
    def historical(cls, confidence: float) -> DecompositionConfig:
        """
        Build a historical VaR decomposition configuration.

        Parameters
        ----------
        confidence : float
            VaR confidence as a decimal probability in ``(0, 1)``, such as
            ``0.95`` for a 95% confidence level.

        Returns
        -------
        DecompositionConfig
            Result of historical for this `DecompositionConfig` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import DecompositionConfig
        >>> callable(DecompositionConfig.historical)
        True
        """
        ...

    def with_incremental(self) -> DecompositionConfig:
        """
        Return a copy that requests incremental VaR.
        Returns
        -------
        DecompositionConfig
        """
        ...

    def with_seed(self, seed: int) -> DecompositionConfig:
        """
        Return a copy with a deterministic simulation seed.

        Parameters
        ----------
        seed : int
            Integer seed used to reproduce any randomized decomposition steps.

        Returns
        -------
        DecompositionConfig
            Result of with seed for this `DecompositionConfig` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def confidence(self) -> float:
        """
        VaR/ES confidence level.
        Returns
        -------
        float
            The confidence exposed by this `DecompositionConfig`.
        """
        ...

    @property
    def method(self) -> str:
        """
        Decomposition method label.
        Returns
        -------
        str
            The method exposed by this `DecompositionConfig`.
        """
        ...

    @property
    def compute_incremental(self) -> bool:
        """
        Whether incremental VaR is requested.
        Returns
        -------
        bool
            The compute incremental exposed by this `DecompositionConfig`.
        """
        ...

    @property
    def seed(self) -> int | None:
        """
        Optional deterministic seed.
        Returns
        -------
        int or None
            The seed exposed by this `DecompositionConfig`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def parametric_var_decomposition_typed(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]] | npt.NDArray[np.float64],
    confidence: float = 0.95,
    compute_incremental: bool = False,
) -> PositionRiskDecomposition:
    """
    Return a typed parametric VaR decomposition.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers aligned with weights and covariance rows/columns.
    weights : list[float]
        Decimal portfolio weights aligned one-for-one with ``position_ids``.
    covariance : list[list[float]] or numpy.ndarray
        Square covariance matrix aligned to ``position_ids`` in row and column
        order, using returns at the selected risk horizon. C-contiguous
        ``float64`` arrays use the direct buffer path.
    confidence : float
        VaR confidence as a decimal probability; defaults to ``0.95``.
    compute_incremental : bool
        Whether to include incremental VaR estimates for each position.

    Returns
    -------
    PositionRiskDecomposition
        Result of parametric var decomposition typed for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import parametric_var_decomposition_typed
    >>> callable(parametric_var_decomposition_typed)
    True
    """
    ...

def historical_var_decomposition_typed(
    position_ids: list[str],
    position_pnls: list[list[float]] | npt.NDArray[np.float64],
    confidence: float = 0.95,
) -> PositionRiskDecomposition:
    """
    Return a typed historical VaR decomposition.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers aligned with the P&L matrix columns.
    position_pnls : list[list[float]] or numpy.ndarray
        Position-major matrix shaped ``len(position_ids) x n_scenarios``.
        C-contiguous ``float64`` arrays use the direct buffer path.
    confidence : float
        VaR confidence as a decimal probability; defaults to ``0.95``.

    Returns
    -------
    PositionRiskDecomposition
        Result of historical var decomposition typed for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import historical_var_decomposition_typed
    >>> callable(historical_var_decomposition_typed)
    True
    """
    ...

def evaluate_risk_budget_typed(
    position_ids: list[str],
    actual_var: list[float],
    target_var_pct: list[float],
    portfolio_var: float,
    utilization_threshold: float = 1.20,
) -> RiskBudgetResult:
    """
    Return a typed comparison of actual and target risk budgets.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers aligned with all per-position risk vectors.
    actual_var : list[float]
        Actual component VaR amounts aligned with ``position_ids``.
    target_var_pct : list[float]
        Target decimal shares of total portfolio VaR for each position.
    portfolio_var : float
        Total portfolio VaR used to convert target shares into amounts.
    utilization_threshold : float
        Actual-to-target ratio that flags a budget breach; defaults to ``1.20``.

    Returns
    -------
    RiskBudgetResult
        Result of evaluate risk budget typed for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import evaluate_risk_budget_typed
    >>> callable(evaluate_risk_budget_typed)
    True
    """
    ...

def factor_stress(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    factor_model_config_json: str,
    as_of: str,
    stresses: list[tuple[str, float]],
) -> StressResult:
    """
    Run a factor-stress scenario and revalue the portfolio.

    Builds the Rust factor model from ``factor_model_config_json``, analyzes
    the base portfolio, computes sensitivities, applies the requested factor
    shifts, and returns the stressed result.

    Parameters
    ----------
    portfolio : Portfolio or str
        Portfolio instance or JSON portfolio specification accepted
        by the compiled portfolio extractor.
    market : MarketContext or str
        MarketContext instance or JSON market context accepted by the
        compiled market extractor.
    factor_model_config_json : str
        JSON-encoded ``finstack_quant_factor_model::FactorModelConfig``.
    as_of : str
        ISO calculation date, ``YYYY-MM-DD``.
    stresses : list[tuple[str, float]]
        ``(factor_id, shift)`` pairs. Factor IDs must match the
        configured model; shifts use the Rust factor model's units for that
        factor.

    Returns
    -------
    StressResult
        StressResult containing base/stressed portfolio risk and per-factor
        deltas.

    Raises
    ------
    ValueError
        If JSON parsing, date parsing, model construction, market
        lookup, or portfolio valuation fails.
    TypeError
        If ``portfolio`` or ``market`` cannot be converted to the
        expected Rust types.

    Examples
    --------
    >>> from finstack_quant.portfolio import factor_stress
    >>> callable(factor_stress)
    True
    """
    ...

def position_what_if(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    factor_model_config_json: str,
    as_of: str,
    changes: list[dict[str, Any]],
) -> WhatIfResult:
    """
    Run position remove/resize what-if analysis.

    The Python binding accepts JSON-like dictionaries for remove and resize
    changes, then delegates the sensitivity reallocation and result generation
    to Rust.

    Parameters
    ----------
    portfolio : Portfolio or str
        Portfolio instance or JSON portfolio specification accepted
        by the compiled portfolio extractor.
    market : MarketContext or str
        MarketContext instance or JSON market context accepted by the
        compiled market extractor.
    factor_model_config_json : str
        JSON-encoded ``finstack_quant_factor_model::FactorModelConfig``.
    as_of : str
        ISO calculation date, ``YYYY-MM-DD``.
    changes : list[dict[str, Any]]
        List of dictionaries. Remove changes use
        ``{"kind": "remove", "position_id": "..."}``; resize changes use
        ``{"kind": "resize", "position_id": "...", "new_quantity": 123.0}``.
        Add changes are not supported by this JSON-shaped Python helper
        because adding requires a typed Rust position object.

    Returns
    -------
    WhatIfResult
        WhatIfResult with base and scenario risk decomposition deltas.

    Raises
    ------
    ValueError
        If a change kind is unknown, resize omits
        ``new_quantity``, add is requested, JSON/config parsing fails, or
        Rust factor-model evaluation fails.
    TypeError
        If ``portfolio`` or ``market`` cannot be converted to the
        expected Rust types.

    Examples
    --------
    >>> from finstack_quant.portfolio import position_what_if
    >>> callable(position_what_if)
    True
    """
    ...

def build_stress_attribution(
    position_ids: list[str],
    position_pnls: list[list[float]] | npt.NDArray[np.float64],
    confidence: float = 0.95,
) -> StressAttribution:
    """
    Build tail-scenario stress attribution from position P&Ls.

    Python input is position-major: one row per position, and each row contains
    that position's P&L across all scenarios. The binding transposes this into
    Rust's scenario-major buffer before selecting tail scenarios.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers, one per row in ``position_pnls``.
    position_pnls : list[list[float]] or numpy.ndarray
        Matrix shaped ``len(position_ids) x n_scenarios``.
        Every row must have the same number of finite scenario P&Ls.
        C-contiguous ``float64`` arrays use the direct buffer path.
    confidence : float, default 0.95
        Tail confidence level in ``(0.5, 1)``. The Rust engine
        selects ``floor((1 - confidence) * n_scenarios)`` tail scenarios.

    Returns
    -------
    StressAttribution
        StressAttribution containing VaR threshold, tail scenario count,
        per-position tail contributions, and scenario-level P&L breakdowns.

    Raises
    ------
    ValueError
        If dimensions are inconsistent, confidence is outside
        ``(0.5, 1)``, the requested tail has zero scenarios, or any P&L is
        non-finite.

    Examples
    --------
    >>> from finstack_quant.portfolio import build_stress_attribution
    >>> callable(build_stress_attribution)
    True
    """
    ...

def build_credit_vol_report(
    decomposition: RiskDecomposition,
    model: CreditFactorModel,
    by_position: bool = False,
) -> CreditVolReport:
    """
    Build a credit volatility report from decomposition outputs.

    Aggregates a Rust ``RiskDecomposition`` against the supplied credit factor
    model hierarchy into generic, level, idiosyncratic, and optional
    per-position volatility contributions.

    Parameters
    ----------
    decomposition : RiskDecomposition
        Factor risk decomposition to summarize.
    model : CreditFactorModel
        Credit factor model whose hierarchy and factor taxonomy label
        the report levels and buckets.
    by_position : bool, default False
        Include per-position contribution rows when ``True``.

    Returns
    -------
    CreditVolReport
        CreditVolReport with total, generic, level, idiosyncratic, and optional
        position-level volatility contribution fields.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import build_credit_vol_report
    >>> callable(build_credit_vol_report)
    True
    """
    ...

def position_component_var(
    decomp: PositionRiskDecomposition,
    position_id: str,
) -> float:
    """
    Look up a position's component VaR inside a decomposition.

    Parameters
    ----------
    decomp : PositionRiskDecomposition
        Typed risk decomposition containing component VaR by position.
    position_id : str
        Position identifier whose component VaR is required; absent IDs raise
        ``KeyError``.

    Returns
    -------
    float
        Result of position component var for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import position_component_var
    >>> callable(position_component_var)
    True
    """
    ...

# ---------------------------------------------------------------------------
# optimization spec/result classes (Slice 9)
# ---------------------------------------------------------------------------

class WeightingScheme:
    """
    How optimization weights are defined.

    Examples
    --------
    >>> from finstack_quant.portfolio import WeightingScheme
    >>> WeightingScheme.__name__
    'WeightingScheme'
    """

    @classmethod
    def value_weight(cls) -> WeightingScheme:
        """
        Weight positions by market value.

        Returns
        -------
        WeightingScheme
            Result of value weight for this `WeightingScheme` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import WeightingScheme
        >>> callable(WeightingScheme.value_weight)
        True
        """
        ...

    @classmethod
    def notional_weight(cls) -> WeightingScheme:
        """
        Weight positions by notional.

        Returns
        -------
        WeightingScheme
            Result of notional weight for this `WeightingScheme` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import WeightingScheme
        >>> callable(WeightingScheme.notional_weight)
        True
        """
        ...

    @classmethod
    def unit_scaling(cls) -> WeightingScheme:
        """
        Use unit scaling rather than value/notional scaling.

        Returns
        -------
        WeightingScheme
            Result of unit scaling for this `WeightingScheme` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import WeightingScheme
        >>> callable(WeightingScheme.unit_scaling)
        True
        """
        ...

    @property
    def label(self) -> str:
        """
        Rust enum label for this weighting scheme.
        Returns
        -------
        str
            The label exposed by this `WeightingScheme`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class MissingMetricPolicy:
    """
    Policy for handling positions missing required metrics.

    Examples
    --------
    >>> from finstack_quant.portfolio import MissingMetricPolicy
    >>> MissingMetricPolicy.__name__
    'MissingMetricPolicy'
    """

    @classmethod
    def zero(cls) -> MissingMetricPolicy:
        """
        Treat missing metric values as zero.

        Returns
        -------
        MissingMetricPolicy
            Result of zero for this `MissingMetricPolicy` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import MissingMetricPolicy
        >>> callable(MissingMetricPolicy.zero)
        True
        """
        ...

    @classmethod
    def exclude(cls) -> MissingMetricPolicy:
        """
        Exclude positions with missing required metrics.

        Returns
        -------
        MissingMetricPolicy
            Result of exclude for this `MissingMetricPolicy` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import MissingMetricPolicy
        >>> callable(MissingMetricPolicy.exclude)
        True
        """
        ...

    @classmethod
    def strict(cls) -> MissingMetricPolicy:
        """
        Reject optimization when required metrics are missing.

        Returns
        -------
        MissingMetricPolicy
            Result of strict for this `MissingMetricPolicy` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import MissingMetricPolicy
        >>> callable(MissingMetricPolicy.strict)
        True
        """
        ...

    @property
    def label(self) -> str:
        """
        Rust enum label for this policy.
        Returns
        -------
        str
            The label exposed by this `MissingMetricPolicy`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class Inequality:
    """
    Inequality / equality operator (`<=`, `>=`, `==`).

    Examples
    --------
    >>> from finstack_quant.portfolio import Inequality
    >>> Inequality.__name__
    'Inequality'
    """

    @classmethod
    def le(cls) -> Inequality:
        """
        Less-than-or-equal inequality.

        Returns
        -------
        Inequality
            Result of le for this `Inequality` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import Inequality
        >>> callable(Inequality.le)
        True
        """
        ...

    @classmethod
    def ge(cls) -> Inequality:
        """
        Greater-than-or-equal inequality.

        Returns
        -------
        Inequality
            Result of ge for this `Inequality` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import Inequality
        >>> callable(Inequality.ge)
        True
        """
        ...

    @classmethod
    def eq(cls) -> Inequality:
        """
        Equality constraint.

        Returns
        -------
        Inequality
            Result of eq for this `Inequality` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import Inequality
        >>> callable(Inequality.eq)
        True
        """
        ...

    @property
    def label(self) -> str:
        """
        Return the label for `Inequality`.
        Operator label.
        Returns
        -------
        str
            The label exposed by this `Inequality`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TradeDirection:
    """
    Trade direction (buy/sell/hold).

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeDirection
    >>> TradeDirection.__name__
    'TradeDirection'
    """

    @classmethod
    def buy(cls) -> TradeDirection:
        """
        Compute buy for `TradeDirection`.
        Buy direction.

        Returns
        -------
        TradeDirection
            Result of buy for this `TradeDirection` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeDirection
        >>> callable(TradeDirection.buy)
        True
        """
        ...

    @classmethod
    def sell(cls) -> TradeDirection:
        """
        Compute sell for `TradeDirection`.
        Sell direction.

        Returns
        -------
        TradeDirection
            Result of sell for this `TradeDirection` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeDirection
        >>> callable(TradeDirection.sell)
        True
        """
        ...

    @classmethod
    def hold(cls) -> TradeDirection:
        """
        Hold/no-trade direction.

        Returns
        -------
        TradeDirection
            Result of hold for this `TradeDirection` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeDirection
        >>> callable(TradeDirection.hold)
        True
        """
        ...

    @property
    def label(self) -> str:
        """
        Direction label.
        Returns
        -------
        str
            The label exposed by this `TradeDirection`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TradeType:
    """
    Trade type (existing/new-position/close-out).

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeType
    >>> TradeType.__name__
    'TradeType'
    """

    @classmethod
    def existing(cls) -> TradeType:
        """
        Trade an existing position.

        Returns
        -------
        TradeType
            Result of existing for this `TradeType` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeType
        >>> callable(TradeType.existing)
        True
        """
        ...

    @classmethod
    def new_position(cls) -> TradeType:
        """
        Open a new candidate position.

        Returns
        -------
        TradeType
            Result of new position for this `TradeType` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeType
        >>> callable(TradeType.new_position)
        True
        """
        ...

    @classmethod
    def close_out(cls) -> TradeType:
        """
        Close an existing position.

        Returns
        -------
        TradeType
            Result of close out for this `TradeType` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeType
        >>> callable(TradeType.close_out)
        True
        """
        ...

    @property
    def label(self) -> str:
        """
        Trade-type label.
        Returns
        -------
        str
            The label exposed by this `TradeType`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PerPositionMetric:
    """
    Per-position metric source for optimization expressions.

    Examples
    --------
    >>> from finstack_quant.portfolio import PerPositionMetric
    >>> PerPositionMetric.__name__
    'PerPositionMetric'
    """

    @classmethod
    def metric(cls, metric_id: str) -> PerPositionMetric:
        """
        Use a valuation metric by fully qualified metric ID.

        Parameters
        ----------
        metric_id : str
            Per-position metric key, such as ``"pv01::usd_ois"`` or
            ``"cs01::BOND_A"``.

        Returns
        -------
        PerPositionMetric
            Result of metric for this `PerPositionMetric` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> callable(PerPositionMetric.metric)
        True
        """
        ...

    @classmethod
    def custom_key(cls, key: str) -> PerPositionMetric:
        """
        Use a custom per-position metric key.

        Parameters
        ----------
        key : str
            Custom metric key emitted by the portfolio valuation pipeline.

        Returns
        -------
        PerPositionMetric
            Result of custom key for this `PerPositionMetric` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> callable(PerPositionMetric.custom_key)
        True
        """
        ...

    @classmethod
    def pv_base(cls) -> PerPositionMetric:
        """
        Use present value converted to portfolio base currency.

        Returns
        -------
        PerPositionMetric
            Result of pv base for this `PerPositionMetric` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> callable(PerPositionMetric.pv_base)
        True
        """
        ...

    @classmethod
    def pv_native(cls) -> PerPositionMetric:
        """
        Use present value in the instrument native currency.

        Returns
        -------
        PerPositionMetric
            Result of pv native for this `PerPositionMetric` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> callable(PerPositionMetric.pv_native)
        True
        """
        ...

    @classmethod
    def attribute(cls, key: str) -> PerPositionMetric:
        """
        Use a position attribute as a metric source.

        Parameters
        ----------
        key : str
            Attribute name stored on positions, whose numeric values become the
            metric for selected positions.

        Returns
        -------
        PerPositionMetric
            Result of attribute for this `PerPositionMetric` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> callable(PerPositionMetric.attribute)
        True
        """
        ...

    @classmethod
    def attribute_indicator(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PerPositionMetric:
        """
        Use a boolean position-attribute comparison as an indicator metric.

        Parameters
        ----------
        key : str
            Position attribute name to compare.
        op : str
            Comparison operator accepted by Rust, such as ``"eq"`` or ``"gt"``.
        text : str or None
            Optional string comparison value; supply when ``op`` compares text.
        number : float or None
            Optional numeric comparison value; supply when ``op`` compares numbers.

        Returns
        -------
        PerPositionMetric
            Result of attribute indicator for this `PerPositionMetric` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> callable(PerPositionMetric.attribute_indicator)
        True
        """
        ...

    @classmethod
    def constant(cls, value: float) -> PerPositionMetric:
        """
        Use a constant metric value for every selected position.

        Parameters
        ----------
        value : float
            Numeric metric value assigned identically to each selected position.

        Returns
        -------
        PerPositionMetric
            Result of constant for this `PerPositionMetric` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> callable(PerPositionMetric.constant)
        True
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PerPositionMetric:
        """
        Deserialize a per-position metric expression from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized metric-source expression, normally produced by
            ``PerPositionMetric.to_json``.

        Returns
        -------
        PerPositionMetric
            Validated `PerPositionMetric` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> callable(PerPositionMetric.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this per-position metric expression to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PerPositionMetric`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Metric-source variant label.
        Returns
        -------
        str
            The kind exposed by this `PerPositionMetric`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionFilter:
    """
    Declarative filter for selecting which positions a rule applies to.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionFilter
    >>> PositionFilter.__name__
    'PositionFilter'
    """

    @classmethod
    def all(cls) -> PositionFilter:
        """
        Select all positions.

        Returns
        -------
        PositionFilter
            Result of all for this `PositionFilter` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> callable(PositionFilter.all)
        True
        """
        ...

    @classmethod
    def by_entity_id(cls, entity_id: str) -> PositionFilter:
        """
        Select positions for one entity ID.

        Parameters
        ----------
        entity_id : str
            Entity identifier assigned to positions that should be selected.

        Returns
        -------
        PositionFilter
            Result of by entity id for this `PositionFilter` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> callable(PositionFilter.by_entity_id)
        True
        """
        ...

    @classmethod
    def by_attribute(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PositionFilter:
        """
        Select positions by attribute comparison.

        Parameters
        ----------
        key : str
            Position attribute name to compare.
        op : str
            Comparison operator accepted by Rust, such as ``"eq"`` or ``"gte"``.
        text : str or None
            Optional string comparison value for text-valued attributes.
        number : float or None
            Optional numeric comparison value for numeric attributes.

        Returns
        -------
        PositionFilter
            Result of by attribute for this `PositionFilter` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> callable(PositionFilter.by_attribute)
        True
        """
        ...

    @classmethod
    def by_position_ids(cls, position_ids: list[str]) -> PositionFilter:
        """
        Select positions by explicit position IDs.

        Parameters
        ----------
        position_ids : list[str]
            Explicit portfolio position identifiers to include in the filter.

        Returns
        -------
        PositionFilter
            Result of by position ids for this `PositionFilter` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> callable(PositionFilter.by_position_ids)
        True
        """
        ...

    @classmethod
    def not_(cls, inner: PositionFilter) -> PositionFilter:
        """
        Negate another filter.

        Parameters
        ----------
        inner : PositionFilter
            Existing filter whose matching positions should be excluded.

        Returns
        -------
        PositionFilter
            Result of not for this `PositionFilter` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> callable(PositionFilter.not_)
        True
        """
        ...

    @classmethod
    def and_(cls, filters: list[PositionFilter]) -> PositionFilter:
        """
        Select positions matching all child filters.

        Parameters
        ----------
        filters : list[PositionFilter]
            Child filters that every selected position must satisfy.

        Returns
        -------
        PositionFilter
            Result of and for this `PositionFilter` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> callable(PositionFilter.and_)
        True
        """
        ...

    @classmethod
    def or_(cls, filters: list[PositionFilter]) -> PositionFilter:
        """
        Select positions matching any child filter.

        Parameters
        ----------
        filters : list[PositionFilter]
            Child filters of which at least one must match each selected position.

        Returns
        -------
        PositionFilter
            Result of or for this `PositionFilter` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> callable(PositionFilter.or_)
        True
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PositionFilter:
        """
        Deserialize a position filter from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized filter tree, normally produced by
            ``PositionFilter.to_json``.

        Returns
        -------
        PositionFilter
            Validated `PositionFilter` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> callable(PositionFilter.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position filter to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionFilter`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Filter variant label.
        Returns
        -------
        str
            The kind exposed by this `PositionFilter`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class MetricExpr:
    """
    Portfolio-level metric expression.

    Examples
    --------
    >>> from finstack_quant.portfolio import MetricExpr
    >>> MetricExpr.__name__
    'MetricExpr'
    """

    @classmethod
    def weighted_sum(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr:
        """
        Build a weighted-sum portfolio metric expression.

        Parameters
        ----------
        metric : PerPositionMetric
            Per-position value to multiply by each selected position weight.
        filter : PositionFilter or None
            Optional selector limiting the expression to matching positions.

        Returns
        -------
        MetricExpr
            Result of weighted sum for this `MetricExpr` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr
        >>> callable(MetricExpr.weighted_sum)
        True
        """
        ...

    @classmethod
    def value_weighted_average(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr:
        """
        Build a value-weighted-average portfolio metric expression.

        Parameters
        ----------
        metric : PerPositionMetric
            Per-position value to average using portfolio market-value weights.
        filter : PositionFilter or None
            Optional selector limiting the expression to matching positions.

        Returns
        -------
        MetricExpr
            Result of value weighted average for this `MetricExpr` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr
        >>> callable(MetricExpr.value_weighted_average)
        True
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> MetricExpr:
        """
        Deserialize a metric expression from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized portfolio metric expression, normally produced
            by ``MetricExpr.to_json``.

        Returns
        -------
        MetricExpr
            Validated `MetricExpr` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr
        >>> callable(MetricExpr.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this metric expression to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `MetricExpr`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Metric-expression variant label.
        Returns
        -------
        str
            The kind exposed by this `MetricExpr`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class Objective:
    """
    Optimization direction and target.

    Examples
    --------
    >>> from finstack_quant.portfolio import Objective
    >>> Objective.__name__
    'Objective'
    """

    @classmethod
    def maximize(cls, expr: MetricExpr) -> Objective:
        """
        Maximize the supplied metric expression.

        Parameters
        ----------
        expr : MetricExpr
            Portfolio-level metric expression the optimizer should maximize.

        Returns
        -------
        Objective
            Result of maximize for this `Objective` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import Objective
        >>> callable(Objective.maximize)
        True
        """
        ...

    @classmethod
    def minimize(cls, expr: MetricExpr) -> Objective:
        """
        Minimize the supplied metric expression.

        Parameters
        ----------
        expr : MetricExpr
            Portfolio-level metric expression the optimizer should minimize.

        Returns
        -------
        Objective
            Result of minimize for this `Objective` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import Objective
        >>> callable(Objective.minimize)
        True
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> Objective:
        """
        Deserialize an optimization objective from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized objective direction and metric expression,
            normally produced by ``Objective.to_json``.

        Returns
        -------
        Objective
            Validated `Objective` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import Objective
        >>> callable(Objective.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization objective to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `Objective`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def direction(self) -> str:
        """
        Optimization direction label.
        Returns
        -------
        str
            The direction exposed by this `Objective`.
        """
        ...

    @property
    def expr(self) -> MetricExpr:
        """
        Metric expression being optimized.
        Returns
        -------
        MetricExpr
            The expr exposed by this `Objective`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class Constraint:
    """
    Declarative constraint specification.

    Examples
    --------
    >>> from finstack_quant.portfolio import Constraint
    >>> Constraint.__name__
    'Constraint'
    """

    @classmethod
    def metric_bound(
        cls,
        metric: MetricExpr,
        op: Inequality,
        rhs: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain a metric expression against a right-hand side.

        Parameters
        ----------
        metric : MetricExpr
            Portfolio expression whose evaluated value is constrained.
        op : Inequality
            Less-than, greater-than, or equality operator for the bound.
        rhs : float
            Numeric right-hand-side bound in the metric expression's units.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Result of metric bound for this `Constraint` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> callable(Constraint.metric_bound)
        True
        """
        ...

    @classmethod
    def weight_bounds(
        cls,
        filter: PositionFilter,
        min: float,
        max: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain selected position weights to a min/max interval.

        Parameters
        ----------
        filter : PositionFilter
            Selector identifying the positions subject to the weight interval.
        min : float
            Inclusive lower bound on each selected position's decimal weight.
        max : float
            Inclusive upper bound on each selected position's decimal weight.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Result of weight bounds for this `Constraint` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> callable(Constraint.weight_bounds)
        True
        """
        ...

    @classmethod
    def max_turnover(
        cls,
        max_turnover: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain total portfolio turnover.

        Parameters
        ----------
        max_turnover : float
            Maximum permitted aggregate turnover as a decimal weight fraction.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Result of max turnover for this `Constraint` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> callable(Constraint.max_turnover)
        True
        """
        ...

    @classmethod
    def budget(cls, rhs: float) -> Constraint:
        """
        Constrain total portfolio budget/weight to ``rhs``.

        Parameters
        ----------
        rhs : float
            Required total portfolio weight, normally ``1.0`` for a fully
            invested long-only budget.

        Returns
        -------
        Constraint
            Result of budget for this `Constraint` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> callable(Constraint.budget)
        True
        """
        ...

    @classmethod
    def exposure_limit(
        cls,
        key: str,
        value: str,
        max_share: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain maximum exposure share for an attribute key/value.

        Parameters
        ----------
        key : str
            Position attribute name used to define the exposure bucket.
        value : str
            Attribute value identifying the exposure bucket to cap.
        max_share : float
            Maximum decimal portfolio-weight share permitted in the bucket.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Result of exposure limit for this `Constraint` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> callable(Constraint.exposure_limit)
        True
        """
        ...

    @classmethod
    def exposure_minimum(
        cls,
        key: str,
        value: str,
        min_share: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain minimum exposure share for an attribute key/value.

        Parameters
        ----------
        key : str
            Position attribute name used to define the exposure bucket.
        value : str
            Attribute value identifying the exposure bucket to floor.
        min_share : float
            Minimum decimal portfolio-weight share required in the bucket.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Result of exposure minimum for this `Constraint` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> callable(Constraint.exposure_minimum)
        True
        """
        ...

    def with_label(self, label: str) -> Constraint:
        """
        Return a copy with a human-readable label.

        Parameters
        ----------
        label : str
            Stable reader-facing name used in diagnostics and optimizer reports.

        Returns
        -------
        Constraint
            Result of with label for this `Constraint` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> Constraint:
        """
        Deserialize an optimization constraint from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized constraint, normally produced by
            ``Constraint.to_json``.

        Returns
        -------
        Constraint
            Validated `Constraint` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> callable(Constraint.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization constraint to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `Constraint`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Constraint variant label.
        Returns
        -------
        str
            The kind exposed by this `Constraint`.
        """
        ...

    @property
    def label(self) -> str | None:
        """
        Optional human-readable label.
        Returns
        -------
        str or None
            The label exposed by this `Constraint`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class CandidatePosition:
    """
    Candidate instrument that could be added to the portfolio.

    Construction from Python is not yet supported (requires the instrument
    binding bridge). Returned by getters on :class:`TradeUniverse`.

    Examples
    --------
    >>> from finstack_quant.portfolio import CandidatePosition
    >>> CandidatePosition.__name__
    'CandidatePosition'
    """

    @property
    def id(self) -> str:
        """
        Candidate position identifier.
        Returns
        -------
        str
            The id exposed by this `CandidatePosition`.
        """
        ...

    @property
    def entity_id(self) -> str:
        """
        Candidate entity identifier.
        Returns
        -------
        str
            The entity id exposed by this `CandidatePosition`.
        """
        ...

    @property
    def max_weight(self) -> float:
        """
        Maximum allowed candidate weight.
        Returns
        -------
        float
            The max weight exposed by this `CandidatePosition`.
        """
        ...

    @property
    def min_weight(self) -> float:
        """
        Minimum allowed candidate weight.
        Returns
        -------
        float
            The min weight exposed by this `CandidatePosition`.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """
        Underlying instrument identifier.
        Returns
        -------
        str
            The instrument id exposed by this `CandidatePosition`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TradeUniverse:
    """
    Universe of tradeable existing positions and candidate additions.

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeUniverse
    >>> TradeUniverse.__name__
    'TradeUniverse'
    """

    @classmethod
    def all_positions(cls) -> TradeUniverse:
        """
        Make every existing position tradeable.

        Returns
        -------
        TradeUniverse
            Result of all positions for this `TradeUniverse` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeUniverse
        >>> callable(TradeUniverse.all_positions)
        True
        """
        ...

    @property
    def tradeable_filter(self) -> PositionFilter:
        """
        Filter selecting tradeable positions.
        Returns
        -------
        PositionFilter
            The tradeable filter exposed by this `TradeUniverse`.
        """
        ...

    @property
    def held_filter(self) -> PositionFilter | None:
        """
        Optional filter selecting held positions.
        Returns
        -------
        PositionFilter or None
        """
        ...

    @property
    def candidates(self) -> list[CandidatePosition]:
        """
        Candidate new positions.
        Returns
        -------
        list[CandidatePosition]
        """
        ...

    @property
    def allow_short_candidates(self) -> bool:
        """
        Whether candidates may receive negative weights.
        Returns
        -------
        bool
            The allow short candidates exposed by this `TradeUniverse`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class OptimizationStatus:
    """
    Status of an optimization run.

    Examples
    --------
    >>> from finstack_quant.portfolio import OptimizationStatus
    >>> OptimizationStatus.__name__
    'OptimizationStatus'
    """

    @classmethod
    def optimal(cls) -> OptimizationStatus:
        """
        Successful optimal solve.

        Returns
        -------
        OptimizationStatus
            Result of optimal for this `OptimizationStatus` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> callable(OptimizationStatus.optimal)
        True
        """
        ...

    @classmethod
    def feasible_but_suboptimal(cls) -> OptimizationStatus:
        """
        Feasible solution that did not prove optimality.

        Returns
        -------
        OptimizationStatus
            Result of feasible but suboptimal for this `OptimizationStatus` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> callable(OptimizationStatus.feasible_but_suboptimal)
        True
        """
        ...

    @classmethod
    def unbounded(cls) -> OptimizationStatus:
        """
        Optimization problem is unbounded.

        Returns
        -------
        OptimizationStatus
            Result of unbounded for this `OptimizationStatus` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> callable(OptimizationStatus.unbounded)
        True
        """
        ...

    @classmethod
    def infeasible(cls, conflicting_constraints: list[str]) -> OptimizationStatus:
        """
        Create an infeasible status with the listed conflicting constraints.

        Parameters
        ----------
        conflicting_constraints : list[str]
            Constraint labels implicated in the infeasibility diagnosis.

        Returns
        -------
        OptimizationStatus
            Result of infeasible for this `OptimizationStatus` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> callable(OptimizationStatus.infeasible)
        True
        """
        ...

    @classmethod
    def error(cls, message: str) -> OptimizationStatus:
        """
        Create a solver or model-building error status.

        Parameters
        ----------
        message : str
            Reader-facing error or diagnostic message returned by the optimizer.

        Returns
        -------
        OptimizationStatus
            Result of error for this `OptimizationStatus` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> callable(OptimizationStatus.error)
        True
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> OptimizationStatus:
        """
        Deserialize an optimization status from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized solver status, normally produced by
            ``OptimizationStatus.to_json``.

        Returns
        -------
        OptimizationStatus
            Validated `OptimizationStatus` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> callable(OptimizationStatus.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization status to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `OptimizationStatus`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Status variant label.
        Returns
        -------
        str
            The kind exposed by this `OptimizationStatus`.
        """
        ...

    @property
    def is_feasible(self) -> bool:
        """
        Whether the status includes a feasible solution.
        Returns
        -------
        bool
            Whether feasible holds for this `OptimizationStatus`.
        """
        ...

    @property
    def conflicting_constraints(self) -> list[str]:
        """
        Constraint labels implicated in infeasibility.
        Returns
        -------
        list[str]
            The conflicting constraints exposed by this `OptimizationStatus`.
        """
        ...

    @property
    def message(self) -> str | None:
        """
        Error or diagnostic message, if present.
        Returns
        -------
        str or None
            The message exposed by this `OptimizationStatus`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TradeSpec:
    """
    Trade specification for a single position.

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeSpec
    >>> TradeSpec.__name__
    'TradeSpec'
    """

    @classmethod
    def from_json(cls, json_str: str) -> TradeSpec:
        """
        Deserialize a trade specification from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized trade recommendation, normally produced by
            ``TradeSpec.to_json``.

        Returns
        -------
        TradeSpec
            Validated `TradeSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeSpec
        >>> callable(TradeSpec.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this trade specification to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `TradeSpec`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.
        Returns
        -------
        str
            The position id exposed by this `TradeSpec`.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """
        Underlying instrument identifier.
        Returns
        -------
        str
            The instrument id exposed by this `TradeSpec`.
        """
        ...

    @property
    def trade_type(self) -> TradeType:
        """
        Return the trade type for `TradeSpec`.
        Trade type.
        Returns
        -------
        TradeType
            The trade type exposed by this `TradeSpec`.
        """
        ...

    @property
    def direction(self) -> TradeDirection:
        """
        Trade direction.
        Returns
        -------
        TradeDirection
            The direction exposed by this `TradeSpec`.
        """
        ...

    @property
    def current_quantity(self) -> float:
        """
        Current quantity.
        Returns
        -------
        float
            The current quantity exposed by this `TradeSpec`.
        """
        ...

    @property
    def target_quantity(self) -> float:
        """
        Target quantity.
        Returns
        -------
        float
            The target quantity exposed by this `TradeSpec`.
        """
        ...

    @property
    def delta_quantity(self) -> float:
        """
        Target quantity less current quantity.
        Returns
        -------
        float
            The delta quantity exposed by this `TradeSpec`.
        """
        ...

    @property
    def current_weight(self) -> float:
        """
        Current portfolio weight.
        Returns
        -------
        float
            The current weight exposed by this `TradeSpec`.
        """
        ...

    @property
    def target_weight(self) -> float:
        """
        Target portfolio weight.
        Returns
        -------
        float
            The target weight exposed by this `TradeSpec`.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioOptimizationSpec:
    """
    JSON-serializable portfolio optimization specification.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioOptimizationSpec
    >>> PortfolioOptimizationSpec.__name__
    'PortfolioOptimizationSpec'
    """

    @classmethod
    def new(
        cls,
        portfolio_spec_json: str,
        objective: Objective,
    ) -> PortfolioOptimizationSpec:
        """
        Create an optimization specification from portfolio JSON and objective.

        Parameters
        ----------
        portfolio_spec_json : str
            Canonical ``PortfolioSpec`` JSON defining positions and starting weights.
        objective : Objective
            Direction and metric expression the optimizer should solve for.

        Returns
        -------
        PortfolioOptimizationSpec
            Result of new for this `PortfolioOptimizationSpec` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioOptimizationSpec
        >>> callable(PortfolioOptimizationSpec.new)
        True
        """
        ...

    def with_constraint(self, constraint: Constraint) -> PortfolioOptimizationSpec:
        """
        Return a copy with an additional constraint.

        Parameters
        ----------
        constraint : Constraint
            Constraint to append to the current optimization specification.

        Returns
        -------
        PortfolioOptimizationSpec
            Result of with constraint for this `PortfolioOptimizationSpec` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def with_objective(self, objective: Objective) -> PortfolioOptimizationSpec:
        """
        Return a copy with a replacement objective.

        Parameters
        ----------
        objective : Objective
            New direction and metric expression that replaces the existing one.

        Returns
        -------
        PortfolioOptimizationSpec
            Result of with objective for this `PortfolioOptimizationSpec` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def with_weighting(self, weighting: WeightingScheme) -> PortfolioOptimizationSpec:
        """
        Return a copy with a replacement weighting scheme.

        Parameters
        ----------
        weighting : WeightingScheme
            Market-value or notional convention used to calculate portfolio weights.

        Returns
        -------
        PortfolioOptimizationSpec
            Result of with weighting for this `PortfolioOptimizationSpec` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def with_missing_metric_policy(self, policy: MissingMetricPolicy) -> PortfolioOptimizationSpec:
        """
        Return a copy with a replacement missing-metric policy.

        Parameters
        ----------
        policy : MissingMetricPolicy
            Policy defining how positions without required metric data are handled.

        Returns
        -------
        PortfolioOptimizationSpec
            Result of with missing metric policy for this `PortfolioOptimizationSpec` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def with_label(self, label: str) -> PortfolioOptimizationSpec:
        """
        Return a copy with a human-readable label.

        Parameters
        ----------
        label : str
            Stable reader-facing name for reports, diagnostics, and persistence.

        Returns
        -------
        PortfolioOptimizationSpec
            Result of with label for this `PortfolioOptimizationSpec` in the annotated representation.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PortfolioOptimizationSpec:
        """
        Deserialize an optimization specification from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized portfolio optimization specification, normally
            produced by ``PortfolioOptimizationSpec.to_json``.

        Returns
        -------
        PortfolioOptimizationSpec
            Validated `PortfolioOptimizationSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        PortfolioError
            If the JSON payload cannot be parsed or does not satisfy the `PortfolioError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioOptimizationSpec
        >>> callable(PortfolioOptimizationSpec.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization specification to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioOptimizationSpec`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def objective(self) -> Objective:
        """
        Optimization objective.
        Returns
        -------
        Objective
            The objective exposed by this `PortfolioOptimizationSpec`.
        """
        ...

    @property
    def constraints(self) -> list[Constraint]:
        """
        Optimization constraints.
        Returns
        -------
        list[Constraint]
            The constraints exposed by this `PortfolioOptimizationSpec`.
        """
        ...

    @property
    def weighting(self) -> WeightingScheme:
        """
        Weighting scheme used by the optimization.
        Returns
        -------
        WeightingScheme
            The weighting exposed by this `PortfolioOptimizationSpec`.
        """
        ...

    @property
    def missing_metric_policy(self) -> MissingMetricPolicy:
        """
        Policy for missing per-position metrics.
        Returns
        -------
        MissingMetricPolicy
        """
        ...

    @property
    def label(self) -> str | None:
        """
        Optional human-readable label.
        Returns
        -------
        str or None
            The label exposed by this `PortfolioOptimizationSpec`.
        """
        ...

    def portfolio_spec_json(self) -> str:
        """
        Return the embedded portfolio specification JSON.
        Returns
        -------
        str
            Result of portfolio spec json for this `PortfolioOptimizationSpec` in the annotated representation.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioOptimizationResult:
    """
    Result of an optimization run (Serialize-only; no ``from_json``).

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioOptimizationResult
    >>> PortfolioOptimizationResult.__name__
    'PortfolioOptimizationResult'
    """

    def to_json(self) -> str:
        """
        Serialize this optimization result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioOptimizationResult`, suitable for a matching `from_json` call.
        """
        ...

    @property
    def status(self) -> OptimizationStatus:
        """
        Optimization status.
        Returns
        -------
        OptimizationStatus
        """
        ...

    @property
    def is_feasible(self) -> bool:
        """
        Whether the solver returned a feasible portfolio.
        Returns
        -------
        bool
            Whether feasible holds for this `PortfolioOptimizationResult`.
        """
        ...

    @property
    def objective_value(self) -> float:
        """
        Objective value at the solution.
        Returns
        -------
        float
            The objective value exposed by this `PortfolioOptimizationResult`.
        """
        ...

    @property
    def current_weights(self) -> dict[str, float]:
        """
        Current weights by position ID.
        Returns
        -------
        dict[str, float]
            The current weights exposed by this `PortfolioOptimizationResult`.
        """
        ...

    @property
    def optimal_weights(self) -> dict[str, float]:
        """
        Optimized target weights by position ID.
        Returns
        -------
        dict[str, float]
            The optimal weights exposed by this `PortfolioOptimizationResult`.
        """
        ...

    @property
    def weight_deltas(self) -> dict[str, float]:
        """
        Target less current weight by position ID.
        Returns
        -------
        dict[str, float]
            The weight deltas exposed by this `PortfolioOptimizationResult`.
        """
        ...

    @property
    def implied_quantities(self) -> dict[str, float]:
        """
        Implied target quantities by position ID.
        Returns
        -------
        dict[str, float]
            The implied quantities exposed by this `PortfolioOptimizationResult`.
        """
        ...

    @property
    def metric_values(self) -> dict[str, float]:
        """
        Portfolio metric values at the solution.
        Returns
        -------
        dict[str, float]
            The metric values exposed by this `PortfolioOptimizationResult`.
        """
        ...

    @property
    def dual_values(self) -> dict[str, float]:
        """
        Dual values by constraint label when available.
        Returns
        -------
        dict[str, float]
            The dual values exposed by this `PortfolioOptimizationResult`.
        """
        ...

    @property
    def constraint_slacks(self) -> dict[str, float]:
        """
        Constraint slack values by constraint label.
        Returns
        -------
        dict[str, float]
            The constraint slacks exposed by this `PortfolioOptimizationResult`.
        """
        ...

    @property
    def turnover(self) -> float:
        """
        Total turnover implied by the solution.
        Returns
        -------
        float
            The turnover exposed by this `PortfolioOptimizationResult`.
        """
        ...

    def to_trade_list(self) -> list[TradeSpec]:
        """
        Convert weight deltas into trade specifications.
        Returns
        -------
        list[TradeSpec]
            Result of to trade list for this `PortfolioOptimizationResult` in the annotated representation.
        """
        ...

    def new_position_trades(self) -> list[TradeSpec]:
        """
        Return trade specs for new candidate positions only.
        Returns
        -------
        list[TradeSpec]
            Result of new position trades for this `PortfolioOptimizationResult` in the annotated representation.
        """
        ...

    def binding_constraints(self) -> list[tuple[str, float]]:
        """
        Return constraints with near-zero slack as ``(label, slack)`` pairs.
        Returns
        -------
        list[tuple[str, float]]
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def optimize_portfolio_typed(
    spec: PortfolioOptimizationSpec,
    market: MarketContext | str,
) -> PortfolioOptimizationResult:
    """
    Typed sibling of :func:`optimize_portfolio`.

    Accepts a typed :class:`PortfolioOptimizationSpec` and returns a typed
    :class:`PortfolioOptimizationResult` rather than JSON strings.

    Parameters
    ----------
    spec : PortfolioOptimizationSpec
        Typed portfolio definition, objective, constraints, and solver policy.
    market : MarketContext or str
        Market context object or JSON used to value positions and calculate metrics.

    Returns
    -------
    PortfolioOptimizationResult
        Result of optimize portfolio typed for the binding in the annotated representation.

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.portfolio import optimize_portfolio_typed
    >>> callable(optimize_portfolio_typed)
    True
    """
    ...

# ---------------------------------------------------------------------------
# Factor Sensitivity
# ---------------------------------------------------------------------------

class SensitivityMatrix:
    """
    Positions-by-factors sensitivity matrix.

    Each element ``(i, j)`` is the first-order sensitivity of position *i* to
    factor *j*, denominated in the factor's bump units (e.g. PV change per 1 bp
    for a rates factor).

    Construct via :func:`compute_factor_sensitivities`.

    Examples
    --------
    >>> from finstack_quant.portfolio import compute_factor_sensitivities
    >>> matrix = compute_factor_sensitivities(pos_json, fac_json, mkt_json, "2025-01-15")  # doctest: +SKIP
    """

    @property
    def position_ids(self) -> list[str]:
        """
        Ordered position identifiers (row axis).
        Returns
        -------
        list[str]
            The position ids exposed by this `SensitivityMatrix`.
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """
        Ordered factor identifiers (column axis).
        Returns
        -------
        list[str]
            The factor ids exposed by this `SensitivityMatrix`.
        """
        ...

    @property
    def n_positions(self) -> int:
        """
        Number of positions (rows).
        Returns
        -------
        int
            The n positions exposed by this `SensitivityMatrix`.
        """
        ...

    @property
    def n_factors(self) -> int:
        """
        Number of factors (columns).
        Returns
        -------
        int
            The n factors exposed by this `SensitivityMatrix`.
        """
        ...

    def delta(self, position_idx: int, factor_idx: int) -> float:
        """
        Read a single sensitivity element.

        Parameters
        ----------
        position_idx : int
            Zero-based row index into ``position_ids`` for the requested position.
        factor_idx : int
            Column index.

        Returns
        -------
        float
            Sensitivity value.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def position_deltas(self, position_idx: int) -> list[float]:
        """
        Sensitivity row for a single position across all factors.

        Parameters
        ----------
        position_idx : int
            Zero-based row index into ``position_ids`` for the requested position.

        Returns
        -------
        list[float]
            List of delta values, one per factor.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def factor_deltas(self, factor_idx: int) -> list[float]:
        """
        Sensitivity column for a single factor across all positions.

        Parameters
        ----------
        factor_idx : int
            Column index.

        Returns
        -------
        list[float]
            List of delta values, one per position.

        Raises
        ------
        PortfolioError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a pandas DataFrame with positions as rows and factors as columns.

        Returns
        -------
        pd.DataFrame
            DataFrame indexed by position IDs with factor IDs as column names.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class FactorPnlProfile:
    """
    P&L profile for one factor across a scenario grid.

    Each profile captures the hypothetical P&L for every position at each
    scenario shift, enabling non-linear (gamma, convexity) analysis.

    Construct via :func:`compute_pnl_profiles`.

    Examples
    --------
    >>> from finstack_quant.portfolio import compute_pnl_profiles
    >>> profiles = compute_pnl_profiles(pos_json, fac_json, mkt_json, "2025-01-15")  # doctest: +SKIP
    """

    @property
    def factor_id(self) -> str:
        """
        Factor identifier.
        Returns
        -------
        str
            The factor id exposed by this `FactorPnlProfile`.
        """
        ...

    @property
    def shifts(self) -> list[float]:
        """
        Scenario shift coordinates (bump-size multiples).
        Returns
        -------
        list[float]
            The shifts exposed by this `FactorPnlProfile`.
        """
        ...

    @property
    def position_pnls(self) -> list[list[float]]:
        """
        Per-shift P&L vectors indexed as ``[shift_idx][position_idx]``.
        Returns
        -------
        list[list[float]]
            The position pnls exposed by this `FactorPnlProfile`.
        """
        ...

    def to_dataframe(self, position_ids: list[str]) -> pd.DataFrame:
        """
        Export as a pandas DataFrame with shifts as rows and positions as columns.

        Parameters
        ----------
        position_ids : list[str]
            Position identifiers to use as column names.  Must
            match the number of positions in the profile.

        Returns
        -------
        pd.DataFrame
            DataFrame indexed by shift values with position IDs as column names.

        Raises
        ------
        ValueError
            If ``len(position_ids)`` does not match the profile width.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def compute_factor_sensitivities(
    positions_json: str,
    factors_json: str,
    market: MarketContext | str,
    as_of: str,
    bump_config_json: str | None = None,
) -> SensitivityMatrix:
    """
    Compute first-order factor sensitivities using central finite differences.

    Parameters
    ----------
    positions_json : str
        JSON array of position objects, each with ``id`` (str),
        ``instrument`` (tagged instrument JSON), and ``weight`` (float).
    factors_json : str
        JSON array of ``FactorDefinition`` objects.
    market : MarketContext or str
        ``MarketContext`` instance or JSON string.
    as_of : str
        Valuation date in ISO 8601 format.
    bump_config_json : str, optional
        Optional JSON-serialized ``BumpSizeConfig``.
        Defaults to 1 bp / 1 % per factor type.

    Returns
    -------
    SensitivityMatrix
        Positions-by-factors delta matrix.

    Examples
    --------
    >>> from finstack_quant.portfolio import compute_factor_sensitivities
    >>> matrix = compute_factor_sensitivities(pos_json, fac_json, mkt_json, "2025-01-15")  # doctest: +SKIP
    >>> matrix.to_dataframe()  # doctest: +SKIP

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
    """
    ...

def compute_pnl_profiles(
    positions_json: str,
    factors_json: str,
    market: MarketContext | str,
    as_of: str,
    bump_config_json: str | None = None,
    n_scenario_points: int = 5,
) -> list[FactorPnlProfile]:
    """
    Compute scenario P&L profiles via full repricing across a factor grid.

    Parameters
    ----------
    positions_json : str
        JSON array of position objects (same schema as
        :func:`compute_factor_sensitivities`).
    factors_json : str
        JSON array of ``FactorDefinition`` objects.
    market : MarketContext or str
        ``MarketContext`` instance or JSON string.
    as_of : str
        Valuation date in ISO 8601 format.
    bump_config_json : str, optional
        Optional JSON-serialized ``BumpSizeConfig``.
    n_scenario_points : int, default 5
        Number of scenario grid points
        (default 5 produces shifts ``[-2, -1, 0, 1, 2]``).

    Returns
    -------
    list[FactorPnlProfile]
        One profile per factor, each containing scenario P&L for every position.

    Examples
    --------
    >>> from finstack_quant.portfolio import compute_pnl_profiles
    >>> profiles = compute_pnl_profiles(pos_json, fac_json, mkt_json, "2025-01-15")  # doctest: +SKIP
    >>> profiles[0].to_dataframe(["bond_1", "equity_1"])  # doctest: +SKIP

    Raises
    ------
    PortfolioError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
    """
    ...

# ---------------------------------------------------------------------------
# Risk Decomposition
# ---------------------------------------------------------------------------

class FactorRiskDecomposition:
    """
    Portfolio-level decomposition of total risk across factors and positions.

    Obtain via :func:`decompose_factor_risk`.  The decomposition expresses
    forecasted portfolio risk (variance, volatility, VaR, or ES) as a sum of
    Euler-allocated factor-level contributions, each drillable to per-position
    detail.

    Examples
    --------
    >>> from finstack_quant.portfolio import decompose_factor_risk  # doctest: +SKIP
    >>> result = decompose_factor_risk(sens, cov_json)  # doctest: +SKIP
    >>> result.total_risk  # doctest: +SKIP
    0.042
    """

    @property
    def total_risk(self) -> float:
        """
        Total portfolio risk under the selected measure.
        Returns
        -------
        float
            The total risk exposed by this `FactorRiskDecomposition`.
        """
        ...

    @property
    def measure(self) -> str:
        """
        Risk measure used (e.g. ``"Variance"``, ``"Volatility"``).
        Returns
        -------
        str
            The measure exposed by this `FactorRiskDecomposition`.
        """
        ...

    @property
    def residual_risk(self) -> float:
        """
        Residual (idiosyncratic) risk not attributed to any factor.
        Returns
        -------
        float
            The residual risk exposed by this `FactorRiskDecomposition`.
        """
        ...

    def factor_contributions(self) -> list[dict[str, object]]:
        """
        Factor-level contributions as a list of dicts.

        Each dict contains ``factor_id``, ``absolute_risk``, ``relative_risk``,
        and ``marginal_risk``.

        Returns
        -------
        list[dict[str, object]]
            List of per-factor contribution dicts.
        """
        ...

    def position_factor_contributions(self) -> list[dict[str, object]]:
        """
        Position x factor contributions as a list of dicts.

        Each dict contains ``position_id``, ``factor_id``, and
        ``risk_contribution``.

        Returns
        -------
        list[dict[str, object]]
            List of per-position, per-factor contribution dicts.
        """
        ...

    def to_factor_dataframe(self) -> pd.DataFrame:
        """
        Export factor contributions as a pandas DataFrame.

        Columns: ``factor_id``, ``absolute_risk``, ``relative_risk``,
        ``marginal_risk``.

        Returns
        -------
        pd.DataFrame
            DataFrame with one row per factor.
        """
        ...

    def to_position_factor_dataframe(self) -> pd.DataFrame:
        """
        Export position x factor contributions as a pandas DataFrame.

        Columns: ``position_id``, ``factor_id``, ``risk_contribution``.

        Returns
        -------
        pd.DataFrame
            DataFrame with one row per position-factor pair.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def decompose_factor_risk(
    sensitivities: SensitivityMatrix,
    covariance_json: str,
    risk_measure: str | dict[str, Any] | None = None,
) -> FactorRiskDecomposition:
    """
    Decompose portfolio risk into factor and position contributions.

    Uses the parametric (covariance-based) Euler decomposition to attribute
    forecasted portfolio risk across factors and individual positions.

    Parameters
    ----------
    sensitivities : SensitivityMatrix
        Weighted position x factor sensitivity matrix, as
        returned by :func:`compute_factor_sensitivities`.
    covariance_json : str
        JSON-serialized ``FactorCovarianceMatrix``.  Must use
        the same factor IDs and ordering as the sensitivity matrix.
    risk_measure : str or dict[str, Any] or None, optional
        Risk measure.  Defaults to ``"variance"``.
        Accepts Python strings (``"variance"``, ``"volatility"``) or dicts
        (``{"var": {"confidence": 0.99}}``,
        ``{"expected_shortfall": {"confidence": 0.975}}``).

    Returns
    -------
    FactorRiskDecomposition
        Portfolio-level risk decomposition with factor and position detail.

    Raises
    ------
    ValueError
        If factor axes do not match or the covariance matrix is
        invalid.

    Examples
    --------
    >>> from finstack_quant.portfolio import compute_factor_sensitivities, decompose_factor_risk
    >>> sens = compute_factor_sensitivities(pos, fac, mkt, "2025-01-15")  # doctest: +SKIP
    >>> result = decompose_factor_risk(sens, cov_json, "volatility")  # doctest: +SKIP
    >>> result.to_factor_dataframe()  # doctest: +SKIP
    """
    ...
