"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/optimization/optimize.rs``."""

from __future__ import annotations

from finstack_quant.core.market_data import MarketContext
from finstack_quant.portfolio._container import Portfolio
from finstack_quant.portfolio._optimization.spec_result import PortfolioOptimizationResult, PortfolioOptimizationSpec

def optimize_portfolio(
    spec: PortfolioOptimizationSpec,
    market: MarketContext | str,
) -> PortfolioOptimizationResult:
    """
    Optimize portfolio weights using the LP-based optimizer.

    Parameters
    ----------
    spec : PortfolioOptimizationSpec
        Typed portfolio definition, objective, constraints, and solver policy.
    market : MarketContext or str
        Market context object or JSON used to value positions and calculate metrics.

    Returns
    -------
    PortfolioOptimizationResult
        Typed optimal weights, trades, constraint values, and diagnostics.

    Raises
    ------
    TypeError
        If ``market`` is neither a ``MarketContext`` nor a JSON string.
    ValueError
        If supplied market JSON is malformed or schema-incompatible.
    PortfolioError
        If the embedded portfolio specification cannot be constructed, a
        constraint is invalid (non-finite inputs, a negative budget or turnover
        cap, ``min > max`` weight bounds), ``unit_scaling`` weighting has no
        explicit budget, candidate IDs collide with existing positions or one
        another, or an operational optimization failure occurs before a result
        is available.
    KeyError
        If a required position metric cannot be valued for lack of market
        data, or a required base-currency conversion is unavailable.
    RuntimeError
        If a required position metric fails to compute.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import (
    ...     MetricExpr,
    ...     Objective,
    ...     PerPositionMetric,
    ...     PortfolioError,
    ...     PortfolioOptimizationSpec,
    ...     optimize_portfolio,
    ... )
    >>> objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
    >>> portfolio = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> spec = PortfolioOptimizationSpec.new(portfolio, objective)
    >>> try:
    ...     optimize_portfolio(spec, MarketContext())
    ... except PortfolioError as exc:
    ...     print("no decision variables" in str(exc))
    True
    """
    ...

def rebalance_from_spec(
    spec: PortfolioOptimizationSpec,
    result: PortfolioOptimizationResult,
) -> Portfolio:
    """
    Rebalance a spec's portfolio to an optimization result.

    Mirrors Rust ``optimization::rebalance_from_spec`` (WASM
    ``rebalanceFromSpec``): held positions take ``result.implied_quantities``
    and trade-universe candidates with a non-negligible target weight and
    quantity are added as new positions. The same path works for results
    rebuilt with ``from_json`` or unpickled. Retain the matching original specification.

    Parameters
    ----------
    spec : PortfolioOptimizationSpec
        The specification passed to :func:`optimize_portfolio`.
    result : PortfolioOptimizationResult
        The result :func:`optimize_portfolio` returned for ``spec``.

    Returns
    -------
    Portfolio
        The rebalanced, validated portfolio.

    Raises
    ------
    PortfolioError
        If the solution is infeasible, the result names a position that is
        neither in the spec portfolio nor a trade-universe candidate (a result
        paired with the wrong spec), or portfolio validation fails.

    Examples
    --------
    >>> from finstack_quant.portfolio import (
    ...     MetricExpr,
    ...     Objective,
    ...     PerPositionMetric,
    ...     PortfolioError,
    ...     PortfolioOptimizationResult,
    ...     PortfolioOptimizationSpec,
    ...     rebalance_from_spec,
    ... )
    >>> objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
    >>> portfolio = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> spec = PortfolioOptimizationSpec.new(portfolio, objective)
    >>> foreign = PortfolioOptimizationResult.from_json(
    ...     '{"meta":{"numeric_mode":"f64","rounding":{"mode":"bankers","ingest_scale_by_currency":{},"output_scale_by_currency":{},"version":1}},"schema_version":1,"status":"optimal",'
    ...     '"objective_value":0.0,"turnover":0.0,"optimal_weights":{"X":1.0},"current_weights":{},'
    ...     '"weight_deltas":{},"implied_quantities":{"X":1.0},"trades":[],'
    ...     '"constraint_slacks":{},"binding_constraints":[]}'
    ... )
    >>> try:
    ...     rebalance_from_spec(spec, foreign)
    ... except PortfolioError as exc:
    ...     print("neither in the spec portfolio" in str(exc))
    True
    """
    ...
