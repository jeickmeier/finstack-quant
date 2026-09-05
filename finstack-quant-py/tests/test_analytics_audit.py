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
