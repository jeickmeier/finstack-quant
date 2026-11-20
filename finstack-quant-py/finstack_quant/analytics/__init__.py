"""Performance analytics: returns, drawdowns, risk metrics, benchmarks.

Bindings for the ``finstack-quant-analytics`` Rust crate. The main entry point is
:class:`Performance`; the remaining names are value-object results surfaced by
`Performance` methods, four scalar free functions (:func:`sharpe`,
:func:`sortino`, :func:`volatility`, :func:`max_drawdown`) for a single return
series, :func:`constrained_least_squares` for factor-Brinson factor returns,
and the :class:`AnalyticsError` exception.

Examples:
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

from finstack_quant.finstack_quant import analytics as _analytics

AnalyticsError = _analytics.AnalyticsError

Performance = _analytics.Performance

LookbackReturns = _analytics.LookbackReturns
PeriodStats = _analytics.PeriodStats
BetaResult = _analytics.BetaResult
LookbackSample = _analytics.LookbackSample
CorrelationResult = _analytics.CorrelationResult
GreeksResult = _analytics.GreeksResult
RollingGreeks = _analytics.RollingGreeks
MultiFactorResult = _analytics.MultiFactorResult
DrawdownEpisode = _analytics.DrawdownEpisode
DatedSeries = _analytics.DatedSeries

constrained_least_squares = _analytics.constrained_least_squares
max_drawdown = _analytics.max_drawdown
multi_factor_greeks = _analytics.multi_factor_greeks
schema = _analytics.schema
sharpe = _analytics.sharpe
sortino = _analytics.sortino
volatility = _analytics.volatility
validate_summary = _analytics.validate_summary

__all__: list[str] = [
    "AnalyticsError",
    "BetaResult",
    "CorrelationResult",
    "DatedSeries",
    "DrawdownEpisode",
    "GreeksResult",
    "LookbackReturns",
    "LookbackSample",
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
    "validate_summary",
    "volatility",
]
