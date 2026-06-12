"""Tests for the ``VolCube`` normal (Bachelier) vol API bindings."""

from __future__ import annotations

import math

from finstack.core.market_data import VolCube, VolSurface
import pytest

FORWARD = 0.03


def _example_cube() -> VolCube:
    """Positive-rates 2x2 SABR cube with a flat forward of 3%."""
    expiries = [1.0, 5.0]
    tenors = [2.0, 10.0]
    params = [{"alpha": 0.2, "beta": 1.0, "rho": -0.2, "nu": 0.4}] * 4
    forwards = [FORWARD] * 4
    return VolCube("SWPT-CUBE", expiries, tenors, params, forwards)


def test_vol_normal_atm_approximates_lognormal_times_forward() -> None:
    cube = _example_cube()
    vol_ln = cube.vol(1.0, 2.0, FORWARD)
    vol_n = cube.vol_normal(1.0, 2.0, FORWARD)
    # At ATM the normal vol is approximately lognormal vol x forward.
    assert vol_n == pytest.approx(vol_ln * FORWARD, rel=0.01)


def test_vol_normal_raises_outside_grid() -> None:
    cube = _example_cube()
    with pytest.raises(ValueError, match="out of bounds"):
        cube.vol_normal(20.0, 2.0, FORWARD)


def test_vol_normal_clamped_finite_and_positive() -> None:
    cube = _example_cube()
    # Inside the grid.
    v = cube.vol_normal_clamped(1.0, 2.0, FORWARD)
    assert math.isfinite(v)
    assert v > 0.0
    # Clamped extrapolation beyond the grid edges never raises.
    v_extrap = cube.vol_normal_clamped(20.0, 50.0, FORWARD)
    assert math.isfinite(v_extrap)
    assert v_extrap > 0.0


def test_materialize_normal_slices_return_vol_surfaces() -> None:
    cube = _example_cube()
    strikes = [0.02, FORWARD, 0.04]

    tenor_slice = cube.materialize_tenor_slice_normal(2.0, strikes)
    assert isinstance(tenor_slice, VolSurface)
    assert tenor_slice.quote_type == "normal"

    expiry_slice = cube.materialize_expiry_slice_normal(1.0, strikes)
    assert isinstance(expiry_slice, VolSurface)
    assert expiry_slice.quote_type == "normal"

    # ATM node of the materialized slice matches the point query.
    assert tenor_slice.value_clamped(1.0, FORWARD) == pytest.approx(
        cube.vol_normal_clamped(1.0, 2.0, FORWARD), rel=1e-9
    )
