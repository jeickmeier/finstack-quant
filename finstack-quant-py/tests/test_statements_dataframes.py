"""Tests for the ``statements`` DataFrame accessors.

Covers ``CheckReport.to_dataframe()`` / ``to_findings_dataframe()`` and
``MonteCarloResults.to_dataframe()`` / ``to_paths_dataframe()``.

``CheckReport`` objects are built through the public ``from_json`` constructor
against hand-written payloads so the tests stay self-contained and do not need a
full check-suite run (the suite runner is Rust-side only). Monte Carlo results
are produced end-to-end through ``ModelBuilder`` and ``Evaluator.evaluate_monte_carlo``.
"""

from __future__ import annotations

import json

import pandas as pd
import pytest

from finstack_quant.statements import (
    CheckReport,
    Evaluator,
    ForecastSpec,
    ModelBuilder,
    MonteCarloConfig,
    MonteCarloResults,
)

CHECK_RESULT_COLUMNS = [
    "check_id",
    "check_name",
    "category",
    "passed",
    "finding_count",
]

CHECK_FINDING_COLUMNS = [
    "check_id",
    "check_name",
    "category",
    "severity",
    "message",
    "period",
    "nodes",
    "materiality_absolute",
    "materiality_relative_pct",
    "materiality_reference_value",
    "materiality_reference_label",
    "comparison_actual",
    "comparison_expected",
    "comparison_tolerance",
]

PATH_COLUMNS = ["path_id", "period", "metric", "value"]


def _empty_report() -> CheckReport:
    """A report from a suite that ran no checks at all."""
    payload = {
        "results": [],
        "summary": {
            "total_checks": 0,
            "passed": 0,
            "failed": 0,
            "errors": 0,
            "warnings": 0,
            "infos": 0,
        },
    }
    return CheckReport.from_json(json.dumps(payload))


def _populated_report() -> CheckReport:
    """Two check results carrying three findings (1 error, 1 warning, 1 info)."""
    payload = {
        "results": [
            {
                "check_id": "balance_sheet_articulation",
                "check_name": "Balance Sheet Articulation",
                "category": "accounting_identity",
                "passed": False,
                "findings": [
                    {
                        "check_id": "balance_sheet_articulation",
                        "severity": "error",
                        "message": "Balance sheet does not articulate in 2025Q1",
                        "period": "2025Q1",
                        "materiality": {
                            "absolute": 250.0,
                            "relative_pct": 2.5,
                            "reference_value": 10000.0,
                            "reference_label": "total_assets",
                        },
                        "nodes": ["total_assets", "total_liabilities"],
                    },
                    {
                        "check_id": "balance_sheet_articulation",
                        "severity": "warning",
                        "message": "Assets grew faster than liabilities",
                        "period": "2025Q2",
                        "materiality": None,
                        "nodes": [],
                    },
                ],
            },
            {
                "check_id": "missing_values",
                "check_name": "Missing Values",
                "category": "data_quality",
                "passed": True,
                "findings": [
                    {
                        "check_id": "missing_values",
                        "severity": "info",
                        "message": "All required nodes populated",
                        "period": None,
                        "materiality": None,
                        "nodes": [],
                    },
                ],
            },
        ],
        "summary": {
            "total_checks": 2,
            "passed": 1,
            "failed": 1,
            "errors": 1,
            "warnings": 1,
            "infos": 1,
        },
    }
    return CheckReport.from_json(json.dumps(payload))


def _monte_carlo_results(*, include_path_data: bool, n_paths: int = 16) -> MonteCarloResults:
    """Run a small stochastic model: one actual quarter, three forecast quarters."""
    builder = ModelBuilder("mc-frames")
    builder.periods("2025Q1..Q4", "2025Q1")
    builder.value("revenue", [("2025Q1", 100.0)])
    builder.forecast("revenue", ForecastSpec.normal(0.0, 5.0, 42))
    model = builder.build()
    config = MonteCarloConfig(n_paths, 7, [0.05, 0.5, 0.95], include_path_data)
    return Evaluator().evaluate_monte_carlo(model, config)


# CheckReport


def test_check_report_to_dataframe_is_empty_with_schema() -> None:
    """An empty suite still yields the documented columns."""
    df = _empty_report().to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == CHECK_RESULT_COLUMNS
    assert len(df) == 0


def test_check_report_to_findings_dataframe_is_empty_with_schema() -> None:
    """A report with no findings still yields the documented columns."""
    df = _empty_report().to_findings_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == CHECK_FINDING_COLUMNS
    assert len(df) == 0


def test_check_report_to_dataframe_has_one_row_per_check() -> None:
    """Row count matches the underlying results collection."""
    report = _populated_report()
    df = report.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert set(CHECK_RESULT_COLUMNS) <= set(df.columns)
    assert len(df) == report.total_checks == 2

    by_id = df.set_index("check_id")
    assert by_id.loc["balance_sheet_articulation", "category"] == "accounting_identity"
    assert bool(by_id.loc["balance_sheet_articulation", "passed"]) is False
    assert int(by_id.loc["balance_sheet_articulation", "finding_count"]) == 2
    assert bool(by_id.loc["missing_values", "passed"]) is True
    assert int(by_id.loc["missing_values", "finding_count"]) == 1


def test_check_report_to_findings_dataframe_has_one_row_per_finding() -> None:
    """Row count matches the flattened findings collection."""
    report = _populated_report()
    df = report.to_findings_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert set(CHECK_FINDING_COLUMNS) <= set(df.columns)
    assert len(df) == report.total_findings == 3
    assert sorted(df["severity"]) == ["error", "info", "warning"]


def test_check_report_findings_carry_materiality_and_nodes() -> None:
    """Materiality is flattened to float columns; nodes are comma-joined."""
    df = _populated_report().to_findings_dataframe()
    error_row = df[df["severity"] == "error"].iloc[0]

    assert error_row["check_id"] == "balance_sheet_articulation"
    assert error_row["check_name"] == "Balance Sheet Articulation"
    assert error_row["period"] == "2025Q1"
    assert error_row["nodes"] == "total_assets,total_liabilities"
    assert error_row["materiality_absolute"] == pytest.approx(250.0)
    # `relative_pct` is a percentage (2.5 == 2.5%), not a decimal fraction.
    assert error_row["materiality_relative_pct"] == pytest.approx(2.5)
    assert error_row["materiality_reference_value"] == pytest.approx(10000.0)
    assert error_row["materiality_reference_label"] == "total_assets"


def test_check_report_findings_without_materiality_are_null() -> None:
    """Findings with no materiality context leave the four columns null."""
    df = _populated_report().to_findings_dataframe()
    info_row = df[df["severity"] == "info"].iloc[0]

    assert pd.isna(info_row["period"])
    assert info_row["nodes"] == ""
    assert pd.isna(info_row["materiality_absolute"])
    assert pd.isna(info_row["materiality_reference_label"])


# MonteCarloResults


def test_monte_carlo_to_dataframe_is_percentile_by_period() -> None:
    """Rows are forecast periods; one column per configured percentile."""
    results = _monte_carlo_results(include_path_data=False)
    df = results.to_dataframe("revenue")

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["p5", "p50", "p95"]
    assert df.index.name == "period"
    assert list(df.index) == results.forecast_periods
    assert len(df) == len(results.forecast_periods) == 3


def test_monte_carlo_to_dataframe_percentiles_are_ordered() -> None:
    """Each period's percentiles are non-decreasing across the fan."""
    df = _monte_carlo_results(include_path_data=False).to_dataframe("revenue")

    assert (df["p5"] <= df["p50"]).all()
    assert (df["p50"] <= df["p95"]).all()


def test_monte_carlo_to_dataframe_matches_get_percentile_series() -> None:
    """The frame reshapes exactly what the scalar accessor returns."""
    results = _monte_carlo_results(include_path_data=False)
    df = results.to_dataframe("revenue")
    series = results.percentile_by_period("revenue", 0.5)

    assert series is not None
    for period, value in series.items():
        assert df.loc[period, "p50"] == pytest.approx(value)


def test_monte_carlo_to_dataframe_keeps_missing_percentile_as_nan() -> None:
    """A missing stored pair does not shift or remove its forecast-period row."""
    results = _monte_carlo_results(include_path_data=False)
    payload = json.loads(results.to_json())
    values = payload["percentile_results"]["revenue"]["values"]
    first_period = next(iter(values))
    values[first_period] = [pair for pair in values[first_period] if pair[0] != 0.5]

    df = MonteCarloResults.from_json(json.dumps(payload)).to_dataframe("revenue")

    assert list(df.index) == results.forecast_periods
    assert pd.isna(df.loc[first_period, "p50"])


def test_monte_carlo_to_dataframe_raises_for_unknown_metric() -> None:
    """An unsimulated metric is a KeyError, not an empty frame."""
    results = _monte_carlo_results(include_path_data=False)
    with pytest.raises(KeyError):
        results.to_dataframe("not_a_node")


def test_monte_carlo_to_paths_dataframe_is_empty_without_path_data() -> None:
    """Without ``include_path_data`` the frame is empty but keeps its schema."""
    df = _monte_carlo_results(include_path_data=False).to_paths_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == PATH_COLUMNS
    assert len(df) == 0


def test_monte_carlo_to_paths_dataframe_has_one_row_per_path_period() -> None:
    """Row count is ``n_paths * metrics * forecast periods``."""
    n_paths = 8
    results = _monte_carlo_results(include_path_data=True, n_paths=n_paths)
    df = results.to_paths_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert set(PATH_COLUMNS) <= set(df.columns)
    assert set(df["metric"]) == {"revenue"}
    assert len(df) == n_paths * len(results.forecast_periods)
    assert sorted(set(df["path_id"])) == list(range(n_paths))
    assert set(df["period"]) == set(results.forecast_periods)
