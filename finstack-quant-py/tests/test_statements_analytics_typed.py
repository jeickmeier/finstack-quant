"""Typed Python surfaces for statement-analysis configuration and results."""

from __future__ import annotations

import json

import pytest

from finstack_quant.statements import Evaluator, ModelBuilder
from finstack_quant.statements_analytics import (
    MonteCarloConfig,
    MonteCarloResults,
    ScenarioResultSet,
    ScenarioSet,
    SensitivityConfig,
    SensitivityResult,
    VarianceConfig,
    VarianceReport,
    evaluate_scenario_set,
    generate_tornado_entries,
    run_monte_carlo,
    run_sensitivity,
    run_variance,
)


def _model() -> object:
    builder = ModelBuilder("typed-analytics")
    builder.periods("2025Q1..Q2", "2025Q1")
    builder.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    builder.value("cost", [("2025Q1", 60.0), ("2025Q2", 65.0)])
    builder.compute("profit", "revenue - cost")
    return builder.build()


def test_typed_configs_round_trip_structured_fields() -> None:
    sensitivity = SensitivityConfig(
        mode="Diagonal",
        parameters=[("revenue", "2025Q2", 110.0, [100.0, 110.0, 120.0])],
        target_metrics=["profit"],
    )
    sensitivity_doc = json.loads(sensitivity.to_json())
    assert sensitivity_doc["parameters"][0]["period_id"] == "2025Q2"
    assert SensitivityConfig.from_json(sensitivity.to_json()).target_metrics == ["profit"]

    variance = VarianceConfig("base", "upside", ["profit"], ["2025Q1", "2025Q2"])
    assert VarianceConfig.from_json(variance.to_json()).periods == ["2025Q1", "2025Q2"]

    scenarios = ScenarioSet(
        {"base": {}, "downside": {"revenue": 90.0}},
        parents={"downside": "base"},
        model_ids={"base": "typed-analytics"},
    )
    scenario_doc = json.loads(scenarios.to_json())
    assert scenario_doc["scenarios"]["downside"]["parent"] == "base"
    assert ScenarioSet.from_json(scenarios.to_json()).names == ["base", "downside"]

    monte_carlo = MonteCarloConfig(
        n_paths=8,
        seed=17,
        percentiles=[0.1, 0.5, 0.9],
        include_path_data=False,
    )
    assert MonteCarloConfig.from_json(monte_carlo.to_json()).n_paths == 8
    assert monte_carlo.percentiles == [0.1, 0.5, 0.9]


def test_monte_carlo_config_json_uses_canonical_omitted_field_defaults() -> None:
    structured = MonteCarloConfig(n_paths=8, seed=17)
    compatible_json = MonteCarloConfig.from_json('{"n_paths":8,"seed":17}')

    expected_percentiles = [0.05, 0.5, 0.95]
    assert structured.percentiles == expected_percentiles
    assert compatible_json.percentiles == expected_percentiles
    assert not structured.include_path_data
    assert not compatible_json.include_path_data


def test_analysis_functions_accept_typed_configs_and_return_typed_results() -> None:
    model = _model()
    sensitivity = run_sensitivity(
        model,
        SensitivityConfig(
            "Diagonal",
            [("revenue", "2025Q2", 110.0, [100.0, 110.0, 120.0])],
            ["profit"],
        ),
    )
    assert isinstance(sensitivity, SensitivityResult)
    assert len(sensitivity) == 3
    assert sensitivity.target_metrics == ["profit"]
    assert sensitivity.get_parameter_value(0, "revenue@2025Q2") == pytest.approx(100.0)
    assert sensitivity.get_value(0, "profit", "2025Q2") == pytest.approx(35.0)
    assert json.loads(generate_tornado_entries(sensitivity, "profit", "2025Q2"))
    assert SensitivityResult.from_json(sensitivity.to_json()).get_value(2, "profit", "2025Q2") == pytest.approx(55.0)

    base = Evaluator().evaluate(model)
    comparison_builder = ModelBuilder("comparison")
    comparison_builder.periods("2025Q1..Q2", "2025Q1")
    comparison_builder.value("revenue", [("2025Q1", 105.0), ("2025Q2", 115.0)])
    comparison_builder.value("cost", [("2025Q1", 60.0), ("2025Q2", 65.0)])
    comparison_builder.compute("profit", "revenue - cost")
    comparison = Evaluator().evaluate(comparison_builder.build())
    variance = run_variance(
        base,
        comparison,
        VarianceConfig("base", "comparison", ["profit"], ["2025Q1", "2025Q2"]),
    )
    assert isinstance(variance, VarianceReport)
    assert variance.baseline_label == "base"
    assert variance.comparison_label == "comparison"
    assert variance.rows[0].metric == "profit"
    assert variance.rows[0].abs_var == pytest.approx(5.0)
    assert VarianceReport.from_json(variance.to_json()).rows[1].period == "2025Q2"

    scenario_results = evaluate_scenario_set(
        model,
        ScenarioSet({"base": {}, "downside": {"revenue": 90.0}}),
    )
    assert isinstance(scenario_results, ScenarioResultSet)
    assert scenario_results.names == ["base", "downside"]
    downside = scenario_results.get("downside")
    assert downside is not None
    assert downside.get("profit", "2025Q2") == pytest.approx(25.0)
    assert ScenarioResultSet.from_json(scenario_results.to_json()).get("missing") is None


def test_analysis_functions_retain_json_config_input() -> None:
    model = _model()
    config_json = SensitivityConfig(
        "Diagonal",
        [("revenue", "2025Q2", 110.0, [100.0, 120.0])],
        ["profit"],
    ).to_json()
    result = run_sensitivity(model.to_json(), config_json)
    assert isinstance(result, SensitivityResult)
    assert len(result) == 2


def test_statement_monte_carlo_result_is_typed_and_deterministic() -> None:
    model = _model()
    config = MonteCarloConfig(4, 123, [0.5])
    first = run_monte_carlo(model, config)
    second = run_monte_carlo(model, config.to_json())
    assert isinstance(first, MonteCarloResults)
    assert first.n_paths == 4
    assert first.percentiles == [0.5]
    assert first.forecast_periods == ["2025Q2"]
    assert first.to_json() == second.to_json()
    assert MonteCarloResults.from_json(first.to_json()).n_paths == 4


def test_statement_monte_carlo_percentile_series_preserves_period_order() -> None:
    periods = ["2025M01", "2025M02", "2025M03", "2025M04", "2025M05", "2025M06"]
    builder = ModelBuilder("ordered-percentiles")
    builder.periods("2025M01..M06", "2025M01")
    builder.value(
        "revenue",
        list(zip(periods, [100.0, 101.0, 102.0, 103.0, 104.0, 105.0], strict=True)),
    )
    model = builder.build()

    results = run_monte_carlo(model, MonteCarloConfig(2, 7, [0.5]))
    series = results.get_percentile_series("revenue", 0.5)

    assert series is not None
    assert list(series) == periods[1:]
