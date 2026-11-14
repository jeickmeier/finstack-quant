"""Host boundaries retain accepted analytics economic and result contracts."""

from datetime import date
import json
import math

import pandas as pd
import pytest

from finstack_quant.analytics import AnalyticsError, Performance, PerformancePanel, RollingGreeks, max_drawdown


def test_return_frame_uses_the_supplied_actual_opening_valuation() -> None:
    returns = pd.Series([0.01], index=pd.to_datetime(["2025-01-06"]), name="FUND")
    with pytest.raises(TypeError):
        Performance.from_returns(returns)
    perf = Performance.from_returns(returns, date(2025, 1, 3))
    assert perf.get_dates() == [date(2025, 1, 6)]
    assert perf.cagr().iloc[0] == pytest.approx(math.expm1(math.log1p(0.01) * 365 / 3))
    with pytest.raises(AnalyticsError):
        Performance.from_returns(returns, date(2025, 1, 6))


def test_panel_rejects_infinity_before_it_can_become_missing_wire_data() -> None:
    with pytest.raises(AnalyticsError):
        PerformancePanel(
            dates=[date(2025, 1, 1), date(2025, 1, 2), date(2025, 1, 3)],
            series=[[math.inf, 0.01]],
            kind="returns",
            ticker_names=["FUND"],
        )


def test_reversed_date_reset_fails_atomically() -> None:
    perf = Performance.from_panel(
        PerformancePanel(
            dates=[date(2025, 1, 1), date(2025, 1, 2), date(2025, 1, 3)],
            series=[[0.01, -0.02]],
            kind="returns",
            ticker_names=["FUND"],
        )
    )
    before = perf.to_json()
    with pytest.raises(AnalyticsError):
        perf.reset_date_range(date(2025, 1, 3), date(2025, 1, 2))
    assert perf.to_json() == before


def test_terminal_wipeout_is_exact_and_cannot_be_followed_by_returns() -> None:
    assert max_drawdown([0.1, -1.0]) == -1.0
    with pytest.raises(AnalyticsError):
        max_drawdown([-1.0, 0.0])
    with pytest.raises(AnalyticsError):
        max_drawdown([-1.01])


def test_undefined_rolling_regression_fields_round_trip_losslessly() -> None:
    wire = {"dates": ["2025-01-02"], "alphas": ["nan"], "betas": ["nan"]}
    result = RollingGreeks.from_json(json.dumps(wire))
    assert math.isnan(result.alphas[0])
    assert math.isnan(result.betas[0])
    assert json.loads(result.to_json()) == wire


def test_restored_summary_retains_nan_sentinels_and_validates_provenance() -> None:
    from finstack_quant.analytics import validate_summary

    metrics = [
        "cagr",
        "mean_return",
        "volatility",
        "sharpe",
        "sortino",
        "calmar",
        "max_drawdown",
        "value_at_risk",
        "expected_shortfall",
        "tracking_error",
        "information_ratio",
        "skewness",
        "kurtosis",
        "geometric_mean",
        "downside_deviation",
        "omega_ratio",
        "gain_to_pain",
        "ulcer_index",
        "pain_index",
        "recovery_factor",
        "tail_ratio",
        "r_squared",
    ]
    table = {
        "row_count": 1,
        "columns": [
            {"name": "ticker", "role": "dimension", "data": {"type": "string", "values": ["A"]}},
            *[{"name": name, "role": "measure", "data": {"type": "float64", "values": ["nan"]}} for name in metrics],
        ],
        "metadata": {
            "kind": "analytics_summary",
            "risk_free_rate": 0.0,
            "mar": 0.0,
            "omega_threshold": 0.0,
            "confidence": 0.95,
            "frequency": "daily",
            "benchmark": "A",
            "cagr_day_count": "act_365f",
            "units": {
                "mean_return": "annual_arithmetic_decimal",
                "volatility": "annual_decimal",
                "downside_deviation": "annual_decimal",
                "sharpe": "annual_ratio",
                "sortino": "annual_ratio",
                "geometric_mean": "observation_decimal",
                "value_at_risk": "observation_signed_decimal",
                "expected_shortfall": "observation_signed_decimal",
                "drawdown": "signed_decimal",
                "kurtosis": "excess",
            },
            "rows": [
                {
                    "ticker": "A",
                    "own_sample_count": 0,
                    "own_start": None,
                    "own_end": None,
                    "holding_start": None,
                    "paired_sample_count": 0,
                    "paired_start": None,
                    "paired_end": None,
                    "paired_holding_start": None,
                }
            ],
        },
    }
    assert validate_summary(table) is None
    assert validate_summary(json.dumps(table)) is None
    table["metadata"]["benchmark"] = "MISSING"
    with pytest.raises(AnalyticsError):
        validate_summary(table)


def test_performance_summary_produces_validatable_table_envelope_with_nonfinite_sentinels() -> None:
    from finstack_quant.analytics import PerformancePanel, validate_summary

    panel = PerformancePanel(
        dates=[date(2025, 1, 1), date(2025, 1, 2), date(2025, 1, 3), date(2025, 1, 4)],
        series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        kind="returns",
        ticker_names=["FUND", "BENCH"],
        frequency="daily",
        benchmark_ticker="BENCH",
    )
    perf = Performance.from_panel(panel)
    report = perf.summary(0.0, 0.0, 0.0, 0.95)

    assert validate_summary(report) is None
    assert validate_summary(json.dumps(report, allow_nan=False)) is None
    assert report["row_count"] == 2
    assert report["metadata"]["kind"] == "analytics_summary"

    df = perf.to_dataframe(0.0, 0.0, 0.0, 0.95)
    by_col = {col["name"]: col["data"] for col in report["columns"]}
    assert by_col["ticker"]["values"] == ["FUND", "BENCH"]
    # Bench self-tracking metrics yield exact zero/NaN; wire format retains canonical sentinels.
    assert by_col["information_ratio"]["values"][1] == "nan"
    assert math.isnan(df.loc["BENCH", "information_ratio"])
    assert by_col["cagr"]["values"][0] == pytest.approx(df.loc["FUND", "cagr"])


def test_correlation_and_multi_factor_result_wrappers() -> None:
    from finstack_quant.analytics import CorrelationResult, MultiFactorResult, PerformancePanel

    panel = PerformancePanel(
        dates=[date(2025, 1, 1), date(2025, 1, 2), date(2025, 1, 3), date(2025, 1, 4)],
        series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        kind="returns",
        ticker_names=["FUND", "BENCH"],
        frequency="daily",
        benchmark_ticker="BENCH",
    )
    perf = Performance.from_panel(panel)

    corr = perf.correlation_matrix()
    assert isinstance(corr, CorrelationResult)
    corr_df = corr.to_dataframe()
    assert list(corr_df.index) == ["FUND", "BENCH"]
    assert list(corr_df.columns) == ["FUND", "BENCH"]
    assert corr_df.loc["FUND", "FUND"] == pytest.approx(1.0)
    assert corr_df.loc["FUND", "BENCH"] == pytest.approx(corr.matrix[0][1])
    assert corr_df.loc["BENCH", "FUND"] == pytest.approx(corr.matrix[1][0])
    assert "CorrelationResult(" in repr(corr)
    corr_html = corr._repr_html_()
    assert corr_html is not None
    assert "<table" in corr_html

    mf = perf.multi_factor_greeks(0, [[0.02, -0.03, 0.01]], 0.0)
    assert isinstance(mf, MultiFactorResult)
    mf_html = mf._repr_html_()
    assert mf_html is not None
    assert "<table" in mf_html
    assert "factor_0" in mf_html
