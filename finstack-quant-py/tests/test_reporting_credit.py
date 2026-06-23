# finstack-quant-py/tests/test_reporting_credit.py
from __future__ import annotations

import datetime as dt
import json
from typing import Any

import pytest

from finstack_quant import statements
from finstack_quant.reporting import credit_tearsheet
from finstack_quant.reporting.document import TearSheet
from finstack_quant.statements_analytics import credit_assessment


def _results() -> Any:
    b = statements.ModelBuilder("borrower")
    b.periods("2025Q1..Q4", None)
    b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 108.0), ("2025Q3", 112.0), ("2025Q4", 118.0)])
    b.value("cogs", [("2025Q1", 55.0), ("2025Q2", 58.0), ("2025Q3", 60.0), ("2025Q4", 63.0)])
    b.compute("gross_profit", "revenue - cogs")
    b.value("opex", [("2025Q1", 18.0), ("2025Q2", 19.0), ("2025Q3", 20.0), ("2025Q4", 21.0)])
    b.compute("ebitda", "gross_profit - opex")
    b.value("interest_expense", [("2025Q1", 4.0), ("2025Q2", 4.0), ("2025Q3", 4.0), ("2025Q4", 4.0)])
    b.value("total_debt", [("2025Q1", 300.0), ("2025Q2", 300.0), ("2025Q3", 300.0), ("2025Q4", 300.0)])
    b.value("free_cash_flow", [("2025Q1", 5.0), ("2025Q2", 6.0), ("2025Q3", 7.0), ("2025Q4", 8.0)])
    return statements.Evaluator().evaluate(b.build())


def _assessment(results: Any) -> Any:
    return credit_assessment(results, "2025Q4")


_COVERAGE = [{"instrument": "TLB", "dscr": 1.8, "interest_coverage": 3.2, "ltv": 0.55}]
_COVENANTS = [
    {"covenant": "Max Leverage", "threshold": 4.0, "current": 3.0, "headroom": 1.0, "status": "Pass"},
    {"covenant": "Min Coverage", "threshold": 2.0, "current": 1.5, "headroom": -0.5, "status": "Breach"},
]


def test_credit_tearsheet_renders_all_sections() -> None:
    res = _results()
    ts = credit_tearsheet(
        _assessment(res), results=res, coverage=_COVERAGE, covenants=_COVENANTS, generated=dt.date(2026, 6, 22)
    )
    assert isinstance(ts, TearSheet)
    html = ts.to_html()
    assert "Leverage" in html
    assert "Interest Coverage" in html
    assert "Leverage &amp; Coverage" in html
    assert "Per-Instrument Coverage" in html
    assert "TLB" in html
    assert "Covenant Compliance" in html
    assert "Max Leverage" in html
    assert "EBITDA Build" in html


def test_credit_tearsheet_accepts_json_assessment() -> None:
    res = _results()
    html = credit_tearsheet(json.dumps(_assessment(res)), results=res, generated=dt.date(2026, 6, 22)).to_html()
    assert "Credit Assessment" in html


def test_credit_tearsheet_deterministic() -> None:
    res = _results()
    a = credit_tearsheet(_assessment(res), results=res, generated=dt.date(2026, 6, 22)).to_html()
    b = credit_tearsheet(_assessment(res), results=res, generated=dt.date(2026, 6, 22)).to_html()
    assert a == b


def test_credit_tearsheet_optional_sections_omitted() -> None:
    res = _results()
    html = credit_tearsheet(_assessment(res), results=res, generated=dt.date(2026, 6, 22)).to_html()
    assert "Per-Instrument Coverage" not in html
    assert "Covenant Compliance" not in html


def test_credit_tearsheet_rejects_unknown_section() -> None:
    res = _results()
    with pytest.raises(ValueError, match="unknown section"):
        credit_tearsheet(_assessment(res), sections=["ratios", "nope"])
