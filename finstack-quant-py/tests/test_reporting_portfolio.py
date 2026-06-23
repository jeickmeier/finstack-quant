# finstack-quant-py/tests/test_reporting_portfolio.py
from __future__ import annotations

import datetime as dt
import json

import pytest

from finstack_quant.reporting import portfolio_tearsheet
from finstack_quant.reporting.document import TearSheet

_VAL = {
    "as_of": "2025-01-15",
    "position_values": {
        "POS-1": {
            "position_id": "POS-1",
            "entity_id": "FUND-1",
            "value_native": {"amount": "998782.54", "currency": "USD"},
            "value_base": {"amount": "998782.54", "currency": "USD"},
            "risk_metrics_complete": True,
        },
        "POS-2": {
            "position_id": "POS-2",
            "entity_id": "FUND-2",
            "value_native": {"amount": "1994539.05", "currency": "USD"},
            "value_base": {"amount": "1994539.05", "currency": "USD"},
            "risk_metrics_complete": True,
        },
    },
    "total_base_ccy": {"amount": "2993321.59", "currency": "USD"},
    "by_entity": {
        "FUND-1": {"amount": "998782.54", "currency": "USD"},
        "FUND-2": {"amount": "1994539.05", "currency": "USD"},
    },
    "fx_collapse_policy": "CashflowDate",
}
_METRICS = {
    "aggregated": {
        "dv01": {"metric_id": "dv01", "total": 250.0, "by_entity": {"FUND-1": 100.0, "FUND-2": 150.0}},
        "theta": {"metric_id": "theta", "total": 410.06, "by_entity": {"FUND-1": 136.82, "FUND-2": 273.23}},
        "bucketed_dv01::USD-OIS::1y": {"metric_id": "bucketed_dv01::USD-OIS::1y", "total": 40.0, "by_entity": {}},
        "bucketed_dv01::USD-OIS::5y": {"metric_id": "bucketed_dv01::USD-OIS::5y", "total": 120.0, "by_entity": {}},
        "bucketed_dv01::USD-OIS::10y": {"metric_id": "bucketed_dv01::USD-OIS::10y", "total": 90.0, "by_entity": {}},
    },
    "by_position": {},
}
_CASHFLOWS = {
    "by_date": {
        "2025-01-15": {"USD": {"Notional": {"amount": "-3000000", "currency": "USD"}}},
        "2025-04-15": {"USD": {"Fixed": {"amount": "1011250", "currency": "USD"}}},
        "2025-07-15": {"USD": {"Fixed": {"amount": "2050277.78", "currency": "USD"}}},
    }
}


def test_portfolio_tearsheet_renders_all_sections() -> None:
    ts = portfolio_tearsheet(_VAL, metrics=_METRICS, cashflows=_CASHFLOWS, generated=dt.date(2026, 6, 23))
    assert isinstance(ts, TearSheet)
    html = ts.to_html()
    assert "Total Value" in html
    assert "Holdings" in html
    assert "POS-2" in html
    assert "Exposure by Entity" in html
    assert "FUND-1" in html
    assert "Aggregated Sensitivities" in html
    assert "DV01" in html
    assert "Tenor Risk Profile" in html
    assert "Cashflow Ladder" in html


def test_portfolio_tearsheet_accepts_json() -> None:
    html = portfolio_tearsheet(json.dumps(_VAL), generated=dt.date(2026, 6, 23)).to_html()
    assert "Holdings" in html


def test_portfolio_tearsheet_optional_sections_omitted() -> None:
    html = portfolio_tearsheet(_VAL, generated=dt.date(2026, 6, 23)).to_html()
    assert "Holdings" in html
    assert "Aggregated Sensitivities" not in html
    assert "Cashflow Ladder" not in html


def test_portfolio_tearsheet_deterministic() -> None:
    a = portfolio_tearsheet(_VAL, metrics=_METRICS, generated=dt.date(2026, 6, 23)).to_html()
    b = portfolio_tearsheet(_VAL, metrics=_METRICS, generated=dt.date(2026, 6, 23)).to_html()
    assert a == b


def test_portfolio_tearsheet_rejects_unknown_section() -> None:
    with pytest.raises(ValueError, match="unknown section"):
        portfolio_tearsheet(_VAL, sections=["holdings", "nope"])


def test_portfolio_tearsheet_real_portfolio() -> None:
    # Exercises the real value_portfolio JSON shape (decimal-string Money).
    from datetime import date

    from finstack_quant.core.market_data import DiscountCurve, MarketContext
    from finstack_quant.portfolio import value_portfolio

    mc = MarketContext()
    mc.insert(
        DiscountCurve(
            "USD-OIS", date(2025, 1, 15), [(0.0, 1.0), (0.25, 0.9875), (1.0, 0.95), (5.0, 0.75)], day_count="act_365f"
        )
    )
    spec = {
        "id": "demo",
        "as_of": "2025-01-15",
        "base_ccy": "USD",
        "entities": {"FUND-1": {"id": "FUND-1"}},
        "positions": [
            {
                "position_id": "POS-1",
                "entity_id": "FUND-1",
                "instrument_id": "DEP-1",
                "instrument_spec": {
                    "type": "deposit",
                    "spec": {
                        "id": "DEP-1",
                        "notional": {"amount": 1_000_000.0, "currency": "USD"},
                        "start_date": "2025-01-15",
                        "maturity": "2025-04-15",
                        "day_count": "Act360",
                        "quote_rate": 0.045,
                        "discount_curve_id": "USD-OIS",
                        "attributes": {},
                    },
                },
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    }
    valuation = value_portfolio(json.dumps(spec), mc.to_json())
    html = portfolio_tearsheet(valuation, generated=dt.date(2026, 6, 23)).to_html()
    assert "Holdings" in html
    assert "POS-1" in html
