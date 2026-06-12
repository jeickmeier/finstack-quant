"""Tests for the `Performance`-centric analytics binding.

After the analytics paredown, every analytic is a method on
:class:`Performance`. These tests construct a small panel from prices or
returns and exercise the methods that answer the five core questions:
prices→returns, return/risk metrics, periodic returns, benchmark
alpha/beta, and basic factor models.
"""

from __future__ import annotations

from datetime import date, timedelta
import math
from pathlib import Path

from finstack.statements_analytics import (
    compute_multiple,
    percentile_rank,
    regression_fair_value,
    score_relative_value,
    z_score,
)
import pandas as pd
import pytest

from finstack.analytics import (
    AnalyticsError,
    BetaResult,
    DatedSeries,
    GreeksResult,
    LookbackReturns,
    MultiFactorResult,
    Performance,
    PeriodStats,
    RollingGreeks,
)

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _daily_dates(n: int, start: date = date(2024, 1, 1)) -> list[date]:
    return [start + timedelta(days=i) for i in range(n)]


def _prices_panel() -> pd.DataFrame:
    """Two-ticker daily price panel: ACME oscillates, BENCH drifts up."""
    n = 60
    dates = _daily_dates(n)
    acme = [100.0]
    bench = [100.0]
    for i in range(1, n):
        acme.append(acme[-1] * (1.0 + (0.01 if i % 2 == 0 else -0.005)))
        bench.append(bench[-1] * (1.0 + 0.002))
    return pd.DataFrame({"ACME": acme, "BENCH": bench}, index=pd.to_datetime(dates))


def _returns_panel(prices: pd.DataFrame) -> pd.DataFrame:
    """Simple returns aligned with the price index (leading row = 0)."""
    return prices.pct_change().fillna(0.0)


@pytest.fixture
def perf_prices() -> Performance:
    return Performance(_prices_panel(), benchmark_ticker="BENCH", freq="daily")


@pytest.fixture
def perf_returns() -> Performance:
    return Performance.from_returns(
        _returns_panel(_prices_panel()),
        benchmark_ticker="BENCH",
        freq="daily",
    )


# ---------------------------------------------------------------------------
# Construction
# ---------------------------------------------------------------------------


class TestConstruction:
    def test_from_prices_dataframe(self, perf_prices: Performance) -> None:
        assert perf_prices.ticker_names == ["ACME", "BENCH"]
        assert perf_prices.benchmark_idx == 1
        assert perf_prices.freq == "daily"
        active = perf_prices.dates()
        # Returns from N prices yield N-1 active observation dates (first
        # price date is dropped because returns are pct_change).
        assert len(active) == 59
        assert active[0] == date(2024, 1, 2)

    def test_from_returns_dataframe(self, perf_returns: Performance) -> None:
        assert perf_returns.ticker_names == ["ACME", "BENCH"]
        assert perf_returns.benchmark_idx == 1
        assert len(perf_returns.dates()) == 60

    def test_from_arrays(self) -> None:
        dates = _daily_dates(5)
        prices = [[100.0, 101.0, 102.0, 103.0, 104.0], [50.0, 50.5, 51.0, 51.5, 52.0]]
        perf = Performance.from_arrays(dates, prices, ["A", "B"])
        assert perf.ticker_names == ["A", "B"]
        assert len(perf.cagr()) == 2

    def test_from_returns_arrays(self) -> None:
        dates = _daily_dates(4)
        returns = [[0.01, -0.02, 0.015, 0.0], [0.005, -0.01, 0.0, 0.005]]
        perf = Performance.from_returns_arrays(
            dates,
            returns,
            ["A", "B"],
            benchmark_ticker="B",
        )
        assert perf.benchmark_idx == 1
        assert len(perf.cagr()) == 2

    def test_prices_and_returns_paths_agree_on_volatility(self) -> None:
        """Prices and returns paths should agree on volatility on the same window.

        Constructing a `Performance` from prices and from the returns of those
        prices must produce identical volatility once both objects are restricted
        to the same active window.
        """
        prices = _prices_panel()
        returns = _returns_panel(prices)
        # Drop the leading synthetic zero so the active windows match exactly.
        returns_no_lead = returns.iloc[1:]

        perf_p = Performance(prices, benchmark_ticker="BENCH")
        perf_p.reset_date_range(returns_no_lead.index[0].date(), prices.index[-1].date())

        perf_r = Performance.from_returns(returns_no_lead, benchmark_ticker="BENCH")

        vol_p = perf_p.volatility(annualize=False)
        vol_r = perf_r.volatility(annualize=False)
        for a, b in zip(vol_p, vol_r, strict=False):
            assert a == pytest.approx(b, rel=1e-12, abs=1e-12)


# ---------------------------------------------------------------------------
# Return / risk metrics
# ---------------------------------------------------------------------------


class TestReturnRiskMetrics:
    def test_cagr_returns_one_per_ticker(self, perf_prices: Performance) -> None:
        values = perf_prices.cagr()
        assert len(values) == 2
        assert all(isinstance(v, float) for v in values)

    def test_volatility_positive_for_oscillating_series(self, perf_prices: Performance) -> None:
        vols = perf_prices.volatility(annualize=True)
        assert vols[0] > 0.0  # ACME oscillates
        assert vols[1] >= 0.0  # BENCH drifts smoothly

    def test_sharpe_sortino_finite(self, perf_prices: Performance) -> None:
        for values in [perf_prices.sharpe(0.0), perf_prices.sortino(0.0)]:
            assert len(values) == 2
            assert all(not math.isnan(v) for v in values)

    def test_max_drawdown_non_positive(self, perf_prices: Performance) -> None:
        for dd in perf_prices.max_drawdown():
            assert dd <= 0.0

    def test_tail_metrics_finite(self, perf_prices: Performance) -> None:
        for getter in (perf_prices.value_at_risk, perf_prices.expected_shortfall):
            values = getter(0.95)
            assert len(values) == 2
            assert all(not math.isnan(v) for v in values)

    def test_higher_moments_finite(self, perf_prices: Performance) -> None:
        for getter in (perf_prices.skewness, perf_prices.kurtosis):
            values = getter()
            assert len(values) == 2
            assert all(not math.isnan(v) for v in values)

    def test_summary_to_dataframe_has_one_row_per_ticker(self, perf_prices: Performance) -> None:
        summary = perf_prices.summary_to_dataframe()
        assert list(summary.index) == ["ACME", "BENCH"]
        assert "cagr" in summary.columns
        assert "sharpe" in summary.columns
        assert "max_drawdown" in summary.columns


# ---------------------------------------------------------------------------
# Periodic returns
# ---------------------------------------------------------------------------


class TestPeriodicReturns:
    def test_lookback_returns_returns_per_ticker_vectors(self, perf_prices: Performance) -> None:
        lb = perf_prices.lookback_returns(date(2024, 2, 29))
        assert isinstance(lb, LookbackReturns)
        assert len(lb.mtd) == 2
        assert len(lb.qtd) == 2
        assert len(lb.ytd) == 2

    def test_lookback_with_fiscal_month(self, perf_prices: Performance) -> None:
        lb = perf_prices.lookback_returns(date(2024, 2, 29), fiscal_year_start_month=4)
        assert lb.fytd is not None
        assert len(lb.fytd) == 2

    def test_lookback_rejects_invalid_fiscal_month(self, perf_prices: Performance) -> None:
        with pytest.raises(AnalyticsError, match="Invalid"):
            perf_prices.lookback_returns(date(2024, 2, 29), fiscal_year_start_month=13)

    def test_period_stats_monthly(self, perf_prices: Performance) -> None:
        stats = perf_prices.period_stats(0, agg_freq="monthly")
        assert isinstance(stats, PeriodStats)
        assert 0.0 <= stats.win_rate <= 1.0

    def test_rolling_returns_matches_dated_series_shape(self, perf_prices: Performance) -> None:
        rr = perf_prices.rolling_returns(0, 5)
        assert isinstance(rr, DatedSeries)
        assert len(rr.values) == len(rr.dates)
        assert len(rr.values) > 0


# ---------------------------------------------------------------------------
# Benchmark comparison
# ---------------------------------------------------------------------------


class TestBenchmark:
    def test_beta_returns_per_ticker(self, perf_prices: Performance) -> None:
        results = perf_prices.beta()
        assert len(results) == 2
        assert all(isinstance(r, BetaResult) for r in results)

    def test_greeks_returns_per_ticker(self, perf_prices: Performance) -> None:
        results = perf_prices.greeks()
        assert len(results) == 2
        assert all(isinstance(r, GreeksResult) for r in results)

    def test_rolling_greeks(self, perf_prices: Performance) -> None:
        rg = perf_prices.rolling_greeks(0, window=10)
        assert isinstance(rg, RollingGreeks)
        assert len(rg.alphas) == len(rg.betas)
        assert len(rg.dates) == len(rg.alphas)

    def test_rolling_window_metrics(self, perf_prices: Performance) -> None:
        rs = perf_prices.rolling_sharpe(0, window=10)
        rso = perf_prices.rolling_sortino(0, window=10)
        rv = perf_prices.rolling_volatility(0, window=10)
        assert isinstance(rs, DatedSeries)
        assert isinstance(rso, DatedSeries)
        assert isinstance(rv, DatedSeries)
        assert len(rs.values) == len(rs.dates)

    def test_information_and_tracking(self, perf_prices: Performance) -> None:
        te = perf_prices.tracking_error()
        ir = perf_prices.information_ratio()
        assert len(te) == 2
        assert len(ir) == 2

    def test_reset_bench_ticker_changes_index(self, perf_prices: Performance) -> None:
        perf_prices.reset_bench_ticker("ACME")
        assert perf_prices.benchmark_idx == 0


# ---------------------------------------------------------------------------
# Multi-factor
# ---------------------------------------------------------------------------


class TestMultiFactor:
    def test_multi_factor_returns_structured_result(self, perf_prices: Performance) -> None:
        n = len(perf_prices.dates())
        factor1 = [0.001 * (i % 5) for i in range(n)]
        factor2 = [0.002 if i % 3 == 0 else -0.001 for i in range(n)]
        result = perf_prices.multi_factor_greeks(0, [factor1, factor2])
        assert isinstance(result, MultiFactorResult)
        assert len(result.betas) == 2
        assert 0.0 <= result.r_squared <= 1.0

    def test_multi_factor_rejects_non_finite_inputs(self, perf_prices: Performance) -> None:
        n = len(perf_prices.dates())
        bad = [float("nan")] + [0.001] * (n - 1)
        with pytest.raises(AnalyticsError):
            perf_prices.multi_factor_greeks(0, [bad])


# ---------------------------------------------------------------------------
# Date window mutation
# ---------------------------------------------------------------------------


class TestDateRange:
    def test_reset_date_range_narrows_active_grid(self, perf_prices: Performance) -> None:
        perf_prices.reset_date_range(date(2024, 1, 10), date(2024, 1, 20))
        active = perf_prices.active_dates()
        assert active[0] == date(2024, 1, 10)
        assert active[-1] == date(2024, 1, 20)
        # `dates()` keeps Rust semantics: the full constructed grid.
        assert len(perf_prices.dates()) == 59


# ---------------------------------------------------------------------------
# Stubs
# ---------------------------------------------------------------------------


class TestStubs:
    """Smoke tests that the `.pyi` stays in sync with the registered API."""

    def test_stub_lists_performance_first(self) -> None:
        stub_path = Path(__file__).resolve().parents[1] / "finstack" / "analytics" / "__init__.pyi"
        stub_text = stub_path.read_text()
        assert "class Performance:" in stub_text
        assert "from_returns" in stub_text
        assert "rolling_returns" in stub_text
        assert '"Performance"' in stub_text

    def test_stub_drops_legacy_freestanding_functions(self) -> None:
        stub_path = Path(__file__).resolve().parents[1] / "finstack" / "analytics" / "__init__.pyi"
        stub_text = stub_path.read_text()
        # These freestanding functions were deleted; ensure the stub matches the runtime.
        assert "def estimate_ruin" not in stub_text
        assert "def fit_garch11" not in stub_text
        assert "def rolling_var_forecasts" not in stub_text
        assert "def classify_breaches" not in stub_text


# ---------------------------------------------------------------------------
# Cross-binding sanity (comps live in statements_analytics)
# ---------------------------------------------------------------------------


class TestCompsBindings:
    def test_compute_multiple(self) -> None:
        metrics = {"enterprise_value": 8_500.0, "ebitda": 1_000.0}
        assert compute_multiple(metrics, "EvEbitda") == pytest.approx(8.5)

    def test_regression_fair_value(self) -> None:
        result = regression_fair_value([1.0, 2.0, 3.0, 4.0], [3.0, 5.0, 7.0, 9.0], 3.0, 10.0)
        assert result["fitted_value"] == pytest.approx(7.0)
        assert result["residual"] == pytest.approx(3.0)

    def test_percentile_rank(self) -> None:
        assert percentile_rank(250.0, [100.0, 200.0, 300.0, 400.0, 500.0]) == pytest.approx(0.4)
        assert percentile_rank(100.0, []) is None

    def test_z_score(self) -> None:
        assert z_score(3.0, [1.0, 2.0, 3.0, 4.0, 5.0]) == pytest.approx(0.0)
        assert z_score(1.0, [1.0]) is None
        assert z_score(5.0, [5.0, 5.0, 5.0]) is None

    def test_score_relative_value(self) -> None:
        subject = {"leverage": 2.0, "oas_bps": 250.0}
        peers = [
            {"leverage": 1.0, "oas_bps": 100.0},
            {"leverage": 2.0, "oas_bps": 200.0},
            {"leverage": 3.0, "oas_bps": 300.0},
        ]
        result = score_relative_value(
            subject,
            peers,
            [{"label": "Spread vs Leverage", "y": "oas_bps", "x": ["leverage"], "weight": 1.0}],
        )
        assert result["composite_score"] > 0.0

    def test_peer_stats_uses_rust_field_names(self) -> None:
        from finstack.statements_analytics import peer_stats

        stats = peer_stats([1.0, 2.0, 3.0, 4.0, 5.0])
        assert stats["count"] == 5
        assert "n" not in stats
        assert stats["iqr"] == pytest.approx(stats["q3"] - stats["q1"])
        assert peer_stats([]) == {}

    def test_score_relative_value_direction_flips_sign(self) -> None:
        subject = {"pe": 30.0}
        peers = [{"pe": 10.0}, {"pe": 15.0}, {"pe": 20.0}]
        cheap_convention = score_relative_value(subject, peers, [("pe", 1.0)])
        rich_convention = score_relative_value(
            subject, peers, [{"y": "pe", "weight": 1.0, "direction": "higher_is_rich"}]
        )
        # High multiple vs peers: rich (negative) under higher_is_rich, cheap
        # (positive) under the default higher_is_cheap convention.
        assert cheap_convention["composite_score"] > 0.0
        assert rich_convention["composite_score"] < 0.0
        assert rich_convention["composite_score"] == pytest.approx(-cheap_convention["composite_score"])

    def test_score_relative_value_rejects_unknown_direction(self) -> None:
        with pytest.raises(ValueError, match="unknown direction"):
            score_relative_value(
                {"pe": 30.0},
                [{"pe": 10.0}, {"pe": 15.0}, {"pe": 20.0}],
                [{"y": "pe", "direction": "down_is_up"}],
            )

    def test_score_relative_value_multiple_extractor(self) -> None:
        subject = {"enterprise_value": 12_000.0, "ebitda": 1_000.0}
        peers = [
            {"enterprise_value": 8_000.0, "ebitda": 1_000.0},
            {"enterprise_value": 9_000.0, "ebitda": 1_000.0},
            {"enterprise_value": 10_000.0, "ebitda": 1_000.0},
        ]
        result = score_relative_value(subject, peers, [("multiple:ev_ebitda", 1.0)])
        assert result["peer_count"] == 3
        assert "multiple:ev_ebitda" in result["by_dimension"]

    def test_non_numeric_metric_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match="oas_bps"):
            score_relative_value(
                {"oas_bps": "wide"},
                [{"oas_bps": 100.0}],
                [("oas_bps", 1.0)],
            )

    def test_none_metric_treated_as_missing(self) -> None:
        metrics = {"enterprise_value": 8_500.0, "ebitda": 1_000.0, "revenue": None}
        assert compute_multiple(metrics, "EvEbitda") == pytest.approx(8.5)
