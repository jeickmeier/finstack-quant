"""
Performance analytics: returns, drawdowns, risk metrics, and benchmarks.

The panel entry point is :class:`Performance`. Construct from a price panel
(``Performance(prices_df)`` / ``Performance.from_arrays(...)``) or from a
return panel (``Performance.from_returns(returns_df)`` /
``Performance.from_returns_arrays(...)``); every analytic — return / risk
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
>>> from finstack_quant.analytics import Performance
>>> perf = Performance.from_returns_arrays([date(2024, 1, 1), date(2024, 1, 2)], [[0.01, 0.02]], ["FUND"])
>>> perf.get_ticker_names()
['FUND']
"""

from __future__ import annotations

from finstack_quant.core import FinstackError
from finstack_quant.analytics._types import (
    BetaResult as BetaResult,
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
    max_drawdown as max_drawdown,
    sharpe as sharpe,
    sortino as sortino,
    volatility as volatility,
)
from finstack_quant.analytics._regression import constrained_least_squares as constrained_least_squares

__all__ = [
    "AnalyticsError",
    "BetaResult",
    "DatedSeries",
    "DrawdownEpisode",
    "GreeksResult",
    "LookbackReturns",
    "MultiFactorResult",
    "Performance",
    "PeriodStats",
    "RollingGreeks",
    "constrained_least_squares",
    "max_drawdown",
    "sharpe",
    "sortino",
    "volatility",
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
