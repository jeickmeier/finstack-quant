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

import pandas as pd
import polars as pl
import pytest

from finstack_quant.analytics import Performance

GOLDEN: dict[str, Any] = json.loads((Path(__file__).parent / "data" / "analytics_wasm_parity.json").read_text())
NAMES: list[str] = GOLDEN["ticker_names"]


def price_panel() -> Performance:
    """Price panel of the shared golden."""
    prices = GOLDEN["prices"]
    frame = pd.DataFrame(dict(zip(NAMES, prices["values"], strict=True)), index=prices["dates"])
    return Performance(
        frame,
        benchmark_ticker=prices["benchmark_ticker"],
        frequency=prices["frequency"],
    )


def test_price_panel_matches_the_shared_golden() -> None:
    perf = price_panel()
    assert perf.sharpe(risk_free_rate=0.0).values() == pytest.approx(GOLDEN["prices"]["sharpe"], rel=1e-12)
    prices = GOLDEN["prices"]
    pl_frame = pl.DataFrame({"date": prices["dates"], **dict(zip(NAMES, prices["values"], strict=True))})
    assert (
        Performance(
            pl_frame,
            benchmark_ticker=prices["benchmark_ticker"],
            frequency=prices["frequency"],
            date_col="date",
        ).to_json()
        == perf.to_json()
    )
    with pytest.raises(KeyError, match="missing"):
        Performance(
            pd.DataFrame(dict(zip(NAMES, GOLDEN["prices"]["values"], strict=True)), index=GOLDEN["prices"]["dates"]),
            benchmark_ticker="missing",
            frequency="daily",
        )


def test_return_panel_matches_the_shared_golden() -> None:
    returns = GOLDEN["returns"]
    frame = pd.DataFrame(dict(zip(NAMES, returns["values"], strict=True)), index=returns["dates"][1:])
    panel = Performance.from_returns(
        frame,
        initial_date=returns["dates"][0],
        frequency="daily",
        benchmark_ticker=NAMES[0],
    )
    assert panel.volatility(annualize=True).values() == pytest.approx(returns["volatility"], rel=1e-12)
    pl_frame = pl.DataFrame({"date": returns["dates"][1:], **dict(zip(NAMES, returns["values"], strict=True))})
    assert (
        Performance.from_returns(
            pl_frame,
            initial_date=returns["dates"][0],
            frequency="daily",
            benchmark_ticker=NAMES[0],
            date_col="date",
        ).to_json()
        == panel.to_json()
    )


def test_json_round_trip_matches_the_shared_golden() -> None:
    panel = price_panel()
    assert json.loads(panel.to_json()) == GOLDEN["prices"]["json"]
    rebuilt = Performance.from_json(panel.to_json())
    assert rebuilt.to_json() == panel.to_json()
    assert rebuilt.sharpe(risk_free_rate=0.0) == panel.sharpe(risk_free_rate=0.0)
    with pytest.raises(ValueError, match="invalid Performance JSON"):
        Performance.from_json("{ not json")
