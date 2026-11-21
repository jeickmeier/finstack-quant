"""Tests for the `Performance`-centric analytics binding.

After the analytics paredown, every analytic is a method on
:class:`Performance`. These tests construct a small panel from prices or
returns and exercise the methods that answer the five core questions:
prices→returns, return/risk metrics, periodic returns, benchmark
alpha/beta, and basic factor models.
"""

from __future__ import annotations

from datetime import UTC, date, datetime, timedelta
import math
import pickle

import pandas as pd
import polars as pl
import pyarrow as pa
import pytest

import finstack_quant.analytics as analytics_mod
from finstack_quant.analytics import (
    AnalyticsError,
    BetaResult,
    DatedSeries,
    GreeksResult,
    LookbackReturns,
    MultiFactorResult,
    Performance,
    PeriodStats,
    RollingGreeks,
    max_drawdown,
    multi_factor_greeks,
    sharpe,
    sortino,
    volatility,
)
from finstack_quant.core.dates import DayCountContext, FiscalConfig
from finstack_quant.core.table import LabeledSeries
from tests.analytics_helpers import opening_date
from tests.stub_helpers import stub_text as read_stub_text


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


def test_beta_confidence_interval_uses_actual_degrees_of_freedom() -> None:
    benchmark = [-0.02 + i * 0.001 for i in range(42)]
    fund = [1.4 * value + (0.002 if i % 2 == 0 else -0.0015) for i, value in enumerate(benchmark)]
    grid = _daily_dates(42)
    perf = Performance.from_returns(
        pd.DataFrame({"FUND": fund, "BENCH": benchmark}, index=grid),
        initial_date=opening_date(grid),
        benchmark_ticker="BENCH",
        frequency="daily",
    )
    result = perf.beta()[0]
    assert (result.ci_upper - result.beta) / result.std_err == pytest.approx(2.0210753903062733, abs=1e-9)


def test_native_frames_preserve_infinite_period_statistics() -> None:
    grid = _daily_dates(4)
    perf = Performance.from_returns(
        pd.DataFrame({"FUND": [0.01, 0.02, 0.03, 0.04]}, index=grid),
        initial_date=opening_date(grid),
        frequency="daily",
        benchmark_ticker="FUND",
    )
    stats = perf.period_stats(0, aggregation_frequency="daily")
    frame = stats.to_pandas()
    assert math.isinf(frame["profit_factor"].iloc[0])
    for name in ("payoff_ratio", "cpc_index"):
        assert math.isnan(frame[name].iloc[0])
        assert frame[name].dtype == "float64"
    assert frame["consecutive_wins"].dtype == "int64"
    assert frame["consecutive_losses"].dtype == "int64"
    assert list(frame.columns) == list(stats.to_series().index)
    assert stats.to_polars().equals(pl.DataFrame(stats))
    assert pl.from_arrow(stats.to_pyarrow()).equals(pl.from_arrow(pa.table(stats)))


def test_degenerate_regression_frames_keep_numeric_nan_columns() -> None:
    grid = _daily_dates(4)
    perf = Performance.from_returns(
        pd.DataFrame({"FUND": [0.01, -0.02, 0.03, 0.04], "BENCH": [0.01] * 4}, index=grid),
        initial_date=opening_date(grid),
        benchmark_ticker="BENCH",
        frequency="daily",
    )
    for res in (perf.beta()[0], perf.greeks(risk_free_rate=0.0)[0]):
        frame = res.to_pandas()
        # The estimates are NaN; the sample description is always reported.
        described = {"n_obs", "confidence_level"}
        estimates = frame.drop(columns=described & set(frame.columns))
        assert estimates.isna().all().all()
        assert all(dtype == "float64" for dtype in estimates.dtypes)
        assert (frame["n_obs"] == 4).all()
        assert res.to_polars().equals(pl.DataFrame(res))
        assert pl.from_arrow(res.to_pyarrow()).equals(pl.from_arrow(pa.table(res)))


def test_drawdown_episode_frame_retains_populated_dtypes() -> None:
    perf = Performance(
        pd.DataFrame({"FUND": [100.0, 90.0, 100.0, 110.0]}, index=_daily_dates(4)),
        frequency="daily",
        benchmark_ticker="FUND",
    )
    ep = perf.drawdown_details(0, n=5)[0]
    frame = ep.to_pandas()
    assert frame["duration_days"].dtype == "int64"
    assert "truncated_at_start" not in frame
    assert frame["max_drawdown"].iloc[0] == pytest.approx(-0.10)
    assert ep.to_polars().equals(pl.DataFrame(ep))
    assert ep.to_pyarrow().equals(pa.table(ep))


def test_lookback_frame_preserves_ticker_and_metric_labels() -> None:
    grid = _daily_dates(4)
    perf = Performance.from_returns(
        pd.DataFrame({"A": [0.01, 0.02, 0.03, 0.04], "B": [-0.01, 0.03, 0.02, 0.01]}, index=grid),
        initial_date=opening_date(grid),
        frequency="daily",
        benchmark_ticker="A",
    )
    lb = perf.lookback_returns(_daily_dates(4)[-1])
    frame = lb.to_pandas()
    assert list(frame.index) == ["A", "B"]
    assert list(frame.columns) == ["mtd", "qtd", "ytd", "fytd"]
    assert lb.to_polars().equals(pl.DataFrame(lb))
    assert lb.to_pyarrow().equals(pa.table(lb))


@pytest.fixture
def perf_prices() -> Performance:
    return Performance(_prices_panel(), benchmark_ticker="BENCH", frequency="daily")


@pytest.fixture
def perf_returns() -> Performance:
    return Performance.from_returns(
        _returns_panel(_prices_panel()),
        benchmark_ticker="BENCH",
        frequency="daily",
        initial_date=(_returns_panel(_prices_panel())).index[0].date() - timedelta(days=1),
    )


# Construction


class TestConstruction:
    def test_from_prices_dataframe(self, perf_prices: Performance) -> None:
        assert perf_prices.get_ticker_names() == ["ACME", "BENCH"]
        assert perf_prices.get_benchmark_idx() == 1
        assert perf_prices.get_frequency() == "daily"
        active = perf_prices.get_dates()
        # Returns from N prices yield N-1 active observation dates (first
        # price date is dropped because returns are pct_change).
        assert len(active) == 59
        assert active[0] == date(2024, 1, 2)

    def test_from_prices_series(self) -> None:
        prices = _prices_panel()["ACME"]
        perf_named = Performance(prices, frequency="daily")
        assert perf_named.get_ticker_names() == ["ACME"]
        perf_unnamed = Performance(prices.rename(None), frequency="daily")
        assert perf_unnamed.get_ticker_names() == ["asset"]

    def test_from_returns_dataframe(self, perf_returns: Performance) -> None:
        assert perf_returns.get_ticker_names() == ["ACME", "BENCH"]
        assert perf_returns.get_benchmark_idx() == 1
        assert len(perf_returns.get_dates()) == 60

    def test_ragged_price_dataframe_trims_active_spans(self) -> None:
        dates = pd.to_datetime(_daily_dates(6))
        prices = pd.DataFrame(
            {
                "BENCH": [100.0, 101.0, 102.0, 103.0, 104.0, 105.0],
                "PORT": [math.nan, math.nan, 50.0, 55.0, 60.5, math.nan],
            },
            index=dates,
        )

        perf = Performance(prices, benchmark_ticker="BENCH", frequency="daily")
        assert perf.get_active_dates_for_ticker(1) == [date(2024, 1, 4), date(2024, 1, 5)]
        assert perf.cumulative_returns()[1] == pytest.approx([0.10, 0.21])

    def test_panel_pathway_is_removed(self) -> None:
        assert not hasattr(analytics_mod, "PerformancePanel")
        assert not hasattr(Performance, "from_panel")
        with pytest.raises(TypeError) as exc_info:
            Performance([[100.0, 101.0]])  # type: ignore[arg-type]
        assert "from_panel" not in str(exc_info.value)
        assert "PerformancePanel" not in str(exc_info.value)

    def test_from_prices_dataframe_with_date_col(self) -> None:
        dates = _daily_dates(5)
        frame = pd.DataFrame({
            "date": dates,
            "A": [100.0, 101.0, 102.0, 103.0, 104.0],
            "B": [50.0, 50.5, 51.0, 51.5, 52.0],
        })
        perf = Performance(frame, frequency="daily", benchmark_ticker="A", date_col="date")
        assert perf.get_ticker_names() == ["A", "B"]
        assert list(perf.cagr().index) == ["A", "B"]

    def test_from_returns_dataframe_with_date_col(self) -> None:
        dates = _daily_dates(4)
        frame = pd.DataFrame({
            "date": dates,
            "A": [0.01, -0.02, 0.015, 0.0],
            "B": [0.005, -0.01, 0.0, 0.005],
        })
        perf = Performance.from_returns(
            frame,
            initial_date=opening_date(dates),
            benchmark_ticker="B",
            frequency="daily",
            date_col="date",
        )
        assert perf.get_benchmark_idx() == 1
        assert list(perf.cagr().index) == ["A", "B"]

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

        perf_p = Performance(prices, benchmark_ticker="BENCH", frequency="daily")
        perf_p.reset_date_range(returns_no_lead.index[0].date(), prices.index[-1].date())

        perf_r = Performance.from_returns(
            returns_no_lead,
            benchmark_ticker="BENCH",
            initial_date=(returns_no_lead).index[0].date() - timedelta(days=1),
        )

        vol_p = perf_p.volatility(annualize=False)
        vol_r = perf_r.volatility(annualize=False)
        for ticker in perf_p.get_ticker_names():
            assert vol_p[ticker] == pytest.approx(vol_r[ticker], rel=1e-12, abs=1e-12)


# Return / risk metrics


class TestReturnRiskMetrics:
    def test_cagr_returns_one_per_ticker(self, perf_prices: Performance) -> None:
        values = perf_prices.cagr()
        assert list(values.index) == ["ACME", "BENCH"]
        assert all(isinstance(v, float) for v in values.values())

    def test_cagr_act365f_label_matches_default(self, perf_prices: Performance) -> None:
        default = perf_prices.cagr()
        labeled = perf_prices.cagr(day_count="act_365f")
        assert default["ACME"] == pytest.approx(labeled["ACME"])
        assert default["BENCH"] == pytest.approx(labeled["BENCH"])

    def test_cagr_bus252_requires_calendar(self, perf_prices: Performance) -> None:
        with pytest.raises(AnalyticsError):
            perf_prices.cagr(day_count="bus_252")
        values = perf_prices.cagr(day_count="bus_252", ctx=DayCountContext(calendar_id="nyse"))
        assert list(values.index) == ["ACME", "BENCH"]
        assert not math.isnan(values["ACME"])

    def test_parametric_var_horizon_changes_result(self, perf_prices: Performance) -> None:
        one = perf_prices.parametric_var(0.95)
        ten = perf_prices.parametric_var(0.95, horizon_periods=10)
        assert list(one.index) == ["ACME", "BENCH"]
        assert one["ACME"] != pytest.approx(ten["ACME"])

    def test_excess_returns_rejects_length_mismatch(self, perf_prices: Performance) -> None:
        with pytest.raises(AnalyticsError):
            perf_prices.excess_returns([0.0])

    def test_excess_returns_zero_rf_matches_returns(self, perf_prices: Performance) -> None:
        rf = [0.0] * len(perf_prices.get_active_dates())
        excess = perf_prices.excess_returns(rf)
        raw = perf_prices.get_returns()
        assert excess[0] == pytest.approx(raw[0])

    def test_correlation_matrix_is_square_psd_identity_diag(self) -> None:
        dates = _daily_dates(6)
        perf = Performance.from_returns(
            pd.DataFrame(
                {
                    "A": [0.01, -0.02, 0.015, 0.0, 0.01, -0.01],
                    "B": [0.005, -0.01, 0.0, 0.005, 0.02, -0.015],
                },
                index=dates,
            ),
            initial_date=opening_date(dates),
            frequency="daily",
            benchmark_ticker="A",
        )
        result = perf.correlation_matrix()
        corr = result.matrix
        assert result.n_obs == 6
        assert result.tickers == ["A", "B"]
        assert len(corr) == 2
        assert len(corr[0]) == 2
        assert corr[0][0] == pytest.approx(1.0)
        assert corr[1][1] == pytest.approx(1.0)

    def test_volatility_positive_for_oscillating_series(self, perf_prices: Performance) -> None:
        vols = perf_prices.volatility(annualize=True)
        assert vols["ACME"] > 0.0  # ACME oscillates
        assert vols["BENCH"] >= 0.0  # BENCH drifts smoothly

    def test_sharpe_sortino_finite(self, perf_prices: Performance) -> None:
        for values in [perf_prices.sharpe(0.0), perf_prices.sortino(0.0)]:
            assert list(values.index) == ["ACME", "BENCH"]
            assert not math.isnan(values["ACME"])
            assert not math.isnan(values["BENCH"])

    def test_max_drawdown_non_positive(self, perf_prices: Performance) -> None:
        for ticker, dd in perf_prices.max_drawdown().items():
            assert dd <= 0.0, ticker

    def test_tail_metrics_finite(self, perf_prices: Performance) -> None:
        for getter in (perf_prices.value_at_risk, perf_prices.expected_shortfall):
            values = getter(0.95)
            assert list(values.index) == ["ACME", "BENCH"]
            assert all(math.isfinite(v) for v in values.values())

    def test_higher_moments_finite(self, perf_prices: Performance) -> None:
        for getter in (perf_prices.skewness, perf_prices.kurtosis):
            values = getter()
            assert list(values.index) == ["ACME", "BENCH"]
            assert all(math.isfinite(v) for v in values.values())

    def test_to_pandas_and_polars_have_one_row_per_ticker(self, perf_prices: Performance) -> None:
        summary = perf_prices.to_pandas(risk_free_rate=0.0, mar=0.0, omega_threshold=0.0, confidence=0.95)
        assert list(summary.index) == ["ACME", "BENCH"]
        assert "cagr" in summary.columns
        assert "sharpe" in summary.columns
        assert "max_drawdown" in summary.columns
        custom = perf_prices.to_pandas(risk_free_rate=0.02, confidence=0.99, mar=0.0, omega_threshold=0.0)
        assert custom.loc["ACME", "sharpe"] != pytest.approx(summary.loc["ACME", "sharpe"])
        summary_pl = perf_prices.to_polars(risk_free_rate=0.0, mar=0.0, omega_threshold=0.0, confidence=0.95)
        assert summary_pl["ticker"].to_list() == ["ACME", "BENCH"]
        assert pl.from_arrow(perf_prices.to_pyarrow(0.0, 0.0, 0.0, 0.95)).equals(
            pl.from_arrow(pa.table(perf_prices.to_arrow(0.0, 0.0, 0.0, 0.95)))
        )


# Scalar-metric Series contract


class TestScalarMetricSeries:
    """Per-ticker scalar metrics return ``LabeledSeries`` with pandas/Polars/Arrow exports."""

    def test_metric_returns_series(self, perf_prices: Performance) -> None:
        assert isinstance(perf_prices.sharpe(risk_free_rate=0.0), LabeledSeries)
        assert isinstance(perf_prices.max_drawdown_duration(), LabeledSeries)

    def test_metric_index_equals_ticker_names(self, perf_prices: Performance) -> None:
        assert list(perf_prices.volatility(annualize=True).index) == perf_prices.get_ticker_names()

    def test_metric_series_name_is_metric_name(self, perf_prices: Performance) -> None:
        for metric in ("cagr", "sharpe", "max_drawdown", "tail_ratio", "max_drawdown_duration"):
            assert (
                getattr(perf_prices, metric)(*({"sharpe": (0.0,), "tail_ratio": (0.95,)}.get(metric, ()))).name
                == metric
            )

    def test_label_access_matches_positional(self, perf_prices: Performance) -> None:
        vols = perf_prices.volatility(annualize=True)
        assert vols["ACME"] == vols.values()[0]
        assert vols["BENCH"] == vols.values()[1]

    def test_concat_yields_metric_named_columns(self, perf_prices: Performance) -> None:
        df = pd.concat(
            [perf_prices.sharpe(risk_free_rate=0.0).to_pandas(), perf_prices.sortino(mar=0.0).to_pandas()], axis=1
        )
        assert list(df.columns) == ["sharpe", "sortino"]
        assert list(df.index) == ["ACME", "BENCH"]
        assert perf_prices.sharpe(risk_free_rate=0.0).to_polars().columns == ["ticker", "sharpe"]
        assert perf_prices.sharpe(risk_free_rate=0.0).to_arrow().num_rows == 2

    def test_max_drawdown_duration_keeps_integer_dtype(self, perf_prices: Performance) -> None:
        durations = perf_prices.max_drawdown_duration()
        assert durations.to_pandas().dtype == pd.Int64Dtype()
        assert durations["ACME"] >= 0


# Periodic returns


class TestPeriodicReturns:
    def test_lookback_returns_returns_per_ticker_vectors(self, perf_prices: Performance) -> None:
        lb = perf_prices.lookback_returns(date(2024, 2, 29))
        assert isinstance(lb, LookbackReturns)
        assert len(lb.mtd) == 2
        assert len(lb.qtd) == 2
        assert len(lb.ytd) == 2
        assert len(lb.fytd) == 2
        assert lb.ticker_names == perf_prices.get_ticker_names()
        assert list(lb.to_pandas().columns) == [
            "mtd",
            "qtd",
            "ytd",
            "fytd",
        ]
        assert list(lb.to_polars().columns) == ["ticker", "mtd", "qtd", "ytd", "fytd"]
        assert "fytd_len=2" in repr(lb)
        assert "has_fytd" not in repr(lb)

    def test_lookback_with_fiscal_config(self, perf_prices: Performance) -> None:
        lb = perf_prices.lookback_returns(date(2024, 2, 29), fiscal_config=FiscalConfig(4, 1))
        assert len(lb.fytd) == 2

    def test_lookback_rejects_null_fytd_json(self) -> None:
        with pytest.raises(ValueError, match="invalid type: null"):
            LookbackReturns.from_json('{"ticker_names":[],"mtd":[],"qtd":[],"ytd":[],"fytd":null}')

    def test_lookback_rejects_non_fiscal_config(self, perf_prices: Performance) -> None:
        with pytest.raises(TypeError, match="fiscal_config"):
            perf_prices.lookback_returns(date(2024, 2, 29), fiscal_config=4)
        with pytest.raises(ValueError, match="start_month"):
            FiscalConfig(13, 1)

    def test_period_stats_monthly(self, perf_prices: Performance) -> None:
        stats = perf_prices.period_stats(0, aggregation_frequency="monthly")
        assert isinstance(stats, PeriodStats)
        assert 0.0 <= stats.win_rate <= 1.0

    def test_rolling_returns_matches_dated_series_shape(self, perf_prices: Performance) -> None:
        rr = perf_prices.rolling_returns(0, 5)
        assert isinstance(rr, DatedSeries)
        assert len(rr.values) == len(rr.dates)
        assert len(rr.values) > 0


# Benchmark comparison


class TestBenchmark:
    def test_beta_returns_per_ticker(self, perf_prices: Performance) -> None:
        results = perf_prices.beta()
        assert len(results) == 2
        assert all(isinstance(r, BetaResult) for r in results)

    def test_greeks_returns_per_ticker(self, perf_prices: Performance) -> None:
        results = perf_prices.greeks(risk_free_rate=0.0)
        assert len(results) == 2
        assert all(isinstance(r, GreeksResult) for r in results)

    def test_degenerate_beta_and_greeks_wire_round_trips_preserve_nan(self) -> None:
        grid = [date(2024, 1, 1)]
        perf = Performance.from_returns(
            pd.DataFrame({"TARGET": [0.01], "BENCH": [0.01]}, index=grid),
            initial_date=opening_date(grid),
            benchmark_ticker="BENCH",
            frequency="daily",
        )

        beta_result = perf.beta()[0]
        beta_from_json = BetaResult.from_json(beta_result.to_json())
        beta_from_pickle = pickle.loads(  # noqa: S301 - trusted in-process round trip
            pickle.dumps(beta_result)
        )
        for result in (beta_from_json, beta_from_pickle):
            assert math.isnan(result.beta)
            assert math.isnan(result.std_err)
            assert math.isnan(result.ci_lower)
            assert math.isnan(result.ci_upper)

        greeks_result = perf.greeks(risk_free_rate=0.0)[0]
        greeks_from_json = GreeksResult.from_json(greeks_result.to_json())
        greeks_from_pickle = pickle.loads(  # noqa: S301 - trusted in-process round trip
            pickle.dumps(greeks_result)
        )
        for result in (greeks_from_json, greeks_from_pickle):
            assert math.isnan(result.alpha)
            assert math.isnan(result.beta)
            assert math.isnan(result.r_squared)
            assert math.isnan(result.adjusted_r_squared)

    def test_greeks_risk_free_rate_changes_jensen_alpha(self) -> None:
        dates = _daily_dates(6)
        benchmark = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03]
        target = [2.0 * value for value in benchmark]
        perf = Performance.from_returns(
            pd.DataFrame({"TARGET": target, "BENCH": benchmark}, index=dates),
            initial_date=opening_date(dates),
            benchmark_ticker="BENCH",
            frequency="monthly",
        )

        zero_rf = perf.greeks(risk_free_rate=0.0)[0]
        nonzero_rf = perf.greeks(risk_free_rate=0.12)[0]

        assert zero_rf.alpha == pytest.approx(0.0, abs=1e-12)
        assert nonzero_rf.alpha > zero_rf.alpha

    def test_rolling_greeks(self, perf_prices: Performance) -> None:
        rg = perf_prices.rolling_greeks(0, window=10, risk_free_rate=0.0)
        assert isinstance(rg, RollingGreeks)
        assert len(rg.alphas) == len(rg.betas)
        assert len(rg.dates) == len(rg.alphas)

    def test_rolling_greeks_risk_free_rate_changes_jensen_alpha(self) -> None:
        dates = _daily_dates(8)
        benchmark = [-0.03, -0.02, -0.01, 0.0, 0.01, 0.02, 0.03, 0.04]
        target = [1.5 * value for value in benchmark]
        perf = Performance.from_returns(
            pd.DataFrame({"TARGET": target, "BENCH": benchmark}, index=dates),
            initial_date=opening_date(dates),
            benchmark_ticker="BENCH",
            frequency="monthly",
        )

        zero_rf = perf.rolling_greeks(0, window=5, risk_free_rate=0.0)
        nonzero_rf = perf.rolling_greeks(0, window=5, risk_free_rate=0.12)

        assert list(zero_rf.alphas) == pytest.approx([0.0] * len(zero_rf.alphas), abs=1e-12)
        assert all(actual > base for actual, base in zip(nonzero_rf.alphas, zero_rf.alphas, strict=True))

    def test_rolling_window_metrics(self, perf_prices: Performance) -> None:
        rs = perf_prices.rolling_sharpe(0, window=10, risk_free_rate=0.0)
        rso = perf_prices.rolling_sortino(0, window=10, mar=0.0)
        rv = perf_prices.rolling_volatility(0, window=10)
        assert isinstance(rs, DatedSeries)
        assert isinstance(rso, DatedSeries)
        assert isinstance(rv, DatedSeries)
        assert len(rs.values) == len(rs.dates)

    def test_information_and_tracking(self, perf_prices: Performance) -> None:
        te = perf_prices.tracking_error()
        ir = perf_prices.information_ratio()
        assert list(te.index) == ["ACME", "BENCH"]
        assert list(ir.index) == ["ACME", "BENCH"]
        assert not math.isnan(te["ACME"])

    def test_reset_bench_ticker_changes_index(self, perf_prices: Performance) -> None:
        perf_prices.reset_bench_ticker("ACME")
        assert perf_prices.get_benchmark_idx() == 0


# Multi-factor


class TestMultiFactor:
    def test_multi_factor_returns_structured_result(self, perf_prices: Performance) -> None:
        n = len(perf_prices.get_dates())
        factor1 = [0.001 * (i % 5) for i in range(n)]
        factor2 = [0.002 if i % 3 == 0 else -0.001 for i in range(n)]
        result = perf_prices.multi_factor_greeks(0, [factor1, factor2], risk_free_rate=0.0)
        assert isinstance(result, MultiFactorResult)
        assert len(result.betas) == 2
        assert 0.0 <= result.r_squared <= 1.0

    def test_multi_factor_dataframe_matches_active_ticker_dates(self) -> None:
        panel_dates = _daily_dates(8)
        prices = pd.DataFrame(
            {
                "ACME": [math.nan, 100.0, 102.0, 101.0, 105.0, 107.0, 106.0, math.nan],
                "BENCH": [100.0 + i for i in range(8)],
            },
            index=panel_dates,
        )
        perf_prices = Performance(prices, benchmark_ticker="BENCH", frequency="daily")
        dates = perf_prices.get_active_dates_for_ticker("ACME")
        assert dates == panel_dates[2:7]
        factor = [0.001 * (i % 5) for i in range(len(dates))]
        frame = pd.DataFrame({"factor": [*factor, "outside active sample"]}, index=[*dates, panel_dates[1]]).iloc[::-1]
        expected = perf_prices.multi_factor_greeks("ACME", [factor], 0.0)
        fit = perf_prices.multi_factor_greeks("ACME", frame, 0.0)
        assert fit.betas == pytest.approx(expected.betas)
        assert fit.alpha == pytest.approx(expected.alpha)
        with pytest.raises(KeyError, match=str(dates[0])):
            perf_prices.multi_factor_greeks("ACME", frame.drop(dates[0]), 0.0)
        duplicate = pd.concat([frame, frame.iloc[[0]]])
        with pytest.raises(ValueError, match="duplicate factor date"):
            perf_prices.multi_factor_greeks("ACME", duplicate, 0.0)
        frame.loc[dates[0], "factor"] = float("inf")
        with pytest.raises(ValueError, match="aligned factor return must be finite"):
            perf_prices.multi_factor_greeks("ACME", frame, 0.0)

    def test_multi_factor_rejects_non_finite_inputs(self, perf_prices: Performance) -> None:
        n = len(perf_prices.get_dates())
        bad = [float("nan")] + [0.001] * (n - 1)
        with pytest.raises(AnalyticsError):
            perf_prices.multi_factor_greeks(0, [bad], risk_free_rate=0.0)

    def test_multi_factor_total_zero_matches_excess(self, perf_prices: Performance) -> None:
        n = len(perf_prices.get_dates())
        factor = [0.001 * (i % 5) for i in range(n)]
        excess = multi_factor_greeks(perf_prices.get_returns_for_ticker(0), [factor], 252.0, "excess")
        total = perf_prices.multi_factor_greeks(0, [factor], risk_free_rate=0.0)
        assert excess.alpha == pytest.approx(total.alpha)
        assert float(excess.betas[0]) == pytest.approx(float(total.betas[0]))

    def test_multi_factor_rejects_unknown_kind(self, perf_prices: Performance) -> None:
        n = len(perf_prices.get_dates())
        factor = [0.001] * n
        with pytest.raises(ValueError, match="return_kind"):
            multi_factor_greeks(perf_prices.get_returns_for_ticker(0), [factor], 252.0, "jensen")
        with pytest.raises(ValueError, match="risk_free_rate"):
            multi_factor_greeks(perf_prices.get_returns_for_ticker(0), [factor], 252.0, "excess", 0.02)
        with pytest.raises(ValueError, match="risk_free_rate"):
            multi_factor_greeks(perf_prices.get_returns_for_ticker(0), [factor], 252.0, "total")


# Date window mutation


class TestDateRange:
    def test_reset_date_range_narrows_active_grid(self, perf_prices: Performance) -> None:
        perf_prices.reset_date_range(date(2024, 1, 10), date(2024, 1, 20))
        active = perf_prices.get_active_dates()
        assert active[0] == date(2024, 1, 10)
        assert active[-1] == date(2024, 1, 20)
        # `dates()` keeps Rust semantics: the full constructed grid.
        assert len(perf_prices.get_dates()) == 59


# Polars & date_col standardization


class TestPolarsAndDateCol:
    def test_polars_prices_auto_detects_date_datetime_and_tz_aware(self) -> None:
        dates = _daily_dates(5)
        pd_df = pd.DataFrame(
            {"ACME": [100.0, 102.0, 101.0, 104.0, 105.0], "BENCH": [100.0, 100.5, 101.0, 101.5, 102.0]},
            index=pd.to_datetime(dates),
        )
        expected = Performance(pd_df, benchmark_ticker="BENCH", frequency="daily")

        pl_date = pl.DataFrame({
            "obs_date": dates,
            "ACME": [100.0, 102.0, 101.0, 104.0, 105.0],
            "BENCH": [100.0, 100.5, 101.0, 101.5, 102.0],
        })
        pl_naive_dt = pl.DataFrame({
            "timestamp": [datetime(d.year, d.month, d.day, 16, 0) for d in dates],
            "ACME": [100.0, 102.0, 101.0, 104.0, 105.0],
            "BENCH": [100.0, 100.5, 101.0, 101.5, 102.0],
        })
        pl_utc_dt = pl.DataFrame({
            "timestamp": [datetime(d.year, d.month, d.day, 21, 0, tzinfo=UTC) for d in dates],
            "ACME": [100.0, 102.0, 101.0, 104.0, 105.0],
            "BENCH": [100.0, 100.5, 101.0, 101.5, 102.0],
        })

        for frame in (pl_date, pl_naive_dt, pl_utc_dt):
            actual = Performance(frame, benchmark_ticker="BENCH", frequency="daily")
            assert actual.to_json() == expected.to_json()
            assert actual.sharpe(0.0) == expected.sharpe(0.0)
            assert actual.cagr() == expected.cagr()

    def test_polars_explicit_date_col_with_iso_strings_and_multiple_date_cols(self) -> None:
        dates = _daily_dates(4)
        pd_perf = Performance(
            pd.DataFrame({"A": [100.0, 101.0, 102.0, 103.0]}, index=dates),
            frequency="daily",
        )
        pl_str = pl.DataFrame({
            "A": [100.0, 101.0, 102.0, 103.0],
            "iso_date": [d.isoformat() for d in dates],
        })
        assert Performance(pl_str, frequency="daily", date_col="iso_date").to_json() == pd_perf.to_json()

        pl_multi = pl.DataFrame({
            "trade_date": dates,
            "settle_date": [d + timedelta(days=2) for d in dates],
            "A": [100.0, 101.0, 102.0, 103.0],
        })
        with pytest.raises(ValueError, match="multiple Date/Datetime columns"):
            Performance(pl_multi, frequency="daily")
        with pytest.raises(ValueError, match="multiple Date/Datetime columns"):
            Performance(
                pl_multi.select([
                    pl.col("trade_date"),
                    pl.col("settle_date").cast(pl.Datetime),
                    pl.col("A"),
                ]),
                frequency="daily",
            )
        with pytest.raises(TypeError, match="non-numeric Polars dtype"):
            Performance(pl_multi, frequency="daily", date_col="settle_date")
        selected = Performance(
            pl_multi.select(["settle_date", "A"]),
            frequency="daily",
            date_col="settle_date",
        )
        assert selected.get_dates() == [d + timedelta(days=2) for d in dates[1:]]

    def test_polars_from_returns_and_int_promotion(self) -> None:
        dates = _daily_dates(4)
        init = opening_date(dates)
        pd_perf = Performance.from_returns(
            pd.DataFrame({"A": [0.01, -0.02, 0.03, 0.01]}, index=dates),
            initial_date=init,
            frequency="daily",
            benchmark_ticker="A",
        )
        pl_df = pl.DataFrame({
            "date": dates,
            "A": [0.01, -0.02, 0.03, 0.01],
        })
        assert (
            Performance.from_returns(
                pl_df,
                initial_date=init,
                frequency="daily",
                benchmark_ticker="A",
            ).to_json()
            == pd_perf.to_json()
        )
        assert (
            Performance.from_returns(
                pl_df,
                initial_date=init,
                frequency="daily",
                benchmark_ticker="A",
                date_col="date",
            ).to_json()
            == pd_perf.to_json()
        )
        # Integer price columns promote cleanly to float64.
        pl_int = pl.DataFrame({"date": dates, "A": [100, 101, 102, 103]})
        assert Performance(pl_int, frequency="daily").cagr()["A"] > 0.0

    def test_polars_null_and_nan_edge_gaps_and_domain_errors(self) -> None:
        dates = _daily_dates(6)
        pd_prices = pd.DataFrame(
            {
                "BENCH": [100.0, 101.0, 102.0, 103.0, 104.0, 105.0],
                "PORT": [math.nan, math.nan, 50.0, 55.0, 60.5, math.nan],
            },
            index=dates,
        )
        pl_null = pl.DataFrame({
            "date": dates,
            "BENCH": [100.0, 101.0, 102.0, 103.0, 104.0, 105.0],
            "PORT": [None, None, 50.0, 55.0, 60.5, None],
        })
        pl_nan = pl.DataFrame({
            "date": dates,
            "BENCH": [100.0, 101.0, 102.0, 103.0, 104.0, 105.0],
            "PORT": [math.nan, math.nan, 50.0, 55.0, 60.5, math.nan],
        })
        expected = Performance(pd_prices, benchmark_ticker="BENCH", frequency="daily")
        assert Performance(pl_null, benchmark_ticker="BENCH", frequency="daily").to_json() == expected.to_json()
        assert Performance(pl_nan, benchmark_ticker="BENCH", frequency="daily").to_json() == expected.to_json()

        # Interior missing and infinite values are rejected by Rust validation.
        with pytest.raises(AnalyticsError):
            Performance(
                pl.DataFrame({"date": dates[:4], "A": [100.0, None, 102.0, 103.0]}),
                frequency="daily",
            )
        with pytest.raises(AnalyticsError):
            Performance(
                pl.DataFrame({"date": dates[:3], "A": [100.0, math.inf, 102.0]}),
                frequency="daily",
            )
        # Non-numeric value columns fail with TypeError.
        for bad_col in (["a", "b", "c"], [True, False, True]):
            with pytest.raises(TypeError, match="non-numeric Polars dtype"):
                Performance(pl.DataFrame({"date": dates[:3], "A": bad_col}), frequency="daily")

    def test_polars_and_pandas_date_col_error_paths(self) -> None:
        dates = _daily_dates(3)
        with pytest.raises(ValueError, match="no Date or Datetime column"):
            Performance(pl.DataFrame({"A": [100.0, 101.0, 102.0]}))
        # pl.Time and pl.Duration are temporal in Polars but are not dates.
        with pytest.raises(ValueError, match="no Date or Datetime column"):
            Performance(
                pl.DataFrame({
                    "dur": [timedelta(days=1), timedelta(days=2), timedelta(days=3)],
                    "A": [100.0, 101.0, 102.0],
                })
            )
        with pytest.raises(KeyError, match="missing_col"):
            Performance(pl.DataFrame({"date": dates, "A": [100.0, 101.0, 102.0]}), date_col="missing_col")
        with pytest.raises(TypeError, match="Polars Series"):
            Performance(pl.Series("A", [100.0, 101.0, 102.0]))  # type: ignore[arg-type]
        with pytest.raises(TypeError, match="Polars Series"):
            Performance.from_returns(pl.Series("A", [0.01, 0.02]), initial_date=opening_date(dates))  # type: ignore[arg-type]

        pd_df = pd.DataFrame({"date": dates, "A": [100.0, 101.0, 102.0]})
        with pytest.raises(KeyError, match="missing_col"):
            Performance(pd_df, date_col="missing_col")
        pd_dup_date = pd.DataFrame(
            [[dates[0], dates[0], 100.0], [dates[1], dates[1], 101.0], [dates[2], dates[2], 102.0]],
            columns=["date", "date", "A"],
        )
        with pytest.raises(ValueError, match="duplicate date_col"):
            Performance(pd_dup_date, date_col="date")
        with pytest.raises(ValueError, match="date_col cannot be used with a pandas Series"):
            Performance(pd.Series([100.0, 101.0, 102.0], index=dates, name="A"), date_col="date")

    def test_multi_factor_polars_dataframe_and_series_inputs(self) -> None:
        panel_dates = _daily_dates(8)
        prices = pl.DataFrame({
            "obs": panel_dates,
            "ACME": [math.nan, 100.0, 102.0, 101.0, 105.0, 107.0, 106.0, math.nan],
            "BENCH": [100.0 + i for i in range(8)],
        })
        perf = Performance(prices, benchmark_ticker="BENCH", frequency="daily")
        active = perf.get_active_dates_for_ticker("ACME")
        assert active == panel_dates[2:7]
        factor = [0.001 * (i % 5) for i in range(len(active))]
        expected = perf.multi_factor_greeks("ACME", [factor], 0.0)

        # Unsorted rows + extra out-of-sample rows containing null/inf are ignored.
        factor_pl = pl.DataFrame({
            "obs": [*active, panel_dates[1], panel_dates[0]],
            "factor": [*factor, None, math.inf],
        }).reverse()
        fit = perf.multi_factor_greeks("ACME", factor_pl, 0.0)
        assert fit.betas == pytest.approx(expected.betas)
        assert fit.alpha == pytest.approx(expected.alpha)

        fit_explicit = perf.multi_factor_greeks(
            "ACME",
            factor_pl.rename({"obs": "f_date"}).with_columns(pl.col("f_date").cast(pl.String)),
            0.0,
            date_col="f_date",
        )
        assert fit_explicit.betas == pytest.approx(expected.betas)

        with pytest.raises(KeyError, match=str(active[0])):
            perf.multi_factor_greeks("ACME", factor_pl.filter(pl.col("obs") != active[0]), 0.0)
        with pytest.raises(ValueError, match="duplicate factor date"):
            perf.multi_factor_greeks("ACME", pl.concat([factor_pl, factor_pl.head(1)]), 0.0)
        with pytest.raises(ValueError, match="aligned factor return must be finite"):
            perf.multi_factor_greeks(
                "ACME",
                factor_pl.with_columns(
                    pl
                    .when(pl.col("obs") == active[0])
                    .then(pl.lit(float("inf")))
                    .otherwise(pl.col("factor"))
                    .alias("factor")
                ),
                0.0,
            )
        with pytest.raises(ValueError, match="date_col"):
            perf.multi_factor_greeks("ACME", [factor], 0.0, date_col="obs")

    def test_polars_series_scalar_and_vector_inputs(self) -> None:
        rets = [0.01, -0.02, 0.015, 0.003]
        pl_rets = pl.Series("rets", rets)
        assert sharpe(pl_rets, 0.0, 252.0) == pytest.approx(sharpe(rets, 0.0, 252.0))
        assert sortino(pl_rets, 0.0, 252.0) == pytest.approx(sortino(rets, 0.0, 252.0))
        assert volatility(pl_rets, 252.0) == pytest.approx(volatility(rets, 252.0))
        assert max_drawdown(pl_rets) == pytest.approx(max_drawdown(rets))

        with pytest.raises(AnalyticsError):
            sharpe(pl.Series("bad", [0.01, None, 0.02]), 0.0, 252.0)
        with pytest.raises(TypeError, match="non-numeric Polars dtype"):
            volatility(pl.Series("bad", ["0.01", "0.02"]), 252.0)

        perf = Performance(
            pl.DataFrame({"date": _daily_dates(5), "A": [100.0, 101.0, 99.0, 102.0, 103.0]}),
            frequency="daily",
        )
        rf_list = [0.02] * len(perf.get_active_dates())
        assert perf.excess_returns(pl.Series("rf", rf_list)) == perf.excess_returns(rf_list)

        factor = [0.0, 0.01, 0.02, -0.01]
        assert multi_factor_greeks(pl_rets, [pl.Series("f", factor)], 252.0, "excess").betas == pytest.approx(
            multi_factor_greeks(rets, [factor], 252.0, "excess").betas
        )


# Stubs


class TestStubs:
    """Smoke tests that the `.pyi` stays in sync with the registered API."""

    def test_stub_lists_performance_first(self) -> None:
        stub_text = read_stub_text("finstack_quant.analytics")
        assert "class Performance:" in stub_text
        assert "from_returns" in stub_text
        assert "to_arrow" in stub_text
        assert "rolling_returns" in stub_text
        assert '"Performance"' in stub_text

    def test_stub_drops_legacy_freestanding_functions(self) -> None:
        stub_text = read_stub_text("finstack_quant.analytics")
        # These freestanding functions and the panel pathway were removed; ensure stub matches runtime.
        assert "def estimate_ruin" not in stub_text
        assert "def fit_garch11" not in stub_text
        assert "def rolling_var_forecasts" not in stub_text
        assert "def classify_breaches" not in stub_text
        assert "class PerformancePanel" not in stub_text
        assert "def from_panel" not in stub_text
