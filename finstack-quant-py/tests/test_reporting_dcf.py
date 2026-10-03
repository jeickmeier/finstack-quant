from __future__ import annotations

import datetime as dt
import json
from typing import Any

import pytest

from finstack_quant import statements
from finstack_quant.reporting import dcf_tearsheet
from finstack_quant.reporting.document import TearSheet

_VAL: dict[str, Any] = {
    "equity_value": {"amount": "850.00", "currency": "USD"},
    "enterprise_value": {"amount": "1000.00", "currency": "USD"},
    "net_debt": {"amount": "150.00", "currency": "USD"},
    "valuation_discount": {"amount": "0.00", "currency": "USD"},
    "terminal_value_pv": {"amount": "600.00", "currency": "USD"},
    "equity_value_per_share": 8.5,
    "diluted_shares": 100.0,
}

_SENSITIVITY = [
    {"parameter_id": "WACC", "downside": -120.0, "upside": 140.0},
    {"parameter_id": "Terminal Growth", "downside": -80.0, "upside": 90.0},
]


def _results() -> Any:
    b = statements.ModelBuilder("acme")
    b.periods("2025Q1..Q4", None)
    b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 108.0), ("2025Q3", 112.0), ("2025Q4", 118.0)])
    b.compute("ebitda", "revenue * 0.27")
    b.compute("ufcf", "ebitda * 0.6")
    b.compute("net_income", "ebitda * 0.5")
    return statements.Evaluator().evaluate(b.build())


def test_dcf_tearsheet_renders_all_sections() -> None:
    ts = dcf_tearsheet(_VAL, results=_results(), sensitivity=_SENSITIVITY, generated=dt.date(2026, 6, 22))
    assert isinstance(ts, TearSheet)
    html = ts.to_html()
    assert "Enterprise Value" in html
    assert "Equity Value" in html
    assert "Equity Bridge" in html
    assert "Unlevered Free Cash Flow" in html
    assert "Equity Value Sensitivity" in html
    assert "WACC" in html
    assert "Forecast Summary" in html
    assert "USD" in html


def test_dcf_tearsheet_accepts_json_valuation() -> None:
    html = dcf_tearsheet(json.dumps(_VAL), generated=dt.date(2026, 6, 22)).to_html()
    assert "DCF Valuation" in html
    assert "Equity Bridge" in html


def test_dcf_tearsheet_deterministic() -> None:
    a = dcf_tearsheet(_VAL, results=_results(), generated=dt.date(2026, 6, 22)).to_html()
    b = dcf_tearsheet(_VAL, results=_results(), generated=dt.date(2026, 6, 22)).to_html()
    assert a == b


def test_dcf_tearsheet_optional_sections_omitted() -> None:
    html = dcf_tearsheet(_VAL, generated=dt.date(2026, 6, 22)).to_html()
    assert "Unlevered Free Cash Flow" not in html
    assert "Equity Value Sensitivity" not in html
    assert "Forecast Summary" not in html
    assert "Equity Bridge" in html


def test_dcf_tearsheet_rejects_unknown_section() -> None:
    with pytest.raises(ValueError, match="unknown section"):
        dcf_tearsheet(_VAL, sections=["bridge", "nope"])


def test_dcf_tearsheet_rejects_bad_valuation() -> None:
    with pytest.raises(TypeError):
        dcf_tearsheet(12345)
    with pytest.raises(TypeError):
        dcf_tearsheet("[1, 2, 3]")  # valid JSON, not an object


def test_dcf_tearsheet_zero_net_debt_renders_bridge() -> None:
    val = dict(_VAL)
    val["net_debt"] = {"amount": "0", "currency": "USD"}
    html = dcf_tearsheet(val, generated=dt.date(2026, 6, 22)).to_html()
    assert "Equity Bridge" in html


def test_dcf_tearsheet_custom_ufcf_node() -> None:
    b = statements.ModelBuilder("acme2")
    b.periods("2025Q1..Q2", None)
    b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    b.compute("fcff", "revenue * 0.15")
    res = statements.Evaluator().evaluate(b.build())
    html = dcf_tearsheet(
        _VAL, results=res, ufcf_node="fcff", sections=["ufcf"], generated=dt.date(2026, 6, 22)
    ).to_html()
    assert "Unlevered Free Cash Flow" in html


def test_dcf_tearsheet_tolerates_bad_sensitivity() -> None:
    html = dcf_tearsheet(
        _VAL, sensitivity=["bad", None], sections=["sensitivity", "bridge"], generated=dt.date(2026, 6, 22)
    ).to_html()
    assert "Equity Value Sensitivity" not in html
    assert "Equity Bridge" in html


def test_bridge_reconciles_to_rust_equity_value_with_valuation_discounts() -> None:
    """The bridge total is Rust's equity value, with the DLOM shown as its own step."""
    import re

    from finstack_quant.core.money import Money
    from finstack_quant.statements_analytics import (
        EquityBridge,
        TerminalValueSpec,
        ValuationDiscounts,
        evaluate_dcf,
    )

    builder = statements.ModelBuilder("dcf")
    builder.periods("2025..2026")
    builder.value_money("ufcf", [("2025", Money(100.0, "USD")), ("2026", Money(110.0, "USD"))])
    builder.with_meta("currency", '"USD"')
    result = evaluate_dcf(
        builder.build(),
        0.10,
        TerminalValueSpec.gordon_growth(0.02),
        options={
            "equity_bridge": EquityBridge(total_debt=500.0, cash=100.0, minority_interest=50.0),
            "valuation_discounts": ValuationDiscounts(dlom=0.25),
        },
    )
    equity = result.equity_value.amount
    bridge = result.enterprise_value.amount - result.net_debt.amount - result.valuation_discount.amount
    assert bridge == pytest.approx(equity, abs=0.01)
    assert result.valuation_discount.amount > 0.0

    html = dcf_tearsheet(result, sections=["bridge"]).to_html()
    bars = dict(re.findall(r'data-label="([^"]+)" data-val="([^"]+)"', html))
    assert "− Valuation Discounts" in bars
    assert float(bars["Equity Value"].replace(",", "")) == pytest.approx(equity, abs=1.0)


def test_dcf_sensitivity_keeps_rust_swing_order() -> None:
    """PYPY-005: rows keep the |upside - downside| order Rust returns."""
    entries = [
        {"parameter_id": "margin", "downside": -5.0, "upside": 5.0},  # swing 10
        {"parameter_id": "capex", "downside": -10.0, "upside": -8.0},  # swing 2
    ]
    sheet = dcf_tearsheet(_VAL, sensitivity=entries, sections=["sensitivity"])
    body = sheet.sections[0].body
    assert body.index("margin") < body.index("capex")
