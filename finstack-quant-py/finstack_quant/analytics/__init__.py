"""Performance analytics: returns, drawdowns, risk metrics, benchmarks.

Bindings for the ``finstack-quant-analytics`` Rust crate. The sole entry point is
:class:`Performance`; the remaining names are value-object results and
inputs surfaced by `Performance` methods.

Examples:
--------
>>> import finstack_quant.analytics as analytics
>>> analytics.__name__
'finstack_quant.analytics'
"""

from finstack_quant.finstack_quant import analytics as _analytics

AnalyticsError = _analytics.AnalyticsError

Performance = _analytics.Performance

LookbackReturns = _analytics.LookbackReturns
PeriodStats = _analytics.PeriodStats
BetaResult = _analytics.BetaResult
GreeksResult = _analytics.GreeksResult
RollingGreeks = _analytics.RollingGreeks
MultiFactorResult = _analytics.MultiFactorResult
DrawdownEpisode = _analytics.DrawdownEpisode
DatedSeries = _analytics.DatedSeries

__all__: list[str] = [
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
]
