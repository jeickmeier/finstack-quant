"""Presentation preserves currency partitions and canonical metric units."""

import datetime as dt

import pandas as pd

from finstack_quant.reporting import instrument as ins, portfolio as port, statement_tearsheet
from finstack_quant.reporting.theme import INSTITUTIONAL


def test_instrument_cashflows_keep_both_currency_ladders_and_every_flow() -> None:
    frame = pd.DataFrame({
        "date": [dt.date(2025, 6, 30)] * 3,
        "kind": ["coupon", "coupon", "principal"],
        "amount": [1000000.0, 2000000.0, 3000000.0],
        "currency": ["USD", "EUR", "EUR"],
        "pv": [900000.0, 1800000.0, 2700000.0],
    })
    ladders, rows = ins._cashflow_blocks(frame)
    assert set(ladders) == {"USD", "EUR"}
    assert ladders["USD"] == [("2025", 1.0, 0.0, 0.9)]
    assert ladders["EUR"] == [("2025", 2.0, 3.0, 4.5)]
    assert len(rows) == 3
    assert [row["Currency"] for row in rows] == ["USD", "EUR", "EUR"]


def test_portfolio_cashflows_render_each_native_currency() -> None:
    cashflows = {
        "by_date": {
            "2025-06-30": {
                "USD": {"coupon": {"amount": "123", "currency": "USD"}},
                "EUR": {"principal": {"amount": "456", "currency": "EUR"}},
            }
        }
    }
    section = port._section_cashflows(cashflows, INSTITUTIONAL)
    assert section is not None
    assert "EUR" in section.body
    assert "USD" in section.body
    assert "456" in section.body
    assert "123" in section.body


def test_decimal_statement_ratios_and_basis_point_cds_spread() -> None:
    sheet = statement_tearsheet(
        {"nodes": {"gross_margin": {"2025Q1": 0.25}, "revenue_growth": {"2025Q1": 0.1}}},
        sections=["summary", "margins"],
    )
    html = sheet.to_html()
    assert "25.0%" in html
    assert "+10.0%" in html
    assert ins._metric_cell("par_spread", 125.0)[1] == "125 bp"
