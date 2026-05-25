"""Smoke tests for ``FxDeltaVolSurface`` and ``MarketContext`` integration."""

from __future__ import annotations

import math

from finstack.core.market_data import FxDeltaVolSurface, MarketContext, VolSurface
import pytest


def _example_surface(*, with_10d: bool = False) -> FxDeltaVolSurface:
    expiries = [0.25, 0.5, 1.0]
    atm = [0.08, 0.085, 0.09]
    rr_25d = [0.01, 0.012, 0.015]
    bf_25d = [0.005, 0.006, 0.007]
    if with_10d:
        return FxDeltaVolSurface(
            "EURUSD-DELTA-VOL",
            expiries,
            atm,
            rr_25d,
            bf_25d,
            rr_10d=[0.018, 0.020, 0.024],
            bf_10d=[0.010, 0.011, 0.013],
        )
    return FxDeltaVolSurface("EURUSD-DELTA-VOL", expiries, atm, rr_25d, bf_25d)


def test_construction_and_accessors() -> None:
    surface = _example_surface()
    assert surface.id == "EURUSD-DELTA-VOL"
    assert surface.expiries == [0.25, 0.5, 1.0]
    assert surface.num_expiries == 3
    atm, put_25, call_25 = surface.pillar_vols(0)
    assert math.isclose(atm, 0.08, abs_tol=1e-12)
    # 25D wings recovered from ATM + RR + BF — values should bracket ATM
    assert put_25 != call_25
    assert min(put_25, call_25) <= atm <= max(put_25, call_25)


def test_pillar_vols_out_of_range_raises_index_error() -> None:
    surface = _example_surface()
    with pytest.raises(IndexError):
        surface.pillar_vols(99)


def test_rr_bf_10d_consistency_required() -> None:
    # Supplying only one of rr_10d / bf_10d must error (the Rust wrapper guards).
    with pytest.raises(ValueError, match="rr_10d and bf_10d"):
        FxDeltaVolSurface(
            "BAD",
            [0.25, 0.5],
            [0.08, 0.085],
            [0.01, 0.012],
            [0.005, 0.006],
            rr_10d=[0.018, 0.020],
        )


def test_implied_vol_lookup_recovers_atm_at_atm_strike() -> None:
    # At expiry=1.0 and a strike that maps to the ATM DNS, the lookup should
    # return the ATM vol exactly (up to interpolation rounding).
    surface = _example_surface()
    forward = 1.20
    atm_vol = 0.09  # pillar at expiry 1.0
    # ATM DNS strike: K_ATM = F * exp(0.5 * sigma^2 * T)
    k_atm = forward * math.exp(0.5 * atm_vol * atm_vol * 1.0)
    vol = surface.implied_vol(1.0, k_atm, forward, r_d=0.05, r_f=0.03)
    assert math.isclose(vol, atm_vol, abs_tol=1e-9)


def test_to_vol_surface_conversion_roundtrip() -> None:
    surface = _example_surface()
    strike_surface = surface.to_vol_surface(spot=1.20, r_d=0.05, r_f=0.03)
    assert isinstance(strike_surface, VolSurface)
    # The strike-axis surface must expose at least the same expiry count.
    assert len(strike_surface.expiries) >= surface.num_expiries


def test_static_delta_strike_roundtrip() -> None:
    forward = 1.20
    vol = 0.08
    expiry = 1.0
    r_f = 0.03
    # call delta 0.50 maps to a strike near the ATM DNS — converting back
    # should recover (approximately) the original delta.
    strike = FxDeltaVolSurface.delta_to_strike(0.50, forward, vol, expiry, r_f)
    delta = FxDeltaVolSurface.strike_to_delta(strike, forward, vol, expiry, r_f)
    assert math.isclose(delta, 0.50, abs_tol=1e-9)


def test_market_context_insert_and_get() -> None:
    surface = _example_surface()
    ctx = MarketContext().insert(surface)
    retrieved = ctx.get_fx_delta_vol_surface("EURUSD-DELTA-VOL")
    assert retrieved.id == "EURUSD-DELTA-VOL"
    assert retrieved.expiries == surface.expiries


def test_with_10d_wings_smoke() -> None:
    surface = _example_surface(with_10d=True)
    assert surface.num_expiries == 3
    # 10D wings produce a 5-point smile in implied_vol; a sanity probe.
    vol = surface.implied_vol(0.5, 1.30, forward=1.20, r_d=0.05, r_f=0.03)
    assert vol > 0.0
