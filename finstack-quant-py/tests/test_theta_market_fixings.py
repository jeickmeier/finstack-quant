"""Theta holds market-held fixings at their as-of projections.

The Bloomberg SWPM SOFR OIS golden is valued on its effective date, so a
one-day theta crosses the SOFR fixing dated the valuation date. Rust owns the
convention; the same case runs in
``finstack-quant-wasm/tests/facade/theta_market_fixings.test.mjs``.
"""

from __future__ import annotations

import json

import pytest

from finstack_quant.core.market_data import MarketContext, ScalarTimeSeries
from finstack_quant.valuations.instruments import price_instrument
from tests.golden.conftest import fixture_path
from tests.golden.runners.pricing_common import _resolve_market
from tests.golden.schema import GoldenFixture

AS_OF = "2026-05-04"
ROLLED = "2026-05-05"


def _golden() -> tuple[str, MarketContext]:
    fixture = GoldenFixture.from_path(fixture_path("pricing/bloomberg/irs/usd_sofr_5y_receive_fixed_swpm.json"))
    return json.dumps(fixture.body["instrument"]), _resolve_market(fixture.body["market"])


def _theta(instrument: str, market: MarketContext) -> float:
    return price_instrument(instrument, market, AS_OF, metrics=["theta"])["theta"]


def test_ois_theta_observes_the_same_day_sofr_fixing_at_the_as_of_forward() -> None:
    """Theta equals the one-day reprice with the SOFR fixing set to its as-of forward."""
    instrument, market = _golden()
    # Single-curve OIS: the overnight forward for [2026-05-04, 2026-05-05] on ACT/360.
    forward = (1.0 / market.get_discount("USD-SOFR").df_between_dates(AS_OF, ROLLED) - 1.0) * 360.0
    fixing = ScalarTimeSeries("FIXING:USD-SOFR", [(AS_OF, forward)])
    rolled_pv = float(price_instrument(instrument, market.insert_series(fixing).roll_forward(1), ROLLED).price)
    base_pv = float(price_instrument(instrument, market, AS_OF).price)

    assert _theta(instrument, market) == pytest.approx(rolled_pv - base_pv, rel=1e-9, abs=1e-6)


def test_rolled_pricing_still_requires_the_fixing() -> None:
    """Only theta observes the crossed fixing; ordinary pricing still raises ValueError."""
    instrument, market = _golden()
    with pytest.raises(ValueError, match="FIXING:USD-SOFR"):
        price_instrument(instrument, market.roll_forward(1), ROLLED)
