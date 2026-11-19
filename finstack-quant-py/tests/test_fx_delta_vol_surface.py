"""Smoke tests for ``FxDeltaVolSurface`` and ``MarketContext`` integration."""

from __future__ import annotations

import datetime as dt
import json
import math

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, DayCountContext, VolatilityClock
from finstack_quant.core.market_data import FxDeltaVolSurface, MarketContext, VolSurface
from finstack_quant.models.volatility import (
    delta_to_strike,
    get_fx_delta_pillar_vols,
    get_fx_delta_vol,
    materialize_fx_delta_surface,
    strike_to_delta,
)


def _example_surface(*, with_10d: bool = False) -> FxDeltaVolSurface:
    expiries = [0.25, 0.5, 1.0]
    atm = [0.08, 0.085, 0.09]
    rr_25d = [0.01, 0.012, 0.015]
    bf_25d = [0.005, 0.006, 0.007]
    if with_10d:
        return FxDeltaVolSurface(
            "EURUSD-DELTA-VOL",
            "EUR/USD",
            expiries,
            atm,
            rr_25d,
            bf_25d,
            delta_convention="forward_unadjusted",
            atm_convention="delta_neutral_straddle",
            butterfly_convention="smile",
            rr_10d=[0.018, 0.020, 0.024],
            bf_10d=[0.010, 0.011, 0.013],
        )
    return FxDeltaVolSurface(
        "EURUSD-DELTA-VOL",
        "EUR/USD",
        expiries,
        atm,
        rr_25d,
        bf_25d,
        delta_convention="forward_unadjusted",
        atm_convention="delta_neutral_straddle",
        butterfly_convention="smile",
    )


@pytest.mark.parametrize("missing", ["delta_convention", "atm_convention", "butterfly_convention"])
def test_constructor_requires_each_fx_quote_convention(missing: str) -> None:
    conventions = {
        "delta_convention": "forward_unadjusted",
        "atm_convention": "delta_neutral_straddle",
        "butterfly_convention": "smile",
    }
    del conventions[missing]
    with pytest.raises(TypeError, match=missing):
        FxDeltaVolSurface("FX", "EUR/USD", [1.0], [0.10], [0.01], [0.002], **conventions)  # type: ignore[arg-type]


def test_construction_and_accessors() -> None:
    surface = _example_surface()
    assert surface.id == "EURUSD-DELTA-VOL"
    assert surface.pair == "EUR/USD"
    assert surface.delta_convention == "forward_unadjusted"
    assert surface.atm_convention == "delta_neutral_straddle"
    assert surface.butterfly_convention == "smile"
    assert surface.quote_type == "black_lognormal"
    assert surface.expiry_clock is None
    assert surface.expiries == [0.25, 0.5, 1.0]
    assert len(surface.expiries) == 3
    pillars = get_fx_delta_pillar_vols(surface, 0)
    assert len(pillars) == 3
    atm, put_25, call_25 = pillars
    assert math.isclose(atm, 0.08, abs_tol=1e-12)
    # 25D wings recovered from ATM + RR + BF — values should bracket ATM
    assert put_25 != call_25
    assert min(put_25, call_25) <= atm <= max(put_25, call_25)

    pair_surface = FxDeltaVolSurface(
        "EURUSD-TUPLE",
        (Currency("EUR"), "USD"),
        [1.0],
        [0.09],
        [0.01],
        [0.002],
        delta_convention="forward_unadjusted",
        atm_convention="delta_neutral_straddle",
        butterfly_convention="smile",
        expiry_clock=VolatilityClock(dt.date(2025, 1, 15), DayCount.ACT_365F, DayCountContext()),
    )
    assert pair_surface.pair == "EUR/USD"
    assert pair_surface.expiry_clock is not None
    assert pair_surface.expiry_clock.reference_date == dt.date(2025, 1, 15)


def test_pillar_vols_out_of_range_raises_value_error() -> None:
    surface = _example_surface()
    with pytest.raises(ValueError, match="out of range for the FX delta surface"):
        get_fx_delta_pillar_vols(surface, 99)


def test_rr_bf_10d_consistency_required() -> None:
    # Supplying only one of rr_10d / bf_10d must error (the Rust wrapper guards).
    with pytest.raises(ValueError, match="rr_10d and bf_10d"):
        FxDeltaVolSurface(
            "BAD",
            "EUR/USD",
            [0.25, 0.5],
            [0.08, 0.085],
            [0.01, 0.012],
            [0.005, 0.006],
            delta_convention="forward_unadjusted",
            atm_convention="delta_neutral_straddle",
            butterfly_convention="smile",
            rr_10d=[0.018, 0.020],
        )


def test_rejects_unknown_kwarg() -> None:
    with pytest.raises(TypeError, match="unexpected keyword argument"):
        FxDeltaVolSurface(
            "BAD",
            "EUR/USD",
            [0.25],
            [0.08],
            [0.01],
            [0.005],
            delta_convention="forward_unadjusted",
            atm_convention="delta_neutral_straddle",
            butterfly_convention="smile",
            unknown_option=123,  # type: ignore[call-arg]
        )


def test_implied_vol_lookup_recovers_atm_at_atm_strike() -> None:
    # At expiry=1.0 and a strike that maps to the ATM DNS, the lookup should
    # return the ATM vol exactly (up to interpolation rounding).
    surface = _example_surface()
    forward = 1.20
    atm_vol = 0.09  # pillar at expiry 1.0
    # ATM DNS strike: K_ATM = F * exp(0.5 * sigma^2 * T)
    k_atm = forward * math.exp(0.5 * atm_vol * atm_vol * 1.0)
    vol = get_fx_delta_vol(surface, 1.0, k_atm, forward)
    assert math.isclose(vol, atm_vol, abs_tol=1e-9)


def test_to_vol_surface_conversion_roundtrip() -> None:
    surface = _example_surface()
    delivery_forwards = [1.20 * math.exp((0.05 - 0.03) * t) for t in surface.expiries]
    strike_surface = materialize_fx_delta_surface(surface, delivery_forwards, "total_variance")
    assert isinstance(strike_surface, VolSurface)
    assert strike_surface.interpolation_mode == "total_variance"
    # The strike-axis surface must expose at least the same expiry count.
    assert len(strike_surface.expiries) >= len(surface.expiries)


def test_static_delta_strike_roundtrip() -> None:
    forward = 1.20
    vol = 0.08
    expiry = 1.0
    # call delta 0.50 maps to a strike near the ATM DNS — converting back
    # should recover (approximately) the original delta.
    strike = delta_to_strike(True, 0.50, forward, vol, expiry)
    delta = strike_to_delta(True, strike, forward, vol, expiry)
    assert math.isclose(delta, 0.50, abs_tol=1e-9)

    put_strike = delta_to_strike(False, 0.25, forward, vol, expiry)
    put_delta = strike_to_delta(False, put_strike, forward, vol, expiry)
    assert put_strike < forward
    assert math.isclose(put_delta, 0.25, abs_tol=1e-9)


def test_market_context_insert_and_get() -> None:
    surface = _example_surface()
    ctx = MarketContext().insert(surface)
    retrieved = ctx.get_fx_delta_vol_surface("EURUSD-DELTA-VOL")
    assert retrieved.id == "EURUSD-DELTA-VOL"
    assert retrieved.expiries == surface.expiries


def test_with_10d_wings_smoke() -> None:
    surface = _example_surface(with_10d=True)
    assert len(surface.expiries) == 3
    pillars = get_fx_delta_pillar_vols(surface, 0)
    assert len(pillars) == 5
    # 10D wings produce a 5-point smile in implied_vol; a sanity probe.
    vol = get_fx_delta_vol(surface, 0.5, 1.30, forward=1.20)
    assert vol > 0.0


@pytest.mark.parametrize("field", ["rr_10d", "bf_10d"])
def test_json_rejects_unpaired_wings(field: str) -> None:
    payload = json.loads(_example_surface(with_10d=True).to_json())
    del payload[field]
    with pytest.raises(ValueError, match="must both be provided"):
        FxDeltaVolSurface.from_json(json.dumps(payload))
