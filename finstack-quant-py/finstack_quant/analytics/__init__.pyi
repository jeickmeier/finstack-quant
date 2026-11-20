"""
Performance analytics: returns, drawdowns, risk metrics, and benchmarks.

The main entry point is :class:`Performance`. Construct from a pandas or Polars
``DataFrame`` (or pandas ``Series``) of prices (``Performance(prices)``) or
returns (``Performance.from_returns(returns, initial_date=...)``); every
analytic — return / risk scalars, drawdown statistics, rolling windows, periodic
returns (MTD / QTD / YTD / FYTD), benchmark alpha/beta, basic factor models —
is a method on the resulting instance.

The remaining classes are value-object outputs returned by `Performance`
methods (`LookbackReturns`, `PeriodStats`, etc.) and the
:class:`AnalyticsError` exception. Four scalar free functions
(:func:`sharpe`, :func:`sortino`, :func:`volatility`, :func:`max_drawdown`)
cover a single return series without a DataFrame, and
:func:`constrained_least_squares` fits factor returns for factor-Brinson
attribution.

Examples
--------
>>> import pandas as pd
>>> from finstack_quant.analytics import Performance
>>> prices = pd.DataFrame(
...     {"FUND": [100.0, 101.0]},
...     index=pd.to_datetime(["2024-01-01", "2024-01-02"]),
... )
>>> perf = Performance(prices)
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
