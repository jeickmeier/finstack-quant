"""Cross-host goldens for the ``Performance`` array constructors and JSON round trip.

``tests/data/analytics_wasm_parity.json`` holds the Python outputs for a fixed
price panel and a fixed return panel. This module pins Python to that file and
``finstack-quant-wasm/tests/facade/analytics_parity.test.mjs`` pins the WASM
constructor ``new Performance(...)`` and members ``fromReturns``, ``toJson`` and
``fromJson`` to the same file.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.analytics import Performance, PerformancePanel

GOLDEN: dict[str, Any] = json.loads((Path(__file__).parent / "data" / "analytics_wasm_parity.json").read_text())
NAMES: list[str] = GOLDEN["ticker_names"]


def price_panel() -> Performance:
    """Price panel of the shared golden."""
    prices = GOLDEN["prices"]
    return Performance.from_panel(
        PerformancePanel(
            dates=prices["dates"],
            series=prices["values"],
            kind="prices",
            ticker_names=NAMES,
            benchmark_ticker=prices["benchmark_ticker"],
            frequency=prices["frequency"],
        )
    )


def test_price_panel_matches_the_shared_golden() -> None:
    assert list(price_panel().sharpe()) == pytest.approx(GOLDEN["prices"]["sharpe"], rel=1e-12)
    with pytest.raises(KeyError, match="missing"):
        Performance.from_panel(
            PerformancePanel(
                dates=GOLDEN["prices"]["dates"],
                series=GOLDEN["prices"]["values"],
                kind="prices",
                ticker_names=NAMES,
                benchmark_ticker="missing",
            )
        )


def test_return_panel_matches_the_shared_golden() -> None:
    returns = GOLDEN["returns"]
    panel = Performance.from_panel(
        PerformancePanel(dates=returns["dates"], series=returns["values"], kind="returns", ticker_names=NAMES)
    )
    assert list(panel.volatility()) == pytest.approx(returns["volatility"], rel=1e-12)


def test_json_round_trip_matches_the_shared_golden() -> None:
    panel = price_panel()
    assert json.loads(panel.to_json()) == GOLDEN["prices"]["json"]
    rebuilt = Performance.from_json(panel.to_json())
    assert rebuilt.to_json() == panel.to_json()
    assert list(rebuilt.sharpe()) == list(panel.sharpe())
    with pytest.raises(ValueError, match="invalid Performance JSON"):
        Performance.from_json("{ not json")
