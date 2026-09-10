"""Production evidence for forecast continuity and failure-atomic valuation helpers."""

import json

import pytest

from finstack_quant.statements import ModelBuilder
from finstack_quant.statements_analytics import goal_seek, run_checks


def test_goal_seek_rejects_unattainable_target_without_mutation() -> None:
    b = ModelBuilder("discontinuous")
    b.periods("2025Q1..Q1", None)
    b.value("driver", [("2025Q1", 1.0)])
    b.compute("target", "if(driver < 0, 0, 1)")
    model = b.build()
    before = model.to_json()
    for bounds in [None, (-1.0, 1.0)]:
        with pytest.raises(ValueError, match="residual"):
            goal_seek(model, "target", "2025Q1", 0.5, "driver", "2025Q1", bounds=bounds)
        assert model.to_json() == before


def test_residual_tolerance_boundary_and_signed_dividends() -> None:
    b = ModelBuilder("checks")
    b.periods("2025Q1..Q4", None)
    periods = [f"2025Q{q}" for q in range(1, 5)]
    for name, values in {
        "residual": [0.0, 0.1, -0.1, 0.2],
        "income": [100.0] * 4,
        "re": [1000.0, 1080.0, 1160.0, 1240.0],
        "div": [-20.0] * 4,
    }.items():
        b.value(name, list(zip(periods, values, strict=True)))
    suite = {
        "name": "production",
        "formula_checks": [
            {
                "id": "r",
                "name": "Residual",
                "category": "accounting_identity",
                "severity": "error",
                "formula": "residual",
                "message_template": "bad {period}",
                "tolerance": 0.1,
            }
        ],
        "builtin_checks": [
            {
                "type": "retained_earnings_reconciliation",
                "retained_earnings_node": "re",
                "net_income_node": "income",
                "dividends_node": "div",
                "dividends_sign_convention": "inflow_positive",
            }
        ],
    }
    report = json.loads(run_checks(b.build(), json.dumps(suite)).to_json())
    assert report["summary"]["failed"] == 1
    assert [f["period"] for r in report["results"] for f in r["findings"]] == ["2025Q4"]


@pytest.mark.parametrize(
    "forecast_json",
    [
        '{"method":"growth_pct","params":{"rate":0.1}}',
        '{"method":"seasonal","params":{"historical":[100,60,120,80,100,60,120,80],'
        '"season_length":4,"mode":"additive"}}',
    ],
)
def test_explicit_forecast_keeps_growth_anchor_and_seasonal_phase(forecast_json: str) -> None:
    from finstack_quant.statements import Evaluator, FinancialModelSpec

    model = {
        "id": "forecast-continuity",
        "schema_version": 1,
        "periods": [
            {
                "id": f"2025Q{q}",
                "start": f"2025-{q * 3 - 2:02d}-01",
                "end": "2026-01-01" if q == 4 else f"2025-{q * 3 + 1:02d}-01",
                "is_actual": q == 1,
            }
            for q in range(1, 5)
        ],
        "nodes": {
            "revenue": {
                "node_id": "revenue",
                "node_type": "value",
                "values": {"2025Q1": 80.0},
                "forecast": json.loads(forecast_json),
            }
        },
    }
    expected = Evaluator().evaluate(FinancialModelSpec.from_json(json.dumps(model)))
    model["nodes"]["revenue"]["values"]["2025Q2"] = 999.0
    actual = Evaluator().evaluate(FinancialModelSpec.from_json(json.dumps(model)))
    assert actual.get("revenue", "2025Q2") == 999.0
    for period in ["2025Q3", "2025Q4"]:
        assert actual.get("revenue", period) == expected.get("revenue", period)
