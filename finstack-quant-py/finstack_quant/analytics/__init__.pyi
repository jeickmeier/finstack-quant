"""
Performance analytics: returns, drawdowns, risk metrics, and benchmarks.

The panel entry point is :class:`Performance`. Construct from a pandas price
panel (``Performance(prices_df)``), a pandas return panel
(``Performance.from_returns(returns_df, initial_date=...)``), or a :class:`PerformancePanel` of
raw lists (``Performance.from_panel(panel)``); every analytic — return / risk
scalars, drawdown statistics, rolling windows, periodic returns
(MTD / QTD / YTD / FYTD), benchmark alpha/beta, basic factor models — is a
method on the resulting instance.

The remaining classes are value-object outputs returned by `Performance`
methods (`LookbackReturns`, `PeriodStats`, etc.) and the
:class:`AnalyticsError` exception. Four scalar free functions
(:func:`sharpe`, :func:`sortino`, :func:`volatility`, :func:`max_drawdown`)
cover a single return series without a panel, and
:func:`constrained_least_squares` fits factor returns for factor-Brinson
attribution.

Examples
--------
>>> from datetime import date
>>> from finstack_quant.analytics import Performance, PerformancePanel
>>> panel = PerformancePanel(
...     dates=[date(2024, 1, 1), date(2024, 1, 2)], series=[[0.01]], kind="returns", ticker_names=["FUND"]
... )
>>> perf = Performance.from_panel(panel)
>>> perf.get_ticker_names()
['FUND']
"""

from __future__ import annotations

from finstack_quant.core import FinstackError
from finstack_quant.analytics._types import (
    BetaResult as BetaResult,
    LookbackSample as LookbackSample,
    CorrelationResult as CorrelationResult,
    DatedSeries as DatedSeries,
    DrawdownEpisode as DrawdownEpisode,
    GreeksResult as GreeksResult,
    LookbackReturns as LookbackReturns,
    MultiFactorResult as MultiFactorResult,
    PeriodStats as PeriodStats,
    RollingGreeks as RollingGreeks,
)
from finstack_quant.analytics._performance import (
    Performance as Performance,
    PerformancePanel as PerformancePanel,
    max_drawdown as max_drawdown,
    multi_factor_greeks as multi_factor_greeks,
    sharpe as sharpe,
    sortino as sortino,
    volatility as volatility,
    validate_summary as validate_summary,
)
from finstack_quant.analytics import schema as schema
from finstack_quant.analytics._regression import constrained_least_squares as constrained_least_squares

__all__ = [
    "AnalyticsError",
    "BetaResult",
    "LookbackSample",
    "CorrelationResult",
    "DatedSeries",
    "DrawdownEpisode",
    "GreeksResult",
    "LookbackReturns",
    "MultiFactorResult",
    "Performance",
    "PerformancePanel",
    "PeriodStats",
    "RollingGreeks",
    "constrained_least_squares",
    "max_drawdown",
    "multi_factor_greeks",
    "schema",
    "sharpe",
    "sortino",
    "volatility",
    "validate_summary",
]

# Errors

class AnalyticsError(FinstackError):
    """
    Analytics validation or calculation failure.

    Examples
    --------
    >>> from finstack_quant.analytics import AnalyticsError
    >>> str(AnalyticsError("invalid analytics input"))
    'invalid analytics input'
    """
