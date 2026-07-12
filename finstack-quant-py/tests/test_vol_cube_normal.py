"""Tests for the ``VolCube`` normal (Bachelier) vol API bindings."""

from __future__ import annotations

import math

import pytest

from finstack_quant.core.market_data import VolCube, VolSurface

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


def test_interpolation_mode_is_exposed_and_changes_expiry_interpolation() -> None:
    expiries = [1.0, 5.0]
    tenors = [2.0, 10.0]
    params = [
        {"alpha": 0.15, "beta": 1.0, "rho": -0.2, "nu": 0.4},
        {"alpha": 0.15, "beta": 1.0, "rho": -0.2, "nu": 0.4},
        {"alpha": 0.35, "beta": 1.0, "rho": -0.2, "nu": 0.4},
        {"alpha": 0.35, "beta": 1.0, "rho": -0.2, "nu": 0.4},
    ]
    forwards = [FORWARD] * 4
    vol_cube = VolCube("VOL", expiries, tenors, params, forwards, "vol")
    variance_cube = VolCube("VAR", expiries, tenors, params, forwards, "total_variance")

    assert vol_cube.interpolation_mode == "vol"
    assert variance_cube.interpolation_mode == "total_variance"
    assert vol_cube.vol(3.0, 2.0, FORWARD) != pytest.approx(variance_cube.vol(3.0, 2.0, FORWARD))


def test_materialized_expiry_slice_uses_direct_vol_tenor_interpolation() -> None:
    params = [{"alpha": 0.15, "beta": 1.0, "rho": -0.2, "nu": 0.4}] * 2
    cube = VolCube(
        "VAR",
        [1.0],
        [1.0, 4.0],
        params,
        [0.02, 0.05],
        "total_variance",
    )
    surface = cube.materialize_expiry_slice(1.0, [FORWARD])
    low = cube.vol(1.0, 1.0, FORWARD)
    high = cube.vol(1.0, 4.0, FORWARD)

    assert surface.value_checked(1.0, FORWARD) == pytest.approx(low)
    assert surface.value_checked(4.0, FORWARD) == pytest.approx(high)
    assert surface.value_checked(2.5, FORWARD) == pytest.approx((low + high) / 2.0)


def test_non_finite_sabr_shift_is_rejected() -> None:
    params = [{"alpha": 0.2, "beta": 1.0, "rho": -0.2, "nu": 0.4, "shift": math.inf}]
    with pytest.raises(ValueError, match="shift"):
        VolCube("BAD-SHIFT", [1.0], [2.0], params, [FORWARD])


def test_normal_sabr_rejects_nonpositive_shifted_levels_for_positive_beta() -> None:
    cev = [{"alpha": 0.01, "beta": 0.5, "rho": -0.2, "nu": 0.4}]
    cube = VolCube("CEV", [1.0], [2.0], cev, [-0.01])
    with pytest.raises(ValueError, match="Invalid input data"):
        cube.vol_normal(1.0, 2.0, -0.01)
    assert math.isnan(cube.vol_normal_clamped(1.0, 2.0, -0.01))

    normal = [{"alpha": 0.01, "beta": 0.0, "rho": -0.2, "nu": 0.4}]
    normal_cube = VolCube("NORMAL", [1.0], [2.0], normal, [-0.01])
    assert math.isfinite(normal_cube.vol_normal(1.0, 2.0, -0.02))
