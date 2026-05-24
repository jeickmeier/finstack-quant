"""Performance analytics: returns, drawdowns, risk metrics, benchmarks.

Bindings for the ``finstack-analytics`` Rust crate. The sole entry point is
:class:`Performance`; the remaining names are value-object results and
inputs surfaced by `Performance` methods.
"""

from __future__ import annotations

from finstack.finstack import analytics as _analytics

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
