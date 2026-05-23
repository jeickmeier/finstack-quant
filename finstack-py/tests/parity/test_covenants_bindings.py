"""Focused parity checks for the covenant JSON binding slice."""

from __future__ import annotations

import json

from finstack import covenants


def test_covenant_template_roundtrip_and_evaluate() -> None:
    specs_json = covenants.lbo_standard(5.0, 1.5, 1.2, 10_000_000.0)
    specs = json.loads(specs_json)
    engine_json = json.dumps({"specs": [specs[0]], "breach_history": [], "windows": [], "waivers": []})

    canonical_engine = covenants.validate_covenant_engine(engine_json)
    reports = json.loads(
        covenants.evaluate_engine(
            canonical_engine,
            json.dumps({"debt_to_ebitda": 4.0}),
            "2026-03-31",
        )
    )

    assert reports["Debt/EBITDA <= 5.00x"]["passed"] is True


def test_covenant_report_json_roundtrip() -> None:
    report = {
        "covenant_type": "Debt/EBITDA <= 5.00x",
        "covenant_id": "max_debt_to_ebitda",
        "passed": False,
        "actual_value": 5.5,
        "threshold": 5.0,
        "details": "Exceeded",
        "headroom": -0.1,
    }

    assert json.loads(covenants.validate_covenant_report(json.dumps(report))) == report
