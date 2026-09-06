"""Typed Python surfaces for statement-analysis configuration and results."""

from __future__ import annotations

import json

import pytest

from finstack_quant.statements import (
    Evaluator,
    ModelBuilder,
    MonteCarloConfig,
    MonteCarloResults,
)
from finstack_quant.statements_analytics import (
    BridgeChart,
    ScenarioDiff,
    ScenarioResults,
    ScenarioSet,
    SensitivityConfig,
    SensitivityResult,
    TornadoEntry,
    VarianceConfig,
    VarianceReport,
    evaluate_scenario_set,
    generate_tornado_entries,
    run_sensitivity,
    run_variance,
    scenario_diff,
    variance_bridge,
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
        mode="diagonal",
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
    )
    scenario_doc = json.loads(scenarios.to_json())
    assert scenario_doc["scenarios"]["downside"]["parent"] == "base"
    assert ScenarioSet.from_json(scenarios.to_json()).names == ["base", "downside"]
    with pytest.raises(ValueError, match="unknown field `model_id`"):
        ScenarioSet.from_json('{"scenarios":{"base":{"model_id":"legacy"}}}')

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
            "diagonal",
            [("revenue", "2025Q2", 110.0, [100.0, 110.0, 120.0])],
            ["profit"],
        ),
    )
    assert isinstance(sensitivity, SensitivityResult)
    assert len(sensitivity) == 3
    assert sensitivity.target_metrics == ["profit"]
    assert sensitivity.get_parameter_value(0, "revenue@2025Q2") == pytest.approx(100.0)
    assert sensitivity.get_value(0, "profit", "2025Q2") == pytest.approx(35.0)
    entries = generate_tornado_entries(sensitivity, "profit", "2025Q2")
    assert isinstance(entries[0], TornadoEntry)
    assert entries[0].parameter_id == "revenue"
    assert TornadoEntry.from_json(entries[0].to_json()).swing == pytest.approx(entries[0].swing)
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
    assert isinstance(scenario_results, ScenarioResults)
    assert scenario_results.names == ["base", "downside"]
    downside = scenario_results.get("downside")
    assert downside is not None
    assert downside.get("profit", "2025Q2") == pytest.approx(25.0)
    assert ScenarioResults.from_json(scenario_results.to_json()).get("missing") is None


def test_analysis_functions_retain_json_config_input() -> None:
    model = _model()
    config_json = SensitivityConfig(
        "diagonal",
        [("revenue", "2025Q2", 110.0, [100.0, 120.0])],
        ["profit"],
    ).to_json()
    result = run_sensitivity(model.to_json(), config_json)
    assert isinstance(result, SensitivityResult)
    assert len(result) == 2


def test_statement_monte_carlo_result_is_typed_and_deterministic() -> None:
    model = _model()
    config = MonteCarloConfig(4, 123, [0.5])
    first = Evaluator().evaluate_monte_carlo(model, config)
    second = Evaluator().evaluate_monte_carlo(model, config.to_json())
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

    results = Evaluator().evaluate_monte_carlo(model, MonteCarloConfig(2, 7, [0.5]))
    series = results.percentile_by_period("revenue", 0.5)

    assert series is not None
    assert list(series) == periods[1:]


def test_scenario_analyst_tools_are_reachable_and_decompose_variance() -> None:
    """`trace`, `scenario_diff`, `to_comparison_table` and `variance_bridge` work end to end."""
    model = _model()
    scenarios = ScenarioSet({"base": {}, "downside": {"revenue": 90.0}}, {"downside": "base"})
    results = evaluate_scenario_set(model, scenarios)

    # Lineage resolution walks parent links root-first.
    assert scenarios.trace("downside") == ["base", "downside"]
    assert scenarios.trace("base") == ["base"]

    # Diff carries the scenario names alongside a real variance report.
    diff = scenario_diff(scenarios, results, "base", "downside", ["profit"], ["2025Q2"])
    assert isinstance(diff, ScenarioDiff)
    assert (diff.baseline, diff.comparison) == ("base", "downside")
    row = diff.variance.rows[0]
    assert row.metric == "profit"
    assert row.baseline == pytest.approx(45.0)
    assert row.comparison == pytest.approx(25.0)
    assert row.abs_var == pytest.approx(-20.0)

    # Comparison table has one column per scenario plus the index columns.
    table = results.to_comparison_table(["profit"])
    assert table.num_rows > 0
    assert table.num_columns > 0

    # The bridge attributes the whole profit move to revenue, leaving no residual.
    chart = variance_bridge(
        results.get("base"),
        results.get("downside"),
        "profit",
        "2025Q2",
        ["revenue", "cost"],
        "base",
        "downside",
    )
    assert isinstance(chart, BridgeChart)
    assert chart.target_metric == "profit"
    assert chart.period == "2025Q2"
    assert chart.baseline_value == pytest.approx(45.0)
    assert chart.comparison_value == pytest.approx(25.0)
    contributions = {step.driver: step.contribution for step in chart.steps}
    assert contributions["revenue"] == pytest.approx(-20.0)
    assert contributions["cost"] == pytest.approx(0.0)
    assert chart.unexplained == pytest.approx(0.0)
    assert BridgeChart.from_json(chart.to_json()).comparison_value == pytest.approx(25.0)


def test_scenario_diff_rejects_empty_metrics_and_periods() -> None:
    """Empty metric or period lists are an error, not a silently empty report."""
    model = _model()
    scenarios = ScenarioSet({"base": {}, "downside": {"revenue": 90.0}})
    results = evaluate_scenario_set(model, scenarios)

    with pytest.raises(ValueError, match="metrics cannot be empty"):
        scenario_diff(scenarios, results, "base", "downside", [], ["2025Q2"])
    with pytest.raises(ValueError, match="periods cannot be empty"):
        scenario_diff(scenarios, results, "base", "downside", ["profit"], [])


def test_monetary_goal_seek_round_trip_and_variance_currency_boundary() -> None:
    from finstack_quant.core.money import Money
    from finstack_quant.statements import FinancialModelSpec
    from finstack_quant.statements_analytics import goal_seek

    models = []
    for currency in ["USD", "EUR"]:
        builder = ModelBuilder("typed-boundary")
        builder.periods("2025..2026", None)
        builder.value_money("revenue", [("2025", Money(100.0, currency)), ("2026", Money(110.0, currency))])
        builder.compute("profit", "revenue * 0.5")
        models.append(builder.build())

    outcome = goal_seek(models[0], "profit", "2025", 60.0, "revenue", "2025", bounds=(1.0, 200.0))
    assert outcome.model is not None
    restored = FinancialModelSpec.from_json(outcome.model.to_json())
    revenue = Evaluator().evaluate(restored).get_money("revenue", "2025")
    assert revenue.amount == pytest.approx(120.0)
    assert str(revenue.currency) == "USD"

    baseline, comparison = [Evaluator().evaluate(model) for model in models]
    with pytest.raises(ValueError, match="matching types and currencies"):
        run_variance(baseline, comparison, VarianceConfig("usd", "eur", ["revenue"], ["2025"]))
