"""Valuations boundaries whose rules now live in Rust (WASM-audit slice S16).

Each case pins behaviour the WASM facade tests assert with the same inputs:
whole-basis-point spreads, the typed-envelope mismatch message, the composite
rebalance wire shape, the borrowing-base item bounds, and the
``expected_exercise_time`` metric key.
"""

from __future__ import annotations

import datetime as dt
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Bps
from finstack_quant.valuations.composite import (
    CompositeInstrument,
    CompositeLegSpec,
    CompositeRebalanceResult,
    CompositeSpec,
    RebalanceRule,
    WeightingMethod,
)
from finstack_quant.valuations.instruments import (
    AdvanceRate,
    AmortizationEvent,
    Bond,
    ConcentrationLimit,
    TermLoan,
    list_standard_metrics,
    metric_metadata,
)


def _frn(spread_bp: float | Bps) -> Bond:
    return Bond.floating(
        "FRN",
        1_000_000.0,
        "USD-SOFR-3M",
        spread_bp,
        "2024-01-01",
        "2029-01-01",
        Tenor.quarterly(),
        DayCount.ACT_360,
        "USD-OIS",
        currency="USD",
    )


def _spread_bp(bond: Bond) -> object:
    """The serialized ``spread_bp`` wherever the floating leg nests it."""

    def find(node: object) -> object:
        if isinstance(node, dict):
            if "spread_bp" in node:
                return node["spread_bp"]
            for value in node.values():
                found = find(value)
                if found is not None:
                    return found
        if isinstance(node, list):
            for value in node:
                found = find(value)
                if found is not None:
                    return found
        return None

    return find(json.loads(bond.to_json()))


def test_bond_floating_rejects_fractional_basis_points_like_wasm_bps() -> None:
    with pytest.raises(ValueError, match=r"whole number"):
        _frn(125.4)
    assert float(_spread_bp(_frn(125.0))) == 125.0
    # A Bps object hands over its exact Rust value (no decimal round trip).
    assert float(_spread_bp(_frn(Bps(29)))) == 29.0


def test_typed_from_json_reports_the_rust_type_mismatch() -> None:
    with pytest.raises(ValueError, match=r"expected instrument type `bond`, got `term_loan`"):
        Bond.from_json(TermLoan.example().to_json())


def _equity_envelope(instrument_id: str, price: float) -> str:
    return json.dumps({
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "equity",
            "spec": {
                "id": instrument_id,
                "ticker": instrument_id,
                "currency": "USD",
                "quantity": 1.0,
                "quoted_spot": price,
                "spot_id": None,
                "div_yield_id": None,
                "discrete_dividends": [],
                "discount_curve_id": "USD",
                "attributes": {},
            },
        },
    })


def test_composite_rebalance_result_wire_carries_the_instrument_envelope() -> None:
    spec = CompositeSpec(
        "A-B",
        Currency("USD"),
        Money(100.0, Currency("USD")),
        [
            CompositeLegSpec("A", _equity_envelope("A", 100.0), 1.0),
            CompositeLegSpec("B", _equity_envelope("B", 90.0), -1.0),
        ],
        WeightingMethod.fixed_quantity(),
        RebalanceRule.manual(),
    )
    result = spec.initialize(MarketContext(), dt.date(2025, 1, 1))
    wire = json.loads(result.to_json())
    # Same shape WASM `valuations.composite.initialize` returns.
    assert wire["instrument"]["schema"] == "finstack_quant.instrument/1"
    assert wire["instrument"]["instrument"]["type"] == "composite"
    assert [trade["instrument_id"] for trade in wire["trades"]] == ["A", "B"]
    # The nested envelope is accepted by every instrument entry point as-is.
    instrument = CompositeInstrument.from_json(json.dumps(wire["instrument"]))
    assert instrument.to_json() == result.instrument.to_json()
    assert CompositeRebalanceResult.from_json(result.to_json()).to_json() == result.to_json()
    wire["instrument"] = json.loads(_equity_envelope("A", 100.0))
    with pytest.raises(ValueError, match=r"expected instrument type `composite`, got `equity`"):
        CompositeRebalanceResult.from_json(json.dumps(wire))


def test_borrowing_base_items_use_the_rust_bounds() -> None:
    assert AdvanceRate("commercial_mortgage", 0.8).rate == 0.8
    with pytest.raises(ValueError, match=r"asset_class must not be empty"):
        AdvanceRate("", 0.8)
    with pytest.raises(ValueError, match=r"\[0, 1\]"):
        AdvanceRate("commercial_mortgage", 1.5)
    assert ConcentrationLimit("industry", 100.0).scope == "industry"
    for bad in (0.0, 150.0):
        with pytest.raises(ValueError, match=r"\(0, 100\]"):
            ConcentrationLimit("obligor", bad)
    with pytest.raises(ValueError, match=r"scope"):
        ConcentrationLimit("sector", 10.0)
    with pytest.raises(ValueError, match=r"\(0, 1\]"):
        AmortizationEvent.cumulative_loss(2.5)
    with pytest.raises(ValueError, match=r"finite"):
        AmortizationEvent.excess_spread(float("nan"))


def test_bermudan_expected_exercise_time_is_a_years_metric() -> None:
    assert "expected_exercise_time" in list_standard_metrics()
    assert "exercise_probability" not in list_standard_metrics()
    assert metric_metadata(["expected_exercise_time"])[0]["unit"] == "years"
