"""Portfolio construction, valuation, optimization, cashflows, scenarios, and metrics."""

from __future__ import annotations

from typing import Any

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
    "twrr_linked",
    "twrr_modified_dietz",
    "validate_allocation_json",
    "value_portfolio",
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
    """Portfolio validation or calculation failure."""

class FinstackValuationError(PortfolioError):
    """Portfolio valuation failure."""

class FinstackFxError(PortfolioError):
    """Portfolio FX conversion or market-data failure."""

class FinstackOptimizationError(PortfolioError):
    """Portfolio optimization failure."""

class Portfolio:
    """Built runtime portfolio. Cheap to clone; pass directly to pipeline functions.

    Build once with :meth:`from_spec` and reuse across ``value_portfolio``,
    ``aggregate_full_cashflows``, ``aggregate_metrics``, and
    ``apply_scenario_and_revalue`` to skip the per-call spec parse + index
    rebuild.
    """

    @staticmethod
    def from_spec(spec_json: str) -> Portfolio:
        """Parse a ``PortfolioSpec`` JSON string into a runtime portfolio."""
        ...

    @property
    def id(self) -> str:
        """Portfolio identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def as_of(self) -> str:
        """Portfolio as-of date as an ISO 8601 string.
        Returns
        -------
        str
        """
        ...

    @property
    def base_ccy(self) -> str:
        """Base currency code used for valuation and aggregation.
        Returns
        -------
        str
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
        """Serialize the portfolio back to its canonical ``PortfolioSpec`` JSON.
        Returns
        -------
        str
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
    """Rust-computed portfolio P&L attribution in portfolio base currency."""

    def to_json(self) -> str:
        """Serialize the complete canonical attribution payload."""
        ...

    def by_position_json(self) -> str:
        """Serialize nested attribution in Rust ``IndexMap`` position order."""
        ...

    def reconciliation_check(self, tolerance: float) -> dict[str, float | bool]:
        """Reconcile aggregate factor P&L to total P&L."""
        ...

    @property
    def total_pnl(self) -> Money: ...
    @property
    def carry(self) -> Money: ...
    @property
    def rates_curves_pnl(self) -> Money: ...
    @property
    def credit_curves_pnl(self) -> Money: ...
    @property
    def inflation_curves_pnl(self) -> Money: ...
    @property
    def correlations_pnl(self) -> Money: ...
    @property
    def fx_pnl(self) -> Money: ...
    @property
    def fx_translation_pnl(self) -> Money: ...
    @property
    def cross_factor_pnl(self) -> Money: ...
    @property
    def vol_pnl(self) -> Money: ...
    @property
    def model_params_pnl(self) -> Money: ...
    @property
    def market_scalars_pnl(self) -> Money: ...
    @property
    def residual(self) -> Money: ...
    @property
    def result_invalid(self) -> bool: ...
    def __repr__(self) -> str: ...

class PortfolioValuation:
    """Typed wrapper around a ``PortfolioValuation`` result.

    Wrap the JSON returned by :func:`value_portfolio` once and pass the typed
    object to :func:`aggregate_metrics` to skip re-parsing.
    """

    @staticmethod
    def from_json(valuation_json: str) -> PortfolioValuation:
        """Deserialize a ``PortfolioValuation`` from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this valuation to canonical JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def total_value(self) -> float:
        """Total portfolio value in ``base_ccy``.
        Returns
        -------
        float
        """
        ...

    @property
    def base_ccy(self) -> str:
        """Base currency code for this valuation.
        Returns
        -------
        str
        """
        ...

    @property
    def as_of(self) -> str:
        """Valuation date as an ISO 8601 string.
        Returns
        -------
        str
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
    """Typed wrapper around a ``PortfolioCashflows`` ladder.

    Returned by :func:`aggregate_full_cashflows`; survives multiple drill-in
    calls (``events_json``, ``by_date_json``, ``issues_json``,
    :meth:`collapse_to_base_by_date_kind`) without re-parsing.
    """

    @staticmethod
    def from_json(cashflows_json: str) -> PortfolioCashflows:
        """Deserialize a cashflow ladder from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize the full cashflow ladder to canonical JSON.
        Returns
        -------
        str
        """
        ...

    def events_json(self) -> str:
        """Return all dated cashflow events as JSON.
        Returns
        -------
        str
        """
        ...

    def by_date_json(self) -> str:
        """Return cashflows grouped by date as JSON.
        Returns
        -------
        str
        """
        ...

    def issues_json(self) -> str:
        """Return cashflow aggregation or FX-conversion issues as JSON.
        Returns
        -------
        str
        """
        ...

    def num_positions(self) -> int:
        """Number of positions represented in the ladder.
        Returns
        -------
        int
        """
        ...

    def num_issues(self) -> int:
        """Number of diagnostic issues recorded on the ladder.
        Returns
        -------
        int
        """
        ...

    def collapse_to_base_by_date_kind(
        self,
        market: MarketContext | str,
        base_ccy: str,
        as_of: str,
    ) -> str:
        """Collapse the ladder to a base-currency ``(date, kind) → Money`` JSON.

        Uses **spot-equivalent** FX at each payment date. ``as_of`` is the
        valuation/run date used to flag far-future conversions.
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
    """Typed wrapper around a ``PortfolioResult`` envelope.

    Use the scalar accessors (``total_value``, ``get_metric``) to read single
    values without re-parsing the JSON envelope.
    """

    @staticmethod
    def from_json(result_json: str) -> PortfolioResult:
        """Deserialize a portfolio result envelope from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this result envelope to canonical JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def total_value(self) -> float:
        """Total value stored in the result envelope.
        Returns
        -------
        float
        """
        ...

    def get_metric(self, metric_id: str) -> float | None:
        """Return a metric value, or ``None`` when it is absent."""
        ...

    def require_metric(self, metric_id: str) -> float:
        """Return a metric value, raising ``KeyError`` when it is absent."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioMetrics:
    """Typed wrapper around Rust-aggregated portfolio metrics."""

    @staticmethod
    def from_json(metrics_json: str) -> PortfolioMetrics:
        """Deserialize canonical ``PortfolioMetrics`` JSON."""
        ...

    def to_json(self) -> str:
        """Serialize canonical ``PortfolioMetrics`` JSON."""
        ...

    def metric_series(
        self,
        base: str,
    ) -> list[tuple[list[str], float, dict[str, float]]]:
        """Return decoded components, total, and entity values in wire order.

        Entity mappings preserve Rust ``IndexMap`` insertion order. Malformed
        legacy escapes remain literal; decoded coordinate collisions use
        literal wire components so no aggregate entry is lost.
        """
        ...

    def __repr__(self) -> str: ...

def parse_portfolio_spec(json_str: str) -> str:
    """Parse and canonicalize a ``PortfolioSpec`` from JSON."""
    ...

def build_portfolio_from_spec(spec_json: str) -> str:
    """Build a runtime portfolio from JSON and return the round-tripped spec.

    Prefer :meth:`Portfolio.from_spec` for real work — it returns the typed
    object that pipeline functions reuse without rebuilding.
    """
    ...

def portfolio_result_total_value(result: PortfolioResult | str) -> float:
    """Read total portfolio value from a ``PortfolioResult`` envelope.

    Accepts a typed :class:`PortfolioResult` (O(1)) or a JSON string
    (O(size-of-envelope)).
    """
    ...

def portfolio_result_get_metric(result: PortfolioResult | str, metric_id: str) -> float | None:
    """Read one metric from a ``PortfolioResult``.

    Accepts a typed :class:`PortfolioResult` or a JSON string.
    """
    ...

def aggregate_metrics(
    valuation: PortfolioValuation | str,
    base_ccy: str,
    market: MarketContext | str,
    as_of: str,
) -> str:
    """Aggregate portfolio metrics from a valuation.

    Accepts a typed :class:`PortfolioValuation` (fast path) or a JSON string.
    """
    ...

def value_portfolio(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    strict_risk: bool = False,
) -> str:
    """Value a portfolio.

    Accepts either a typed :class:`Portfolio` (no rebuild) or a JSON
    ``PortfolioSpec`` string, and either a typed ``MarketContext`` or a JSON
    string. Returns JSON for backwards compatibility — wrap with
    :meth:`PortfolioValuation.from_json` once to enable the fast downstream
    path into ``aggregate_metrics``.
    """
    ...

def aggregate_full_cashflows(portfolio: Portfolio | str, market: MarketContext | str) -> PortfolioCashflows:
    """Build the full classified cashflow ladder for the portfolio.

    Returns a typed :class:`PortfolioCashflows` wrapper; call ``to_json()``
    to get the raw ladder or use the typed accessors to drill in.
    """
    ...

def apply_scenario_and_revalue(
    portfolio: Portfolio | str,
    scenario_json: str,
    market: MarketContext | str,
) -> tuple[str, str]:
    """Apply a scenario and revalue the portfolio.

    Returns ``(valuation_json, report_json)``.
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
    """Attribute portfolio P&L with Rust-owned aggregation and FX translation."""
    ...

def allocate_weights(spec_json: str) -> str:
    """Allocate strategy weights from a JSON specification.

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
    """
    ...

def validate_allocation_json(spec_json: str) -> None:
    """Validate a strategy allocation JSON specification.

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
    """
    ...

def optimize_portfolio(spec_json: str, market: MarketContext | str) -> str:
    """Optimize portfolio weights using the LP-based optimizer.

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
    """Replay a portfolio through dated market snapshots.

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
    """
    ...

def parametric_var_decomposition(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]],
    confidence: float = 0.95,
) -> dict[str, object]:
    """Decompose portfolio parametric VaR across positions.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers aligned with ``weights``.
    weights : list[float]
        Portfolio weights or exposures.
    covariance : list[list[float]]
        Square covariance matrix aligned with ``position_ids``.
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
    """
    ...

def parametric_es_decomposition(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]],
    confidence: float = 0.95,
) -> dict[str, object]:
    """Decompose portfolio parametric expected shortfall across positions.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers aligned with ``weights``.
    weights : list[float]
        Portfolio weights or exposures.
    covariance : list[list[float]]
        Square covariance matrix aligned with ``position_ids``.
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
    """
    ...

def historical_var_decomposition(
    position_ids: list[str],
    position_pnls: list[list[float]],
    confidence: float = 0.95,
) -> dict[str, object]:
    """Decompose historical VaR from scenario or realized position P&Ls.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers.
    position_pnls : list[list[float]]
        Matrix of position P&Ls, one scenario row per list and
        one column per ``position_ids`` entry.
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
    """
    ...

def evaluate_risk_budget(
    position_ids: list[str],
    actual_var: list[float],
    target_var_pct: list[float],
    portfolio_var: float,
    utilization_threshold: float = 1.20,
) -> dict[str, object]:
    """Compare actual position VaR against target risk-budget shares.

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
    """
    ...

def roll_effective_spread(returns: list[float]) -> float | None:
    """Estimate the Roll effective bid-ask spread from returns.

    Returns ``None`` when there are too few observations or the first-order
    autocovariance does not imply a positive spread.
    """
    ...

def amihud_illiquidity(returns: list[float], volumes: list[float]) -> float | None:
    """Compute Amihud illiquidity from absolute returns and traded volumes.

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
    """
    ...

def days_to_liquidate(
    position_value: float,
    avg_daily_volume: float,
    participation_rate: float,
) -> float:
    """Estimate liquidation horizon in trading days.

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
    """
    ...

def liquidity_tier(days_to_liquidate: float) -> str:
    """Classify liquidation horizon into the Rust liquidity-tier labels."""
    ...

def lvar_bangia(
    var: float,
    spread_mean: float,
    spread_vol: float,
    confidence: float,
    position_value: float,
) -> dict[str, float]:
    """Compute Bangia-style liquidity-adjusted VaR.

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
    """Estimate Almgren-Chriss execution impact components.

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
    """
    ...

def kyle_lambda(volumes: list[float], returns: list[float]) -> float | None:
    """Estimate Kyle's lambda from volume and return observations.

    Returns ``None`` when the aligned sample has too few valid observations or
    cannot support the regression-style estimate.
    """
    ...

def brinson_fachler(sectors_json: str) -> str:
    """Compute single-period Brinson-Fachler attribution from sector JSON.

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
    """Compute Carino-linked multi-period Brinson attribution from period JSON.

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
    """Compute a Modified-Dietz TWRR sub-period return from period JSON.

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
    """Geometrically link TWRR sub-period returns over a horizon.

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
    """Compute money-weighted return via XIRR from dated cashflow JSON.

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
    """Aggregate contribution of a single factor to portfolio risk."""

    @classmethod
    def from_json(cls, json_str: str) -> FactorContribution:
        """Deserialize a factor contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this factor contribution to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def factor_id(self) -> str:
        """Factor identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def absolute_risk(self) -> float:
        """Absolute risk contribution.
        Returns
        -------
        float
        """
        ...

    @property
    def relative_risk(self) -> float:
        """Share of total portfolio risk.
        Returns
        -------
        float
        """
        ...

    @property
    def marginal_risk(self) -> float:
        """Marginal risk contribution.
        Returns
        -------
        float
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
    """Per-position contribution to a specific factor bucket."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionFactorContribution:
        """Deserialize a position-factor contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position-factor contribution to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def factor_id(self) -> str:
        """Factor identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def risk_contribution(self) -> float:
        """Risk contribution for this position-factor pair.
        Returns
        -------
        float
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
    """Annualized residual variance contributed by a single position."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionResidualContribution:
        """Deserialize a residual contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this residual contribution to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def residual_variance(self) -> float:
        """Residual variance assigned to this position.
        Returns
        -------
        float
        """
        ...

    @property
    def source_kind(self) -> str:
        """Source category used to derive residual risk.
        Returns
        -------
        str
        """
        ...

    @property
    def source_issuer_id(self) -> str | None:
        """Issuer identifier for issuer-sourced residual risk, if present.
        Returns
        -------
        str or None
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
    """Portfolio-level risk decomposition across factors and residuals."""

    @classmethod
    def from_json(cls, json_str: str) -> RiskDecomposition:
        """Deserialize a risk decomposition from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this risk decomposition to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def total_risk(self) -> float:
        """Total portfolio risk under the decomposition measure.
        Returns
        -------
        float
        """
        ...

    @property
    def measure_json(self) -> str:
        """Risk measure specification as JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def residual_risk(self) -> float:
        """Residual risk not explained by factor contributions.
        Returns
        -------
        float
        """
        ...

    @property
    def factor_contributions(self) -> list[FactorContribution]:
        """Factor-level risk contributions.
        Returns
        -------
        list[FactorContribution]
        """
        ...

    @property
    def position_factor_contributions(self) -> list[PositionFactorContribution]:
        """Position-by-factor risk contributions.
        Returns
        -------
        list[PositionFactorContribution]
        """
        ...

    @property
    def position_residual_contributions(self) -> list[PositionResidualContribution]:
        """Per-position residual risk contributions.
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
    """Per-position component / marginal VaR."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionVarContribution:
        """Deserialize a position VaR contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position VaR contribution to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def component_var(self) -> float:
        """Component VaR assigned to this position.
        Returns
        -------
        float
        """
        ...

    @property
    def relative_var(self) -> float:
        """Share of total portfolio VaR.
        Returns
        -------
        float
        """
        ...

    @property
    def marginal_var(self) -> float | None:
        """Marginal VaR, if computed.
        Returns
        -------
        float or None
        """
        ...

    @property
    def incremental_var(self) -> float | None:
        """Incremental VaR, if requested in the decomposition config.
        Returns
        -------
        float or None
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
    """Per-position component / marginal ES."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionEsContribution:
        """Deserialize a position ES contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position ES contribution to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def component_es(self) -> float:
        """Component expected shortfall assigned to this position.
        Returns
        -------
        float
        """
        ...

    @property
    def relative_es(self) -> float:
        """Share of total portfolio expected shortfall.
        Returns
        -------
        float
        """
        ...

    @property
    def marginal_es(self) -> float | None:
        """Marginal expected shortfall, if computed.
        Returns
        -------
        float or None
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
    """Complete position-level risk decomposition."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionRiskDecomposition:
        """Deserialize a position risk decomposition from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position risk decomposition to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def portfolio_var(self) -> float:
        """Portfolio VaR.
        Returns
        -------
        float
        """
        ...

    @property
    def portfolio_es(self) -> float:
        """Portfolio expected shortfall.
        Returns
        -------
        float
        """
        ...

    @property
    def confidence(self) -> float:
        """Confidence level used for VaR/ES.
        Returns
        -------
        float
        """
        ...

    @property
    def n_positions(self) -> int:
        """Number of positions included in the decomposition.
        Returns
        -------
        int
        """
        ...

    @property
    def method(self) -> str:
        """Decomposition method label.
        Returns
        -------
        str
        """
        ...

    @property
    def euler_residual(self) -> float | None:
        """Euler allocation residual, if reported.
        Returns
        -------
        float or None
        """
        ...

    @property
    def var_contributions(self) -> list[PositionVarContribution]:
        """Per-position VaR contributions.
        Returns
        -------
        list[PositionVarContribution]
        """
        ...

    @property
    def es_contributions(self) -> list[PositionEsContribution]:
        """Per-position expected shortfall contributions.
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
    """Per-position budget comparison entry."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionBudgetEntry:
        """Deserialize a risk-budget entry from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this risk-budget entry to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def actual_component_var(self) -> float:
        """Actual component VaR for this position.
        Returns
        -------
        float
        """
        ...

    @property
    def target_component_var(self) -> float:
        """Target component VaR for this position.
        Returns
        -------
        float
        """
        ...

    @property
    def utilization(self) -> float:
        """Actual-to-target utilization ratio.
        Returns
        -------
        float
        """
        ...

    @property
    def excess(self) -> float:
        """Actual component VaR less target component VaR.
        Returns
        -------
        float
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
    """Budget evaluation result across positions."""

    @classmethod
    def from_json(cls, json_str: str) -> RiskBudgetResult:
        """Deserialize a risk-budget result from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this risk-budget result to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def total_overbudget(self) -> float:
        """Total amount above target risk budgets.
        Returns
        -------
        float
        """
        ...

    @property
    def has_breach(self) -> bool:
        """Whether any position exceeds the utilization threshold.
        Returns
        -------
        bool
        """
        ...

    @property
    def positions(self) -> list[PositionBudgetEntry]:
        """Per-position risk-budget entries.
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
    """Per-factor contribution change between a baseline and a scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> FactorContributionDelta:
        """Deserialize a factor contribution delta from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this factor contribution delta to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def factor_id(self) -> str:
        """Factor identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def absolute_change(self) -> float:
        """Absolute contribution change.
        Returns
        -------
        float
        """
        ...

    @property
    def relative_change(self) -> float:
        """Relative contribution change.
        Returns
        -------
        float
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
    """Result of a position what-if scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> WhatIfResult:
        """Deserialize a what-if result from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this what-if result to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def before(self) -> RiskDecomposition:
        """Baseline risk decomposition.
        Returns
        -------
        RiskDecomposition
        """
        ...

    @property
    def after(self) -> RiskDecomposition:
        """Post-scenario risk decomposition.
        Returns
        -------
        RiskDecomposition
        """
        ...

    @property
    def delta(self) -> list[FactorContributionDelta]:
        """Per-factor contribution changes.
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
    """Result of a factor-stress scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> StressResult:
        """Deserialize a stress result from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this stress result to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def total_pnl(self) -> float:
        """Total portfolio P&L under the stress scenario.
        Returns
        -------
        float
        """
        ...

    @property
    def position_pnl(self) -> list[tuple[str, float]]:
        """Per-position P&L pairs.
        Returns
        -------
        list[tuple[str, float]]
        """
        ...

    @property
    def stressed_decomposition(self) -> RiskDecomposition:
        """Risk decomposition after applying the stress.
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
    """Single position's contribution to tail stress."""

    @classmethod
    def from_json(cls, json_str: str) -> StressPositionEntry:
        """Deserialize a stress position entry from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this stress position entry to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def avg_tail_pnl(self) -> float:
        """Average P&L across tail scenarios.
        Returns
        -------
        float
        """
        ...

    @property
    def pct_of_tail_loss(self) -> float:
        """Share of aggregate tail loss.
        Returns
        -------
        float
        """
        ...

    @property
    def worst_scenario_pnl(self) -> float:
        """Worst single-scenario P&L for this position.
        Returns
        -------
        float
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
    """Breakdown of a single tail scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> TailScenarioBreakdown:
        """Deserialize a tail scenario breakdown from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this tail scenario breakdown to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def scenario_index(self) -> int:
        """Scenario index in the source P&L matrix.
        Returns
        -------
        int
        """
        ...

    @property
    def portfolio_pnl(self) -> float:
        """Portfolio P&L for this tail scenario.
        Returns
        -------
        float
        """
        ...

    @property
    def position_pnls(self) -> list[float]:
        """Per-position P&L for this scenario, index-aligned to
        ``StressAttribution.position_ids`` (entry ``i`` is the P&L for
        ``position_ids[i]``).
        Returns
        -------
        list[float]
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
    """Per-position attribution of portfolio losses in tail scenarios."""

    @classmethod
    def from_json(cls, json_str: str) -> StressAttribution:
        """Deserialize stress attribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this stress attribution to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def var_threshold(self) -> float:
        """VaR threshold used to select tail scenarios.
        Returns
        -------
        float
        """
        ...

    @property
    def n_tail_scenarios(self) -> int:
        """Number of scenarios classified as tail scenarios.
        Returns
        -------
        int
        """
        ...

    @property
    def position_ids(self) -> list[str]:
        """Canonical position ordering shared by every ``tail_scenarios`` entry.
        ``tail_scenarios[k].position_pnls[i]`` is the P&L for ``position_ids[i]``.
        Returns
        -------
        list[str]
        """
        ...

    @property
    def position_contributions(self) -> list[StressPositionEntry]:
        """Per-position tail-loss contributions.
        Returns
        -------
        list[StressPositionEntry]
        """
        ...

    @property
    def tail_scenarios(self) -> list[TailScenarioBreakdown]:
        """Detailed tail scenario breakdowns.
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
    """Matched factor assignments for a single portfolio position.

    The full ``(dependency, factor_id)`` pairs are available as JSON via
    :meth:`mappings_json`; matched factor identifiers are accessible directly
    via the :attr:`factor_ids` property.
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionAssignment:
        """Deserialize a position assignment from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position assignment to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def n_mappings(self) -> int:
        """Number of dependency-to-factor mappings.
        Returns
        -------
        int
        """
        ...

    def mappings_json(self) -> str:
        """Return detailed dependency-to-factor mappings as JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """Matched factor identifiers.
        Returns
        -------
        list[str]
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
    """Single unmatched dependency surfaced during assignment."""

    @classmethod
    def from_json(cls, json_str: str) -> UnmatchedEntry:
        """Deserialize an unmatched dependency entry from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this unmatched entry to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    def dependency_json(self) -> str:
        """Return the unmatched dependency payload as JSON.
        Returns
        -------
        str
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
    """Assignment results for a portfolio-level factor mapping pass."""

    @classmethod
    def from_json(cls, json_str: str) -> FactorAssignmentReport:
        """Deserialize a factor assignment report from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this factor assignment report to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def assignments(self) -> list[PositionAssignment]:
        """Matched assignments by position.
        Returns
        -------
        list[PositionAssignment]
        """
        ...

    @property
    def unmatched(self) -> list[UnmatchedEntry]:
        """Dependencies that could not be mapped to factors.
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
    """Aggregated risk contribution for a single hierarchy level."""

    @property
    def level_name(self) -> str:
        """Hierarchy level name.
        Returns
        -------
        str
        """
        ...

    @property
    def total(self) -> float:
        """Total contribution for this level.
        Returns
        -------
        float
        """
        ...

    @property
    def by_bucket(self) -> dict[str, float]:
        """Contribution by hierarchy bucket.
        Returns
        -------
        dict[str, float]
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
    """Per-position vol breakdown under :class:`CreditVolReport`."""

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def factor_total(self) -> float:
        """Factor-driven volatility contribution.
        Returns
        -------
        float
        """
        ...

    @property
    def idiosyncratic(self) -> float:
        """Idiosyncratic volatility contribution.
        Returns
        -------
        float
        """
        ...

    @property
    def total(self) -> float:
        """Total position volatility contribution.
        Returns
        -------
        float
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
    """Aggregated vol report grouped by hierarchy level."""

    @property
    def total(self) -> float:
        """Total portfolio volatility under the report measure.
        Returns
        -------
        float
        """
        ...

    @property
    def measure_json(self) -> str:
        """Risk measure specification as JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def generic(self) -> float:
        """Generic factor contribution.
        Returns
        -------
        float
        """
        ...

    @property
    def idiosyncratic_total(self) -> float:
        """Aggregate idiosyncratic contribution.
        Returns
        -------
        float
        """
        ...

    @property
    def by_level(self) -> list[LevelVolContribution]:
        """Volatility contribution by hierarchy level.
        Returns
        -------
        list[LevelVolContribution]
        """
        ...

    @property
    def by_position(self) -> list[PositionVolContribution] | None:
        """Optional per-position volatility contributions.
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
    """Forecast horizon used to scale a calibrated `Sample` vol estimate."""

    @classmethod
    def one_step(cls) -> VolHorizon:
        """Use the calibrated one-step forecast horizon."""
        ...

    @classmethod
    def unconditional(cls) -> VolHorizon:
        """Use the unconditional long-run forecast horizon."""
        ...

    @classmethod
    def n_steps(cls, n: int) -> VolHorizon:
        """Scale the forecast to ``n`` discrete steps."""
        ...

    @classmethod
    def years(cls, years: float) -> VolHorizon:
        """Scale the forecast to a year fraction."""
        ...

    @classmethod
    def parse(cls, s: str) -> VolHorizon:
        """Parse a horizon string accepted by the Rust factor model."""
        ...

    @property
    def kind(self) -> str:
        """Horizon variant label.
        Returns
        -------
        str
        """
        ...

    @property
    def n(self) -> int | None:
        """Step count for ``n_steps`` horizons.
        Returns
        -------
        int or None
        """
        ...

    @property
    def years_value(self) -> float | None:
        """Year fraction for ``years`` horizons.
        Returns
        -------
        float or None
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
    """Configuration for position-level VaR decomposition."""

    @classmethod
    def parametric_95(cls) -> DecompositionConfig:
        """Default 95% parametric VaR decomposition config."""
        ...

    @classmethod
    def parametric_99(cls) -> DecompositionConfig:
        """Default 99% parametric VaR decomposition config."""
        ...

    @classmethod
    def historical(cls, confidence: float) -> DecompositionConfig:
        """Historical decomposition config at ``confidence``."""
        ...

    def with_incremental(self) -> DecompositionConfig:
        """Return a copy that requests incremental VaR.
        Returns
        -------
        DecompositionConfig
        """
        ...

    def with_seed(self, seed: int) -> DecompositionConfig:
        """Return a copy with deterministic simulation/randomization seed."""
        ...

    @property
    def confidence(self) -> float:
        """VaR/ES confidence level.
        Returns
        -------
        float
        """
        ...

    @property
    def method(self) -> str:
        """Decomposition method label.
        Returns
        -------
        str
        """
        ...

    @property
    def compute_incremental(self) -> bool:
        """Whether incremental VaR is requested.
        Returns
        -------
        bool
        """
        ...

    @property
    def seed(self) -> int | None:
        """Optional deterministic seed.
        Returns
        -------
        int or None
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
    covariance: list[list[float]],
    confidence: float = 0.95,
    compute_incremental: bool = False,
) -> PositionRiskDecomposition:
    """Typed sibling of :func:`parametric_var_decomposition`."""
    ...

def historical_var_decomposition_typed(
    position_ids: list[str],
    position_pnls: list[list[float]],
    confidence: float = 0.95,
) -> PositionRiskDecomposition:
    """Typed sibling of :func:`historical_var_decomposition`."""
    ...

def evaluate_risk_budget_typed(
    position_ids: list[str],
    actual_var: list[float],
    target_var_pct: list[float],
    portfolio_var: float,
    utilization_threshold: float = 1.20,
) -> RiskBudgetResult:
    """Typed sibling of :func:`evaluate_risk_budget`."""
    ...

def factor_stress(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    factor_model_config_json: str,
    as_of: str,
    stresses: list[tuple[str, float]],
) -> StressResult:
    """Run a factor-stress scenario and revalue the portfolio.

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
    """
    ...

def position_what_if(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    factor_model_config_json: str,
    as_of: str,
    changes: list[dict[str, Any]],
) -> WhatIfResult:
    """Run position remove/resize what-if analysis.

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
    """
    ...

def build_stress_attribution(
    position_ids: list[str],
    position_pnls: list[list[float]],
    confidence: float = 0.95,
) -> StressAttribution:
    """Build tail-scenario stress attribution from position P&Ls.

    Python input is position-major: one row per position, and each row contains
    that position's P&L across all scenarios. The binding transposes this into
    Rust's scenario-major buffer before selecting tail scenarios.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers, one per row in ``position_pnls``.
    position_pnls : list[list[float]]
        Matrix shaped ``len(position_ids) x n_scenarios``.
        Every row must have the same number of finite scenario P&Ls.
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
    """
    ...

def build_credit_vol_report(
    decomposition: RiskDecomposition,
    model: CreditFactorModel,
    by_position: bool = False,
) -> CreditVolReport:
    """Build a credit volatility report from decomposition outputs.

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
    """
    ...

def position_component_var(
    decomp: PositionRiskDecomposition,
    position_id: str,
) -> float:
    """Look up a position's component VaR inside a decomposition (raises KeyError)."""
    ...

# ---------------------------------------------------------------------------
# optimization spec/result classes (Slice 9)
# ---------------------------------------------------------------------------

class WeightingScheme:
    """How optimization weights are defined."""

    @classmethod
    def value_weight(cls) -> WeightingScheme:
        """Weight positions by market value."""
        ...

    @classmethod
    def notional_weight(cls) -> WeightingScheme:
        """Weight positions by notional."""
        ...

    @classmethod
    def unit_scaling(cls) -> WeightingScheme:
        """Use unit scaling rather than value/notional scaling."""
        ...

    @property
    def label(self) -> str:
        """Rust enum label for this weighting scheme.
        Returns
        -------
        str
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
    """Policy for handling positions missing required metrics."""

    @classmethod
    def zero(cls) -> MissingMetricPolicy:
        """Treat missing metric values as zero."""
        ...

    @classmethod
    def exclude(cls) -> MissingMetricPolicy:
        """Exclude positions with missing required metrics."""
        ...

    @classmethod
    def strict(cls) -> MissingMetricPolicy:
        """Reject optimization when required metrics are missing."""
        ...

    @property
    def label(self) -> str:
        """Rust enum label for this policy.
        Returns
        -------
        str
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
    """Inequality / equality operator (`<=`, `>=`, `==`)."""

    @classmethod
    def le(cls) -> Inequality:
        """Less-than-or-equal inequality."""
        ...

    @classmethod
    def ge(cls) -> Inequality:
        """Greater-than-or-equal inequality."""
        ...

    @classmethod
    def eq(cls) -> Inequality:
        """Equality constraint."""
        ...

    @property
    def label(self) -> str:
        """Operator label.
        Returns
        -------
        str
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
    """Trade direction (buy/sell/hold)."""

    @classmethod
    def buy(cls) -> TradeDirection:
        """Buy direction."""
        ...

    @classmethod
    def sell(cls) -> TradeDirection:
        """Sell direction."""
        ...

    @classmethod
    def hold(cls) -> TradeDirection:
        """Hold/no-trade direction."""
        ...

    @property
    def label(self) -> str:
        """Direction label.
        Returns
        -------
        str
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
    """Trade type (existing/new-position/close-out)."""

    @classmethod
    def existing(cls) -> TradeType:
        """Trade an existing position."""
        ...

    @classmethod
    def new_position(cls) -> TradeType:
        """Open a new candidate position."""
        ...

    @classmethod
    def close_out(cls) -> TradeType:
        """Close an existing position."""
        ...

    @property
    def label(self) -> str:
        """Trade-type label.
        Returns
        -------
        str
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
    """Per-position metric source for optimization expressions."""

    @classmethod
    def metric(cls, metric_id: str) -> PerPositionMetric:
        """Use a valuation metric by metric ID."""
        ...

    @classmethod
    def custom_key(cls, key: str) -> PerPositionMetric:
        """Use a custom per-position metric key."""
        ...

    @classmethod
    def pv_base(cls) -> PerPositionMetric:
        """Use present value converted to portfolio base currency."""
        ...

    @classmethod
    def pv_native(cls) -> PerPositionMetric:
        """Use present value in the instrument native currency."""
        ...

    @classmethod
    def attribute(cls, key: str) -> PerPositionMetric:
        """Use a position attribute as a metric source."""
        ...

    @classmethod
    def attribute_indicator(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PerPositionMetric:
        """Use a boolean position-attribute comparison as an indicator metric."""
        ...

    @classmethod
    def constant(cls, value: float) -> PerPositionMetric:
        """Use a constant metric value for every selected position."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PerPositionMetric:
        """Deserialize a per-position metric expression from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this per-position metric expression to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def kind(self) -> str:
        """Metric-source variant label.
        Returns
        -------
        str
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
    """Declarative filter for selecting which positions a rule applies to."""

    @classmethod
    def all(cls) -> PositionFilter:
        """Select all positions."""
        ...

    @classmethod
    def by_entity_id(cls, entity_id: str) -> PositionFilter:
        """Select positions for one entity ID."""
        ...

    @classmethod
    def by_attribute(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PositionFilter:
        """Select positions by attribute comparison."""
        ...

    @classmethod
    def by_position_ids(cls, position_ids: list[str]) -> PositionFilter:
        """Select positions by explicit position IDs."""
        ...

    @classmethod
    def not_(cls, inner: PositionFilter) -> PositionFilter:
        """Negate another filter."""
        ...

    @classmethod
    def and_(cls, filters: list[PositionFilter]) -> PositionFilter:
        """Select positions matching all child filters."""
        ...

    @classmethod
    def or_(cls, filters: list[PositionFilter]) -> PositionFilter:
        """Select positions matching any child filter."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PositionFilter:
        """Deserialize a position filter from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position filter to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def kind(self) -> str:
        """Filter variant label.
        Returns
        -------
        str
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
    """Portfolio-level metric expression."""

    @classmethod
    def weighted_sum(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr:
        """Build a weighted-sum portfolio metric expression."""
        ...

    @classmethod
    def value_weighted_average(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr:
        """Build a value-weighted-average portfolio metric expression."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> MetricExpr:
        """Deserialize a metric expression from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this metric expression to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def kind(self) -> str:
        """Metric-expression variant label.
        Returns
        -------
        str
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
    """Optimization direction and target."""

    @classmethod
    def maximize(cls, expr: MetricExpr) -> Objective:
        """Maximize the supplied metric expression."""
        ...

    @classmethod
    def minimize(cls, expr: MetricExpr) -> Objective:
        """Minimize the supplied metric expression."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> Objective:
        """Deserialize an optimization objective from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this optimization objective to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def direction(self) -> str:
        """Optimization direction label.
        Returns
        -------
        str
        """
        ...

    @property
    def expr(self) -> MetricExpr:
        """Metric expression being optimized.
        Returns
        -------
        MetricExpr
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
    """Declarative constraint specification."""

    @classmethod
    def metric_bound(
        cls,
        metric: MetricExpr,
        op: Inequality,
        rhs: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain a metric expression against a right-hand side."""
        ...

    @classmethod
    def weight_bounds(
        cls,
        filter: PositionFilter,
        min: float,
        max: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain selected position weights to a min/max interval."""
        ...

    @classmethod
    def max_turnover(
        cls,
        max_turnover: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain total portfolio turnover."""
        ...

    @classmethod
    def budget(cls, rhs: float) -> Constraint:
        """Constrain total portfolio budget/weight to ``rhs``."""
        ...

    @classmethod
    def exposure_limit(
        cls,
        key: str,
        value: str,
        max_share: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain maximum exposure share for an attribute key/value."""
        ...

    @classmethod
    def exposure_minimum(
        cls,
        key: str,
        value: str,
        min_share: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain minimum exposure share for an attribute key/value."""
        ...

    def with_label(self, label: str) -> Constraint:
        """Return a copy with a human-readable label."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> Constraint:
        """Deserialize an optimization constraint from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this optimization constraint to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def kind(self) -> str:
        """Constraint variant label.
        Returns
        -------
        str
        """
        ...

    @property
    def label(self) -> str | None:
        """Optional human-readable label.
        Returns
        -------
        str or None
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
    """Candidate instrument that could be added to the portfolio.

    Construction from Python is not yet supported (requires the instrument
    binding bridge). Returned by getters on :class:`TradeUniverse`.
    """

    @property
    def id(self) -> str:
        """Candidate position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def entity_id(self) -> str:
        """Candidate entity identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def max_weight(self) -> float:
        """Maximum allowed candidate weight.
        Returns
        -------
        float
        """
        ...

    @property
    def min_weight(self) -> float:
        """Minimum allowed candidate weight.
        Returns
        -------
        float
        """
        ...

    @property
    def instrument_id(self) -> str:
        """Underlying instrument identifier.
        Returns
        -------
        str
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
    """Universe of tradeable existing positions and candidate additions."""

    @classmethod
    def all_positions(cls) -> TradeUniverse:
        """Make every existing position tradeable."""
        ...

    @property
    def tradeable_filter(self) -> PositionFilter:
        """Filter selecting tradeable positions.
        Returns
        -------
        PositionFilter
        """
        ...

    @property
    def held_filter(self) -> PositionFilter | None:
        """Optional filter selecting held positions.
        Returns
        -------
        PositionFilter or None
        """
        ...

    @property
    def candidates(self) -> list[CandidatePosition]:
        """Candidate new positions.
        Returns
        -------
        list[CandidatePosition]
        """
        ...

    @property
    def allow_short_candidates(self) -> bool:
        """Whether candidates may receive negative weights.
        Returns
        -------
        bool
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
    """Status of an optimization run."""

    @classmethod
    def optimal(cls) -> OptimizationStatus:
        """Successful optimal solve."""
        ...

    @classmethod
    def feasible_but_suboptimal(cls) -> OptimizationStatus:
        """Feasible solution that did not prove optimality."""
        ...

    @classmethod
    def unbounded(cls) -> OptimizationStatus:
        """Optimization problem is unbounded."""
        ...

    @classmethod
    def infeasible(cls, conflicting_constraints: list[str]) -> OptimizationStatus:
        """Optimization problem is infeasible with the listed constraints."""
        ...

    @classmethod
    def error(cls, message: str) -> OptimizationStatus:
        """Solver or model-building error status."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> OptimizationStatus:
        """Deserialize an optimization status from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this optimization status to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def kind(self) -> str:
        """Status variant label.
        Returns
        -------
        str
        """
        ...

    @property
    def is_feasible(self) -> bool:
        """Whether the status includes a feasible solution.
        Returns
        -------
        bool
        """
        ...

    @property
    def conflicting_constraints(self) -> list[str]:
        """Constraint labels implicated in infeasibility.
        Returns
        -------
        list[str]
        """
        ...

    @property
    def message(self) -> str | None:
        """Error or diagnostic message, if present.
        Returns
        -------
        str or None
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
    """Trade specification for a single position."""

    @classmethod
    def from_json(cls, json_str: str) -> TradeSpec:
        """Deserialize a trade specification from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this trade specification to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def instrument_id(self) -> str:
        """Underlying instrument identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def trade_type(self) -> TradeType:
        """Trade type.
        Returns
        -------
        TradeType
        """
        ...

    @property
    def direction(self) -> TradeDirection:
        """Trade direction.
        Returns
        -------
        TradeDirection
        """
        ...

    @property
    def current_quantity(self) -> float:
        """Current quantity.
        Returns
        -------
        float
        """
        ...

    @property
    def target_quantity(self) -> float:
        """Target quantity.
        Returns
        -------
        float
        """
        ...

    @property
    def delta_quantity(self) -> float:
        """Target quantity less current quantity.
        Returns
        -------
        float
        """
        ...

    @property
    def current_weight(self) -> float:
        """Current portfolio weight.
        Returns
        -------
        float
        """
        ...

    @property
    def target_weight(self) -> float:
        """Target portfolio weight.
        Returns
        -------
        float
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
    """JSON-serializable portfolio optimization specification."""

    @classmethod
    def new(
        cls,
        portfolio_spec_json: str,
        objective: Objective,
    ) -> PortfolioOptimizationSpec:
        """Create a portfolio optimization specification from portfolio JSON and objective."""
        ...

    def with_constraint(self, constraint: Constraint) -> PortfolioOptimizationSpec:
        """Return a copy with an additional constraint."""
        ...

    def with_objective(self, objective: Objective) -> PortfolioOptimizationSpec:
        """Return a copy with a replacement objective."""
        ...

    def with_weighting(self, weighting: WeightingScheme) -> PortfolioOptimizationSpec:
        """Return a copy with a replacement weighting scheme."""
        ...

    def with_missing_metric_policy(self, policy: MissingMetricPolicy) -> PortfolioOptimizationSpec:
        """Return a copy with a replacement missing-metric policy."""
        ...

    def with_label(self, label: str) -> PortfolioOptimizationSpec:
        """Return a copy with a human-readable label."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PortfolioOptimizationSpec:
        """Deserialize an optimization specification from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this optimization specification to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def objective(self) -> Objective:
        """Optimization objective.
        Returns
        -------
        Objective
        """
        ...

    @property
    def constraints(self) -> list[Constraint]:
        """Optimization constraints.
        Returns
        -------
        list[Constraint]
        """
        ...

    @property
    def weighting(self) -> WeightingScheme:
        """Weighting scheme used by the optimization.
        Returns
        -------
        WeightingScheme
        """
        ...

    @property
    def missing_metric_policy(self) -> MissingMetricPolicy:
        """Policy for missing per-position metrics.
        Returns
        -------
        MissingMetricPolicy
        """
        ...

    @property
    def label(self) -> str | None:
        """Optional human-readable label.
        Returns
        -------
        str or None
        """
        ...

    def portfolio_spec_json(self) -> str:
        """Return the embedded portfolio specification JSON.
        Returns
        -------
        str
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
    """Result of an optimization run (Serialize-only; no ``from_json``)."""

    def to_json(self) -> str:
        """Serialize this optimization result to JSON.
        Returns
        -------
        str
        """
        ...

    @property
    def status(self) -> OptimizationStatus:
        """Optimization status.
        Returns
        -------
        OptimizationStatus
        """
        ...

    @property
    def is_feasible(self) -> bool:
        """Whether the solver returned a feasible portfolio.
        Returns
        -------
        bool
        """
        ...

    @property
    def objective_value(self) -> float:
        """Objective value at the solution.
        Returns
        -------
        float
        """
        ...

    @property
    def current_weights(self) -> dict[str, float]:
        """Current weights by position ID.
        Returns
        -------
        dict[str, float]
        """
        ...

    @property
    def optimal_weights(self) -> dict[str, float]:
        """Optimized target weights by position ID.
        Returns
        -------
        dict[str, float]
        """
        ...

    @property
    def weight_deltas(self) -> dict[str, float]:
        """Target less current weight by position ID.
        Returns
        -------
        dict[str, float]
        """
        ...

    @property
    def implied_quantities(self) -> dict[str, float]:
        """Implied target quantities by position ID.
        Returns
        -------
        dict[str, float]
        """
        ...

    @property
    def metric_values(self) -> dict[str, float]:
        """Portfolio metric values at the solution.
        Returns
        -------
        dict[str, float]
        """
        ...

    @property
    def dual_values(self) -> dict[str, float]:
        """Dual values by constraint label when available.
        Returns
        -------
        dict[str, float]
        """
        ...

    @property
    def constraint_slacks(self) -> dict[str, float]:
        """Constraint slack values by constraint label.
        Returns
        -------
        dict[str, float]
        """
        ...

    @property
    def turnover(self) -> float:
        """Total turnover implied by the solution.
        Returns
        -------
        float
        """
        ...

    def to_trade_list(self) -> list[TradeSpec]:
        """Convert weight deltas into trade specifications.
        Returns
        -------
        list[TradeSpec]
        """
        ...

    def new_position_trades(self) -> list[TradeSpec]:
        """Return trade specs for new candidate positions only.
        Returns
        -------
        list[TradeSpec]
        """
        ...

    def binding_constraints(self) -> list[tuple[str, float]]:
        """Return constraints with near-zero slack as ``(label, slack)`` pairs.
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
    """Typed sibling of :func:`optimize_portfolio`.

    Accepts a typed :class:`PortfolioOptimizationSpec` and returns a typed
    :class:`PortfolioOptimizationResult` rather than JSON strings.
    """
    ...

# ---------------------------------------------------------------------------
# Factor Sensitivity
# ---------------------------------------------------------------------------

class SensitivityMatrix:
    """Positions-by-factors sensitivity matrix.

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
        """Ordered position identifiers (row axis).
        Returns
        -------
        list[str]
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """Ordered factor identifiers (column axis).
        Returns
        -------
        list[str]
        """
        ...

    @property
    def n_positions(self) -> int:
        """Number of positions (rows).
        Returns
        -------
        int
        """
        ...

    @property
    def n_factors(self) -> int:
        """Number of factors (columns).
        Returns
        -------
        int
        """
        ...

    def delta(self, position_idx: int, factor_idx: int) -> float:
        """Read a single sensitivity element.

        Parameters
        ----------
        position_idx : int
            Row index.
        factor_idx : int
            Column index.

        Returns
        -------
        float
            Sensitivity value.
        """
        ...

    def position_deltas(self, position_idx: int) -> list[float]:
        """Sensitivity row for a single position across all factors.

        Parameters
        ----------
        position_idx : int
            Row index.

        Returns
        -------
        list[float]
            List of delta values, one per factor.
        """
        ...

    def factor_deltas(self, factor_idx: int) -> list[float]:
        """Sensitivity column for a single factor across all positions.

        Parameters
        ----------
        factor_idx : int
            Column index.

        Returns
        -------
        list[float]
            List of delta values, one per position.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Export as a pandas DataFrame with positions as rows and factors as columns.

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
    """P&L profile for one factor across a scenario grid.

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
        """Factor identifier.
        Returns
        -------
        str
        """
        ...

    @property
    def shifts(self) -> list[float]:
        """Scenario shift coordinates (bump-size multiples).
        Returns
        -------
        list[float]
        """
        ...

    @property
    def position_pnls(self) -> list[list[float]]:
        """Per-shift P&L vectors indexed as ``[shift_idx][position_idx]``.
        Returns
        -------
        list[list[float]]
        """
        ...

    def to_dataframe(self, position_ids: list[str]) -> pd.DataFrame:
        """Export as a pandas DataFrame with shifts as rows and positions as columns.

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
    """Compute first-order factor sensitivities using central finite differences.

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
    """Compute scenario P&L profiles via full repricing across a factor grid.

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
    """
    ...

# ---------------------------------------------------------------------------
# Risk Decomposition
# ---------------------------------------------------------------------------

class FactorRiskDecomposition:
    """Portfolio-level decomposition of total risk across factors and positions.

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
        """Total portfolio risk under the selected measure.
        Returns
        -------
        float
        """
        ...

    @property
    def measure(self) -> str:
        """Risk measure used (e.g. ``"Variance"``, ``"Volatility"``).
        Returns
        -------
        str
        """
        ...

    @property
    def residual_risk(self) -> float:
        """Residual (idiosyncratic) risk not attributed to any factor.
        Returns
        -------
        float
        """
        ...

    def factor_contributions(self) -> list[dict[str, object]]:
        """Factor-level contributions as a list of dicts.

        Each dict contains ``factor_id``, ``absolute_risk``, ``relative_risk``,
        and ``marginal_risk``.

        Returns
        -------
        list[dict[str, object]]
            List of per-factor contribution dicts.
        """
        ...

    def position_factor_contributions(self) -> list[dict[str, object]]:
        """Position x factor contributions as a list of dicts.

        Each dict contains ``position_id``, ``factor_id``, and
        ``risk_contribution``.

        Returns
        -------
        list[dict[str, object]]
            List of per-position, per-factor contribution dicts.
        """
        ...

    def to_factor_dataframe(self) -> pd.DataFrame:
        """Export factor contributions as a pandas DataFrame.

        Columns: ``factor_id``, ``absolute_risk``, ``relative_risk``,
        ``marginal_risk``.

        Returns
        -------
        pd.DataFrame
            DataFrame with one row per factor.
        """
        ...

    def to_position_factor_dataframe(self) -> pd.DataFrame:
        """Export position x factor contributions as a pandas DataFrame.

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
    """Decompose portfolio risk into factor and position contributions.

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
