"""Theta for seasoned FX barrier and touch options holds spot fixed.

The roll observes the as-of spot, so the barrier state over the roll is the
one that spot implies. Rust owns the convention; the same cases run in
``finstack-quant-wasm/tests/facade/theta_observed_state.test.mjs``.
"""

from __future__ import annotations

import copy
import json
from typing import Any

import pytest

from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations.instruments import price_instrument
from tests.golden.conftest import fixture_path
from tests.golden.schema import GoldenFixture

AS_OF = "2026-04-30"
ROLLED = "2026-05-01"


def _golden() -> tuple[dict[str, Any], MarketContext]:
    fixture = GoldenFixture.from_path(
        fixture_path("pricing/quantlib/fx_barrier_option/eurusd_up_out_call_3m_quantlib.json")
    )
    return fixture.body["instrument"], MarketContext.from_json(json.dumps(fixture.body["market"]["data"]))


def _with_spec(envelope: dict[str, Any], **fields: Any) -> str:
    copied = copy.deepcopy(envelope)
    copied["instrument"]["spec"].update(fields)
    return json.dumps(copied)


def _touch(**fields: Any) -> dict[str, Any]:
    spec = {
        "attributes": {},
        "barrier": 1.25,
        "barrier_direction": "up",
        "base_currency": "EUR",
        "day_count": "act_365f",
        "domestic_discount_curve_id": "USD-OIS",
        "expiry": "2026-07-30",
        "foreign_discount_curve_id": "EUR-OIS",
        "id": "FXTOUCH-EURUSD-UP-THETA",
        "monitoring_start_date": AS_OF,
        "payout_amount": {"amount": "1000000", "currency": "USD"},
        "payout_timing": "at_expiry",
        "quote_currency": "USD",
        "touch_type": "one_touch",
        "vol_surface_id": "EURUSD-BARRIER-VOL-QL",
    }
    spec.update(fields)
    return {"schema": "finstack_quant.instrument/1", "instrument": {"type": "fx_touch_option", "spec": spec}}


def _theta(instrument: str, market: MarketContext) -> float:
    return price_instrument(instrument, market, AS_OF, metrics=["theta"])["theta"]


@pytest.mark.parametrize(
    "case",
    ["fx_barrier_option", "fx_touch_option"],
)
def test_theta_equals_spot_held_reprice(case: str) -> None:
    """Theta equals the one-day reprice with the as-of spot's observed state."""
    golden, market = _golden()
    envelope = golden if case == "fx_barrier_option" else _touch()
    instrument = json.dumps(envelope)
    # EURUSD spot 1.10 sits below the 1.25 barrier: the roll observes no breach.
    observed = _with_spec(envelope, observed_barrier_breached=False)

    theta = _theta(instrument, market)
    rolled_pv = float(price_instrument(observed, market.roll_forward(1), ROLLED).price)
    base_pv = float(price_instrument(instrument, market, AS_OF).price)
    assert theta == pytest.approx(rolled_pv - base_pv, rel=1e-9, abs=1e-9)
    assert theta == _theta(observed, market)


@pytest.mark.parametrize(
    "envelope",
    ["fx_barrier_option", "fx_touch_option"],
)
def test_seasoned_pricing_still_requires_observed_state(envelope: str) -> None:
    """Only theta rolls the observed state; ordinary seasoned pricing still raises ValueError."""
    golden, market = _golden()
    instrument = json.dumps(golden if envelope == "fx_barrier_option" else _touch())
    with pytest.raises(ValueError, match="observed_barrier_breached"):
        price_instrument(instrument, market, ROLLED)


def test_fx_barrier_golden_theta_is_negative() -> None:
    """A long up-and-out call far from its barrier decays."""
    golden, market = _golden()
    assert _theta(json.dumps(golden), market) < 0.0
