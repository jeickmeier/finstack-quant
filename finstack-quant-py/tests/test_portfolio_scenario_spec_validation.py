"""Portfolio scenario entry points validate the ScenarioSpec in Rust.

JSON scenarios go through ``ScenarioSpec::from_json`` (serde plus
``validate``), exactly as the scenarios namespace and the WASM portfolio
exports do, so an invalid spec is rejected before any revaluation.
"""

from __future__ import annotations

import json

import pytest

from finstack_quant import portfolio
from finstack_quant.core.market_data import MarketContext

BLANK_ID = json.dumps({"id": "", "operations": []})


def _portfolio() -> portfolio.Portfolio:
    spec = {"id": "P", "as_of": "2025-01-15", "base_currency": "USD", "entities": {}, "positions": []}
    return portfolio.Portfolio.from_spec(json.dumps(spec))


def test_apply_scenario_and_revalue_rejects_an_invalid_spec() -> None:
    with pytest.raises(ValueError, match="Scenario ID cannot be empty"):
        portfolio.apply_scenario_and_revalue(_portfolio(), BLANK_ID, MarketContext())


def test_scenario_pnl_batch_validates_every_spec() -> None:
    specs = json.dumps([{"id": "ok", "operations": []}, {"id": " ", "operations": []}])
    with pytest.raises(ValueError, match="Scenario ID cannot be empty"):
        portfolio.scenario_pnl_batch(_portfolio(), specs, MarketContext())
