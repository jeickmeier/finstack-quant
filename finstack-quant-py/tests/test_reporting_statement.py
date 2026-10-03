from __future__ import annotations

import datetime as dt
from typing import Any

import pytest

from finstack_quant import statements
from finstack_quant.reporting import statement_tearsheet
from finstack_quant.reporting.document import TearSheet


def _results() -> object:
    """Evaluate a small P&L model with margin formula nodes in decimal units."""
    b = statements.ModelBuilder("acme")
    b.periods("2025Q1..Q4", None)
    b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 108.0), ("2025Q3", 112.0), ("2025Q4", 118.0)])
    b.value("cogs", [("2025Q1", 55.0), ("2025Q2", 58.0), ("2025Q3", 60.0), ("2025Q4", 63.0)])
    b.compute("gross_profit", "revenue - cogs")
    b.value("opex", [("2025Q1", 18.0), ("2025Q2", 19.0), ("2025Q3", 20.0), ("2025Q4", 21.0)])
    b.compute("ebitda", "gross_profit - opex")
    b.compute("net_income", "ebitda * 0.7")
    b.compute("ebitda_margin", "ebitda / revenue")
    b.compute("gross_margin", "gross_profit / revenue")
    return statements.Evaluator().evaluate(b.build())


def test_statement_tearsheet_renders_sections_and_kpis() -> None:
    ts = statement_tearsheet(_results(), generated=dt.date(2026, 6, 22))
    assert isinstance(ts, TearSheet)
    html = ts.to_html()
    assert "Income Statement" in html
    assert "Revenue" in html
    assert "EBITDA Margin" in html
    assert "<th>2025Q1</th>" in html
    assert "<th>2025Q4</th>" in html
    assert "Revenue &amp; EBITDA" in html
    assert "Margins" in html


def test_statement_tearsheet_accepts_json_input() -> None:
    html = statement_tearsheet(_results().to_json(), generated=dt.date(2026, 6, 22)).to_html()
    assert "Income Statement" in html


def test_statement_tearsheet_deterministic() -> None:
    a = statement_tearsheet(_results(), generated=dt.date(2026, 6, 22)).to_html()
    b = statement_tearsheet(_results(), generated=dt.date(2026, 6, 22)).to_html()
    assert a == b


def test_statement_tearsheet_rejects_unknown_section() -> None:
    with pytest.raises(ValueError, match="unknown section"):
        statement_tearsheet(_results(), sections=["summary", "nope"])


def test_statement_tearsheet_variance_section() -> None:
    variance = {
        "rows": [
            {
                "period": "2025Q3",
                "metric": "ebitda",
                "baseline": 34.0,
                "comparison": 31.5,
                "abs_var": -2.5,
                "pct_var": -0.0735,
            },
        ]
    }
    html = statement_tearsheet(
        _results(), variance=variance, sections=["variance"], generated=dt.date(2026, 6, 22)
    ).to_html()
    assert "Variance vs Baseline" in html
    assert "ebitda" in html


def test_statement_tearsheet_variance_absent_omits_section() -> None:
    html = statement_tearsheet(_results(), sections=["variance"], generated=dt.date(2026, 6, 22)).to_html()
    assert "Variance vs Baseline" not in html


def test_statement_tearsheet_renders_typed_variance_report() -> None:
    """PYPY-006: a typed ``run_variance`` result renders the variance section."""
    from finstack_quant.statements import Evaluator, ModelBuilder
    from finstack_quant.statements_analytics import VarianceConfig, run_variance

    def model(revenue: float) -> Any:
        b = ModelBuilder("m")
        b.periods("2025Q1..Q1", None)
        b.value("revenue", [("2025Q1", revenue)])
        b.compute("profit", "revenue * 0.5")
        return Evaluator().evaluate(b.build())

    report = run_variance(model(100.0), model(120.0), VarianceConfig("base", "actual", ["profit"], ["2025Q1"]))
    sheet = statement_tearsheet(model(100.0), variance=report, sections=["variance"])
    assert [s.title for s in sheet.sections] == ["Variance vs Baseline"]
    assert "profit" in sheet.sections[0].body


def test_statement_tearsheet_stamps_the_results_own_numeric_mode() -> None:
    """PYPY-008: provenance is read from the result, never asserted."""
    results = _results()
    assert str(results.numeric_mode) == 'NumericMode("float64")'
    sheet = statement_tearsheet(results, sections=[])
    assert sheet.meta_lines == ["Numeric: float64"]
    assert "Decimal mode" not in sheet.to_html()
    # A bare node map carries no stamp: no provenance line is printed.
    assert statement_tearsheet({"nodes": {}}, sections=[]).meta_lines == []
