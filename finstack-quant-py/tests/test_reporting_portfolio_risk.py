from __future__ import annotations

import datetime as dt
import json

import pytest

from finstack_quant.models.factor.risk import (
    historical_var_decomposition,
    parametric_es_decomposition,
    parametric_var_decomposition,
)
from finstack_quant.reporting import portfolio_risk_tearsheet
from finstack_quant.reporting.document import TearSheet

_IDS = ["Equity", "Credit", "Rates"]
_WEIGHTS = [0.5, 0.3, 0.2]
_COV = [[0.04, 0.006, 0.001], [0.006, 0.0225, 0.0015], [0.001, 0.0015, 0.0025]]

# Real Rust output: ``parametric_var_decomposition`` returns the typed
# ``PositionRiskDecomposition`` (``var_contributions`` / ``relative_var``).
_DECOMP_TYPED = parametric_var_decomposition(_IDS, _WEIGHTS, _COV, confidence=0.95)
_DECOMP = json.loads(_DECOMP_TYPED.to_json())
_ES = json.loads(parametric_es_decomposition(_IDS, _WEIGHTS, _COV, confidence=0.95).to_json())
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
    decomp["var_contributions"] = [*_DECOMP["var_contributions"], None, "bad"]
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


def test_portfolio_risk_tearsheet_renders_var_contributions_from_typed_rust_results() -> None:
    """PYPY-007: the typed decomposition both VaR engines return renders its contributions."""
    sheet = portfolio_risk_tearsheet(_DECOMP_TYPED, generated=dt.date(2026, 6, 23))
    assert [s.title for s in sheet.sections] == ["VaR Contributions"]
    body = sheet.sections[0].body
    for row in _DECOMP["var_contributions"]:
        # The share column is the Rust ``relative_var`` scaled to percent.
        assert f"{row['relative_var'] * 100:.1f}%" in body
    pnls = [[float((s * (i + 3)) % 7 - 3) * (i + 1) for s in range(60)] for i in range(3)]
    historical = historical_var_decomposition(_IDS, pnls, confidence=0.95)
    assert [s.title for s in portfolio_risk_tearsheet(historical).sections] == ["VaR Contributions"]
