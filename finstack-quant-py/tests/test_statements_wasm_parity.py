"""Cross-host goldens for the statements and statements-analytics parity surface.

Mirrors ``finstack-quant-wasm/tests/facade/statements_parity.test.mjs`` and
``statements_analytics_parity.test.mjs``. All three read their inputs from
``tests/fixtures/statements_parity_inputs.json`` and assert the outputs pinned
in ``tests/fixtures/statements_parity_expected.json``, so the Python bindings
and the WASM facade are checked against the same numbers and JSON documents.

Regenerate the pinned outputs after an intentional change with
``UPDATE_STATEMENTS_PARITY=1 uv run --no-sync pytest <this file>`` and review
the diff.
"""

from __future__ import annotations

import json
import math
import os
from pathlib import Path
from typing import Any

import pytest

from finstack_quant import statements as st, statements_analytics as sa
from finstack_quant.core.money import Money

FIXTURES = Path(__file__).parent / "fixtures"
INPUTS: dict[str, Any] = json.loads((FIXTURES / "statements_parity_inputs.json").read_text())
EXPECTED_PATH = FIXTURES / "statements_parity_expected.json"

MODEL = json.dumps(INPUTS["model"])
Q1, Q2 = "2025Q1", "2025Q2"


def doc(obj: Any) -> Any:
    """The JSON document of a typed binding object."""
    return json.loads(obj.to_json())


def money(value: Money | None) -> dict[str, Any] | None:
    return None if value is None else {"amount": float(value.amount), "currency": value.currency.code}


def built_model() -> st.FinancialModelSpec:
    builder = st.ModelBuilder("built")
    builder.periods("2025Q1..Q2", "2025Q1")
    builder.value_scalar("revenue", {Q1: 100.0})
    builder.forecast("revenue", st.ForecastSpec.growth(0.05))
    builder.compute("profit", "revenue * 0.5")
    builder.value_money("cash", {Q1: Money(100.0, "USD"), Q2: Money(110.0, "USD")})
    builder.with_meta("currency", "USD")
    mixed = builder.mixed("capex")
    mixed.values({Q1: 10.0})
    mixed.formula("revenue * 0.1")
    mixed.name("Capex")
    return mixed.build().build()


def statements_cases() -> dict[str, Any]:
    result = st.Evaluator().evaluate(MODEL)
    model = st.FinancialModelSpec.from_json(MODEL)

    built = built_model()
    built_result = st.Evaluator().evaluate(built)

    config = st.NormalizationConfig.from_json(json.dumps(INPUTS["normalization_config"]))
    normalized = st.normalize(result, config)

    one_off = st.Adjustment.fixed("one_off", "One-off cost", {Q1: 5.0})
    synergy = st.Adjustment.percentage("synergy", "Synergies", "revenue", 0.1)
    extended = st.NormalizationConfig("profit")
    extended.add_adjustment(one_off)
    extended.add_adjustment(synergy.with_cap("revenue", 0.05))

    suite = st.CheckSuiteSpec.from_json(json.dumps(INPUTS["check_suite"]))
    evaluator = st.Evaluator()
    evaluator.with_checks(suite)
    report = evaluator.evaluate(MODEL).check_report
    assert report is not None

    cs = st.CapitalStructureCashflows.from_json(json.dumps(INPUTS["cs_cashflows"]))

    registry = st.Registry.with_builtins()
    first = registry.metric_ids()[0]

    return {
        "result.get": result.get("profit", Q2),
        "result.get_missing": result.get("nope", Q2),
        "result.get_or": result.get_or("nope", Q2, -1.0),
        "result.all_periods": [list(pair) for pair in result.all_periods("profit")],
        "result.get_node": result.get_node("profit"),
        "result.get_node_missing": result.get_node("nope"),
        "result.node_ids": result.node_ids(),
        "result.get_scalar": result.get_scalar("margin", Q1),
        "result.get_money": money(built_result.get_money("cash", Q2)),
        "result.get_money_of_scalar": money(built_result.get_money("revenue", Q2)),
        "result.get_scalar_of_money": built_result.get_scalar("cash", Q2),
        "model.has_node": [model.has_node("profit"), model.has_node("nope")],
        "model.get_node": doc(model.get_node("profit")),
        "model.node_ids": model.node_ids(),
        "builder.node_ids": built.node_ids(),
        "builder.content_hash": built.content_hash(),
        "builder.values": [
            built_result.get("revenue", Q2),
            built_result.get("profit", Q2),
            built_result.get("capex", Q1),
            built_result.get("capex", Q2),
        ],
        "normalize": [doc(row) for row in normalized],
        "normalize_json": json.loads(st.normalize_json(result, config)),
        "forecast_specs": [
            doc(spec)
            for spec in (
                st.ForecastSpec.forward_fill(),
                st.ForecastSpec.growth(0.05),
                st.ForecastSpec.curve([0.05, 0.04]),
                st.ForecastSpec.normal(100.0, 10.0, 7),
                st.ForecastSpec.log_normal(0.0, 0.2, 7),
                st.ForecastSpec.override({Q2: 125.0}),
                st.ForecastSpec.seasonal([1.0, 2.0, 3.0, 4.0], 4, "additive"),
                st.ForecastSpec.time_series([1.0, 2.0, 3.0]),
                st.ForecastSpec.fade_to_target(50.0),
                st.ForecastSpec.mean_reverting(100.0, 0.5, 5.0, 11),
                st.ForecastSpec.bootstrap([1.0, 2.0, 3.0], 13),
            )
        ],
        "adjustments": [
            doc(one_off),
            doc(synergy),
            doc(synergy.with_cap("revenue", 0.05)),
            doc(synergy.with_cap_mode(None, 3.0, "progressive")),
            doc(one_off.with_category("non_recurring")),
        ],
        "normalization_config.add_adjustment": doc(extended),
        "check_report": {
            "has_errors": report.has_errors(),
            "has_warnings": report.has_warnings(),
            "errors": [[f.check_id, f.period] for f in report.findings_by_severity("error")],
            "warnings": len(report.findings_by_severity("warning")),
        },
        "builtin_check_names": st.CheckSuiteSpec.builtin_check_names(),
        "cs": {
            "interest": cs.get_interest("TL-A", Q1),
            "interest_cash": cs.get_interest_cash("TL-A", Q1),
            "interest_pik": cs.get_interest_pik("TL-A", Q1),
            "principal": cs.get_principal("TL-A", Q1),
            "debt_balance": cs.get_debt_balance("TL-A", Q1),
            "fees": cs.get_fees("TL-A", Q1),
            "accrued_interest": cs.get_accrued_interest("TL-A", Q1),
            "total_interest": cs.get_total_interest(Q1),
            "total_principal": cs.get_total_principal(Q1),
            "total_debt_balance": cs.get_total_debt_balance(Q1),
            "total_fees": cs.get_total_fees(Q1),
        },
        "registry": {
            "count": len(registry.metric_ids()),
            "first": first,
            "has": [registry.has(first), registry.has("fin.no_such_metric")],
            "definition": doc(registry.get(first)),
            "dependencies": registry.dependencies(first),
        },
    }


def analytics_cases() -> dict[str, Any]:
    result = st.Evaluator().evaluate(MODEL)
    tracer = sa.DependencyTracer(MODEL)

    cork_model = st.ModelBuilder("cork")
    cork_model.periods("2025Q1..Q2", None)
    cork_model.value_scalar("cash", {Q1: 100.0, Q2: 120.0})
    cork_model.value_scalar("inflow", {Q1: 0.0, Q2: 20.0})
    cork_spec = cork_model.build()
    cork_result = st.Evaluator().evaluate(cork_spec)
    corkscrew = sa.CorkscrewExtension(sa.CorkscrewConfig.from_json(json.dumps(INPUTS["corkscrew_config"])))
    scorecard = sa.CreditScorecardExtension(sa.ScorecardConfig.from_json(json.dumps(INPUTS["scorecard_config"])))

    def evaluated(model: st.FinancialModelSpec, node: str, period: str) -> float | None:
        return st.Evaluator().evaluate(model).get(node, period)

    rolled = sa.add_roll_forward_with_opening(MODEL, "debt", ["revenue"], ["cogs"], 10.0)
    vintage = sa.add_vintage_buildup(MODEL, "book", "revenue", [1.0, 0.5])
    noi = sa.add_noi_buildup(MODEL, "total_rev", ["revenue"], "total_exp", ["cogs"], "noi")
    ncf = sa.add_ncf_buildup(noi, "noi", ["cogs"], "ncf")
    lease = sa.LeaseSpec.from_json(json.dumps(INPUTS["lease"]))
    rent_roll = sa.add_rent_roll(MODEL, [lease])
    property_model = sa.add_property_operating_statement(MODEL, [lease], opex_nodes=["cogs"])

    exposure = sa.Exposure(
        id="E1",
        ead=1000.0,
        lgd=0.4,
        eir=0.05,
        remaining_maturity=3.0,
        current_pd=0.03,
        origination_pd=0.02,
    )
    schedule = [tuple(knot) for knot in INPUTS["pd_schedule"]]
    stressed = [(t, pd * 1.5) for t, pd in schedule]

    dcf_builder = st.ModelBuilder("dcf")
    dcf_builder.periods("2025..2026", None)
    dcf_builder.value_money("ufcf", {"2025": Money(100.0, "USD"), "2026": Money(110.0, "USD")})
    dcf_builder.with_meta("currency", "USD")
    dcf_model = dcf_builder.build()
    dcf = sa.evaluate_dcf(dcf_model, 0.10, sa.TerminalValueSpec.gordon_growth(0.02), net_debt_override=0.0)

    with pytest.raises(ValueError, match="terminal_value is required"):
        sa.run_corporate_analysis(MODEL, wacc=0.10)

    samples: list[list[float]] = INPUTS["comps_samples"]
    lbo_model = json.dumps(INPUTS["lbo_model"])
    lbo_config: dict[str, Any] = INPUTS["lbo_config"]
    dcf_json = json.dumps(INPUTS["dcf_model"])

    scenario_set = sa.ScenarioSet.from_json(json.dumps(INPUTS["scenario_set"]))
    scenario_results = sa.evaluate_scenario_set(MODEL, scenario_set)
    down = scenario_results.get("down")
    assert down is not None

    subject = sa.CompanyMetrics.from_json(json.dumps(INPUTS["subject"]))
    universe = [sa.CompanyMetrics.from_json(json.dumps(row)) for row in INPUTS["universe"]]
    peer_filter = sa.PeerFilter.from_json(json.dumps(INPUTS["peer_filter"]))

    sensitivity_config = sa.SensitivityConfig.from_json(json.dumps(INPUTS["sensitivity_config"]))
    sensitivity = sa.run_sensitivity(MODEL, sensitivity_config)
    extended = sa.SensitivityConfig.from_json(json.dumps(INPUTS["sensitivity_config"]))
    extended.add_parameter("cogs", Q2, 50.0, perturbations=[45.0, 55.0])
    with pytest.raises(ValueError, match="scenario index 9 out of range"):
        sensitivity.get_value(9, "profit", Q2)

    return {
        "tracer": {
            "direct": tracer.direct_dependencies("margin"),
            "all": tracer.all_dependencies("margin"),
            "dependents": tracer.dependents("revenue"),
            "tree": doc(tracer.dependency_tree("margin")),
            "text": tracer.dependency_tree_text("margin"),
            "detailed": tracer.dependency_tree_detailed(result, "margin", Q1),
        },
        "corkscrew": {
            "config": doc(corkscrew.config()),
            "report": doc(corkscrew.execute(cork_spec, cork_result)),
        },
        "scorecard": {
            "config": doc(scorecard.config()),
            "report": doc(scorecard.execute(MODEL, result)),
        },
        "templates": {
            "roll_forward.node_ids": rolled.node_ids(),
            "roll_forward.end": [evaluated(rolled, "debt_end", Q1), evaluated(rolled, "debt_end", Q2)],
            "vintage.book": [evaluated(vintage, "book", Q1), evaluated(vintage, "book", Q2)],
            "noi": [evaluated(noi, "noi", Q1), evaluated(noi, "noi", Q2)],
            "ncf": [evaluated(ncf, "ncf", Q1), evaluated(ncf, "ncf", Q2)],
            "rent_roll.node_ids": rent_roll.node_ids(),
            "property.node_ids": property_model.node_ids(),
            "property.noi": [evaluated(property_model, "noi", Q1), evaluated(property_model, "noi", Q2)],
        },
        "ecl": {
            "stage": sa.classify_stage(exposure).stage.value,
            "single": sa.compute_ecl(exposure, schedule, stage="stage1").ecl,
            "lifetime": sa.compute_ecl(exposure, schedule, stage="stage2").ecl,
            "weighted": sa.compute_ecl_weighted(exposure, [(0.6, schedule), (0.4, stressed)], stage="stage2").ecl,
        },
        "dcf": {
            "enterprise_value": float(dcf.enterprise_value.amount),
            "equity_value": float(dcf.equity_value.amount),
            "currency": dcf.enterprise_value.currency.code,
        },
        "corporate": {
            "node_count": sa.run_corporate_analysis(MODEL).statement.node_count,
            "equity": sa.run_corporate_analysis(MODEL).equity,
        },
        "terminal_specs": [
            doc(sa.TerminalValueSpec.gordon_growth(0.02)),
            doc(sa.TerminalValueSpec.exit_multiple(8.5, 0.0)),
            doc(sa.TerminalValueSpec.h_model(0.08, 0.02, 5.0)),
        ],
        "scenario_diff": doc(sa.scenario_diff(scenario_set, scenario_results, "base", "down", ["profit"], [Q2])),
        "variance_bridge": doc(sa.variance_bridge(result, down, "profit", Q2, ["revenue"], "base", "down")),
        "scenario_get": [down.get("profit", Q2), scenario_results.get("nope") is None],
        "scenario_trace": scenario_set.trace("down"),
        "pl_summary_text": sa.pl_summary_report(result, ["revenue", "profit"], [Q1, Q2]).to_text(),
        "company_get": [subject.get("ebitda"), subject.get("rule_of_40"), subject.get("nope")],
        "peer_accepts": [peer_filter.accepts(row) for row in universe],
        "peer_set": [
            row["id"] for row in doc(sa.PeerSet.from_universe(subject, universe, peer_filter, "ltm"))["peers"]
        ],
        "sensitivity": {
            "config": doc(extended),
            "parameter_value": sensitivity.get_parameter_value(0, f"revenue@{Q2}"),
            "parameter_missing": sensitivity.get_parameter_value(0, f"cogs@{Q2}"),
            "value": sensitivity.get_value(0, "profit", Q2),
        },
        "explanation_text": sa.explain_formula(MODEL, result, "profit", Q1).to_text(),
        "forecast_summary": sa.backtest_forecast([100.0, 110.0], [98.0, 112.0]).summary(),
        # Execution goldens for the comps statistics, the LBO and the DCF tornado
        # (audit finding F240). The empty and one-element samples pin the Rust
        # "no statistic" answers on both hosts.
        "comps_stats": {
            "peer_stats": [None if stats is None else doc(stats) for stats in map(sa.peer_stats, samples)],
            "percentile_rank": [sa.percentile_rank(sample, 2.5) for sample in samples],
            "z_score": [sa.z_score(sample, 2.5) for sample in samples],
        },
        "lbo": {
            "plain": doc(sa.evaluate_lbo(lbo_model, lbo_config)),
            "checked": doc(sa.evaluate_lbo(lbo_model, {**lbo_config, "check_mappings": INPUTS["lbo_check_mappings"]})),
        },
        "dcf_sensitivity": {
            "gordon_growth": doc(
                sa.dcf_sensitivity(dcf_json, 0.10, sa.TerminalValueSpec.gordon_growth(0.02), "ufcf", 0.0)
            ),
            "exit_multiple": doc(
                sa.dcf_sensitivity(
                    dcf_json,
                    0.10,
                    sa.TerminalValueSpec.exit_multiple(8.0, 999.0),
                    "ufcf",
                    0.0,
                    {"exit_multiple_metric_node": "ebitda"},
                )
            ),
        },
    }


def assert_matches(actual: Any, expected: Any, path: str) -> None:
    """Deep equality with a relative tolerance on floats (native vs wasm32)."""
    if isinstance(expected, float) or (isinstance(actual, float) and isinstance(expected, int)):
        assert isinstance(actual, (int, float)), f"{path}: {actual!r} is not a number"
        assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), f"{path}: {actual!r} != {expected!r}"
    elif isinstance(expected, dict):
        assert isinstance(actual, dict), f"{path}: {actual!r} is not an object"
        assert sorted(actual) == sorted(expected), f"{path}: keys {sorted(actual)} != {sorted(expected)}"
        for key, value in expected.items():
            assert_matches(actual[key], value, f"{path}.{key}")
    elif isinstance(expected, list):
        assert isinstance(actual, list), f"{path}: {actual!r} is not an array"
        assert len(actual) == len(expected), f"{path}: length {len(actual)} != {len(expected)}"
        for index, value in enumerate(expected):
            assert_matches(actual[index], value, f"{path}[{index}]")
    else:
        assert actual == expected, f"{path}: {actual!r} != {expected!r}"


def jsonable(value: Any) -> Any:
    """Round-trip through JSON so tuples and the like compare as the WASM side sees them."""
    return json.loads(json.dumps(value))


@pytest.mark.parametrize(
    ("section", "cases"), [("statements", statements_cases), ("statements_analytics", analytics_cases)]
)
def test_python_matches_the_cross_host_golden(section: str, cases: Any) -> None:
    actual = jsonable(cases())
    if os.environ.get("UPDATE_STATEMENTS_PARITY"):
        pinned = json.loads(EXPECTED_PATH.read_text()) if EXPECTED_PATH.exists() else {}
        pinned[section] = actual
        EXPECTED_PATH.write_text(json.dumps(pinned, indent=2, sort_keys=True) + "\n")
    expected = json.loads(EXPECTED_PATH.read_text())[section]
    assert_matches(actual, expected, section)
