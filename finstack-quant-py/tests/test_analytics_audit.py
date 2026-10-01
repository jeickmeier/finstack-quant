"""Regression coverage for analytics economics and Python boundary validation."""

from datetime import date, timedelta
import json
import math
import pickle

import pytest

from finstack_quant.analytics import AnalyticsError, MultiFactorResult, Performance, max_drawdown, sharpe, sortino


def dates(n: int) -> list[date]:
    return [date(2024, 1, 1) + timedelta(days=i) for i in range(n)]


def panel(values: list[float], frequency: str = "daily") -> Performance:
    return Performance.from_returns_arrays(dates(len(values)), [values], ["A"], frequency=frequency)


def test_empirical_tail_mass_is_preserved_in_metrics_and_summary() -> None:
    values = [-0.2] + [0.0] * 99
    perf = panel(values)
    assert perf.expected_shortfall(0.95).iloc[0] == pytest.approx(-0.04)
    assert perf.to_summary_dataframe().loc["A", "expected_shortfall"] == pytest.approx(-0.04)
    values[1] = 0.25
    assert panel(values).cdar(0.95).iloc[0] == pytest.approx(-0.04)
    assert panel([-0.2, -0.1, 0.1, 0.2]).expected_shortfall(0.625).iloc[0] == pytest.approx(-0.25 / 1.5)


@pytest.mark.parametrize("invalid", [math.nan, math.inf, -math.inf])
def test_invalid_scalar_inputs_preserve_undefined_risk(invalid: float) -> None:
    assert math.isnan(max_drawdown([invalid, -0.2]))
    assert math.isnan(sortino([invalid, 0.01]))
    assert math.isnan(sortino([0.0, 0.0], mar=invalid))
    assert math.isnan(sharpe([0.0, 0.0], rf=invalid))


def test_singleton_sharpe_agrees_with_panel_and_rolling() -> None:
    assert math.isnan(sharpe([0.01]))
    assert math.isnan(sortino([0.01]))
    assert math.isnan(panel([0.01]).sharpe().iloc[0])
    assert all(math.isnan(v) for v in panel([0.01, 0.02]).rolling_sharpe(0, window=1).values)


def test_zero_drawdown_ratios_preserve_infinity_and_zero() -> None:
    for metric in ("calmar", "martin_ratio"):
        assert getattr(panel([0.0, 0.0, 0.0]), metric)().iloc[0] == 0.0
        ratio = getattr(panel([0.01, 0.01, 0.01]), metric)().iloc[0]
        assert math.isinf(ratio)
        assert ratio > 0.0


@pytest.mark.parametrize("invalid", [math.nan, math.inf, -math.inf])
def test_drawdown_ratios_preserve_non_finite_cash_rates(invalid: float) -> None:
    for values in ([0.0, 0.0, 0.0], [0.01, 0.01, 0.01], [-0.01, 0.0, 0.02]):
        perf = panel(values)
        for metric in ("sterling_ratio", "burke_ratio", "pain_ratio"):
            assert math.isnan(getattr(perf, metric)(invalid).iloc[0])


def test_treynor_preserves_non_finite_cash_rates_with_zero_beta() -> None:
    for portfolio_return in (0.01, 0.0):
        perf = Performance.from_returns_arrays(
            dates(3), [[-0.01, 0.01, 0.02], [portfolio_return] * 3], ["BENCH", "PORT"]
        )
        for invalid in (math.nan, math.inf, -math.inf):
            assert math.isnan(perf.treynor(invalid).loc["PORT"])
        expected = math.inf if portfolio_return > 0.0 else 0.0
        assert perf.treynor(0.0).loc["PORT"] == expected


def test_extreme_wealth_preserves_annualized_growth_and_relative_drawdowns() -> None:
    for grid, values, expected in (
        ([date(year, 1, 1) for year in range(2001, 2021)], [-0.9] * 20, 0.1 ** (20 / (7305 / 365.25)) - 1),
        ([date(2021, 1, 1), date(2022, 1, 1)], [1e200] * 2, 1e200 ** (2 / (731 / 365.25))),
    ):
        perf = Performance.from_returns_arrays(grid, [values], ["A"], frequency="annual")
        result = perf.cagr().iloc[0]
        assert math.isfinite(result)
        assert result == pytest.approx(expected, rel=1e-12)

    for values in ([-0.9] * 20, [1e300] * 4):
        perf = Performance.from_returns_arrays(dates(len(values)), [values, values], ["BENCH", "A"])
        assert all(value == 0.0 for series in perf.cumulative_returns_outperformance() for value in series)

    perf = panel([1e300, 1e300, -0.5, 0.0])
    assert perf.drawdown_series()[0] == pytest.approx([0.0, 0.0, -0.5, -0.5])
    assert perf.max_drawdown().iloc[0] == pytest.approx(-0.5)
    assert perf.drawdown_details(0)[0].max_drawdown == pytest.approx(-0.5)
    assert panel([1e-18, 1e-18]).cumulative_returns()[0][-1] == pytest.approx(2e-18, rel=1e-12, abs=0.0)
    near_wipeout = -0.9999999999999999
    mean = panel([near_wipeout] + [0.0] * 7).geometric_mean().iloc[0]
    assert mean > -1.0
    assert mean == pytest.approx((1.0 + near_wipeout) ** (1 / 8) - 1, abs=1e-12)


def test_lookback_returns_accepts_maximum_calendar_date() -> None:
    perf = Performance.from_arrays([date(9999, 12, 30), date.max], [[100.0, 110.0]], ["A"])
    lookback = perf.lookback_returns(date.max)
    for values in (lookback.mtd, lookback.qtd, lookback.ytd, lookback.fytd):
        assert values[0] == pytest.approx(0.1)


def test_rolling_returns_preserve_tiny_windows_after_positive_outlier() -> None:
    values = [0.1, 1e-18, -1e-18, 1e-18, -1e-18]
    for window in (1, 3):
        rolling = panel(values).rolling_returns(0, window)
        assert rolling.dates == dates(len(values))[window - 1 :]
        for start, actual in enumerate(rolling.values):
            expected = panel(values[start : start + window]).cumulative_returns()[0][-1]
            assert actual == pytest.approx(expected, rel=1e-12, abs=0.0)


def test_multi_factor_extremes_return_finite_statistics_or_analytics_error() -> None:
    scale = 1e200
    factors = [[-0.2, -0.1, 0.0, 0.1, 0.2]]
    fit = panel([3 * scale, scale, 3 * scale, scale, 3 * scale]).multi_factor_greeks(0, factors)
    assert fit.alpha / scale == pytest.approx(2.2 * 252, rel=1e-12)
    assert abs(float(fit.betas[0])) / scale < 1e-12
    assert fit.r_squared == pytest.approx(0.0, abs=1e-12)
    assert fit.adjusted_r_squared == pytest.approx(-1 / 3, abs=1e-12)
    assert fit.residual_vol / scale == pytest.approx(math.sqrt(4.8 / 3 * 252), rel=1e-12)

    for values, factor_values in (
        ([1e307] * 5, factors),
        ([2e199, 4e199, 6e199, 8e199, 1e200], [[2e-151, 4e-151, 6e-151, 8e-151, 1e-150]]),
    ):
        with pytest.raises(AnalyticsError, match="finite"):
            panel(values).multi_factor_greeks(0, factor_values)


@pytest.mark.parametrize("tail", [1e-12, 1e-18])
def test_rolling_sortino_preserves_small_returns_after_positive_outlier(tail: float) -> None:
    for values in ([0.1, tail, -tail, tail, -tail, tail], [0.1, tail, tail, -tail, tail, -tail]):
        rolling = panel(values).rolling_sortino(0, window=3)
        assert rolling.dates == dates(len(values))[2:]
        for start, actual in enumerate(rolling.values):
            expected = sortino(values[start : start + 3])
            assert actual == pytest.approx(expected, rel=1e-12, abs=1e-12)


def test_constructors_reject_duplicate_tickers_and_derived_nan() -> None:
    with pytest.raises(AnalyticsError, match="duplicate ticker"):
        Performance.from_returns_arrays(dates(2), [[0.01, 0.02], [0.03, 0.04]], ["A", "A"])
    with pytest.raises(AnalyticsError):
        Performance.from_arrays(dates(2), [[1e-300, 1e300]], ["A"])


def test_restoration_validates_state_and_rebuilds_caches() -> None:
    perf = panel([-0.2, 0.25, 0.01])
    state = json.loads(perf.to_json())
    assert "drawdowns" not in state
    for key, value in [("drawdowns", [[0.0] * 3]), ("benchmark_idx", 99), ("return_spans", [{"start": 0, "end": 99}])]:
        changed = dict(state)
        changed[key] = value
        with pytest.raises(ValueError, match="invalid Performance JSON"):
            Performance.from_json(json.dumps(changed))
    state["returns"][0][0] = -0.3
    assert Performance.from_json(json.dumps(state)).max_drawdown().iloc[0] == pytest.approx(-0.3)
    restored = pickle.loads(pickle.dumps(perf))  # noqa: S301 - trusted in-process round trip
    assert restored.max_drawdown().iloc[0] == pytest.approx(-0.2)


def test_initial_drawdown_and_performance_conventions() -> None:
    perf = Performance.from_arrays(dates(3), [[100.0, 90.0, 100.0]], ["A"])
    episode = perf.drawdown_details(0)[0]
    assert episode.start == dates(3)[0]
    assert episode.duration_days == 2
    assert not episode.truncated_at_start
    assert panel([0.01, -0.02, 0.03, 0.01], "monthly").m_squared(0.12).iloc[0] == pytest.approx(0.09)
    assert panel([0.02, 0.02, -0.01, 0.0]).period_stats(0, "daily").kelly_criterion == pytest.approx(0.5)


@pytest.mark.parametrize("n", [5, 29, 57, 58])
def test_constant_response_fit_preserves_nan_in_json(n: int) -> None:
    fit = panel([0.01] * n).multi_factor_greeks(0, [[0.001 * i for i in range(n)]])
    assert math.isnan(fit.r_squared)
    restored = MultiFactorResult.from_json(fit.to_json())
    assert math.isnan(restored.r_squared)
    assert math.isnan(restored.adjusted_r_squared)
