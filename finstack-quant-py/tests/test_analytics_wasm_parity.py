"""Cross-host goldens for the ``Performance`` array constructors and JSON round trip.

``tests/data/analytics_wasm_parity.json`` holds the Python outputs for a fixed
price panel and a fixed return panel. This module pins Python to that file and
``finstack-quant-wasm/tests/facade/analytics_parity.test.mjs`` pins the WASM
members ``Performance.fromArrays``, ``fromReturnsArrays``, ``toJson`` and
``fromJson`` to the same file.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.analytics import Performance

GOLDEN: dict[str, Any] = json.loads((Path(__file__).parent / "data" / "analytics_wasm_parity.json").read_text())
NAMES: list[str] = GOLDEN["ticker_names"]


def price_panel() -> Performance:
    """Price panel of the shared golden."""
    prices = GOLDEN["prices"]
    return Performance.from_arrays(
        prices["dates"], prices["values"], NAMES, prices["benchmark_ticker"], prices["frequency"]
    )


def test_from_arrays_matches_the_shared_golden() -> None:
    assert list(price_panel().sharpe()) == pytest.approx(GOLDEN["prices"]["sharpe"], rel=1e-12)


def test_from_returns_arrays_matches_the_shared_golden() -> None:
    returns = GOLDEN["returns"]
    panel = Performance.from_returns_arrays(returns["dates"], returns["values"], NAMES)
    assert list(panel.volatility()) == pytest.approx(returns["volatility"], rel=1e-12)


def test_json_round_trip_matches_the_shared_golden() -> None:
    panel = price_panel()
    assert json.loads(panel.to_json()) == GOLDEN["prices"]["json"]
    rebuilt = Performance.from_json(panel.to_json())
    assert rebuilt.to_json() == panel.to_json()
    assert list(rebuilt.sharpe()) == list(panel.sharpe())
    with pytest.raises(ValueError, match="invalid Performance JSON"):
        Performance.from_json("{ not json")
