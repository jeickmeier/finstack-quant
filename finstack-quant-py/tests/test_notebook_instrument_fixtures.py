"""Canonical serde coverage for instrument fixtures used by maintained notebooks."""

from __future__ import annotations

from collections.abc import Callable
import importlib.util
from pathlib import Path
from types import ModuleType

import pytest

from finstack_quant.portfolio import Portfolio


def _load_fixture_module() -> ModuleType:
    path = Path(__file__).parents[1] / "examples" / "notebooks" / "_shared" / "instrument_fixtures.py"
    spec = importlib.util.spec_from_file_location("notebook_instrument_fixtures", path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


FIXTURES = _load_fixture_module()
FACTORY_NAMES = (
    "fixed_bond",
    "floating_bond",
    "term_loan",
    "revolver",
    "cds",
    "cds_index",
    "cds_tranche",
    "cds_option",
    "clo_deal",
    "abs_deal",
    "irs",
    "swaption",
    "ir_future",
    "spot_equity",
    "variance_swap",
    "fx_swap",
    "convertible_bond",
)


@pytest.mark.parametrize("factory_name", FACTORY_NAMES)
def test_notebook_instrument_factory_deserializes(factory_name: str) -> None:
    factory: Callable[[int], tuple[str, dict[str, object]]] = getattr(FIXTURES, factory_name)
    instrument_id, instrument = factory(0)
    portfolio = {
        "id": f"fixture-{factory_name}",
        "as_of": "2025-01-15",
        "base_currency": "USD",
        "entities": {"examples": {"id": "examples"}},
        "positions": [
            {
                "position_id": "position-0",
                "entity_id": "examples",
                "instrument_id": instrument_id,
                "instrument_spec": instrument,
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    }

    loaded = Portfolio.from_spec(portfolio)
    assert len(loaded) == 1


def test_acme_bond_fixture_deserializes() -> None:
    instrument = FIXTURES.acme_bond()
    portfolio = {
        "id": "fixture-acme-bond",
        "as_of": "2026-03-01",
        "base_currency": "USD",
        "entities": {"examples": {"id": "examples"}},
        "positions": [
            {
                "position_id": "position-0",
                "entity_id": "examples",
                "instrument_id": instrument["spec"]["id"],
                "instrument_spec": instrument,
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    }

    loaded = Portfolio.from_spec(portfolio)
    assert len(loaded) == 1
