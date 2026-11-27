"""Portfolio scenario entry points take a validated ``ScenarioSpec``.

JSON text becomes a scenario through ``ScenarioSpec.from_json`` (serde plus
``validate``), exactly as in the scenarios namespace, so an invalid spec is
rejected before it reaches a portfolio entry point. The entry points
themselves refuse JSON text.
"""

from __future__ import annotations

import json

import pytest

from finstack_quant import portfolio
from finstack_quant.core.market_data import MarketContext
from finstack_quant.scenarios import ScenarioSpec

BLANK_ID = json.dumps({"id": "", "operations": []})
VALID = json.dumps({"id": "ok", "operations": []})


def _portfolio() -> portfolio.Portfolio:
    spec = {"id": "P", "as_of": "2025-01-15", "base_currency": "USD", "entities": {}, "positions": []}
    return portfolio.Portfolio.from_spec(spec)


def test_the_loader_rejects_an_invalid_spec() -> None:
    with pytest.raises(ValueError, match="Scenario ID cannot be empty"):
        ScenarioSpec.from_json(BLANK_ID)
    with pytest.raises(ValueError, match="Scenario ID cannot be empty"):
        ScenarioSpec.from_json(json.dumps({"id": " ", "operations": []}))


def test_portfolio_scenario_entry_points_refuse_json_text() -> None:
    book = _portfolio()
    with pytest.raises(TypeError, match="ScenarioSpec"):
        portfolio.apply_scenario_and_revalue(book, VALID, MarketContext())
    with pytest.raises(TypeError, match="ScenarioSpec"):
        portfolio.scenario_pnl(book, [VALID], MarketContext())
    for text in (VALID, f"[{VALID}]"):
        with pytest.raises(TypeError):
            portfolio.scenario_pnl(book, text, MarketContext())
    loaded = ScenarioSpec.from_json(VALID)
    assert portfolio.apply_scenario_and_revalue(book, loaded, MarketContext()).report.operations_applied == 0
    assert [item.scenario_id for item in portfolio.scenario_pnl(book, [loaded], MarketContext())] == ["ok"]
