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
    CompositeInstrument,
    CompositeLegSpec,
    CompositeSpec,
    RebalanceRule,
    WeightingMethod,
    history,
    history_from_spec,
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

    assert resolved.state.resolved_legs == {"A": 1.0, "B": -1.0}
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
    assert restored.state.resolved_legs == resolved.state.resolved_legs
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
    result = history_from_spec(fixed_spec(), observations)
    rows = json.loads(result.to_json())
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
        "from_spec": partial(history_from_spec, spec, observations, metrics=metrics),
        "run": partial(history, resolved, observations, metrics=metrics),
    }
    with pytest.raises(ValueError, match="noncanonical"):
        calls[entry_point]()


# --------------------------------------------------------------------------- calendar rules (VALA-008)


def _monthly_rule(end: str | None = None) -> RebalanceRule:
    return RebalanceRule.calendar("2024-01-02", "1M", "weekends_only", "following", end=end)


@pytest.mark.parametrize("end", [None, "2024-01-02", "2024-12-02", "2024-12-15"])
def test_calendar_rebalance_rule_accepts_open_and_partial_ends(end: str | None) -> None:
    rule = _monthly_rule(end)
    payload = json.loads(rule.to_json())
    assert payload.get("end") == end
    assert json.loads(RebalanceRule.from_json(rule.to_json()).to_json()) == payload


def test_calendar_rebalance_rule_rejects_end_before_start() -> None:
    with pytest.raises(ValueError, match="end precedes start"):
        _monthly_rule("2023-12-01")


def test_open_ended_calendar_rule_rebalances_on_cadence_in_history() -> None:
    spec = CompositeSpec(
        "A-B",
        Currency("USD"),
        Money(100.0, Currency("USD")),
        [
            CompositeLegSpec("A", equity_envelope("A", 100.0), 1.0),
            CompositeLegSpec("B", equity_envelope("B", 90.0), -1.0),
        ],
        WeightingMethod.notional_weighted(Money(100.0, Currency("USD"))),
        _monthly_rule(),
    )
    flat = json.loads(MarketContext().to_json())
    dates = ["2024-01-02", "2024-01-20", "2024-02-02", "2024-02-15"]
    result = history_from_spec(spec, [{"date": date, "state": flat} for date in dates])
    rows = json.loads(result.to_json())
    rebalanced = [row["date"] for row in rows if row["next_state_effective_date"] is not None]
    assert rebalanced == ["2024-02-02"]


# --------------------------------------------------------------------------- recalibrated metrics (VALA-002)


def _hazard_market(base: dt.date) -> MarketContext:
    from pathlib import Path

    from finstack_quant.calibration import calibrate

    path = Path(__file__).parents[2] / "finstack-quant/calibration/examples/market_bootstrap/03_single_name_hazard.json"
    envelope = json.loads(path.read_text())
    envelope.pop("$schema", None)
    for step in envelope["plan"]["steps"]:
        step["base_date"] = base.isoformat()
        if step["kind"] == "hazard":
            step.update({"id": "CORP-HAZARD", "curve_id": "CORP-HAZARD", "entity": "CORP"})
    for quote in envelope["market_data"]:
        if quote["kind"] == "cds_quote":
            quote["entity"] = "CORP"
    return calibrate(json.dumps(envelope)).market


def test_composite_entry_points_price_cs01_like_price_instrument() -> None:
    from finstack_quant.valuations.instruments import CreditDefaultSwap

    base = dt.date(2024, 6, 20)
    market = _hazard_market(base)
    cds = CreditDefaultSwap.example()
    cds_b = CreditDefaultSwap.from_json(cds.to_json().replace(f'"{cds.id}"', f'"{cds.id}-B"'))
    usd = Currency("USD")

    def legs() -> list[CompositeLegSpec]:
        return [CompositeLegSpec(cds.id, cds, 1.0), CompositeLegSpec(cds_b.id, cds_b, -0.5)]

    fixed = CompositeSpec(
        "COMP", usd, Money(1e6, usd), legs(), WeightingMethod.fixed_quantity(), RebalanceRule.manual()
    )
    composite = fixed.initialize(market, base).instrument
    expected = price_instrument(composite, market, base, metrics=["cs01"]).metrics["cs01"]
    assert expected != 0.0

    report = json.loads(composite.primitive_exposures(market, base, metrics=["cs01"]).to_json())
    net = {row["instrument_id"]: row["net_measures"]["cs01"] for row in report["aggregates"]}
    assert sum(net.values()) == pytest.approx(expected, rel=1e-12)

    observations = [{"date": base.isoformat(), "state": json.loads(market.to_json())}]
    for result in (
        history(composite, observations, metrics=["cs01"]),
        history_from_spec(fixed, observations, metrics=["cs01"]),
    ):
        assert len(json.loads(result.to_json())) == 1

    weighted = CompositeSpec(
        "COMP",
        usd,
        Money(1e6, usd),
        legs(),
        WeightingMethod.metric_weighted(metric="cs01", anchor_leg_id=cds.id, anchor_quantity=1.0),
        RebalanceRule.manual(),
    )
    weighted_composite = weighted.initialize(market, base).instrument
    assert weighted_composite.rebalance(market, base).instrument.to_json()
