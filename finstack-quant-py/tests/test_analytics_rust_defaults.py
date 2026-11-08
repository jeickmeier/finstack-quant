"""Analytics require explicit financial assumptions and retain optional context."""

from datetime import date, timedelta
import json
import math

import pytest

from finstack_quant.analytics import DatedSeries, Performance, PerformancePanel, sharpe, sortino, volatility

RETURNS = [0.004 * math.sin(i) + 0.0005 * (i % 7) - 0.001 for i in range(120)]
BENCH = [0.003 * math.cos(i) + 0.0002 for i in range(120)]


def _dates(n: int) -> list[date]:
    return [date(2024, 1, 1) + timedelta(days=i) for i in range(n)]


@pytest.fixture
def perf() -> Performance:
    return Performance.from_panel(
        PerformancePanel(
            dates=_dates(len(RETURNS) + 1),
            series=[RETURNS, BENCH],
            kind="returns",
            ticker_names=["FUND", "BENCH"],
            benchmark_ticker="BENCH",
            frequency="daily",
        )
    )


def test_rolling_series_carry_the_rust_metric_label(perf: Performance) -> None:
    cases = [
        (perf.rolling_volatility(0, 10), "volatility"),
        (perf.rolling_sortino(0, 10, 0.0), "sortino"),
        (perf.rolling_sharpe(0, 10, 0.0), "sharpe"),
        (perf.rolling_returns(0, 10), "return"),
    ]
    for series, label in cases:
        assert series.value_column == label
        wire = json.loads(series.to_json())
        assert sorted(wire) == ["dates", "value_column", "values"]
        assert wire["value_column"] == label
        assert DatedSeries.from_json(series.to_json()).value_column == label


def test_financial_assumptions_cannot_be_omitted(perf: Performance) -> None:
    for call in (
        lambda: sharpe(RETURNS),
        lambda: sortino(RETURNS),
        lambda: volatility(RETURNS),
        perf.value_at_risk,
        perf.mean_return,
        perf.omega_ratio,
        perf.sterling_ratio,
        lambda: perf.rolling_sharpe(0),
        perf.periodic_returns,
        lambda: perf.period_stats(0),
        lambda: perf.multi_factor_greeks(0, [BENCH]),
        perf.to_dataframe,
    ):
        with pytest.raises(TypeError):
            call()


def test_optional_context_remains_absent_until_required() -> None:
    panel = Performance.from_panel(
        PerformancePanel(dates=_dates(3), series=[[100.0, 101.0, 102.0]], kind="prices", ticker_names=["A"])
    )
    assert panel.get_frequency() is None
    assert panel.get_benchmark_idx() is None
    assert panel.cagr().equals(panel.cagr("act_365f"))
    assert math.isfinite(panel.volatility(False).iloc[0])
    with pytest.raises(ValueError, match="frequency"):
        panel.volatility(True)
    with pytest.raises(ValueError, match="benchmark"):
        panel.beta()


def test_dated_series_from_json_rejects_unknown_metric() -> None:
    with pytest.raises(ValueError, match="unknown variant"):
        DatedSeries.from_json('{"values": [], "dates": [], "value_column": "alpha"}')
