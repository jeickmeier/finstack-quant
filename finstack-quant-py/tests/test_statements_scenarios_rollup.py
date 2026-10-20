"""Forecast overrides, capital-structure scenarios, result rollups and formula-check data gaps."""

from __future__ import annotations

from datetime import date
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.statements import (
    CheckSuiteSpec,
    Evaluator,
    ForecastSpec,
    ModelBuilder,
    RollupSpec,
)
from finstack_quant.statements_analytics import (
    ScenarioSet,
    evaluate_scenario_set,
    evaluate_scenario_set_with_market,
)

QUARTERS = [f"2025Q{q}" for q in range(1, 5)]


def _forecast_model() -> object:
    builder = ModelBuilder("forecast-scenarios")
    builder.periods("2025Q1..Q4", "2025Q1")
    builder.value("revenue", [("2025Q1", 100.0)])
    builder.forecast("revenue", ForecastSpec.growth(0.05))
    builder.compute("cogs", "revenue * 0.4")
    return builder.build()


def _bond_model() -> object:
    builder = ModelBuilder("bond-scenarios")
    builder.add_bond(
        id="BOND",
        notional=Money(10_000.0, Currency("USD")),
        coupon_rate=0.05,
        issue_date=date(2025, 1, 1),
        maturity_date=date(2030, 1, 1),
        discount_curve_id="USD-OIS",
    )
    builder.periods("2025Q1..Q4", None)
    builder.value("revenue", [(q, 1_000.0) for q in QUARTERS])
    builder.compute("interest", "cs.interest_expense.total")
    return builder.build()


def test_scenario_forecast_overrides_replace_the_forecast_spec() -> None:
    scenarios = ScenarioSet(
        {
            "base": {},
            "curve": {"revenue": ForecastSpec.curve([0.10, 0.0, -0.10])},
            "flat_cogs": {"cogs": ForecastSpec.forward_fill()},
        },
        parents={"flat_cogs": "curve"},
    )
    results = evaluate_scenario_set(_forecast_model(), scenarios)

    curve = [results.get("curve").get("revenue", q) for q in QUARTERS]
    assert curve == pytest.approx([100.0, 110.0, 110.0, 99.0])
    base_q4 = results.get("base").get("revenue", "2025Q4")
    assert base_q4 == pytest.approx(100.0 * 1.05**3)
    # The child inherits the parent's revenue curve and freezes COGS.
    child = results.get("flat_cogs")
    assert child.get("revenue", "2025Q4") == pytest.approx(99.0)
    assert [child.get("cogs", q) for q in QUARTERS] == pytest.approx([40.0] * 4)

    # The overrides survive the strict wire format.
    doc = json.loads(scenarios.to_json())
    assert doc["scenarios"]["curve"]["forecast_overrides"]["revenue"]["method"] == "curve_pct"
    assert ScenarioSet.from_json(scenarios.to_json()).names == ["base", "curve", "flat_cogs"]


def test_invalid_forecast_override_is_a_value_error() -> None:
    typo = ForecastSpec.from_json('{"method": "growth_pct", "params": {"rate": 0.05}}')
    spec_doc = json.loads(typo.to_json())
    spec_doc["params"]["rat"] = 0.1
    scenarios = ScenarioSet.from_json(
        json.dumps({"scenarios": {"typo": {"forecast_overrides": {"revenue": spec_doc}}}})
    )
    with pytest.raises(ValueError, match="rat"):
        evaluate_scenario_set(_forecast_model(), scenarios)


def test_scenarios_carry_capital_structure_cashflows() -> None:
    model = _bond_model()
    scenarios = ScenarioSet({"base": {}, "down": {"revenue": 900.0}})

    plain = evaluate_scenario_set(model, scenarios)
    priced = evaluate_scenario_set_with_market(model, scenarios, MarketContext(), "2025-01-01")

    for results in (plain, priced):
        for name in ("base", "down"):
            result = results.get(name)
            assert result.cs_cashflows is not None
            # The semi-annual coupon from 1 January pays on 1 July (Q3).
            assert result.get("interest", "2025Q3") == pytest.approx(250.0, rel=0.05)
    assert Evaluator().evaluate(model).cs_cashflows is not None


def test_quarterly_results_roll_up_to_annual() -> None:
    builder = ModelBuilder("rollup")
    builder.periods("2025Q1..Q4", None)
    builder.value("revenue", list(zip(QUARTERS, [100.0, 110.0, 120.0, 130.0], strict=True)))
    builder.value("cash", list(zip(QUARTERS, [10.0, 20.0, 30.0, 40.0], strict=True)))
    builder.compute("ebitda", "revenue * 0.3")
    builder.compute("margin", "ebitda / revenue")
    model = builder.build()
    quarterly = Evaluator().evaluate(model)

    spec = RollupSpec("annual", {"cash": "period_end", "margin": "formula"})
    annual = quarterly.rollup(model, spec)

    assert annual.get("revenue", "2025") == pytest.approx(460.0)
    assert annual.get("ebitda", "2025") == pytest.approx(138.0)
    assert annual.get("cash", "2025") == pytest.approx(40.0)
    assert annual.get("margin", "2025") == pytest.approx(0.3)
    # JSON spec input and round trip.
    assert quarterly.rollup(model, spec.to_json()).get("cash", "2025") == pytest.approx(40.0)
    assert RollupSpec.from_json(spec.to_json()).methods == {"cash": "period_end", "margin": "formula"}

    builder = ModelBuilder("partial")
    builder.periods("2025Q2..Q4", None)
    builder.value("revenue", [(q, 1.0) for q in QUARTERS[1:]])
    partial = builder.build()
    partial_results = Evaluator().evaluate(partial)
    with pytest.raises(ValueError, match="incomplete"):
        partial_results.rollup(partial, spec)
    with pytest.raises(ValueError, match="closing"):
        RollupSpec("annual", {"cash": "closing"})


def test_formula_check_reports_insufficient_data_instead_of_failing() -> None:
    builder = ModelBuilder("checks")
    builder.periods("2025Q1..Q2", None)
    builder.value("revenue", [("2025Q1", 100.0), ("2025Q2", 120.0)])
    model = builder.build()
    suite = CheckSuiteSpec.from_json(
        json.dumps({
            "name": "growth",
            "builtin_checks": [],
            "formula_checks": [
                {
                    "id": "growth_cap",
                    "name": "Growth below 50%",
                    "category": "internal_consistency",
                    "severity": "error",
                    "formula": "revenue / lag(revenue, 1) - 1 < 0.5",
                    "message_template": "Growth above 50% in {period}",
                }
            ],
        })
    )
    report = Evaluator().with_checks(suite).evaluate(model).check_report

    assert report.passed
    assert report.total_findings == 1
    (finding,) = report.findings_by_severity("warning")
    assert finding.period == "2025Q1"
    assert "insufficient data" in finding.message
