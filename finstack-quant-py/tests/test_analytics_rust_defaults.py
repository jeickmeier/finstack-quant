"""Analytics results keep Rust shapes and omitted arguments use Rust defaults.

Omitted arguments resolve to the Rust ``finstack_quant_analytics::DEFAULT_*``
constants, ``ReturnKind::default()`` and ``FiscalConfig::from_parts``. The
same cases run against WASM in
``finstack-quant-wasm/tests/facade/analytics_rust_defaults.test.mjs``.
"""

from __future__ import annotations

from datetime import date, timedelta
import json
import math

import pytest

from finstack_quant.analytics import DatedSeries, Performance, sharpe, sortino, volatility

RETURNS = [0.004 * math.sin(i) + 0.0005 * (i % 7) - 0.001 for i in range(120)]
BENCH = [0.003 * math.cos(i) + 0.0002 for i in range(120)]


def _dates(n: int) -> list[date]:
    return [date(2024, 1, 1) + timedelta(days=i) for i in range(n)]


@pytest.fixture
def perf() -> Performance:
    return Performance.from_returns_arrays(_dates(len(RETURNS)), [RETURNS, BENCH], ["FUND", "BENCH"], "BENCH")


def test_rolling_series_carry_the_rust_metric_label(perf: Performance) -> None:
    cases = [
        (perf.rolling_volatility(0, 10), "volatility"),
        (perf.rolling_sortino(0, 10), "sortino"),
        (perf.rolling_sharpe(0, 10), "sharpe"),
        (perf.rolling_returns(0, 10), "return"),
    ]
    for series, label in cases:
        assert series.value_column == label
        wire = json.loads(series.to_json())
        assert sorted(wire) == ["dates", "value_column", "values"]
        assert wire["value_column"] == label
        assert DatedSeries.from_json(series.to_json()).value_column == label


def test_dated_series_from_json_rejects_unknown_metric() -> None:
    with pytest.raises(ValueError, match="unknown variant"):
        DatedSeries.from_json('{"values": [], "dates": [], "value_column": "alpha"}')


def test_omitted_arguments_equal_the_rust_defaults(perf: Performance) -> None:
    assert sharpe(RETURNS) == sharpe(RETURNS, 0.0, 252.0)
    assert sortino(RETURNS) == sortino(RETURNS, 0.0, 252.0)
    assert volatility(RETURNS) == volatility(RETURNS, 252.0)
    assert perf.value_at_risk().equals(perf.value_at_risk(0.95))
    assert perf.mean_return().equals(perf.mean_return(True))
    assert perf.omega_ratio().equals(perf.omega_ratio(0.0))
    assert perf.sterling_ratio().equals(perf.sterling_ratio(0.0, 5))
    assert list(perf.rolling_sharpe(0).values) == list(perf.rolling_sharpe(0, 63, 0.0).values)
    assert perf.periodic_returns() == perf.periodic_returns("monthly")
    assert perf.period_stats(0).to_json() == perf.period_stats(0, "monthly").to_json()
    assert (
        perf.multi_factor_greeks(0, [BENCH]).to_json() == perf.multi_factor_greeks(0, [BENCH], "excess", 0.0).to_json()
    )
    assert perf.cagr().equals(perf.cagr("act365_25"))
    assert Performance.from_arrays(_dates(3), [[100.0, 101.0, 102.0]], ["A"]).frequency == "daily"


def test_partial_fiscal_start_follows_fiscal_config_from_parts(perf: Performance) -> None:
    ref = date(2024, 4, 29)

    def lookback(*args: int | None) -> str:
        return perf.lookback_returns(ref, *args).to_json()

    assert lookback() == lookback(1, 1)
    assert lookback(3) == lookback(3, 1)
    assert lookback(None, 20) == lookback(1, 20)
    assert perf.period_stats(0, "annual", 3).to_json() == perf.period_stats(0, "annual", 3, 1).to_json()
    with pytest.raises(ValueError, match="start_month"):
        perf.lookback_returns(ref, 13)
