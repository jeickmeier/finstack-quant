"""Statements behaviour Rust owns for both hosts.

Mirrors ``finstack-quant-wasm/tests/facade/statements_rust_owned.test.mjs``:
the same inputs go through the Python bindings and must produce the same
results (model ingest and validation, ``GoalSeekResult``, ``DependencyTree``,
``LboConfig`` / ``DcfOptions`` inputs, non-finite sentinels, strict mappings).
"""

from __future__ import annotations

import json
import pickle

import pytest

from finstack_quant.statements import (
    CheckConfig,
    CheckSuiteSpec,
    EcfSweepSpec,
    Evaluator,
    FinancialModelSpec,
    FormulaCheckSpec,
    PikToggleSpec,
)
from finstack_quant.statements_analytics import (
    DependencyTracer,
    DependencyTree,
    ForecastMetrics,
    GoalSeekResult,
    LboCheckMappings,
    ThreeStatementMapping,
    backtest_forecast,
    compute_multiple,
    dcf_sensitivity,
    evaluate_dcf,
    evaluate_lbo,
    explain_formula,
    goal_seek,
    percentile_rank,
    run_checks,
    run_credit_underwriting_checks,
)


def usd(amount: float) -> dict[str, str]:
    return {"amount": str(amount), "currency": "USD"}


def quarter(q: int, year: int = 2025) -> dict[str, object]:
    start_month = (q - 1) * 3 + 1
    end_year, end_month = (year + 1, 1) if q == 4 else (year, start_month + 3)
    return {
        "id": f"{year}Q{q}",
        "start": f"{year}-{start_month:02d}-01",
        "end": f"{end_year}-{end_month:02d}-01",
        "is_actual": False,
    }


def goal_seek_model() -> str:
    return json.dumps({
        "id": "goal-seek",
        "schema_version": 1,
        "periods": [quarter(1)],
        "nodes": {
            "revenue": {"node_id": "revenue", "node_type": "value", "values": {"2025Q1": 100}},
            "profit": {"node_id": "profit", "node_type": "calculated", "formula_text": "revenue * 0.5"},
        },
    })


def lbo_model() -> str:
    ids = ["2025Q1", "2025Q2", "2025Q3", "2025Q4", "2026Q1"]

    def series(values: list[float]) -> dict[str, dict[str, str]]:
        return {pid: usd(v) for pid, v in zip(ids, values, strict=True)}

    return json.dumps({
        "id": "lbo",
        "schema_version": 1,
        "meta": {"currency": "USD"},
        "periods": [quarter(1), quarter(2), quarter(3), quarter(4), quarter(1, 2026)],
        "nodes": {
            "ebitda": {"node_id": "ebitda", "node_type": "value", "values": series([100, 100, 100, 100, 120])},
            "net_debt": {"node_id": "net_debt", "node_type": "value", "values": series([300, 300, 300, 300, 200])},
        },
    })


LBO_CONFIG = {
    "entry_multiple": 8.0,
    "entry_metric_node": "ebitda",
    "transaction_fees": 0.0,
    "sources": [{"name": "debt", "amount": 500.0}],
    "exit_multiple": 9.0,
    "exit_metric_node": "ebitda",
    "exit_net_debt_node": "net_debt",
    "exit_period": "2026Q1",
}

LBO_CHECK_MAPPINGS = {
    "three_statement": {
        "assets_nodes": ["ebitda"],
        "liabilities_nodes": ["net_debt"],
        "equity_nodes": ["ebitda"],
        "cash_node": "net_debt",
        "retained_earnings_node": "ebitda",
        "net_income_node": "ebitda",
    },
    "credit": {"debt_node": "net_debt", "ebitda_node": "ebitda", "interest_expense_node": "ebitda"},
}


def dcf_model() -> str:
    def annual(year: int) -> dict[str, object]:
        return {"id": str(year), "start": f"{year}-01-01", "end": f"{year + 1}-01-01", "is_actual": False}

    return json.dumps({
        "id": "dcf",
        "schema_version": 1,
        "meta": {"currency": "USD"},
        "periods": [annual(2025), annual(2026)],
        "nodes": {
            "ufcf": {"node_id": "ufcf", "node_type": "value", "values": {"2025": usd(100), "2026": usd(110)}},
            "ebitda": {"node_id": "ebitda", "node_type": "value", "values": {"2025": usd(50), "2026": usd(60)}},
        },
    })


EXIT_MULTIPLE = {"type": "exit_multiple", "terminal_metric": 999.0, "multiple": 8.0}


def test_goal_seek_returns_the_rust_result_with_one_update_model_owner() -> None:
    updated = goal_seek(goal_seek_model(), "profit", "2025Q1", 60.0, "revenue", "2025Q1", True)
    assert updated.solved_value == pytest.approx(120.0)
    assert updated.model is not None
    wire = json.loads(updated.to_json())
    assert sorted(wire) == ["converged", "evaluations", "model", "residual", "solved_value", "tolerance"]
    # Solve diagnostics: the residual of the reported driver, its tolerance and the evaluation count.
    assert updated.converged is True
    assert updated.tolerance == pytest.approx(1e-9 * 60.0)
    assert abs(updated.residual) <= updated.tolerance
    assert updated.evaluations >= 3
    solved = Evaluator().evaluate(updated.model)
    assert solved.get("profit", "2025Q1") - 60.0 == pytest.approx(updated.residual, abs=1e-12)
    restored = GoalSeekResult.from_json(updated.to_json())
    assert (restored.residual, restored.evaluations, restored.converged) == (
        updated.residual,
        updated.evaluations,
        True,
    )
    # JSON written before the diagnostics existed still loads, flagged by evaluations == 0.
    legacy = GoalSeekResult.from_json('{"solved_value": 2.5, "model": null}')
    assert (legacy.evaluations, legacy.converged) == (0, False)
    assert wire["model"]["nodes"]["revenue"]["values"]["2025Q1"] == pytest.approx(120.0)
    assert GoalSeekResult.from_json(updated.to_json()).solved_value == pytest.approx(120.0)
    assert pickle.loads(pickle.dumps(updated)).solved_value == pytest.approx(120.0)  # noqa: S301 - trusted in-process round trip

    bare = goal_seek(goal_seek_model(), "profit", "2025Q1", 60.0, "revenue", "2025Q1", False, (1.0, 200.0))
    assert bare.model is None
    assert json.loads(bare.to_json())["model"] is None
    # update_model has no binding-invented default: Rust and WASM require it.
    with pytest.raises(TypeError):
        goal_seek(goal_seek_model(), "profit", "2025Q1", 60.0, "revenue", "2025Q1")  # type: ignore[call-arg]


def test_evaluate_lbo_takes_the_rust_lbo_config_and_runs_its_check_suite() -> None:
    plain = evaluate_lbo(lbo_model(), LBO_CONFIG)
    # MOIC = (9 x 120 - 200) / (8 x 100 - 500) = 880 / 300.
    assert plain.moic == pytest.approx(880 / 300, abs=1e-12)
    assert plain.checks is None

    checked = evaluate_lbo(lbo_model(), {**LBO_CONFIG, "check_mappings": LBO_CHECK_MAPPINGS})
    assert checked.checks is not None
    assert checked.checks.total_checks > 0
    # A typed LboCheckMappings may sit in place of its dict.
    typed = evaluate_lbo(
        lbo_model(),
        {**LBO_CONFIG, "check_mappings": LboCheckMappings.from_json(json.dumps(LBO_CHECK_MAPPINGS))},
    )
    assert json.loads(typed.to_json())["checks"] == json.loads(checked.to_json())["checks"]
    # JSON text is accepted too.
    assert evaluate_lbo(lbo_model(), json.dumps(LBO_CONFIG)).moic == pytest.approx(plain.moic)

    with pytest.raises(ValueError, match="transaction_fee"):
        evaluate_lbo(lbo_model(), {**LBO_CONFIG, "transaction_fee": 1.0})
    missing_fees = {key: value for key, value in LBO_CONFIG.items() if key != "transaction_fees"}
    with pytest.raises(ValueError, match="transaction_fees"):
        evaluate_lbo(lbo_model(), missing_fees)


def test_dcf_sensitivity_takes_the_rust_dcf_options() -> None:
    explicit = dcf_sensitivity(dcf_model(), 0.1, EXIT_MULTIPLE, "ufcf", 0.0)
    by_node = dcf_sensitivity(dcf_model(), 0.1, EXIT_MULTIPLE, "ufcf", 0.0, {"exit_multiple_metric_node": "ebitda"})
    assert by_node.baseline_enterprise_value.amount != explicit.baseline_enterprise_value.amount
    # ufcf_node defaults to the Rust DEFAULT_UFCF_NODE ("ufcf").
    defaulted = dcf_sensitivity(
        dcf_model(), 0.1, EXIT_MULTIPLE, net_debt_override=0.0, options={"exit_multiple_metric_node": "ebitda"}
    )
    assert defaulted.to_json() == by_node.to_json()
    relative = dcf_sensitivity(dcf_model(), 0.1, EXIT_MULTIPLE, "ufcf", 0.0, {"exit_multiple_bump": {"relative": 0.1}})
    assert relative.entries
    with pytest.raises(ValueError, match="exit_multiple"):
        dcf_sensitivity(dcf_model(), 0.1, EXIT_MULTIPLE, "ufcf", 0.0, {"exit_multiple": 1})


def test_evaluate_dcf_exports_the_working_behind_enterprise_value() -> None:
    result = evaluate_dcf(dcf_model(), 0.1, EXIT_MULTIPLE, "ufcf", 25.0)
    assert result.wacc == 0.1
    rows = result.periods
    assert [row["period_id"] for row in rows] == ["2025", "2026"]
    assert [row["date"] for row in rows] == ["2025-12-31", "2026-12-31"]
    assert [row["free_cash_flow"] for row in rows] == [100.0, 110.0]
    for row in rows:
        assert row["discount_factor"] == pytest.approx(1.1 ** -row["discount_years"], rel=1e-14)
        assert row["present_value"] == pytest.approx(row["free_cash_flow"] * row["discount_factor"], rel=1e-14)
    # Rows sum to the explicit PV; explicit PV plus terminal PV is enterprise value.
    explicit = sum(row["present_value"] for row in rows)
    assert result.pv_explicit.amount == pytest.approx(explicit, abs=0.01)
    assert explicit + result.terminal_value_pv.amount == pytest.approx(result.enterprise_value.amount, abs=0.02)
    # The undiscounted terminal value (multiple x metric) discounts to its PV.
    assert result.terminal_value.amount == pytest.approx(8.0 * 999.0)
    assert result.terminal_value.amount / 1.1**result.terminal_discount_years == pytest.approx(
        result.terminal_value_pv.amount, abs=0.01
    )
    # Gross debt and cash are reported separately and net to net debt.
    bridge = result.equity_bridge
    assert (bridge.total_debt, bridge.cash) == (25.0, 0.0)
    assert bridge.total_debt - bridge.cash == pytest.approx(result.net_debt.amount)

    frame = result.to_dataframe()
    assert list(frame.columns[-6:]) == [
        "wacc",
        "pv_explicit",
        "terminal_value",
        "total_debt",
        "cash",
        "terminal_discount_years",
    ]
    assert frame["pv_explicit"].iloc[0] == pytest.approx(explicit, abs=0.01)
    restored = type(result).from_json(result.to_json())
    assert [row["period_id"] for row in restored.periods] == ["2025", "2026"]
    assert [row["present_value"] for row in restored.periods] == pytest.approx([row["present_value"] for row in rows])
    assert restored.equity_bridge.total_debt == 25.0


def test_explain_formula_reports_the_cell_source() -> None:
    model = json.dumps({
        "id": "explain",
        "schema_version": 1,
        "periods": [{**quarter(1), "is_actual": True}, quarter(2)],
        "nodes": {
            "revenue": {
                "node_id": "revenue",
                "node_type": "mixed",
                "values": {"2025Q1": 100},
                "forecast": {"method": "growth_pct", "params": {"rate": 0.05}},
            },
            "cogs": {"node_id": "cogs", "node_type": "calculated", "formula_text": "revenue * 0.4"},
            "gross_profit": {"node_id": "gross_profit", "node_type": "calculated", "formula_text": "revenue - cogs"},
        },
    })
    results = Evaluator().evaluate(FinancialModelSpec.from_json(model))

    actual = explain_formula(model, results, "revenue", "2025Q1")
    assert (actual.source, actual.forecast, actual.breakdown) == ("value", None, [])

    forecast = explain_formula(model, results, "revenue", "2025Q2")
    assert forecast.final_value == pytest.approx(105.0)
    assert forecast.source == "forecast"
    assert forecast.forecast == {"method": "growth_pct", "params": {"rate": 0.05}}
    assert "Source: forecast" in forecast.to_text()

    # A sum/difference formula is a signed step trace that reconciles to the value.
    profit = explain_formula(model, results, "gross_profit", "2025Q2")
    assert profit.source == "formula"
    assert [(step.component, step.operation) for step in profit.breakdown] == [("revenue", "+"), ("cogs", "-")]
    signed = sum(step.value if step.operation == "+" else -step.value for step in profit.breakdown)
    assert signed == pytest.approx(profit.final_value)
    # Any other formula lists its components without an operation.
    cogs = explain_formula(model, results, "cogs", "2025Q2")
    assert [(step.component, step.operation) for step in cogs.breakdown] == [("revenue", None)]


def test_model_ingest_is_rust_from_json() -> None:
    empty = json.dumps({"id": "empty", "schema_version": 1, "periods": [], "nodes": {}})
    with pytest.raises(ValueError, match="at least one period"):
        FinancialModelSpec.from_json(empty)
    with pytest.raises(ValueError, match="at least one period"):
        DependencyTracer(empty)
    with pytest.raises(ValueError, match="invalid FinancialModelSpec JSON"):
        FinancialModelSpec.from_json("{")


def test_capital_structure_sub_specs_validate() -> None:
    with pytest.raises(ValueError, match="sweep_percentage"):
        EcfSweepSpec("ebitda", 5.0).validate()
    EcfSweepSpec("ebitda", 0.5).validate()
    with pytest.raises(ValueError, match="target_instrument_ids"):
        PikToggleSpec("cash", 1.0, []).validate()
    PikToggleSpec("cash", 1.0, ["TL"]).validate()


def test_compute_multiple_treats_none_as_missing() -> None:
    assert compute_multiple({"enterprise_value": 8500.0, "ebitda": 1000.0, "revenue": None}, "ev_ebitda") == 8.5
    assert compute_multiple({"enterprise_value": 8500.0, "ebitda": None}, "ev_ebitda") is None
    assert percentile_rank(values=[1.0, 2.0, 3.0, 4.0], value=2.5) == 0.5


def test_non_finite_values_use_the_nan_sentinel() -> None:
    metrics = backtest_forecast([0.0, 0.0], [1.0, 2.0])
    # An undefined MAPE (every actual is zero) is Rust `None` / JSON null, as in WASM; it is not a NaN sentinel.
    assert json.loads(metrics.to_json())["mape"] is None
    assert metrics.mape_effective_n == 0
    restored = ForecastMetrics.from_json(metrics.to_json())
    assert restored.mape is None
    assert pickle.loads(pickle.dumps(metrics)).mape is None  # noqa: S301 - trusted in-process round trip

    model = json.dumps({
        "id": "lag",
        "schema_version": 1,
        "periods": [quarter(1), quarter(2)],
        "nodes": {
            "revenue": {"node_id": "revenue", "node_type": "value", "values": {"2025Q1": 100, "2025Q2": 110}},
            "lagged": {"node_id": "lagged", "node_type": "calculated", "formula_text": "lag(revenue, 1)"},
        },
    })
    results = Evaluator().evaluate(FinancialModelSpec.from_json(model))
    explanation = explain_formula(model, results, "lagged", "2025Q1")
    assert json.loads(explanation.to_json())["final_value"] == "nan"
    # Statement warnings are the serde objects WASM returns, not Debug text.
    assert results.warnings == [{"non_finite_value": {"node_id": "lagged", "period": "2025Q1", "value": "nan"}}]


def test_mappings_reject_unknown_keys() -> None:
    mapping = {
        "debt_node": "revenue",
        "ebitda_node": "revenue",
        "interest_expense_node": "revenue",
        "fcf_nodes": "revenue",
    }
    with pytest.raises(ValueError, match="fcf_nodes"):
        run_credit_underwriting_checks(goal_seek_model(), json.dumps(mapping))


def test_check_specs_have_no_python_only_defaults_or_grammar() -> None:
    with pytest.raises(TypeError):
        FormulaCheckSpec("x", "x", "revenue > 0", "m")  # type: ignore[call-arg]
    spec = FormulaCheckSpec("x", "x", "revenue > 0", "m", "internal_consistency", "error")
    assert json.loads(spec.to_json())["severity"] == "error"
    with pytest.raises(ValueError, match="dict"):
        CheckSuiteSpec("s", builtin_checks=["non_finite"])  # type: ignore[list-item]
    suite = CheckSuiteSpec("s", builtin_checks=[{"type": "non_finite"}], formula_checks=[spec])
    report = run_checks(goal_seek_model(), suite)
    assert report.total_findings == 0
    with pytest.raises(TypeError):
        ThreeStatementMapping("cash", "re", "ni")  # type: ignore[call-arg]
    assert json.loads(CheckConfig().to_json()) == json.loads(CheckConfig(0.01, 1e-9, 0.0, "info").to_json())


def test_dependency_tree_is_the_rust_tree_and_text_is_named() -> None:
    tracer = DependencyTracer(goal_seek_model())
    tree = tracer.dependency_tree("profit")
    assert isinstance(tree, DependencyTree)
    assert json.loads(tree.to_json()) == {
        "node_id": "profit",
        "formula": "revenue * 0.5",
        "children": [{"node_id": "revenue", "formula": None, "children": []}],
    }
    assert tracer.dependency_tree_text("profit") == "profit (revenue * 0.5)\n└── revenue\n"
    assert DependencyTree.from_json(tree.to_json()).children[0].node_id == "revenue"
