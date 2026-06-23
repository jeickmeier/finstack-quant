# finstack-quant-py/tests/test_reporting_portfolio_risk.py
from __future__ import annotations

import datetime as dt
import json

import pytest

from finstack_quant.reporting import portfolio_risk_tearsheet
from finstack_quant.reporting.document import TearSheet

_DECOMP = {
    "portfolio_var": 0.2040,
    "portfolio_es": 0.2558,
    "confidence": 0.95,
    "n_positions": 3,
    "euler_residual": 0.0,
    "method": "parametric",
    "contributions": [
        {
            "position_id": "Equity",
            "component_var": 0.1539,
            "marginal_var": 0.3077,
            "pct_contribution": 0.7544,
            "incremental_var": None,
        },
        {
            "position_id": "Credit",
            "component_var": 0.0485,
            "marginal_var": 0.1618,
            "pct_contribution": 0.2380,
            "incremental_var": None,
        },
        {
            "position_id": "Rates",
            "component_var": 0.0015,
            "marginal_var": 0.0077,
            "pct_contribution": 0.0075,
            "incremental_var": None,
        },
    ],
}
_ES = {
    "contributions": [
        {"position_id": "Equity", "component_es": 0.1930, "marginal_es": 0.3859, "pct_contribution": 0.7544},
        {"position_id": "Credit", "component_es": 0.0609, "marginal_es": 0.2029, "pct_contribution": 0.2380},
    ],
}
_BUDGET = {
    "portfolio_var": 0.2040,
    "total_overbudget": 0.0519,
    "has_breach": True,
    "positions": [
        {
            "position_id": "Equity",
            "actual_component_var": 0.1539,
            "target_component_var": 0.1020,
            "target_pct": 0.5,
            "utilization": 1.509,
            "excess": 0.0519,
            "breach": True,
        },
        {
            "position_id": "Credit",
            "actual_component_var": 0.0485,
            "target_component_var": 0.0612,
            "target_pct": 0.3,
            "utilization": 0.793,
            "excess": -0.0126,
            "breach": False,
        },
    ],
}


def test_portfolio_risk_tearsheet_renders_all_sections() -> None:
    ts = portfolio_risk_tearsheet(_DECOMP, es=_ES, budget=_BUDGET, generated=dt.date(2026, 6, 23))
    assert isinstance(ts, TearSheet)
    html = ts.to_html()
    assert "Portfolio VaR" in html
    assert "Portfolio ES" in html
    assert "Confidence" in html
    assert "VaR Contributions" in html
    assert "Equity" in html
    assert "ES Contributions" in html
    assert "Risk Budget" in html
    assert "Breach" in html


def test_portfolio_risk_tearsheet_accepts_json() -> None:
    html = portfolio_risk_tearsheet(json.dumps(_DECOMP), generated=dt.date(2026, 6, 23)).to_html()
    assert "VaR Contributions" in html


def test_portfolio_risk_tearsheet_optional_sections_omitted() -> None:
    html = portfolio_risk_tearsheet(_DECOMP, generated=dt.date(2026, 6, 23)).to_html()
    assert "VaR Contributions" in html
    assert "ES Contributions" not in html
    assert "Risk Budget" not in html


def test_portfolio_risk_tearsheet_deterministic() -> None:
    a = portfolio_risk_tearsheet(_DECOMP, budget=_BUDGET, generated=dt.date(2026, 6, 23)).to_html()
    b = portfolio_risk_tearsheet(_DECOMP, budget=_BUDGET, generated=dt.date(2026, 6, 23)).to_html()
    assert a == b


def test_portfolio_risk_tearsheet_rejects_unknown_section() -> None:
    with pytest.raises(ValueError, match="unknown section"):
        portfolio_risk_tearsheet(_DECOMP, sections=["contributions", "nope"])


def test_portfolio_risk_tearsheet_tolerates_bad_rows() -> None:
    decomp = dict(_DECOMP)
    decomp["contributions"] = [*_DECOMP["contributions"], None, "bad"]
    html = portfolio_risk_tearsheet(decomp, generated=dt.date(2026, 6, 23)).to_html()
    assert "VaR Contributions" in html  # valid rows still render, no crash


def test_portfolio_risk_tearsheet_positions_kpi_without_method() -> None:
    decomp = {k: v for k, v in _DECOMP.items() if k != "method"}
    html = portfolio_risk_tearsheet(decomp, generated=dt.date(2026, 6, 23)).to_html()
    assert "Positions" in html


def test_portfolio_risk_tearsheet_es_budget_bad_rows() -> None:
    es = {"contributions": [*_ES["contributions"], None, "bad"]}
    budget = {**_BUDGET, "positions": [*_BUDGET["positions"], None, 42]}
    html = portfolio_risk_tearsheet(_DECOMP, es=es, budget=budget, generated=dt.date(2026, 6, 23)).to_html()
    assert "ES Contributions" in html
    assert "Risk Budget" in html
