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
import pickle

import pandas as pd
import pytest

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
)
from finstack_quant.statements_analytics import (
    compute_multiple,
    percentile_rank,
    regression_fair_value,
    score_relative_value,
    z_score,
)

# Fixtures


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
    return Performance(_prices_panel(), benchmark_ticker="BENCH", frequency="daily")


@pytest.fixture
def perf_returns() -> Performance:
    return Performance.from_returns(
        _returns_panel(_prices_panel()),
        benchmark_ticker="BENCH",
        frequency="daily",
    )


# Construction


class TestConstruction:
    def test_from_prices_dataframe(self, perf_prices: Performance) -> None:
        assert perf_prices.ticker_names == ["ACME", "BENCH"]
        assert perf_prices.benchmark_idx == 1
        assert perf_prices.frequency == "daily"
        active = perf_prices.dates()
        # Returns from N prices yield N-1 active observation dates (first
        # price date is dropped because returns are pct_change).
        assert len(active) == 59
        assert active[0] == date(2024, 1, 2)

    def test_from_returns_dataframe(self, perf_returns: Performance) -> None:
        assert perf_returns.ticker_names == ["ACME", "BENCH"]
        assert perf_returns.benchmark_idx == 1
        assert len(perf_returns.dates()) == 60

    def test_ragged_price_dataframe_exports_pad_edges(self) -> None:
        dates = pd.to_datetime(_daily_dates(6))
        prices = pd.DataFrame(
            {
                "BENCH": [100.0, 101.0, 102.0, 103.0, 104.0, 105.0],
                "PORT": [math.nan, math.nan, 50.0, 55.0, 60.5, math.nan],
            },
            index=dates,
        )

        perf = Performance(prices, benchmark_ticker="BENCH")
        assert perf.active_dates_for_ticker(1) == [date(2024, 1, 4), date(2024, 1, 5)]

        cumulative = perf.to_cumulative_returns_dataframe()
        assert list(cumulative.index.date) == _daily_dates(5, start=date(2024, 1, 2))
        assert pd.isna(cumulative.loc[pd.Timestamp(date(2024, 1, 2)), "PORT"])
        assert pd.isna(cumulative.loc[pd.Timestamp(date(2024, 1, 3)), "PORT"])
        assert cumulative.loc[pd.Timestamp(date(2024, 1, 4)), "PORT"] == pytest.approx(0.10)
        assert cumulative.loc[pd.Timestamp(date(2024, 1, 5)), "PORT"] == pytest.approx(0.21)
        assert pd.isna(cumulative.loc[pd.Timestamp(date(2024, 1, 6)), "PORT"])

    def test_from_arrays(self) -> None:
        dates = _daily_dates(5)
        prices = [[100.0, 101.0, 102.0, 103.0, 104.0], [50.0, 50.5, 51.0, 51.5, 52.0]]
        perf = Performance.from_arrays(dates, prices, ["A", "B"])
        assert perf.ticker_names == ["A", "B"]
        assert list(perf.cagr().index) == ["A", "B"]

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

        perf_p = Performance(prices, benchmark_ticker="BENCH")
        perf_p.reset_date_range(returns_no_lead.index[0].date(), prices.index[-1].date())

        perf_r = Performance.from_returns(returns_no_lead, benchmark_ticker="BENCH")

        vol_p = perf_p.volatility(annualize=False)
        vol_r = perf_r.volatility(annualize=False)
        for ticker in perf_p.ticker_names:
            assert vol_p[ticker] == pytest.approx(vol_r[ticker], rel=1e-12, abs=1e-12)


# Return / risk metrics


class TestReturnRiskMetrics:
    def test_cagr_returns_one_per_ticker(self, perf_prices: Performance) -> None:
        values = perf_prices.cagr()
        assert list(values.index) == ["ACME", "BENCH"]
        assert all(isinstance(v, float) for v in values)

    def test_cagr_act365_25_label_matches_default(self, perf_prices: Performance) -> None:
        default = perf_prices.cagr()
        labeled = perf_prices.cagr(day_count="act365_25")
        assert default["ACME"] == pytest.approx(labeled["ACME"])
        assert default["BENCH"] == pytest.approx(labeled["BENCH"])

    def test_cagr_bus252_requires_calendar(self, perf_prices: Performance) -> None:
        with pytest.raises(AnalyticsError):
            perf_prices.cagr(day_count="bus_252")
        values = perf_prices.cagr(day_count="bus_252", calendar_id="nyse")
        assert list(values.index) == ["ACME", "BENCH"]
        assert not math.isnan(values["ACME"])

    def test_parametric_var_horizon_changes_result(self, perf_prices: Performance) -> None:
        one = perf_prices.parametric_var(0.95)
        ten = perf_prices.parametric_var(0.95, horizon_periods=10.0)
        assert list(one.index) == ["ACME", "BENCH"]
        assert one["ACME"] != pytest.approx(ten["ACME"])

    def test_excess_returns_rejects_length_mismatch(self, perf_prices: Performance) -> None:
        with pytest.raises(AnalyticsError):
            perf_prices.excess_returns([0.0])

    def test_excess_returns_zero_rf_matches_returns(self, perf_prices: Performance) -> None:
        rf = [0.0] * len(perf_prices.active_dates())
        excess = perf_prices.excess_returns(rf, nperiods=1.0)
        raw = perf_prices.returns()
        assert excess[0] == pytest.approx(raw[0])

    def test_correlation_matrix_is_square_psd_identity_diag(self) -> None:
        dates = _daily_dates(6)
        returns = [
            [0.01, -0.02, 0.015, 0.0, 0.01, -0.01],
            [0.005, -0.01, 0.0, 0.005, 0.02, -0.015],
        ]
        perf = Performance.from_returns_arrays(dates, returns, ["A", "B"])
        corr = perf.correlation_matrix()
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
            assert not values.isna().any()

    def test_higher_moments_finite(self, perf_prices: Performance) -> None:
        for getter in (perf_prices.skewness, perf_prices.kurtosis):
            values = getter()
            assert list(values.index) == ["ACME", "BENCH"]
            assert not values.isna().any()

    def test_summary_to_dataframe_has_one_row_per_ticker(self, perf_prices: Performance) -> None:
        summary = perf_prices.to_summary_dataframe()
        assert list(summary.index) == ["ACME", "BENCH"]
        assert "cagr" in summary.columns
        assert "sharpe" in summary.columns
        assert "max_drawdown" in summary.columns


# Scalar-metric Series contract


class TestScalarMetricSeries:
    """Per-ticker scalar metrics are ticker-labelled `pd.Series`, not lists."""

    def test_metric_returns_series(self, perf_prices: Performance) -> None:
        assert isinstance(perf_prices.sharpe(), pd.Series)
        assert isinstance(perf_prices.max_drawdown_duration(), pd.Series)

    def test_metric_index_equals_ticker_names(self, perf_prices: Performance) -> None:
        assert list(perf_prices.volatility().index) == perf_prices.ticker_names

    def test_metric_series_name_is_metric_name(self, perf_prices: Performance) -> None:
        for metric in ("cagr", "sharpe", "max_drawdown", "tail_ratio", "max_drawdown_duration"):
            assert getattr(perf_prices, metric)().name == metric

    def test_label_access_matches_positional(self, perf_prices: Performance) -> None:
        vols = perf_prices.volatility()
        assert vols["ACME"] == vols.iloc[0]
        assert vols["BENCH"] == vols.iloc[1]

    def test_concat_yields_metric_named_columns(self, perf_prices: Performance) -> None:
        df = pd.concat([perf_prices.sharpe(), perf_prices.sortino()], axis=1)
        assert list(df.columns) == ["sharpe", "sortino"]
        assert list(df.index) == ["ACME", "BENCH"]

    def test_max_drawdown_duration_keeps_integer_dtype(self, perf_prices: Performance) -> None:
        durations = perf_prices.max_drawdown_duration()
        assert durations.dtype.kind == "i"
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
        assert lb.ticker_names == perf_prices.ticker_names
        assert list(lb.to_dataframe().columns) == [
            "mtd",
            "qtd",
            "ytd",
            "fytd",
        ]
        assert "fytd_len=2" in repr(lb)
        assert "has_fytd" not in repr(lb)

    def test_lookback_with_fiscal_month(self, perf_prices: Performance) -> None:
        lb = perf_prices.lookback_returns(date(2024, 2, 29), fiscal_year_start_month=4)
        assert len(lb.fytd) == 2

    def test_lookback_rejects_null_fytd_json(self) -> None:
        with pytest.raises(ValueError, match="invalid type: null"):
            LookbackReturns.from_json('{"ticker_names":[],"mtd":[],"qtd":[],"ytd":[],"fytd":null}')

    def test_lookback_rejects_invalid_fiscal_month(self, perf_prices: Performance) -> None:
        with pytest.raises(AnalyticsError, match="start_month must be in"):
            perf_prices.lookback_returns(date(2024, 2, 29), fiscal_year_start_month=13)

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
        results = perf_prices.greeks()
        assert len(results) == 2
        assert all(isinstance(r, GreeksResult) for r in results)

    def test_degenerate_beta_and_greeks_wire_round_trips_preserve_nan(self) -> None:
        perf = Performance.from_returns_arrays(
            [date(2024, 1, 1)],
            [[0.01], [0.01]],
            ["TARGET", "BENCH"],
            benchmark_ticker="BENCH",
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

        greeks_result = perf.greeks()[0]
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
        perf = Performance.from_returns_arrays(
            dates,
            [target, benchmark],
            ["TARGET", "BENCH"],
            benchmark_ticker="BENCH",
            frequency="monthly",
        )

        zero_rf = perf.greeks()[0]
        nonzero_rf = perf.greeks(risk_free_rate=0.12)[0]

        assert zero_rf.alpha == pytest.approx(0.0, abs=1e-12)
        assert nonzero_rf.alpha > zero_rf.alpha

    def test_rolling_greeks(self, perf_prices: Performance) -> None:
        rg = perf_prices.rolling_greeks(0, window=10)
        assert isinstance(rg, RollingGreeks)
        assert len(rg.alphas) == len(rg.betas)
        assert len(rg.dates) == len(rg.alphas)

    def test_rolling_greeks_risk_free_rate_changes_jensen_alpha(self) -> None:
        dates = _daily_dates(8)
        benchmark = [-0.03, -0.02, -0.01, 0.0, 0.01, 0.02, 0.03, 0.04]
        target = [1.5 * value for value in benchmark]
        perf = Performance.from_returns_arrays(
            dates,
            [target, benchmark],
            ["TARGET", "BENCH"],
            benchmark_ticker="BENCH",
            frequency="monthly",
        )

        zero_rf = perf.rolling_greeks(0, window=5)
        nonzero_rf = perf.rolling_greeks(0, window=5, risk_free_rate=0.12)

        assert list(zero_rf.alphas) == pytest.approx([0.0] * len(zero_rf.alphas), abs=1e-12)
        assert all(actual > base for actual, base in zip(nonzero_rf.alphas, zero_rf.alphas, strict=True))

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
        assert list(te.index) == ["ACME", "BENCH"]
        assert list(ir.index) == ["ACME", "BENCH"]
        assert not math.isnan(te["ACME"])

    def test_reset_bench_ticker_changes_index(self, perf_prices: Performance) -> None:
        perf_prices.reset_bench_ticker("ACME")
        assert perf_prices.benchmark_idx == 0


# Multi-factor


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

    def test_multi_factor_total_zero_matches_excess(self, perf_prices: Performance) -> None:
        n = len(perf_prices.dates())
        factor = [0.001 * (i % 5) for i in range(n)]
        excess = perf_prices.multi_factor_greeks(0, [factor], return_kind="excess")
        total = perf_prices.multi_factor_greeks(0, [factor], return_kind="total", risk_free_rate=0.0)
        assert excess.alpha == pytest.approx(total.alpha)
        assert float(excess.betas[0]) == pytest.approx(float(total.betas[0]))

    def test_multi_factor_rejects_unknown_kind(self, perf_prices: Performance) -> None:
        n = len(perf_prices.dates())
        factor = [0.001] * n
        with pytest.raises(ValueError, match="return_kind"):
            perf_prices.multi_factor_greeks(0, [factor], return_kind="jensen")


# Date window mutation


class TestDateRange:
    def test_reset_date_range_narrows_active_grid(self, perf_prices: Performance) -> None:
        perf_prices.reset_date_range(date(2024, 1, 10), date(2024, 1, 20))
        active = perf_prices.active_dates()
        assert active[0] == date(2024, 1, 10)
        assert active[-1] == date(2024, 1, 20)
        # `dates()` keeps Rust semantics: the full constructed grid.
        assert len(perf_prices.dates()) == 59


# Stubs


class TestStubs:
    """Smoke tests that the `.pyi` stays in sync with the registered API."""

    def test_stub_lists_performance_first(self) -> None:
        stub_path = Path(__file__).resolve().parents[1] / "finstack_quant" / "analytics" / "__init__.pyi"
        stub_text = stub_path.read_text()
        assert "class Performance:" in stub_text
        assert "from_returns" in stub_text
        assert "rolling_returns" in stub_text
        assert '"Performance"' in stub_text

    def test_stub_drops_legacy_freestanding_functions(self) -> None:
        stub_path = Path(__file__).resolve().parents[1] / "finstack_quant" / "analytics" / "__init__.pyi"
        stub_text = stub_path.read_text()
        # These freestanding functions were deleted; ensure the stub matches the runtime.
        assert "def estimate_ruin" not in stub_text
        assert "def fit_garch11" not in stub_text
        assert "def rolling_var_forecasts" not in stub_text
        assert "def classify_breaches" not in stub_text


# Cross-binding sanity (comps live in statements_analytics)


_COMPANY_METRIC_FIELDS = [
    "enterprise_value",
    "market_cap",
    "share_price",
    "oas_bp",
    "yield_pct",
    "ebitda",
    "revenue",
    "ebit",
    "ufcf",
    "lfcf",
    "net_income",
    "book_value",
    "tangible_book_value",
    "dividends_per_share",
    "leverage",
    "interest_coverage",
    "revenue_growth",
    "ebitda_margin",
]


def _company(cid: str, custom: dict[str, float] | None = None, **metrics: float) -> dict:
    """Build a canonical serde ``CompanyMetrics`` payload."""
    blank: dict = dict.fromkeys(_COMPANY_METRIC_FIELDS)
    return {"id": cid, "attributes": {}, "custom": custom or {}, **blank, **metrics}


class TestCompsBindings:
    def test_compute_multiple(self) -> None:
        metrics = {"enterprise_value": 8_500.0, "ebitda": 1_000.0}
        assert compute_multiple(metrics, "ev_ebitda") == pytest.approx(8.5)

    def test_regression_fair_value(self) -> None:
        result = regression_fair_value([1.0, 2.0, 3.0, 4.0], [3.0, 5.0, 7.0, 9.0], 3.0, 10.0)
        assert result.fitted_value == pytest.approx(7.0)
        assert result.residual == pytest.approx(3.0)

    def test_percentile_rank(self) -> None:
        assert percentile_rank([100.0, 200.0, 300.0, 400.0, 500.0], 250.0) == pytest.approx(0.4)
        assert percentile_rank([], 100.0) is None

    def test_z_score(self) -> None:
        assert z_score([1.0, 2.0, 3.0, 4.0, 5.0], 3.0) == pytest.approx(0.0)
        assert z_score([1.0], 1.0) is None
        assert z_score([5.0, 5.0, 5.0], 5.0) is None

    def test_score_relative_value(self) -> None:
        peer_set = {
            "subject": _company("SUBJ", leverage=2.0, oas_bp=250.0),
            "peers": [
                _company("P1", leverage=1.0, oas_bp=100.0),
                _company("P2", leverage=2.0, oas_bp=200.0),
                _company("P3", leverage=3.0, oas_bp=300.0),
            ],
            "period_basis": "ltm",
        }
        result = score_relative_value(
            peer_set,
            [
                {
                    "label": "Spread vs Leverage",
                    "y_extractor": {"named": "oas_bp"},
                    "x_extractor": {"named": "leverage"},
                    "weight": 1.0,
                }
            ],
        )
        assert result.company_id == "SUBJ"
        assert not hasattr(result, "by_dimension")
        assert result.dimensions[0].label == "Spread vs Leverage"
        assert result.composite_score > 0.0

    def test_score_relative_value_accepts_json_strings(self) -> None:
        import json

        peer_set = {
            "subject": _company("SUBJ", leverage=2.0),
            "peers": [_company("P1", leverage=1.0), _company("P2", leverage=3.0)],
            "period_basis": "ltm",
        }
        dimensions = [
            {
                "label": "Leverage",
                "y_extractor": {"named": "leverage"},
                "x_extractor": None,
                "weight": 1.0,
            }
        ]
        typed = score_relative_value(peer_set, dimensions)
        via_json = score_relative_value(json.dumps(peer_set), json.dumps(dimensions))
        assert via_json.to_json() == typed.to_json()

    def test_peer_stats_uses_rust_field_names(self) -> None:
        from finstack_quant.statements_analytics import peer_stats

        stats = peer_stats([1.0, 2.0, 3.0, 4.0, 5.0])
        assert stats.count == 5
        assert not hasattr(stats, "n")
        assert stats.iqr == pytest.approx(stats.q3 - stats.q1)
        # No-result path returns None, matching the WASM twin's `undefined`.
        assert peer_stats([]) is None

    def test_score_relative_value_direction_flips_sign(self) -> None:
        def pe_peer_set() -> dict:
            return {
                "subject": _company("SUBJ", custom={"pe": 30.0}),
                "peers": [
                    _company("P1", custom={"pe": 10.0}),
                    _company("P2", custom={"pe": 15.0}),
                    _company("P3", custom={"pe": 20.0}),
                ],
                "period_basis": "ltm",
            }

        def dimension(direction: str | None) -> dict:
            spec = {
                "label": "pe",
                "y_extractor": {"custom": "pe"},
                "x_extractor": None,
                "weight": 1.0,
            }
            if direction is not None:
                spec["direction"] = direction
            return spec

        cheap_convention = score_relative_value(pe_peer_set(), [dimension(None)])
        rich_convention = score_relative_value(pe_peer_set(), [dimension("higher_is_rich")])
        # High multiple vs peers: rich (negative) under higher_is_rich, cheap
        # (positive) under the default higher_is_cheap convention.
        assert cheap_convention.composite_score > 0.0
        assert rich_convention.composite_score < 0.0
        assert rich_convention.composite_score == pytest.approx(-cheap_convention.composite_score)

    def test_score_relative_value_rejects_unknown_direction(self) -> None:
        peer_set = {
            "subject": _company("SUBJ", custom={"pe": 30.0}),
            "peers": [_company("P1", custom={"pe": 10.0})],
            "period_basis": "ltm",
        }
        with pytest.raises(ValueError, match="down_is_up"):
            score_relative_value(
                peer_set,
                [
                    {
                        "label": "pe",
                        "y_extractor": {"custom": "pe"},
                        "x_extractor": None,
                        "weight": 1.0,
                        "direction": "down_is_up",
                    }
                ],
            )

    def test_score_relative_value_multiple_extractor(self) -> None:
        peer_set = {
            "subject": _company("SUBJ", enterprise_value=12_000.0, ebitda=1_000.0),
            "peers": [
                _company("P1", enterprise_value=8_000.0, ebitda=1_000.0),
                _company("P2", enterprise_value=9_000.0, ebitda=1_000.0),
                _company("P3", enterprise_value=10_000.0, ebitda=1_000.0),
            ],
            "period_basis": "ltm",
        }
        result = score_relative_value(
            peer_set,
            [
                {
                    "label": "EV/EBITDA",
                    "y_extractor": {"multiple": "ev_ebitda"},
                    "x_extractor": None,
                    "weight": 1.0,
                }
            ],
        )
        assert result.peer_count == 3
        assert result.dimensions[0].label == "EV/EBITDA"

    def test_non_numeric_metric_raises_value_error(self) -> None:
        peer_set = {
            "subject": _company("SUBJ") | {"oas_bp": "wide"},
            "peers": [_company("P1", oas_bp=100.0)],
            "period_basis": "ltm",
        }
        with pytest.raises(ValueError, match="peer_set"):
            score_relative_value(
                peer_set,
                [
                    {
                        "label": "oas_bp",
                        "y_extractor": {"named": "oas_bp"},
                        "x_extractor": None,
                        "weight": 1.0,
                    }
                ],
            )

    def test_none_metric_treated_as_missing(self) -> None:
        metrics = {"enterprise_value": 8_500.0, "ebitda": 1_000.0, "revenue": None}
        assert compute_multiple(metrics, "ev_ebitda") == pytest.approx(8.5)
