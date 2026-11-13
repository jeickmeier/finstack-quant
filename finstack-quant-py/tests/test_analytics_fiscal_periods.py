"""Fiscal aggregation agrees with shared boundary groups at every frequency."""

from datetime import date, timedelta
import json
import math
from pathlib import Path

import pytest

from finstack_quant.analytics import AnalyticsError, Performance, PerformancePanel
from finstack_quant.core.dates import FiscalConfig
from tests.analytics_helpers import valuation_grid

CASES = json.loads(
    (Path(__file__).resolve().parents[2] / "finstack-quant/analytics/tests/data/fiscal_buckets.json").read_text()
)


@pytest.mark.parametrize("case", CASES, ids=[case["name"] for case in CASES])
def test_fiscal_period_stats_matches_boundary_groups(case: dict) -> None:
    perf = Performance.from_panel(
        PerformancePanel(
            dates=valuation_grid([date.fromisoformat(value) for value in case["dates"]]),
            series=[case["returns"]],
            kind="returns",
            ticker_names=["FUND"],
            frequency="daily",
            benchmark_ticker="FUND",
        )
    )
    for frequency, groups in case["groups"].items():
        expected = [math.prod(1.0 + case["returns"][index] for index in group) - 1.0 for group in groups]
        stats = perf.period_stats("FUND", frequency, FiscalConfig(case["start_month"], case["start_day"]))
        assert stats.best == pytest.approx(max(expected), abs=1e-12)
        assert stats.worst == pytest.approx(min(expected), abs=1e-12)
        assert stats.avg_return == pytest.approx(sum(expected) / len(expected), abs=1e-12)


def test_unrepresentable_calendar_boundaries_raise_analytics_error() -> None:
    perf = Performance.from_panel(
        PerformancePanel(
            dates=valuation_grid([date(9999, 12, 30), date.max]),
            series=[[0.01, 0.02]],
            kind="returns",
            ticker_names=["FUND"],
            frequency="daily",
            benchmark_ticker="FUND",
        )
    )
    for frequency in ["daily", "weekly", "monthly", "quarterly", "semi_annual", "annual"]:
        with pytest.raises(AnalyticsError):
            perf.periodic_returns(frequency)
        with pytest.raises(AnalyticsError):
            perf.period_stats(0, frequency)
        with pytest.raises(AnalyticsError):
            perf.period_stats(0, frequency, FiscalConfig(10, 1))


def test_fiscal_period_stats_preserves_ragged_active_alignment() -> None:
    case = CASES[0]
    dates = [date.fromisoformat(value) for value in case["dates"]]
    returns = case["returns"]
    ragged = [float("nan"), *returns[1:-1], float("nan")]
    perf = Performance.from_panel(
        PerformancePanel(
            dates=valuation_grid(dates),
            series=[returns, ragged],
            kind="returns",
            ticker_names=["BENCH", "FUND"],
            frequency="daily",
            benchmark_ticker="BENCH",
        )
    )
    perf.reset_date_range(dates[1], dates[-2])
    assert perf.get_active_dates_for_ticker("FUND") == dates[1:-1]
    expected = math.prod(1.0 + value for value in returns[1:-1]) - 1.0
    stats = perf.period_stats("FUND", "monthly", FiscalConfig(10, 1))
    assert stats.avg_return == pytest.approx(expected, abs=1e-12)


def test_omitted_fiscal_config_buckets_iso_weeks() -> None:
    dates = [date(2024, 12, 30) + timedelta(days=offset) for offset in range(9)]
    returns = [0.01, -0.02, 0.03, 0.01, -0.01, 0.02, 0.01, -0.03, 0.02]
    perf = Performance.from_panel(
        PerformancePanel(
            dates=valuation_grid(dates),
            series=[returns],
            kind="returns",
            ticker_names=["FUND"],
            frequency="daily",
            benchmark_ticker="FUND",
        )
    )
    omitted = perf.period_stats(0, "weekly").to_json()
    assert omitted == perf.period_stats(0, "weekly", None).to_json()
    assert omitted != perf.period_stats(0, "weekly", FiscalConfig.calendar_year()).to_json()
