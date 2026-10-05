"""Dupire local volatility: the `LocalVolSurface` class and the `local_vol` path process."""

import json
import math
import pickle

import pytest

from finstack_quant.core.market_data import VolSurface
from finstack_quant.models import bs_price
from finstack_quant.models.monte_carlo import simulate_paths
from finstack_quant.models.volatility import LocalVolSurface, surface_to_dataframe

EXPIRIES = [0.25, 0.5, 1.0, 2.0]
STRIKES = [80.0, 90.0, 100.0, 110.0, 120.0]
SMILE = [0.24, 0.22, 0.20, 0.19, 0.185]
FORWARDS = [100.5, 101.0, 102.0, 104.0]


def _skew() -> VolSurface:
    return VolSurface("SKEW", EXPIRIES, STRIKES, [SMILE] * len(EXPIRIES))


def _flat(vol: float = 0.2) -> VolSurface:
    return VolSurface("FLAT", EXPIRIES, STRIKES, [[vol] * len(STRIKES)] * len(EXPIRIES))


def test_flat_implied_vol_is_its_own_local_vol() -> None:
    local = LocalVolSurface.from_implied_vol(_flat(), FORWARDS)

    assert local.grid_shape == (4, 5)
    assert local.expiries == EXPIRIES
    assert local.strikes == STRIKES
    assert len(local.local_vols) == 20
    assert all(abs(vol - 0.2) < 1e-12 for vol in local.local_vols)
    assert abs(local.value(0.7, 93.0) - 0.2) < 1e-12
    # Flat outside the grid.
    assert local.value(9.0, 500.0) == local.local_vols[-1]


def test_skewed_surface_has_a_steeper_local_skew() -> None:
    local = LocalVolSurface.from_implied_vol(_skew(), FORWARDS)

    assert local.value(1.0, 90.0) - local.value(1.0, 110.0) > 0.22 - 0.19
    assert local.value(1.0, 100.0) == local.local_vols[2 * 5 + 2]
    smoothed = LocalVolSurface.from_implied_vol_smoothed(_skew(), FORWARDS, 10.0)
    skew = lambda surface: surface.value(1.0, 80.0) - surface.value(1.0, 120.0)  # noqa: E731
    assert skew(smoothed) < skew(local)
    unsmoothed = LocalVolSurface.from_implied_vol_smoothed(_skew(), FORWARDS, 0.0)
    assert unsmoothed.local_vols == local.local_vols


def test_explicit_grid_round_trips_and_tabulates() -> None:
    local = LocalVolSurface([1.0, 2.0], [100.0, 200.0], [0.1, 0.2, 0.3, 0.4])

    assert local.value(1.5, 150.0) == pytest.approx(0.25, abs=1e-15)
    assert json.loads(local.to_json()) == {
        "expiries": [1.0, 2.0],
        "strikes": [100.0, 200.0],
        "local_vols": [0.1, 0.2, 0.3, 0.4],
    }
    for restored in (LocalVolSurface.from_json(local.to_json()), pickle.loads(pickle.dumps(local))):
        assert restored.to_json() == local.to_json()
    assert repr(local) == "LocalVolSurface(expiries=2, strikes=2)"

    frame = local.to_dataframe()
    assert frame.index.name == "expiry"
    assert list(frame.index) == [1.0, 2.0]
    assert list(frame.columns) == [100.0, 200.0]
    assert frame.loc[2.0, 100.0] == 0.3
    assert "<table" in local._repr_html_()
    # Same layout as the implied surface it came from.
    implied = surface_to_dataframe(_skew())
    extracted = LocalVolSurface.from_implied_vol(_skew(), FORWARDS).to_dataframe()
    assert list(extracted.index) == list(implied.index)
    assert list(extracted.columns) == list(implied.columns)


@pytest.mark.parametrize(
    ("expiries", "strikes", "vols"),
    [
        ([], [100.0], []),
        ([2.0, 1.0], [100.0], [0.2, 0.2]),
        ([1.0], [0.0, 100.0], [0.2, 0.2]),
        ([1.0], [100.0, 110.0], [0.2]),
        ([1.0], [100.0, 110.0], [0.2, -0.1]),
    ],
)
def test_invalid_grids_are_value_errors(expiries: list[float], strikes: list[float], vols: list[float]) -> None:
    with pytest.raises(ValueError, match="LocalVolSurface"):
        LocalVolSurface(expiries, strikes, vols)


def test_invalid_json_is_a_value_error() -> None:
    with pytest.raises(ValueError, match="invalid LocalVolSurface JSON"):
        LocalVolSurface.from_json('{"expiries": [1.0], "strikes": [100.0], "local_vols": [0.2], "extra": 1}')
    with pytest.raises(ValueError, match="invalid LocalVolSurface JSON"):
        LocalVolSurface.from_json('{"expiries": [1.0], "strikes": [100.0], "local_vols": [-0.2]}')


def test_extraction_errors_are_value_errors() -> None:
    with pytest.raises(ValueError, match="one forward per expiry"):
        LocalVolSurface.from_implied_vol(_flat(), [100.0])
    with pytest.raises(ValueError, match="forwards must be finite and positive"):
        LocalVolSurface.from_implied_vol(_flat(), [100.0, 100.0, -1.0, 100.0])
    with pytest.raises(ValueError, match="sigma_strikes"):
        LocalVolSurface.from_implied_vol_smoothed(_flat(), FORWARDS, -1.0)

    calendar = VolSurface("BAD", [0.5, 1.0, 2.0], STRIKES, [[0.25] * 5, [0.25] * 5, [0.15] * 5])
    with pytest.raises(ValueError, match="calendar arbitrage"):
        LocalVolSurface.from_implied_vol(calendar, [100.0] * 3)
    butterfly = VolSurface(
        "BAD", [0.5, 1.0, 2.0], [70.0, 85.0, 100.0, 115.0, 130.0], [[0.20, 0.45, 0.20, 0.45, 0.20]] * 3
    )
    with pytest.raises(ValueError, match="butterfly arbitrage"):
        LocalVolSurface.from_implied_vol(butterfly, [100.0] * 3)
    normal = VolSurface("N", EXPIRIES, STRIKES, [SMILE] * 4, quote_type="normal")
    with pytest.raises(ValueError):
        LocalVolSurface.from_implied_vol(normal, FORWARDS)


def test_local_vol_process_with_a_flat_surface_reprices_black_scholes() -> None:
    rate, div_yield, sigma, expiry = 0.03, 0.01, 0.2, 1.0
    flat = LocalVolSurface([0.5, 1.0], [50.0, 150.0], [sigma] * 4)
    paths = simulate_paths({
        "process": {"type": "local_vol", "r": rate, "q": div_yield, "surface": json.loads(flat.to_json())},
        "initial_state": [100.0],
        "time_grid": {"type": "uniform", "expiry": expiry, "num_steps": 10},
        "num_paths": 20_000,
        "seed": 11,
        "antithetic": True,
    })

    assert paths.factor_names == ["spot"]
    stride = len(paths.times)
    terminal = paths.values[stride - 1 :: stride]
    pairs = [
        0.5 * (max(terminal[2 * i] - 100.0, 0.0) + max(terminal[2 * i + 1] - 100.0, 0.0))
        for i in range(paths.num_paths)
    ]
    mean = sum(pairs) / len(pairs)
    stderr = math.sqrt(sum((x - mean) ** 2 for x in pairs) / (len(pairs) - 1) / len(pairs))
    exact = bs_price(100.0, 100.0, rate, div_yield, sigma, expiry, True)
    assert abs(math.exp(-rate * expiry) * mean - exact) < 4 * stderr

    with pytest.raises(ValueError, match="scheme 'milstein' is not available for process 'local_vol'"):
        simulate_paths({
            "process": {"type": "local_vol", "r": rate, "q": div_yield, "surface": json.loads(flat.to_json())},
            "scheme": "milstein",
            "initial_state": [100.0],
            "time_grid": {"type": "uniform", "expiry": expiry, "num_steps": 2},
            "num_paths": 2,
            "seed": 1,
        })
