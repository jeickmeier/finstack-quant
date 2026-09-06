"""Behavioral parity tests for generic composite instruments."""

from __future__ import annotations

import datetime as dt
from functools import partial
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.valuations.composite import (
    CompositeHistoryEngine,
    CompositeInstrument,
    CompositeLegSpec,
    CompositeSpec,
    RebalanceRule,
    WeightingMethod,
)
from finstack_quant.valuations.instruments import price_instrument


def equity_envelope(instrument_id: str, price: float) -> str:
    """Return a canonical explicit-price equity envelope."""
    return json.dumps({
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "equity",
            "spec": {
                "id": instrument_id,
                "ticker": instrument_id,
                "currency": "USD",
                "shares": 1.0,
                "price_quote": price,
                "price_id": None,
                "div_yield_id": None,
                "discrete_dividends": [],
                "discount_curve_id": "USD",
                "attributes": {},
            },
        },
    })


def fixed_spec() -> CompositeSpec:
    """Build a USD long-short fixed-quantity specification."""
    return CompositeSpec(
        "A-B",
        Currency("USD"),
        Money(100.0, Currency("USD")),
        [
            CompositeLegSpec("A", equity_envelope("A", 100.0), 1.0),
            CompositeLegSpec("B", equity_envelope("B", 90.0), -1.0),
        ],
        WeightingMethod.fixed_quantity(),
        RebalanceRule.manual(),
    )


def test_typed_composite_prices_and_decomposes() -> None:
    """Typed generic pricing and primitive reporting use the same Rust model."""
    market = MarketContext()
    resolved_result = fixed_spec().initialize(market, dt.date(2025, 1, 1))
    resolved = resolved_result.instrument

    assert resolved.state.resolved_quantities == {"A": 1.0, "B": -1.0}
    assert [trade["quantity_delta"] for trade in json.loads(resolved_result.trades_json)] == [
        1.0,
        -1.0,
    ]

    priced = price_instrument(resolved, market, "2025-01-02")
    assert priced.instrument_id == "A-B"
    assert priced.price == pytest.approx(10.0)
    assert priced.currency == "USD"

    exposure = json.loads(resolved.primitive_exposures(market, dt.date(2025, 1, 2)).to_json())
    assert [item["instrument_id"] for item in exposure["aggregates"]] == ["A", "B"]
    assert [item["net_quantity"] for item in exposure["aggregates"]] == [1.0, -1.0]
    assert sum(float(item["gross_value"]["amount"]) for item in exposure["aggregates"]) == pytest.approx(190.0)


def test_composite_round_trip_and_flat_execution() -> None:
    """Resolved envelopes retain immutable quantities and execution deltas."""
    resolved = fixed_spec().initialize(MarketContext(), dt.date(2025, 1, 1)).instrument
    restored = CompositeInstrument.from_json(resolved.to_json())
    assert restored.state.resolved_quantities == resolved.state.resolved_quantities
    assert restored.execution_trades() == [
        {"instrument_id": "A", "instrument_type": "equity", "quantity_delta": 1.0},
        {"instrument_id": "B", "instrument_type": "equity", "quantity_delta": -1.0},
    ]


def test_fixed_composite_history_reconciles_flat_market() -> None:
    """Flat explicit-price observations produce zero P&L and a flat index."""
    market = MarketContext()
    market_state = json.loads(market.to_json())
    observations = json.dumps([
        {"date": "2025-01-01", "state": market_state},
        {"date": "2025-01-02", "state": market_state},
        {"date": "2025-01-03", "state": market_state},
    ])
    history = CompositeHistoryEngine.run_from_spec(fixed_spec(), observations)
    rows = json.loads(history.to_json())
    assert len(rows) == 3
    assert [row["return_index"] for row in rows] == [100.0, 100.0, 100.0]
    assert [float(row["pnl"]["amount"]) for row in rows] == [0.0, 0.0, 0.0]


@pytest.mark.parametrize("entry_point", ["exposures", "from_spec", "run"])
def test_composite_metric_requests_reject_noncanonical_keys(entry_point: str) -> None:
    market = MarketContext()
    spec = fixed_spec()
    resolved = spec.initialize(market, dt.date(2025, 1, 1)).instrument
    observations = json.dumps([{"date": "2025-01-01", "state": json.loads(market.to_json())}])
    metrics = ["pv01::USD_x2dOIS"]
    calls = {
        "exposures": partial(resolved.primitive_exposures, market, dt.date(2025, 1, 1), metrics),
        "from_spec": partial(CompositeHistoryEngine.run_from_spec, spec, observations, metrics=metrics),
        "run": partial(CompositeHistoryEngine.run, resolved, observations, metrics=metrics),
    }
    with pytest.raises(ValueError, match="noncanonical"):
        calls[entry_point]()
