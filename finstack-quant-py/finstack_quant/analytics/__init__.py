"""Performance analytics: returns, drawdowns, risk metrics, benchmarks.

Bindings for the ``finstack-quant-analytics`` Rust crate. The main entry point is
:class:`Performance`; the remaining names are value-object results surfaced by
`Performance` methods, four scalar free functions (:func:`sharpe`,
:func:`sortino`, :func:`volatility`, :func:`max_drawdown`) for a single return
series, :func:`constrained_least_squares` for factor-Brinson factor returns,
and the :class:`AnalyticsError` exception.

Examples:
--------
>>> from datetime import date
>>> from finstack_quant.analytics import Performance, PerformancePanel
>>> panel = PerformancePanel(
...     dates=[date(2024, 1, 1), date(2024, 1, 2)], series=[[0.01, 0.02]], kind="returns", ticker_names=["FUND"]
... )
>>> perf = Performance.from_panel(panel)
>>> perf.get_ticker_names()
['FUND']
"""

from finstack_quant.finstack_quant import analytics as _analytics

AnalyticsError = _analytics.AnalyticsError

Performance = _analytics.Performance
PerformancePanel = _analytics.PerformancePanel

LookbackReturns = _analytics.LookbackReturns
PeriodStats = _analytics.PeriodStats
BetaResult = _analytics.BetaResult
GreeksResult = _analytics.GreeksResult
RollingGreeks = _analytics.RollingGreeks
MultiFactorResult = _analytics.MultiFactorResult
DrawdownEpisode = _analytics.DrawdownEpisode
DatedSeries = _analytics.DatedSeries

constrained_least_squares = _analytics.constrained_least_squares
max_drawdown = _analytics.max_drawdown
sharpe = _analytics.sharpe
sortino = _analytics.sortino
volatility = _analytics.volatility

__all__: list[str] = [
    "AnalyticsError",
    "BetaResult",
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
    "sharpe",
    "sortino",
    "volatility",
]
